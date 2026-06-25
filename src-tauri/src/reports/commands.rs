use tauri::State;
use serde::{Deserialize, Serialize};
use crate::db::DbState;
use crate::error::{AppError, Result};
use super::capital_gains;

pub use capital_gains::CapitalGainsReport;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthSnapshot {
    pub snapshot_date: String,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub net_worth: f64,
}

#[tauri::command]
pub fn get_capital_gains(fy: String, method: String, state: State<DbState>) -> Result<CapitalGainsReport> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    capital_gains::calculate(&conn, &fy, &method)
}

#[tauri::command]
pub fn get_net_worth_history(months: i64, state: State<DbState>) -> Result<Vec<NetWorthSnapshot>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT snapshot_date, total_assets, total_liabilities, net_worth
         FROM net_worth_snapshots
         WHERE snapshot_date >= date('now', '-' || ?1 || ' months')
         ORDER BY snapshot_date"
    )?;
    let rows = stmt.query_map([months], |r| Ok(NetWorthSnapshot {
        snapshot_date: r.get(0)?, total_assets: r.get(1)?,
        total_liabilities: r.get(2)?, net_worth: r.get(3)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn take_net_worth_snapshot(state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let summary = crate::portfolio::calculator::calc_net_worth(&conn)?;
    conn.execute(
        "INSERT OR REPLACE INTO net_worth_snapshots
         (snapshot_date, total_assets, total_liabilities, net_worth)
         VALUES (date('now'), ?1, ?2, ?3)",
        rusqlite::params![summary.total_assets, summary.total_liabilities, summary.net_worth],
    )?;
    Ok(())
}
