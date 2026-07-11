use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::{AppError, Result};

pub mod migrations;

pub struct DbPool {
    db_path: String,
    inner: Mutex<Option<Pool<SqliteConnectionManager>>>,
    /// Current master password, kept in memory only while unlocked — needed to
    /// ATTACH sibling SQLCipher databases (backup/restore) with the same key.
    /// Cleared on lock().
    password: Mutex<Option<String>>,
}

/// Wraps the pool in `Arc` so the background reminder scheduler (a tokio task
/// spawned independently of any Tauri command's invoke context) can hold its
/// own clone of the same pool without borrowing from `State<DbState>`.
pub struct DbState(pub Arc<DbPool>);

const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

impl DbPool {
    pub fn new(db_path: String) -> Self {
        Self { db_path, inner: Mutex::new(None), password: Mutex::new(None) }
    }

    /// Returns a pooled connection, or AuthRequired if not yet unlocked.
    pub fn get(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.inner
            .lock()
            .map_err(|_| AppError::Database("pool lock error".into()))?
            .as_ref()
            .ok_or(AppError::AuthRequired)?
            .get()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// True if DB file exists and is encrypted (not plain SQLite).
    pub fn is_password_set(&self) -> bool {
        let path = Path::new(&self.db_path);
        path.exists() && !is_plain_sqlite(path)
    }

    /// First-run: create new encrypted DB with password and run migrations.
    pub fn initialize(&self, password: &str) -> Result<()> {
        tracing::debug!(db_path = %self.db_path, "initializing new db");
        let pool = build_pool(&self.db_path, password)?;
        {
            let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
            migrations::run_migrations(&conn)?;
            ensure_device_id(&conn)?;
        }
        *self.inner.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(pool);
        *self.password.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(password.to_string());
        tracing::debug!("db initialized");
        Ok(())
    }

    /// Path to the live encrypted DB file on disk.
    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// Current master password, or AuthRequired if not unlocked. Needed to ATTACH
    /// sibling SQLCipher databases (e.g. backup/restore) with the same key.
    pub fn current_password(&self) -> Result<String> {
        self.password
            .lock()
            .map_err(|_| AppError::Database("password lock error".into()))?
            .clone()
            .ok_or(AppError::AuthRequired)
    }

    /// Try to open DB with password. On success stores pool. Returns false on wrong password.
    /// Transparently migrates from old random-device-key encryption if present.
    pub fn unlock(&self, password: &str) -> Result<bool> {
        let path = Path::new(&self.db_path);
        if !path.exists() {
            tracing::debug!("unlock attempted but db file does not exist yet");
            return Ok(false);
        }

        if try_open(path, password)? {
            let pool = build_pool(&self.db_path, password)?;
            {
                let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
                migrations::run_migrations(&conn)?;
                ensure_device_id(&conn)?;
            }
            *self.inner.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(pool);
            *self.password.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(password.to_string());
            tracing::debug!("unlocked");
            return Ok(true);
        }

        // Migration: DB encrypted with old random device key → re-encrypt with passphrase.
        // Can be removed after v0.6 once migration window closes.
        if let Ok(entry) = keyring::Entry::new("suvarix", "db_key") {
            if let Ok(device_key) = entry.get_password() {
                if migrate_from_device_key(path, &device_key, password).is_ok() {
                    tracing::debug!("migrated from legacy device-key encryption to passphrase");
                    let _ = entry.delete_credential();
                    let pool = build_pool(&self.db_path, password)?;
                    {
                        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
                        migrations::run_migrations(&conn)?;
                        ensure_device_id(&conn)?;
                    }
                    *self.inner.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(pool);
                    *self.password.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(password.to_string());
                    tracing::debug!("unlocked");
                    return Ok(true);
                }
            }
        }

        tracing::debug!("unlock failed: wrong password");
        Ok(false)
    }

    /// Verify password without disturbing the open pool.
    pub fn verify_password(&self, password: &str) -> Result<bool> {
        let path = Path::new(&self.db_path);
        if !path.exists() { return Ok(false); }
        try_open(path, password)
    }

    /// Drop the connection pool (auto-lock / manual lock).
    pub fn lock(&self) {
        tracing::debug!("locking db");
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.password.lock() {
            *guard = None;
        }
    }

    /// Change passphrase: PRAGMA rekey on existing conn, then rebuild pool.
    pub fn rekey(&self, new_password: &str) -> Result<()> {
        tracing::debug!("rekeying db");
        // Step 1: rekey while holding the guard (prevents new checkouts racing)
        {
            let guard = self.inner.lock().map_err(|_| AppError::Database("lock error".into()))?;
            let conn = guard.as_ref().ok_or(AppError::AuthRequired)?
                .get().map_err(|e| AppError::Database(e.to_string()))?;
            let escaped = new_password.replace('\'', "''");
            conn.execute_batch(&format!("PRAGMA rekey = '{escaped}';"))
                .map_err(|e| AppError::Database(format!("rekey: {e}")))?;
        }
        // Step 2: rebuild pool so with_init uses new password (guard released above)
        let new_pool = build_pool(&self.db_path, new_password)?;
        *self.inner.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(new_pool);
        *self.password.lock().map_err(|_| AppError::Database("lock error".into()))? = Some(new_password.to_string());
        tracing::debug!("rekeyed");
        Ok(())
    }
}

impl DbState {
    pub fn new(db_path: String) -> Self {
        DbState(Arc::new(DbPool::new(db_path)))
    }
}

// ── Private helpers ────────────────────────────────────────────────────────

fn build_pool(db_path: &str, password: &str) -> Result<Pool<SqliteConnectionManager>> {
    let escaped = password.replace('\'', "''");
    let init_sql = format!(
        "PRAGMA key = '{escaped}';\n\
         PRAGMA journal_mode=WAL;\n\
         PRAGMA foreign_keys=ON;\n\
         PRAGMA busy_timeout=5000;\n\
         PRAGMA synchronous=NORMAL;\n\
         PRAGMA temp_store=MEMORY;"
    );
    let manager = SqliteConnectionManager::file(db_path)
        .with_init(move |conn| conn.execute_batch(&init_sql));
    Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| AppError::Database(e.to_string()))
}

