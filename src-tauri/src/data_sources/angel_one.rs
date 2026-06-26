use crate::data_sources::broker::BrokerHolding;
use crate::error::{AppError, Result};

const ANGEL_BASE: &str = "https://apiconnect.angelbroking.com";

#[derive(serde::Deserialize, Debug, Clone)]
pub struct AngelHolding {
    pub tradingsymbol: String,
    pub exchange: String,
    pub isin: String,
    #[serde(default)]
    pub quantity: f64,
    #[serde(rename = "averageprice")]
    pub average_price: f64,
    pub ltp: f64,
}

impl From<AngelHolding> for BrokerHolding {
    fn from(h: AngelHolding) -> Self {
        Self {
            symbol: h.tradingsymbol.clone(),
            exchange: h.exchange,
            isin: h.isin,
            quantity: h.quantity,
            avg_price: h.average_price,
            current_price: h.ltp,
            name: None, // SmartAPI holdings API does not return company name
        }
    }
}

/// Authenticate with Angel One SmartAPI using client credentials + TOTP.
/// Returns the JWT token on success.
pub async fn login(
    api_key: &str,
    client_id: &str,
    password: &str,
    totp: &str,
) -> Result<String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "clientcode": client_id,
        "password": password,
        "totp": totp,
    });

    let response = client
        .post(format!(
            "{ANGEL_BASE}/rest/auth/angelbroking/user/v1/loginByPassword"
        ))
        .header("X-PrivateKey", api_key)
        .header("X-UserType", "USER")
        .header("X-SourceID", "WEB")
        .header("X-ClientLocalIP", "127.0.0.1")
        .header("X-ClientPublicIP", "127.0.0.1")
        .header("X-MACAddress", "00:00:00:00:00:00")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Angel One login request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_else(|e| format!("[failed to read body: {e}]"));
        return Err(AppError::ExternalApi(format!(
            "Angel One API returned {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("Parse Angel One login response: {e}")))?;

    // Check for API-level error
    if json["status"].as_bool() == Some(false) || json["errorCode"].as_str().is_some() {
        let msg = json["message"]
            .as_str()
            .unwrap_or("Login failed")
            .to_string();
        return Err(AppError::ExternalApi(format!("Angel One: {msg}")));
    }

    let jwt_token = json["data"]["jwtToken"]
        .as_str()
        .ok_or_else(|| AppError::Parse("jwtToken missing from Angel One response".into()))?
        .to_string();

    Ok(jwt_token)
}

/// Fetch all holdings from Angel One portfolio API.
pub async fn fetch_holdings(api_key: &str, jwt_token: &str) -> Result<Vec<AngelHolding>> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "{ANGEL_BASE}/rest/secure/angelbroking/portfolio/v1/getAllHolding"
        ))
        .header("Authorization", format!("Bearer {jwt_token}"))
        .header("X-PrivateKey", api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Angel One holdings request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_else(|e| format!("[failed to read body: {e}]"));
        return Err(AppError::ExternalApi(format!(
            "Angel One API returned {status}: {text}"
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("Parse Angel One holdings response: {e}")))?;

    let holdings: Vec<AngelHolding> =
        serde_json::from_value(json["data"]["holdings"].clone())
            .map_err(|e| AppError::Parse(format!("Deserialize Angel One holdings: {e}")))?;

    Ok(holdings)
}
