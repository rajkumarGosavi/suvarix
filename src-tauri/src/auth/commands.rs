use tauri::State;
use crate::db::DbState;
use crate::error::{AppError, Result};
use super::master_password;

#[tauri::command]
pub fn is_password_set(state: State<DbState>) -> Result<bool> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let hash: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'password_hash'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(!hash.is_empty())
}

#[tauri::command]
pub fn setup_master_password(password: String, state: State<DbState>) -> Result<()> {
    let salt = master_password::generate_salt();
    let hash = master_password::hash_password(&password, &salt)?;
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE app_settings SET value = ?1 WHERE key = 'password_hash'",
        [&hash],
    )?;
    conn.execute(
        "UPDATE app_settings SET value = ?1 WHERE key = 'password_salt'",
        [&salt],
    )?;
    Ok(())
}

#[tauri::command]
pub fn verify_master_password(password: String, state: State<DbState>) -> Result<bool> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let hash: String = conn
        .query_row("SELECT value FROM app_settings WHERE key = 'password_hash'", [], |r| r.get(0))
        .map_err(|e| AppError::Database(e.to_string()))?;
    let salt: String = conn
        .query_row("SELECT value FROM app_settings WHERE key = 'password_salt'", [], |r| r.get(0))
        .map_err(|e| AppError::Database(e.to_string()))?;
    if hash.is_empty() {
        return Ok(false);
    }
    master_password::verify_password(&password, &salt, &hash)
}

#[tauri::command]
pub fn change_master_password(
    old_password: String,
    new_password: String,
    state: State<DbState>,
) -> Result<()> {
    let verified = verify_master_password(old_password, state.clone())?;
    if !verified {
        return Err(AppError::WrongPassword);
    }
    setup_master_password(new_password, state)
}
