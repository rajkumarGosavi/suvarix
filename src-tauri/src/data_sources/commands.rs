use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::common::ImportResult;
use super::angel_one;
use super::broker;
use super::csv_importer;
use super::upstox;
use super::zerodha;

// ─── CAS Import ──────────────────────────────────────────────

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CasHoldingInput {
    pub scheme_name: String,
    pub isin: String,
    pub folio_number: String,
    pub units: f64,
    pub nav: f64,
    pub avg_nav: f64,
    pub amc_name: String,
    pub is_direct: bool,
    pub is_growth: bool,
}

#[tauri::command]
pub fn import_cas_mf(holdings: Vec<CasHoldingInput>, state: State<DbState>) -> Result<ImportResult> {
    let mut conn = state.0.get()?;

    // Ensure a "MF Central CAS" account exists
    conn.execute(
        "INSERT OR IGNORE INTO accounts
             (name, type, provider, is_active, created_at, updated_at)
         VALUES ('MF Central CAS', 'broker', 'mfcentral', 1, datetime('now'), datetime('now'))",
        [],
    )?;
    let account_id: i64 = conn.query_row(
        "SELECT id FROM accounts WHERE provider='mfcentral'",
        [],
        |r| r.get(0),
    )?;

    let tx = conn.transaction()?;
    let mut imported = 0i64;
    let mut skipped = 0i64;

    for h in &holdings {
        if h.units <= 0.0 {
            skipped += 1;
            continue;
        }
        let result = tx.execute(
            "INSERT INTO mf_holdings
                 (account_id, scheme_code, scheme_name, amc_name, folio_number,
                  units, avg_nav, current_nav, is_direct, is_growth,
                  created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,datetime('now'),datetime('now'))
             ON CONFLICT(account_id, folio_number, scheme_code)
             DO UPDATE SET
                 units=excluded.units,
                 avg_nav=excluded.avg_nav,
                 current_nav=excluded.current_nav,
                 updated_at=datetime('now')",
            rusqlite::params![
                account_id,
                h.isin,
                h.scheme_name,
                h.amc_name,
                h.folio_number,
                h.units,
                h.avg_nav,
                h.nav,
                h.is_direct as i64,
                h.is_growth as i64,
            ],
        );
        match result {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }

    tx.commit()?;
    Ok(ImportResult { imported, skipped })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZerodhaStatus {
    pub has_config: bool,
    pub is_connected: bool,
    pub token_date: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SyncResult {
    pub synced: i64,
    pub errors: Vec<String>,
}

// ─── Config ──────────────────────────────────────────────────

#[tauri::command]
pub fn save_zerodha_config(
    api_key: String,
    api_secret: String,
    state: State<DbState>,
) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('zerodha_api_key', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&api_key],
    )?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('zerodha_api_secret', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&api_secret],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_zerodha_status(state: State<DbState>) -> Result<ZerodhaStatus> {
    let conn = state.0.get()?;

    let api_key: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='zerodha_api_key'",
            [],
            |r| r.get(0),
        )
        .ok();

    let api_secret: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='zerodha_api_secret'",
            [],
            |r| r.get(0),
        )
        .ok();

    let access_token: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='zerodha_access_token'",
            [],
            |r| r.get(0),
        )
        .ok();

    let token_date: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='zerodha_access_token_date'",
            [],
            |r| r.get(0),
        )
        .ok();

    let has_config = api_key.is_some() && api_secret.is_some();

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let is_connected =
        access_token.is_some() && token_date.as_deref() == Some(today.as_str());

    Ok(ZerodhaStatus {
        has_config,
        is_connected,
        token_date,
    })
}

#[tauri::command]
pub fn disconnect_zerodha(state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "DELETE FROM app_settings WHERE key IN (
            'zerodha_api_key',
            'zerodha_api_secret',
            'zerodha_access_token',
            'zerodha_access_token_date'
         )",
        [],
    )?;
    Ok(())
}

// ─── OAuth ───────────────────────────────────────────────────