/// Try opening the DB with password; returns true if the key is correct.
fn try_open(path: &Path, password: &str) -> Result<bool> {
    let conn = Connection::open(path)
        .map_err(|e| AppError::Database(e.to_string()))?;
    let escaped = password.replace('\'', "''");
    conn.execute_batch(&format!("PRAGMA key = '{escaped}';"))
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(conn
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .is_ok())
}

/// Generates this device's persistent random id once, if not already set.
/// Stamped into `sync_hlc` by the per-table triggers (see
/// `migrations::migration_021_hlc_state_and_triggers`) as the tiebreak suffix
/// when two devices' HLCs otherwise compare equal. Must never travel through a
/// sync import — `backup::commands::SYNC_SETTINGS_KEYS` is an allow-list of the
/// only settings keys that get exported/imported, and `device_id` is
/// deliberately absent from it: importing a peer's payload would otherwise
/// silently overwrite this device's own id with theirs.
fn ensure_device_id(conn: &Connection) -> Result<()> {
    let exists = conn
        .query_row("SELECT 1 FROM app_settings WHERE key = 'device_id'", [], |_| Ok(()))
        .is_ok();
    if exists {
        return Ok(());
    }
    let bytes: [u8; 16] = rand::random();
    let device_id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    conn.execute("INSERT INTO app_settings (key, value) VALUES ('device_id', ?1)", [device_id])
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Unlike `ensure_device_id` (generate-once-if-missing, used on normal
/// unlock/initialize), a restore always mints a *fresh* device_id
/// unconditionally — called from `settings::commands::restore_database`
/// after the DB file swap, since the restored file carries whatever
/// device_id was live on whichever device (possibly a different physical
/// device, possibly this same one at an earlier point) made that backup.
/// Keeping it would risk two physically different devices sharing an HLC
/// tiebreak identity; comparing old vs. restored first wouldn't actually
/// catch that (neither value's history is visible from here), so this
/// always overwrites rather than only overwriting on a detected mismatch.
pub(crate) fn regenerate_device_id(conn: &Connection) -> Result<()> {
    let bytes: [u8; 16] = rand::random();
    let device_id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('device_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [device_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

fn is_plain_sqlite(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut magic = [0u8; 16];
    let _ = f.read_exact(&mut magic);
    magic.as_slice() == SQLITE_MAGIC
}

/// Migrate DB encrypted with random hex device key → passphrase via sqlcipher_export.
fn migrate_from_device_key(path: &Path, device_key: &str, password: &str) -> Result<()> {
    let conn = Connection::open(path)
        .map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{device_key}'\";"))
        .map_err(|e| AppError::Database(e.to_string()))?;
    // Validate device key is correct before proceeding
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(|_| AppError::Database("device key mismatch".into()))?;
    // Export to temp file encrypted with passphrase
    let temp_path = path.with_extension("db.tmp");
    let temp_str = temp_path.to_string_lossy().replace('\\', "/");
    let escaped = password.replace('\'', "''");
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{temp_str}' AS migrated KEY '{escaped}';\
         SELECT sqlcipher_export('migrated');\
         DETACH DATABASE migrated;"
    )).map_err(|e| AppError::Database(format!("migrate export: {e}")))?;
    drop(conn);
    std::fs::rename(&temp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    #[test]
    fn regenerate_device_id_always_overwrites() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();
        let before: String =
            conn.query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap();

        regenerate_device_id(&conn).unwrap();

        let after: String =
            conn.query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap();
        assert_ne!(before, after, "regenerate_device_id must always produce a new value");
    }

    #[test]
    fn ensure_device_id_is_idempotent_unlike_regenerate() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();
        let first: String =
            conn.query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap();

        ensure_device_id(&conn).unwrap(); // device_id already exists — must be a no-op

        let second: String =
            conn.query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap();
        assert_eq!(first, second, "ensure_device_id must never change an existing device_id");
    }
}
