use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

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

fn extract_request_token(http_request: &str) -> Option<String> {
    // First line looks like: GET /callback?request_token=xxx&status=success HTTP/1.1
    let first_line = http_request.lines().next()?;
    let query_start = first_line.find('?')?;
    let path_end = first_line.rfind(' ')?;
    if path_end <= query_start {
        return None;
    }
    let query = &first_line[query_start + 1..path_end];
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("request_token=") {
            return Some(value.to_string());
        }
    }
    None
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

    let login_url = format!(
        "https://kite.zerodha.com/connect/login?api_key={api_key}&v=3"
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
    let success_html = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html; charset=utf-8\r\n",
        "Connection: close\r\n",
        "\r\n",
        "<!DOCTYPE html><html><head><title>FinFolio — Login successful</title>",
        "<style>body{font-family:system-ui,sans-serif;text-align:center;padding:3rem;",
        "background:#0f172a;color:#e2e8f0}h2{color:#10b981}p{color:#94a3b8}</style>",
        "</head><body><h2>&#10003; Login successful!</h2>",
        "<p>You can close this tab and return to FinFolio.</p></body></html>"
    );
    let _ = stream.write_all(success_html.as_bytes()).await;

    let request_token = extract_request_token(&request_str)
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