#[tauri::command]
pub async fn start_zerodha_login(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
) -> Result<()> {
    // Read credentials (lock → read → release)
    let (api_key, api_secret) = {
        let conn = state.0.get()?;

        let api_key = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='zerodha_api_key'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Zerodha API key not configured".into()))?;

        let api_secret = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='zerodha_api_secret'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Zerodha API secret not configured".into()))?;

        (api_key, api_secret)
    };

    // Run OAuth flow — opens browser, waits for redirect, exchanges token
    let access_token = zerodha::run_oauth_flow(&api_key, &api_secret, &app).await?;

    // Store access_token + today's date (lock → write → release)
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    {
        let conn = state.0.get()?;

        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('zerodha_access_token', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&access_token],
        )?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('zerodha_access_token_date', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&today],
        )?;
    }

    Ok(())
}

// ─── Sync ────────────────────────────────────────────────────

#[tauri::command]
pub async fn sync_zerodha_holdings(state: State<'_, DbState>) -> Result<SyncResult> {
    // Read credentials (lock → read → release)
    let (api_key, access_token) = {
        let conn = state.0.get()?;

        let api_key = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='zerodha_api_key'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Zerodha API key not configured".into()))?;

        let access_token = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='zerodha_access_token'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| {
                AppError::Validation(
                    "Not connected to Zerodha. Please reconnect.".into(),
                )
            })?;

        (api_key, access_token)
    };

    // Fetch from Kite API (no lock held during HTTP)
    let raw = zerodha::fetch_holdings(&api_key, &access_token).await?;
    let holdings: Vec<broker::BrokerHolding> = raw.into_iter().map(Into::into).collect();
    let synced = holdings.len() as i64;

    // Write to DB (lock → write → release)
    {
        let mut conn = state.0.get()?;
        broker::write_broker_holdings(&mut *conn, "zerodha", "Zerodha", &holdings)?;
    }

    Ok(SyncResult { synced, errors: vec![] })
}

// ═══════════════════════════════════════════════════════════════
// ─── Upstox ──────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstoxStatus {
    pub has_config: bool,
    pub is_connected: bool,
    pub token_date: Option<String>,
}

#[tauri::command]
pub fn save_upstox_config(
    api_key: String,
    api_secret: String,
    state: State<DbState>,
) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('upstox_api_key', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&api_key],
    )?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('upstox_api_secret', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&api_secret],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_upstox_status(state: State<DbState>) -> Result<UpstoxStatus> {
    let conn = state.0.get()?;

    let api_key: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='upstox_api_key'",
            [],
            |r| r.get(0),
        )
        .ok();

    let api_secret: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='upstox_api_secret'",
            [],
            |r| r.get(0),
        )
        .ok();

    let access_token: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='upstox_access_token'",
            [],
            |r| r.get(0),
        )
        .ok();

    let token_date: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='upstox_token_date'",
            [],
            |r| r.get(0),
        )
        .ok();

    let has_config = api_key.is_some() && api_secret.is_some();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let is_connected = access_token.is_some() && token_date.as_deref() == Some(today.as_str());

    Ok(UpstoxStatus { has_config, is_connected, token_date })
}

#[tauri::command]
pub async fn start_upstox_login(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
) -> Result<()> {
    // Read credentials (lock → read → release)
    let (api_key, api_secret) = {
        let conn = state.0.get()?;
        let api_key = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='upstox_api_key'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Upstox API key not configured".into()))?;
        let api_secret = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='upstox_api_secret'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Upstox API secret not configured".into()))?;
        (api_key, api_secret)
    };

    // Run OAuth flow — opens browser, waits for redirect, exchanges token
    let access_token = upstox::run_oauth_flow(&api_key, &api_secret, &app).await?;

    // Store token + today's date (lock → write → release)
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    {
        let conn = state.0.get()?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('upstox_access_token', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&access_token],
        )?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('upstox_token_date', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&today],
        )?;
    }

    Ok(())
}

