use crate::error::{AppError, Result};

/// Returns (nav, nav_date_as_yyyy_mm_dd)
pub async fn fetch_nav(scheme_code: &str) -> Result<(f64, String)> {
    let url = format!("https://api.mfapi.in/mf/{}", scheme_code);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("scheme {}: {}", scheme_code, e)))?
        .json()
        .await
        .map_err(|e| AppError::ExternalApi(format!("parse error scheme {}: {}", scheme_code, e)))?;

    let nav_str = resp["data"][0]["nav"]
        .as_str()
        .ok_or_else(|| AppError::ExternalApi(format!("no NAV for scheme {}", scheme_code)))?;

    let nav: f64 = nav_str
        .parse()
        .map_err(|_| AppError::Parse(format!("invalid NAV '{}' for scheme {}", nav_str, scheme_code)))?;

    // mfapi.in returns DD-MM-YYYY; convert to YYYY-MM-DD for SQLite
    let nav_date = resp["data"][0]["date"]
        .as_str()
        .map(|d| {
            let parts: Vec<&str> = d.split('-').collect();
            if parts.len() == 3 {
                format!("{}-{}-{}", parts[2], parts[1], parts[0])
            } else {
                d.to_string()
            }
        })
        .unwrap_or_default();

    Ok((nav, nav_date))
}
