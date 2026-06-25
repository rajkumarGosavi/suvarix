use tauri::State;
use serde::Serialize;
use crate::db::DbState;
use crate::error::{AppError, Result};
use super::{equity_price, mf_nav};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResult {
    pub updated: i64,
    pub failed: i64,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn refresh_equity_prices(state: State<'_, DbState>) -> Result<RefreshResult> {
    // Lock only long enough to read symbols, then release before any await
    let holdings: Vec<(i64, String, String)> = {
        let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
        let mut stmt = conn.prepare("SELECT id, symbol, exchange FROM equity_holdings")?;
        let rows: Vec<_> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows // bind before block ends so stmt borrow is fully consumed
    };

    let mut updated = 0i64;
    let mut failed = 0i64;
    let mut errors: Vec<String> = vec![];

    for (id, symbol, exchange) in &holdings {
        match equity_price::fetch_price(symbol, exchange).await {
            Ok(price) => {
                let conn = state.0.lock()
                    .map_err(|_| AppError::Database("lock error".into()))?;
                let ok = conn.execute(
                    "UPDATE equity_holdings \
                     SET current_price=?1, price_updated_at=datetime('now') \
                     WHERE id=?2",
                    rusqlite::params![price, *id],
                ).is_ok();
                if ok { updated += 1; } else { failed += 1; }
            }
            Err(e) => {
                errors.push(format!("{}: {}", symbol, e));
                failed += 1;
            }
        }
    }

    Ok(RefreshResult { updated, failed, errors })
}

#[tauri::command]
pub async fn refresh_mf_navs(state: State<'_, DbState>) -> Result<RefreshResult> {
    let holdings: Vec<(i64, String)> = {
        let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
        let mut stmt = conn.prepare("SELECT id, scheme_code FROM mf_holdings")?;
        let rows: Vec<_> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    let mut updated = 0i64;
    let mut failed = 0i64;
    let mut errors: Vec<String> = vec![];

    for (id, scheme_code) in &holdings {
        match mf_nav::fetch_nav(scheme_code).await {
            Ok((nav, nav_date)) => {
                let conn = state.0.lock()
                    .map_err(|_| AppError::Database("lock error".into()))?;
                let ok = conn.execute(
                    "UPDATE mf_holdings SET current_nav=?1, nav_date=?2 WHERE id=?3",
                    rusqlite::params![nav, nav_date, *id],
                ).is_ok();
                if ok { updated += 1; } else { failed += 1; }
            }
            Err(e) => {
                errors.push(format!("{}: {}", scheme_code, e));
                failed += 1;
            }
        }
    }

    Ok(RefreshResult { updated, failed, errors })
}

#[tauri::command]
pub async fn get_market_indices() -> Result<serde_json::Value> {
    let indices = [
        ("nifty50",  "^NSEI"),
        ("sensex",   "^BSESN"),
        ("usdInr",   "USDINR=X"),
    ];

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut result = serde_json::json!({
        "nifty50":     null,
        "sensex":      null,
        "usdInr":      null,
        "lastUpdated": chrono::Local::now().to_rfc3339(),
    });

    for (key, symbol) in &indices {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=1d",
            symbol
        );
        if let Ok(resp) = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(price) = json["chart"]["result"][0]["meta"]["regularMarketPrice"]
                    .as_f64()
                {
                    result[key] = serde_json::json!(price);
                }
            }
        }
        // On network error or missing data, key stays null — app stays usable offline
    }

    Ok(result)
}