#[tauri::command]
pub async fn sync_upstox_holdings(state: State<'_, DbState>) -> Result<SyncResult> {
    // Read credentials (lock → read → release)
    let (api_key, access_token) = {
        let conn = state.0.get()?;
        let api_key = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='upstox_api_key'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Upstox API key not configured".into()))?;
        let access_token = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='upstox_access_token'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Not connected to Upstox. Please reconnect.".into()))?;
        (api_key, access_token)
    };

    // Fetch from Upstox API (no lock held during HTTP)
    let raw = upstox::fetch_holdings(&api_key, &access_token).await?;
    let holdings: Vec<broker::BrokerHolding> = raw.into_iter().map(Into::into).collect();
    let synced = holdings.len() as i64;

    // Write to DB (lock → write → release)
    {
        let mut conn = state.0.get()?;
        broker::write_broker_holdings(&mut *conn, "upstox", "Upstox", &holdings)?;
    }

    Ok(SyncResult { synced, errors: vec![] })
}

#[tauri::command]
pub fn disconnect_upstox(state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "DELETE FROM app_settings WHERE key IN (
            'upstox_api_key',
            'upstox_api_secret',
            'upstox_access_token',
            'upstox_token_date'
         )",
        [],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════
// ─── Angel One ───────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AngelStatus {
    pub has_config: bool,
    pub is_connected: bool,
    pub token_date: Option<String>,
}

#[tauri::command]
pub fn save_angel_config(
    api_key: String,
    client_id: String,
    state: State<DbState>,
) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('angel_api_key', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&api_key],
    )?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('angel_client_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&client_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_angel_status(state: State<DbState>) -> Result<AngelStatus> {
    let conn = state.0.get()?;

    let api_key: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='angel_api_key'",
            [],
            |r| r.get(0),
        )
        .ok();

    let client_id: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='angel_client_id'",
            [],
            |r| r.get(0),
        )
        .ok();

    let jwt_token: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='angel_jwt_token'",
            [],
            |r| r.get(0),
        )
        .ok();

    let token_date: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='angel_token_date'",
            [],
            |r| r.get(0),
        )
        .ok();

    let has_config = api_key.is_some() && client_id.is_some();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let is_connected = jwt_token.is_some() && token_date.as_deref() == Some(today.as_str());

    Ok(AngelStatus { has_config, is_connected, token_date })
}

#[tauri::command]
pub async fn login_angel(
    password: String,
    totp: String,
    state: State<'_, DbState>,
) -> Result<()> {
    // Read credentials (lock → read → release)
    let (api_key, client_id) = {
        let conn = state.0.get()?;
        let api_key = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='angel_api_key'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Angel One API key not configured".into()))?;
        let client_id = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='angel_client_id'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Angel One client ID not configured".into()))?;
        (api_key, client_id)
    };

    // Login via SmartAPI (no lock held during HTTP)
    let jwt_token = angel_one::login(&api_key, &client_id, &password, &totp).await?;

    // Store JWT + today's date (lock → write → release)
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    {
        let conn = state.0.get()?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('angel_jwt_token', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&jwt_token],
        )?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('angel_token_date', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&today],
        )?;
    }

    Ok(())
}

#[tauri::command]
pub async fn sync_angel_holdings(state: State<'_, DbState>) -> Result<SyncResult> {
    // Read credentials (lock → read → release)
    let (api_key, jwt_token) = {
        let conn = state.0.get()?;
        let api_key = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='angel_api_key'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| AppError::Validation("Angel One API key not configured".into()))?;
        let jwt_token = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='angel_jwt_token'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| {
                AppError::Validation("Not connected to Angel One. Please login again.".into())
            })?;
        (api_key, jwt_token)
    };

    // Fetch holdings (no lock held during HTTP)
    let raw = angel_one::fetch_holdings(&api_key, &jwt_token).await?;
    let holdings: Vec<broker::BrokerHolding> = raw.into_iter().map(Into::into).collect();
    let synced = holdings.len() as i64;

    // Write to DB (lock → write → release)
    {
        let mut conn = state.0.get()?;
        broker::write_broker_holdings(&mut *conn, "angel_one", "Angel One", &holdings)?;
    }

    Ok(SyncResult { synced, errors: vec![] })
}

#[tauri::command]
pub fn disconnect_angel(state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "DELETE FROM app_settings WHERE key IN (
            'angel_api_key',
            'angel_client_id',
            'angel_jwt_token',
            'angel_token_date'
         )",
        [],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════
