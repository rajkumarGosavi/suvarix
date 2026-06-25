use tauri::{AppHandle, Manager, State};
use crate::db::DbState;
use crate::error::{AppError, Result};

#[tauri::command]
pub fn get_setting(key: String, state: State<DbState>) -> Result<String> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        [&key],
        |r| r.get(0),
    ).map_err(|_| AppError::NotFound(key))
}

#[tauri::command]
pub fn set_setting(key: String, value: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&key, &value],
    )?;
    Ok(())
}

#[tauri::command]
pub fn backup_database(dest_path: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut dest = rusqlite::Connection::open(&dest_path)
        .map_err(|e| AppError::Io(e.to_string()))?;
    let backup = rusqlite::backup::Backup::new(&conn, &mut dest)
        .map_err(|e| AppError::Database(e.to_string()))?;
    backup.run_to_completion(5, std::time::Duration::from_millis(250), None)
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn restore_database(src_path: String, state: State<DbState>) -> Result<()> {
    let mut conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let src = rusqlite::Connection::open(&src_path)
        .map_err(|e| AppError::Io(e.to_string()))?;
    let restore = rusqlite::backup::Backup::new(&src, &mut conn)
        .map_err(|e| AppError::Database(e.to_string()))?;
    restore.run_to_completion(5, std::time::Duration::from_millis(250), None)
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn wipe_all_data(state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    // Delete all financial data tables in dependency order (children first)
    let tables = [
        "sip_schedules",
        "transactions",
        "net_worth_snapshots",
        "budgets",
        "equity_holdings",
        "mf_holdings",
        "fd_holdings",
        "ppf_epf_holdings",
        "real_estate_holdings",
        "gold_holdings",
        "crypto_holdings",
        "insurance_holdings",
        "loans",
        "credit_cards",
        "accounts",
    ];
    for table in &tables {
        conn.execute(&format!("DELETE FROM {}", table), [])
            .map_err(|e| AppError::Database(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_data_dir(app: AppHandle) -> Result<String> {
    app.path()
        .app_data_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub fn write_csv(path: String, content: String) -> Result<()> {
    std::fs::write(&path, content)
        .map_err(|e| AppError::Io(e.to_string()))
}
