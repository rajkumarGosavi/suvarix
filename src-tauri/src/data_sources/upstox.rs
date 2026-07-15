use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

use crate::constants::APP_NAME;
use crate::data_sources::broker::BrokerHolding;
use crate::error::{AppError, Result};

const UPSTOX_BASE: &str = "https://api.upstox.com/v2";
pub const CALLBACK_PORT: u16 = 7460;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct UpstoxHolding {
    pub tradingsymbol: String,
    pub exchange: String,
    pub isin: String,
    #[serde(default)]
    pub quantity: f64,
    pub average_price: f64,
    pub last_price: f64,
    pub company_name: Option<String>,
}

impl From<UpstoxHolding> for BrokerHolding {
    fn from(h: UpstoxHolding) -> Self {
        Self {
            symbol: h.tradingsymbol.clone(),
            exchange: h.exchange,
            isin: h.isin,
            quantity: h.quantity,
            avg_price: h.average_price,
            current_price: h.last_price,
            name: h.company_name,
        }
    }
}

/// Extract a single query param's value from a raw HTTP GET request line.
/// First line: `GET /?code=xxx&state=yyy HTTP/1.1`
fn extract_query_param(http_request: &str, name: &str) -> Option<String> {
    let first_line = http_request.lines().next()?;
    let query_start = first_line.find('?')?;
    let path_end = first_line.rfind(' ')?;
    if path_end <= query_start {
        return None;
    }
    let query = &first_line[query_start + 1..path_end];
    let prefix = format!("{name}=");
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

/// Random 128-bit CSRF `state`, hex-encoded — sent as the OAuth2 `state`
/// parameter and verified on the callback so a local process can't race the
/// loopback port with a forged `code` (H2).
fn random_state() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// OAuth2 flow: open browser → wait for redirect on port 7460 → exchange code for token.
pub async fn run_oauth_flow(
    api_key: &str,
    api_secret: &str,
    app: &tauri::AppHandle,
) -> Result<String> {
    use tauri_plugin_opener::OpenerExt;

    let listener = TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}"))
        .await
        .map_err(|e| {
            AppError::ExternalApi(format!(
                "Cannot bind to port {CALLBACK_PORT} — is another process using it? ({e})"
            ))
        })?;

    let redirect_uri = format!("http://127.0.0.1:{CALLBACK_PORT}");
    let state = random_state();
    let login_url = format!(
        "{UPSTOX_BASE}/login/authorization/dialog?client_id={api_key}&redirect_uri={redirect_uri}&response_type=code&state={state}"
    );
    app.opener()
        .open_url(&login_url, None::<&str>)
        .map_err(|e| AppError::ExternalApi(format!("Failed to open browser: {e}")))?;

    // Wait up to 3 minutes for the browser redirect
    let (mut stream, _) = timeout(Duration::from_secs(180), listener.accept())
        .await
        .map_err(|_| {
            AppError::ExternalApi("Login timed out (3 minutes). Please try again.".into())
        })?
        .map_err(|e| AppError::ExternalApi(format!("Connection error: {e}")))?;

    // Read the HTTP request
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| AppError::ExternalApi(format!("Read error: {e}")))?;
    let request_str = String::from_utf8_lossy(&buf[..n]);

    // Respond to the browser immediately
    let success_html = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n\
        <!DOCTYPE html><html><head><title>{APP_NAME} — Login successful</title>\
        <style>body{{font-family:system-ui,sans-serif;text-align:center;padding:3rem;\
        background:#0f172a;color:#e2e8f0}}h2{{color:#10b981}}p{{color:#94a3b8}}</style>\
        </head><body><h2>&#10003; Login successful!</h2>\
        <p>You can close this tab and return to {APP_NAME}.</p></body></html>"
    );
    let _ = stream.write_all(success_html.as_bytes()).await;

    // Reject callbacks whose `state` doesn't match — defeats a local process
    // racing the loopback port with a forged authorization code (H2).
    let returned_state = extract_query_param(&request_str, "state");
    if returned_state.as_deref() != Some(state.as_str()) {
        return Err(AppError::ExternalApi(
            "Login rejected: OAuth state mismatch (possible CSRF). Please try again.".into(),
        ));
    }

    let code = extract_query_param(&request_str, "code").ok_or_else(|| {
        AppError::ExternalApi(
            "Upstox did not return a code. Check that your redirect URL is set to http://127.0.0.1:7460".into(),
        )
    })?;

    // Exchange code for access token
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{UPSTOX_BASE}/login/authorization/token"))
        .form(&[
            ("code", code.as_str()),
            ("client_id", api_key),
            ("client_secret", api_secret),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Token exchange request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_else(|e| format!("[failed to read body: {e}]"));
        return Err(AppError::ExternalApi(format!(
            "Upstox API returned {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("Parse token response: {e}")))?;

    let access_token = json["data"]["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Parse("access_token missing from Upstox response".into()))?
        .to_string();

    Ok(access_token)
}

/// Fetch long-term holdings from Upstox portfolio API.
pub async fn fetch_holdings(
    _api_key: &str,
    access_token: &str,
) -> Result<Vec<UpstoxHolding>> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{UPSTOX_BASE}/portfolio/long-term-holdings"))
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Upstox holdings request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_else(|e| format!("[failed to read body: {e}]"));
        return Err(AppError::ExternalApi(format!(
            "Upstox API returned {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("Parse holdings response: {e}")))?;

    let holdings: Vec<UpstoxHolding> = serde_json::from_value(json["data"].clone())
        .map_err(|e| AppError::Parse(format!("Deserialize Upstox holdings: {e}")))?;

    Ok(holdings)
}
