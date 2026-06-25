use crate::error::{AppError, Result};
use reqwest::header::{ACCEPT, USER_AGENT};

pub async fn fetch_price(symbol: &str, exchange: &str) -> Result<f64> {
    let suffix = if exchange == "NSE" { "NS" } else { "BO" };
    let ticker = format!("{}.{}", symbol, suffix);
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d",
        ticker
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp: serde_json::Value = client
        .get(&url)
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("{}: {}", ticker, e)))?
        .json()
        .await
        .map_err(|e| AppError::ExternalApi(format!("parse error for {}: {}", ticker, e)))?;

    resp["chart"]["result"][0]["meta"]["regularMarketPrice"]
        .as_f64()
        .ok_or_else(|| AppError::ExternalApi(format!("no price data for {}", ticker)))
}

pub async fn fetch_index(yahoo_symbol: &str) -> Result<f64> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d",
        yahoo_symbol
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp: serde_json::Value = client
        .get(&url)
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::ExternalApi(e.to_string()))?;

    resp["chart"]["result"][0]["meta"]["regularMarketPrice"]
        .as_f64()
        .ok_or_else(|| AppError::ExternalApi(format!("no data for {}", yahoo_symbol)))
}
