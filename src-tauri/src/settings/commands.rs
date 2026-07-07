use tauri::{AppHandle, Manager, State};
use rusqlite::params;
use crate::db::DbState;
use crate::error::{AppError, Result};

/// Financial data tables, in dependency order (children first). Shared by
/// `wipe_all_data` and `restore_database` (which must clear existing rows
/// before importing a backup so `sqlcipher_export` inserts don't collide).
const DATA_TABLES: &[&str] = &[
    "sip_schedules",
    "transactions",
    "net_worth_snapshots",
    "budgets",
    "equity_holdings",
    "mf_holdings",
    "fd_holdings",
    "bond_holdings",
    "ppf_epf_holdings",
    "real_estate_holdings",
    "gold_holdings",
    "crypto_holdings",
    "insurance_holdings",
    "loans",
    "credit_cards",
    "accounts",
];

#[tauri::command]
pub fn get_setting(key: String, state: State<DbState>) -> Result<String> {
    let conn = state.0.get()?;
    conn.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        [&key],
        |r| r.get(0),
    ).map_err(|_| AppError::NotFound(key))
}

#[tauri::command]
pub fn set_setting(key: String, value: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&key, &value],
    )?;
    Ok(())
}

/// SQLCipher rejects `rusqlite::backup::Backup` on encrypted connections
/// ("backup is not supported with encrypted databases"), so backup/restore
/// go through `ATTACH ... KEY ...; SELECT sqlcipher_export(...)` instead —
/// the same workaround `migrate_from_device_key` uses in db/mod.rs. Both
/// commands reuse the current master password (kept in-memory only while
/// unlocked, see `DbPool::current_password`) as the key for the sibling file.
#[tauri::command]
pub fn backup_database(dest_path: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    let password = state.0.current_password()?;
    if std::path::Path::new(&dest_path).exists() {
        std::fs::remove_file(&dest_path)?;
    }
    conn.execute("ATTACH DATABASE ?1 AS backup_db KEY ?2", params![dest_path, password])?;
    let result = conn.query_row("SELECT sqlcipher_export('backup_db')", [], |_| Ok(()));
    conn.execute("DETACH DATABASE backup_db", [])?;
    result?;
    Ok(())
}

/// `sqlcipher_export` issues bare `CREATE TABLE` (no `IF NOT EXISTS`) for every
/// table in the source db, so exporting into the live `main` connection fails
/// with "table already exists" as soon as the app's own migrations have run.
/// Instead we export the backup into a brand-new sibling file (same pattern as
/// `migrate_from_device_key`), then swap it in for the live DB file and
/// re-unlock — mirroring how `rekey` rebuilds the pool after changing the key.
#[tauri::command]
pub fn restore_database(src_path: String, state: State<DbState>) -> Result<()> {
    let password = state.0.current_password()?;
    let db_path = state.0.db_path().to_string();
    let temp_path = std::path::Path::new(&db_path).with_extension("db.restoretmp");
    let temp_path_str = temp_path.to_string_lossy().into_owned();

    if temp_path.exists() {
        std::fs::remove_file(&temp_path)?;
    }

    let export_result = (|| -> Result<()> {
        let src = rusqlite::Connection::open(&src_path)?;
        src.execute_batch(&format!("PRAGMA key = '{}';", password.replace('\'', "''")))?;
        src.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
            .map_err(|_| AppError::Database("backup file could not be opened with the current password".into()))?;
        src.execute("ATTACH DATABASE ?1 AS restored KEY ?2", params![temp_path_str, password])?;
        let export = src.query_row("SELECT sqlcipher_export('restored')", [], |_| Ok(()));
        src.execute("DETACH DATABASE restored", [])?;
        export?;
        Ok(())
    })();

    if let Err(e) = export_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    // Drop the live pool before touching the DB file on disk.
    state.0.lock();

    let swap_result = (|| -> Result<()> {
        std::fs::rename(&temp_path, &db_path)?;
        for ext in ["-wal", "-shm"] {
            let sidecar = format!("{db_path}{ext}");
            if std::path::Path::new(&sidecar).exists() {
                std::fs::remove_file(&sidecar)?;
            }
        }
        Ok(())
    })();

    // Re-unlock with the same password regardless, so the app isn't left locked.
    state.0.unlock(&password)?;
    swap_result?;
    Ok(())
}

#[tauri::command]
pub fn wipe_all_data(state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    for table in DATA_TABLES {
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