// ─── Groww CSV Import ────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BrokerCsvRow {
    pub symbol: String,
    pub isin: Option<String>,
    pub quantity: f64,
    pub avg_price: f64,
    pub ltp: Option<f64>,
    pub exchange: Option<String>,
}

#[tauri::command]
pub fn import_broker_equity_csv(
    broker: String,
    display_name: String,
    rows: Vec<BrokerCsvRow>,
    state: State<DbState>,
) -> Result<ImportResult> {
    let holdings: Vec<broker::BrokerHolding> = rows
        .iter()
        .filter(|r| r.quantity > 0.0 && r.avg_price > 0.0)
        .map(|r| {
            let isin = r.isin.clone().unwrap_or_default();
            broker::BrokerHolding {
                symbol: r.symbol.clone(),
                exchange: r.exchange.clone().unwrap_or_else(|| "-".into()),
                isin: if isin.is_empty() { r.symbol.clone() } else { isin },
                quantity: r.quantity,
                avg_price: r.avg_price,
                current_price: r.ltp.unwrap_or(r.avg_price),
                name: None,
            }
        })
        .collect();

    let skipped = (rows.len() as i64) - (holdings.len() as i64);

    let mut conn = state.0.get()?;
    let imported = broker::write_broker_holdings(&mut *conn, &broker, &display_name, &holdings)?;

    Ok(ImportResult { imported, skipped })
}

// ═══════════════════════════════════════════════════════════════
// ─── Rust-side CSV Parsers ───────────────────────────────────
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub fn parse_broker_equity_csv(broker: String, csv_content: String) -> Result<Vec<BrokerCsvRow>> {
    csv_importer::parse_equity_csv(&broker, &csv_content)
}

#[tauri::command]
pub fn import_mf_csv(
    csv_content: String,
    account_name: String,
    state: State<DbState>,
) -> Result<ImportResult> {
    let rows = csv_importer::parse_mf_csv(&csv_content)?;

    let conn = state.0.get()?;

    conn.execute(
        "INSERT OR IGNORE INTO accounts
             (name, type, provider, is_active, created_at, updated_at)
         VALUES (?1, 'broker', 'csv_mf', 1, datetime('now'), datetime('now'))",
        rusqlite::params![account_name],
    )?;
    let account_id: i64 = conn.query_row(
        "SELECT id FROM accounts WHERE provider='csv_mf' AND name=?1",
        rusqlite::params![account_name],
        |r| r.get(0),
    )?;

    let mut imported = 0i64;
    let mut skipped = 0i64;

    for h in &rows {
        let res = conn.execute(
            "INSERT INTO mf_holdings
                 (account_id, scheme_code, scheme_name, amc_name, folio_number,
                  units, avg_nav, current_nav, is_direct, is_growth,
                  created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,datetime('now'),datetime('now'))
             ON CONFLICT(account_id, folio_number, scheme_code)
             DO UPDATE SET
                 units=excluded.units,
                 avg_nav=excluded.avg_nav,
                 current_nav=excluded.current_nav,
                 updated_at=datetime('now')",
            rusqlite::params![
                account_id,
                h.isin,
                h.scheme_name,
                h.amc_name,
                h.folio_number,
                h.units,
                h.avg_nav,
                h.current_nav,
                h.is_direct as i64,
                h.is_growth as i64,
            ],
        );
        match res {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ImportResult { imported, skipped })
}

#[tauri::command]
pub fn import_generic_asset_csv(
    asset_type: String,
    csv_content: String,
    state: State<DbState>,
) -> Result<ImportResult> {
    let conn = state.0.get()?;
    match asset_type.as_str() {
        "fd"      => csv_importer::import_fd_from_csv(&csv_content, &conn),
        "gold"    => csv_importer::import_gold_from_csv(&csv_content, &conn),
        "crypto"  => csv_importer::import_crypto_from_csv(&csv_content, &conn),
        "bond"    => csv_importer::import_bond_from_csv(&csv_content, &conn),
        "ppf_epf" => csv_importer::import_ppf_epf_from_csv(&csv_content, &conn),
        _ => Err(AppError::Validation(format!("Unknown asset type: {asset_type}"))),
    }
}
