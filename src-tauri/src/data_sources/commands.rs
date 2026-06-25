use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};
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
    pub amc_name: String,
    pub is_direct: bool,
    pub is_growth: bool,
}

#[derive(serde::Serialize)]
pub struct ImportResult {
    pub imported: i64,
    pub skipped: i64,
}

#[tauri::command]
pub fn import_cas_mf(holdings: Vec<CasHoldingInput>, state: State<DbState>) -> Result<ImportResult> {
    let mut conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;

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
             VALUES (?1,?2,?3,?4,?5,?6,?7,?7,?8,?9,datetime('now'),datetime('now'))
             ON CONFLICT(account_id, folio_number, scheme_code)
             DO UPDATE SET
                 units=excluded.units,
                 current_nav=excluded.current_nav,
                 updated_at=datetime('now')",
            rusqlite::params![
                account_id,
                h.isin,        // use ISIN as scheme_code until AMFI lookup is added
                h.scheme_name,
                h.amc_name,
                h.folio_number,
                h.units,
                h.nav,         // avg_nav = current NAV from CAS (best available)
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
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
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
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;

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
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
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
        let conn = state
            .0
            .lock()
            .map_err(|_| AppError::Database("lock error".into()))?;

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
        let conn = state
            .0
            .lock()
            .map_err(|_| AppError::Database("lock error".into()))?;

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
        let conn = state
            .0
            .lock()
            .map_err(|_| AppError::Database("lock error".into()))?;

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
    let holdings = zerodha::fetch_holdings(&api_key, &access_token).await?;
    let synced = holdings.len() as i64;

    // Write to DB (lock → write → release)
    {
        let mut conn = state
            .0
            .lock()
            .map_err(|_| AppError::Database("lock error".into()))?;

        // Ensure a Zerodha broker account exists
        conn.execute(
            "INSERT OR IGNORE INTO accounts
                 (name, type, provider, is_active, created_at, updated_at)
             VALUES ('Zerodha', 'broker', 'zerodha', 1, datetime('now'), datetime('now'))",
            [],
        )?;

        let account_id: i64 = conn.query_row(
            "SELECT id FROM accounts WHERE provider='zerodha'",
            [],
            |r| r.get(0),
        )?;

        // Atomically replace all Zerodha equity holdings
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM equity_holdings WHERE account_id=?1",
            [account_id],
        )?;

        let price_ts = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        for h in &holdings {
            tx.execute(
                "INSERT INTO equity_holdings
                     (account_id, isin, symbol, exchange, name,
                      quantity, avg_buy_price, current_price, price_updated_at,
                      created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,datetime('now'),datetime('now'))",
                rusqlite::params![
                    account_id,
                    h.isin,
                    h.tradingsymbol,
                    h.exchange,
                    h.tradingsymbol, // name = tradingsymbol (no company name in Kite holdings API)
                    h.quantity,
                    h.average_price,
                    h.last_price,
                    price_ts,
                ],
            )?;
        }

        tx.commit()?;
    }

    Ok(SyncResult { synced, errors: vec![] })
}
