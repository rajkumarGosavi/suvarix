use crate::error::Result;

/// Normalised holding record produced by any broker adapter.
/// All sync commands convert their broker-specific structs into this before writing to DB.
#[derive(Debug, Clone)]
pub struct BrokerHolding {
    pub symbol: String,
    pub exchange: String,
    pub isin: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub current_price: f64,
    pub name: Option<String>,
}

/// Upsert broker account + atomically replace all its equity holdings.
///
/// Called from every `sync_*_holdings` command after fetching from the remote API.
/// The connection must be obtained with `let mut conn = state.0.lock()...?;` so the
/// transaction borrow can get `&mut Connection`.
pub fn write_broker_holdings(
    conn: &mut rusqlite::Connection,
    provider: &str,
    display_name: &str,
    holdings: &[BrokerHolding],
) -> Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO accounts
             (name, type, provider, is_active, created_at, updated_at)
         VALUES (?1, 'broker', ?2, 1, datetime('now'), datetime('now'))",
        rusqlite::params![display_name, provider],
    )?;

    let account_id: i64 = conn.query_row(
        "SELECT id FROM accounts WHERE provider=?1",
        [provider],
        |r| r.get(0),
    )?;

    let price_ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM equity_holdings WHERE account_id=?1", [account_id])?;

    for h in holdings {
        tx.execute(
            "INSERT INTO equity_holdings
                 (account_id, isin, symbol, exchange, name,
                  quantity, avg_buy_price, current_price, price_updated_at,
                  created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,datetime('now'),datetime('now'))",
            rusqlite::params![
                account_id,
                h.isin,
                h.symbol,
                h.exchange,
                h.name.as_deref().unwrap_or(&h.symbol),
                h.quantity,
                h.avg_price,
                h.current_price,
                price_ts,
            ],
        )?;
    }

    tx.commit()?;
    Ok(holdings.len() as i64)
}
