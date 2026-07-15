use rand::random;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

use crate::constants::APP_NAME;
use crate::data_sources::broker::BrokerHolding;
use crate::error::{AppError, Result};

const KITE_BASE: &str = "https://api.kite.trade";
pub const CALLBACK_PORT: u16 = 7459;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct KiteHolding {
    pub tradingsymbol: String,
    pub exchange: String,
    pub isin: String,
    pub quantity: f64,
    pub average_price: f64,
    pub last_price: f64,
}

impl From<KiteHolding> for BrokerHolding {
    fn from(h: KiteHolding) -> Self {
        Self {
            symbol: h.tradingsymbol.clone(),
            exchange: h.exchange,
            isin: h.isin,
            quantity: h.quantity,
            avg_price: h.average_price,
            current_price: h.last_price,
            name: None, // Kite holdings API does not return company name
        }
    }
}

fn compute_checksum(api_key: &str, request_token: &str, api_secret: &str) -> String {
    let input = format!("{api_key}{request_token}{api_secret}");
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Pulls a single query-string parameter's value out of the raw HTTP GET line.
/// First line looks like: `GET /?request_token=xxx&status=success&state=yyy HTTP/1.1`
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

/// Random 128-bit CSRF `state`, hex-encoded. Passed to Kite via `redirect_params`
/// and echoed back on the callback; a callback whose `state` doesn't match is
/// rejected, so a local process racing the loopback port with a forged
/// `request_token` can't bind this app to an attacker's Zerodha account (H2).
fn random_state() -> String {
    let bytes: [u8; 16] = random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Start local TCP server on port 7459, open Kite login in browser, wait for the OAuth
/// redirect, exchange request_token for access_token. Blocks up to 3 minutes.
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

    // `redirect_params` is echoed back appended to the callback URL by Kite, so
    // we ferry a random `state` through it and verify it below (H2 / CSRF).
    let state = random_state();
    let login_url = format!(
        "https://kite.zerodha.com/connect/login?api_key={api_key}&v=3&redirect_params=state={state}"
    );
    app.opener()
        .open_url(&login_url, None::<&str>)
        .map_err(|e| AppError::ExternalApi(format!("Failed to open browser: {e}")))?;

    // Wait up to 3 minutes for the browser redirect to arrive
    let (mut stream, _) = timeout(Duration::from_secs(180), listener.accept())
        .await
        .map_err(|_| AppError::ExternalApi("Login timed out (3 minutes). Please try again.".into()))?
        .map_err(|e| AppError::ExternalApi(format!("Connection error: {e}")))?;

    // Read the HTTP request (first 4 KB is enough for the GET line)
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| AppError::ExternalApi(format!("Read error: {e}")))?;
    let request_str = String::from_utf8_lossy(&buf[..n]);

    // Always send a success page before processing, so the browser doesn't hang
    let success_html = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n\
        <!DOCTYPE html><html><head><title>{APP_NAME} — Login successful</title>\
        <style>body{{font-family:system-ui,sans-serif;text-align:center;padding:3rem;\
        background:#0f172a;color:#e2e8f0}}h2{{color:#10b981}}p{{color:#94a3b8}}</style>\
        </head><body><h2>&#10003; Login successful!</h2>\
        <p>You can close this tab and return to {APP_NAME}.</p></body></html>"
    );
    let _ = stream.write_all(success_html.as_bytes()).await;


    // Reject callbacks whose `state` doesn't match the one we just sent —
    // defeats a local process racing the loopback port with a forged token (H2).
    let returned_state = extract_query_param(&request_str, "state");
    if returned_state.as_deref() != Some(state.as_str()) {
        return Err(AppError::ExternalApi(
            "Login rejected: OAuth state mismatch (possible CSRF). Please try again.".into(),
        ));
    }

    let request_token = extract_query_param(&request_str, "request_token")
        .ok_or_else(|| AppError::ExternalApi("Zerodha did not return a request_token. Check that your redirect URL is set to http://127.0.0.1:7459".into()))?;

    let checksum = compute_checksum(api_key, &request_token, api_secret);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{KITE_BASE}/session/token"))
        .header("X-Kite-Version", "3")
        .form(&[
            ("api_key", api_key),
            ("request_token", request_token.as_str()),
            ("checksum", checksum.as_str()),
        ])
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Token exchange request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_else(|e| format!("[failed to read body: {e}]"));
        return Err(AppError::ExternalApi(format!(
            "Kite API returned {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("Parse token response: {e}")))?;

    let access_token = json["data"]["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Parse("access_token missing from Kite response".into()))?
        .to_string();

    Ok(access_token)
}

/// Fetch current portfolio holdings from GET /portfolio/holdings.
pub async fn fetch_holdings(api_key: &str, access_token: &str) -> Result<Vec<KiteHolding>> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{KITE_BASE}/portfolio/holdings"))
        .header(
            "Authorization",
            format!("token {api_key}:{access_token}"),
        )
        .header("X-Kite-Version", "3")
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Kite holdings request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_else(|e| format!("[failed to read body: {e}]"));
        return Err(AppError::ExternalApi(format!(
            "Kite API returned {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("Parse holdings response: {e}")))?;

    let holdings: Vec<KiteHolding> = serde_json::from_value(json["data"].clone())
        .map_err(|e| AppError::Parse(format!("Deserialize holdings: {e}")))?;

    Ok(holdings)
}
