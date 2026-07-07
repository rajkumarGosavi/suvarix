use std::collections::HashMap;

use aes_gcm::{
    aead::{generic_array::GenericArray, Aead, KeyInit},
    Aes256Gcm,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::random;
use sha2::Sha256;
use std::io::Write as _;
use tauri::{AppHandle, State};
use tauri_plugin_fs::{FsExt, OpenOptions};

use crate::constants::APP_NAME;
use crate::db::{DbPool, DbState};
use crate::error::{AppError, Result};

// ── File format ───────────────────────────────────────────────

const MAGIC: &[u8; 4] = b"FFBK";
const FORMAT_VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 33

// ── Sync scope ────────────────────────────────────────────────

// FK-safe INSERT order: accounts before anything that references it
const DATA_TABLES: &[&str] = &[
    "accounts",
    "equity_holdings",
    "mf_holdings",
    "sip_schedules",
    "fd_holdings",
    "ppf_epf_holdings",
    "real_estate_holdings",
    "gold_holdings",
    "crypto_holdings",
    "insurance_holdings",
    "bond_holdings",
    "loans",
    "credit_cards",
    "transactions",
    "budgets",
    "net_worth_snapshots",
    "import_log",
    "goals",
    "bills",
    "recurring_transactions",
    "milestones",
];

// Excludes: password_hash, password_salt (auth), *_access_token/*_token_date (session)
const SYNC_SETTINGS_KEYS: &[&str] = &[
    "currency",
    "auto_lock_minutes",
    "theme",
    "zerodha_api_key",
    "zerodha_api_secret",
    "upstox_api_key",
    "upstox_api_secret",
    "angel_api_key",
    "angel_client_id",
];

// ── Return types ──────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub rows_exported: usize,
    pub exported_at: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub rows_imported: usize,
    pub tables_imported: usize,
}

// ── Payload schema ────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(Debug))]
struct BackupPayload {
    version: u8,
    exported_at: String,
    tables: HashMap<String, Vec<serde_json::Value>>,
    settings: HashMap<String, String>,
}

// ── Commands ─────────────────────────────────────────────────

#[tauri::command]
pub fn export_sync_backup(
    app: AppHandle,
    dest_path: String,
    password: String,
    state: State<DbState>,
) -> Result<ExportSummary> {
    export_sync_backup_impl(&app, &dest_path, &password, &state.0)
}

/// Shared by the `export_sync_backup` command and the background auto-sync
/// scheduler (which holds an `Arc<DbPool>` directly, not a command `State`).
pub(crate) fn export_sync_backup_impl(
    app: &AppHandle,
    dest_path: &str,
    password: &str,
    db: &DbPool,
) -> Result<ExportSummary> {
    let (payload, total_rows) = {
        let conn = db.get()?;
        build_backup_payload(&conn)?
    };
    let exported_at = payload.exported_at.clone();
    let file = encrypt_backup_bytes(&payload, password)?;

    write_via_fs(app, dest_path, &file)
        .map_err(|e| AppError::Database(format!("write backup: {e}")))?;

    Ok(ExportSummary { rows_exported: total_rows, exported_at })
}

/// Snapshots all synced tables + allowed settings into a `BackupPayload`.
/// Split from `export_sync_backup_impl` so tests can exercise the payload
/// logic without an `AppHandle` (file IO is the only Tauri-dependent part).
fn build_backup_payload(conn: &rusqlite::Connection) -> Result<(BackupPayload, usize)> {
    let mut tables: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    let mut total_rows = 0usize;
    for &table in DATA_TABLES {
        let rows = dump_table(conn, table)?;
        total_rows += rows.len();
        tables.insert(table.to_string(), rows);
    }

    let mut settings: HashMap<String, String> = HashMap::new();
    for &key in SYNC_SETTINGS_KEYS {
        if let Ok(val) = conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [key],
            |r| r.get::<_, String>(0),
        ) {
            settings.insert(key.to_string(), val);
        }
    }

    // UTC so timestamps from devices in different offsets compare correctly.
    let exported_at = chrono::Utc::now().to_rfc3339();
    Ok((
        BackupPayload { version: FORMAT_VERSION, exported_at, tables, settings },
        total_rows,
    ))
}

/// Serializes + encrypts a payload into the `.svbak` on-disk byte format:
/// MAGIC + version + salt + nonce + ciphertext.
fn encrypt_backup_bytes(payload: &BackupPayload, password: &str) -> Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(payload)
        .map_err(|e| AppError::Parse(format!("serialize backup: {e}")))?;

    let salt: [u8; SALT_LEN] = random();
    let nonce_bytes: [u8; NONCE_LEN] = random();
    let key = derive_key(password, &salt);
    let ciphertext = aes_encrypt(&key, &nonce_bytes, &json_bytes)?;

    let mut file = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    file.extend_from_slice(MAGIC);
    file.push(FORMAT_VERSION);
    file.extend_from_slice(&salt);
    file.extend_from_slice(&nonce_bytes);
    file.extend_from_slice(&ciphertext);
    Ok(file)
}

#[tauri::command]
pub fn import_sync_backup(
    app: AppHandle,
    src_path: String,
    password: String,
    state: State<DbState>,
) -> Result<ImportSummary> {
    import_sync_backup_impl(&app, &src_path, &password, &state.0)
}

/// Shared by the `import_sync_backup` command and the background auto-sync
/// scheduler (which holds an `Arc<DbPool>` directly, not a command `State`).
pub(crate) fn import_sync_backup_impl(
    app: &AppHandle,
    src_path: &str,
    password: &str,
    db: &DbPool,
) -> Result<ImportSummary> {
    let payload = read_backup_payload(app, src_path, password)?;
    apply_backup_payload(db, &payload)
}

/// Replaces all synced tables + settings with the payload's contents.
/// Split from `import_sync_backup_impl` so tests can exercise the DB write
/// logic without an `AppHandle`.
fn apply_backup_payload(db: &DbPool, payload: &BackupPayload) -> Result<ImportSummary> {
    let mut conn = db.get()?;

    // FK checks off during bulk replace so delete/insert order doesn't matter
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let result = (|| -> Result<ImportSummary> {
        let tx = conn.transaction()?;

        for &table in DATA_TABLES {
            tx.execute(&format!("DELETE FROM \"{table}\""), [])?;
        }

        let mut total_rows = 0usize;
        let mut tables_imported = 0usize;
        for &table in DATA_TABLES {
            if let Some(rows) = payload.tables.get(table) {
                if !rows.is_empty() {
                    let n = restore_table(&tx, table, rows)?;
                    total_rows += n;
                    tables_imported += 1;
                }
            }
        }

        for key in SYNC_SETTINGS_KEYS {
            if let Some(val) = payload.settings.get(*key) {
                tx.execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [key, val.as_str()],
                )?;
            }
        }

        tx.commit()?;
        Ok(ImportSummary { rows_imported: total_rows, tables_imported })
    })();

    // Always restore FK enforcement
    let _ = conn.execute_batch("PRAGMA foreign_keys = ON");

    result
}

/// Stores the user's sync password, encrypted at rest with the master
/// password — needed so the background auto-sync scheduler can encrypt/decrypt
/// `.svbak` files unattended, without prompting the user each tick.
#[tauri::command]
pub fn set_sync_password(password: String, state: State<DbState>) -> Result<()> {
    let master = state.0.current_password()?;
    let encrypted = encrypt_sync_password(&password, &master)?;
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('sync_password_encrypted', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&encrypted],
    )?;
    Ok(())
}

#[tauri::command]
pub fn has_sync_password(state: State<DbState>) -> Result<bool> {
    let conn = state.0.get()?;
    Ok(conn
        .query_row(
            "SELECT 1 FROM app_settings WHERE key='sync_password_encrypted'",
            [],
            |_| Ok(()),
        )
        .is_ok())
}

/// Runs one auto-sync pull-then-push cycle immediately (manual "Sync now"),
/// reusing the exact same logic the background scheduler ticks on.
#[tauri::command]
pub fn sync_now(app: AppHandle, state: State<DbState>) -> Result<crate::backup::scheduler::SyncOutcome> {
    crate::backup::scheduler::run_tick(&app, &state.0)
}

// ── File I/O ──────────────────────────────────────────────────
// Uses tauri-plugin-fs instead of std::fs because on Android the path
// picked via the save/open dialog is a content:// SAF URI, which
// std::fs::write/read cannot open directly.

fn write_via_fs(app: &AppHandle, path: &str, bytes: &[u8]) -> std::io::Result<()> {
    let file_path = path.parse::<tauri_plugin_fs::FilePath>().unwrap();
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut file = app.fs().open(file_path, opts)?;
    file.write_all(bytes)
}

/// Reads + decrypts a `.svbak` file into its payload. Shared by
/// `import_sync_backup_impl` and the auto-sync scheduler's staleness check
/// (which only needs `payload.exported_at`, not a full DB write).
fn read_backup_payload(app: &AppHandle, path: &str, password: &str) -> Result<BackupPayload> {
    let data = app
        .fs()
        .read(path.parse::<tauri_plugin_fs::FilePath>().unwrap())
        .map_err(|e| AppError::Database(format!("read backup: {e}")))?;
    decrypt_backup_bytes(&data, password)
}

/// Validates + decrypts raw `.svbak` bytes into a payload. Inverse of
/// `encrypt_backup_bytes`; split from `read_backup_payload` so tests can
/// exercise format validation and crypto without an `AppHandle`.
fn decrypt_backup_bytes(data: &[u8], password: &str) -> Result<BackupPayload> {
    if data.len() < HEADER_LEN {
        return Err(AppError::Validation(format!("not a valid {APP_NAME} backup file")));
    }
    if &data[0..4] != MAGIC {
        return Err(AppError::Validation(format!("not a valid {APP_NAME} backup file")));
    }
    if data[4] != FORMAT_VERSION {
        return Err(AppError::Validation(format!("unsupported backup version {}", data[4])));
    }

    let salt = &data[5..5 + SALT_LEN];
    let nonce_bytes = &data[5 + SALT_LEN..HEADER_LEN];
    let ciphertext = &data[HEADER_LEN..];

    let key = derive_key(password, salt);
    let json_bytes = aes_decrypt(&key, nonce_bytes, ciphertext)?;

    serde_json::from_slice(&json_bytes).map_err(|e| AppError::Parse(format!("parse backup: {e}")))
}

/// Peeks a `.svbak` file's `exported_at` without touching the DB — used by
/// the auto-sync scheduler to decide whether a remote copy is newer than
/// what this device last applied, before deciding to import it.
pub(crate) fn peek_exported_at(app: &AppHandle, path: &str, password: &str) -> Result<String> {
    Ok(read_backup_payload(app, path, password)?.exported_at)
}

/// Encrypts the user's sync password at rest, keyed by the master password —
/// same PBKDF2 + AES-256-GCM primitives as the `.svbak` format, just with a
/// compact `base64(salt || nonce || ciphertext)` encoding instead of a file
/// header, since this is stored as a single `app_settings` value.
pub(crate) fn encrypt_sync_password(sync_password: &str, master_password: &str) -> Result<String> {
    let salt: [u8; SALT_LEN] = random();
    let nonce_bytes: [u8; NONCE_LEN] = random();
    let key = derive_key(master_password, &salt);
    let ciphertext = aes_encrypt(&key, &nonce_bytes, sync_password.as_bytes())?;

    let mut buf = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    buf.extend_from_slice(&salt);
    buf.extend_from_slice(&nonce_bytes);
    buf.extend_from_slice(&ciphertext);
    Ok(B64.encode(buf))
}

pub(crate) fn decrypt_sync_password(encrypted_b64: &str, master_password: &str) -> Result<String> {
    let buf = B64
        .decode(encrypted_b64)
        .map_err(|e| AppError::Validation(format!("bad sync password: {e}")))?;
    if buf.len() < SALT_LEN + NONCE_LEN {
        return Err(AppError::Validation("bad sync password".into()));
    }
    let salt = &buf[0..SALT_LEN];
    let nonce_bytes = &buf[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &buf[SALT_LEN + NONCE_LEN..];
    let key = derive_key(master_password, salt);
    let plain = aes_decrypt(&key, nonce_bytes, ciphertext)?;
    String::from_utf8(plain).map_err(|e| AppError::Validation(format!("bad sync password: {e}")))
}

// ── Crypto ────────────────────────────────────────────────────

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 100_000, &mut key);
    key
}

fn aes_encrypt(key: &[u8; 32], nonce_bytes: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(GenericArray::from_slice(key));
    let nonce = GenericArray::from_slice(nonce_bytes);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| AppError::Validation("encryption failed".into()))
}

fn aes_decrypt(key: &[u8; 32], nonce_bytes: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(GenericArray::from_slice(key));
    let nonce = GenericArray::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::Validation("wrong password or corrupted file".into()))
}

// ── DB helpers ────────────────────────────────────────────────

fn dump_table(conn: &rusqlite::Connection, table: &str) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM \"{table}\""))
        .map_err(|e| AppError::Database(format!("prepare {table}: {e}")))?;
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let col_count = col_names.len();
    let rows = stmt
        .query_map([], |row| {
            let mut map = serde_json::Map::new();
            for i in 0..col_count {
                let val = match row.get_ref(i) {
                    Ok(vref) => value_ref_to_json(vref),
                    Err(_) => serde_json::Value::Null,
                };
                map.insert(col_names[i].clone(), val);
            }
            Ok(serde_json::Value::Object(map))
        })
        .map_err(|e| AppError::Database(format!("dump {table}: {e}")))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn restore_table(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    rows: &[serde_json::Value],
) -> Result<usize> {
    if rows.is_empty() {
        return Ok(0);
    }
    let cols: Vec<String> = rows[0]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    if cols.is_empty() {
        return Ok(0);
    }

    let col_list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let placeholders = (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
    let sql = format!("INSERT OR REPLACE INTO \"{table}\" ({col_list}) VALUES ({placeholders})");

    let mut count = 0usize;
    for row in rows {
        if let Some(obj) = row.as_object() {
            let params: Vec<rusqlite::types::Value> = cols
                .iter()
                .map(|c| json_to_sql_value(obj.get(c).unwrap_or(&serde_json::Value::Null)))
                .collect();
            tx.execute(&sql, rusqlite::params_from_iter(params))
                .map_err(|e| AppError::Database(format!("insert into {table}: {e}")))?;
            count += 1;
        }
    }
    Ok(count)
}

fn value_ref_to_json(v: rusqlite::types::ValueRef<'_>) -> serde_json::Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(i) => serde_json::Value::Number(i.into()),
        ValueRef::Real(f) => serde_json::json!(f),
        ValueRef::Text(s) => serde_json::Value::String(String::from_utf8_lossy(s).into_owned()),
        ValueRef::Blob(b) => serde_json::Value::String(B64.encode(b)),
    }
}

fn json_to_sql_value(v: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(*b as i64),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Value::Integer(i) }
            else { Value::Real(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        _ => Value::Text(v.to_string()),
    }
}

// ── Tests ─────────────────────────────────────────────────────
// File IO goes through `AppHandle` (tauri-plugin-fs) which can't be
// constructed in tests (see tests/common/mod.rs on the MockRuntime issue),
// so tests exercise the split-out pure/DB halves: payload build/apply,
// `.svbak` byte-format encrypt/decrypt, and sync-password crypto.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;
    use rusqlite::params;

    const SYNC_PW: &str = "sync-secret-42";
    const MASTER_PW: &str = "master-pw";

    fn minimal_payload() -> BackupPayload {
        let mut tables = HashMap::new();
        tables.insert(
            "accounts".to_string(),
            vec![serde_json::json!({"id": 1, "name": "Bank A", "type": "bank",
                "provider": null, "external_id": null, "is_active": 1,
                "created_at": "2026-01-01 00:00:00", "updated_at": "2026-01-01 00:00:00"})],
        );
        let mut settings = HashMap::new();
        settings.insert("theme".to_string(), "dark".to_string());
        BackupPayload {
            version: FORMAT_VERSION,
            exported_at: "2026-07-07T10:00:00+05:30".to_string(),
            tables,
            settings,
        }
    }

    // ── Sync password crypto ──

    #[test]
    fn sync_password_encrypt_decrypt_roundtrip() {
        let enc = encrypt_sync_password(SYNC_PW, MASTER_PW).unwrap();
        assert_ne!(enc, SYNC_PW);
        assert_eq!(decrypt_sync_password(&enc, MASTER_PW).unwrap(), SYNC_PW);
    }

    #[test]
    fn sync_password_decrypt_fails_with_wrong_master() {
        let enc = encrypt_sync_password(SYNC_PW, MASTER_PW).unwrap();
        assert!(decrypt_sync_password(&enc, "wrong-master").is_err());
    }

    #[test]
    fn sync_password_decrypt_rejects_malformed_input() {
        assert!(decrypt_sync_password("not-base64!!!", MASTER_PW).is_err());
        // Valid base64 but shorter than salt+nonce
        assert!(decrypt_sync_password(&B64.encode([0u8; 8]), MASTER_PW).is_err());
    }

    #[test]
    fn sync_password_encryption_is_salted() {
        let a = encrypt_sync_password(SYNC_PW, MASTER_PW).unwrap();
        let b = encrypt_sync_password(SYNC_PW, MASTER_PW).unwrap();
        assert_ne!(a, b, "fresh salt+nonce per encryption");
    }

    // ── .svbak byte format ──

    #[test]
    fn backup_bytes_encrypt_decrypt_roundtrip() {
        let payload = minimal_payload();
        let bytes = encrypt_backup_bytes(&payload, SYNC_PW).unwrap();
        assert_eq!(&bytes[0..4], MAGIC);
        assert_eq!(bytes[4], FORMAT_VERSION);

        let restored = decrypt_backup_bytes(&bytes, SYNC_PW).unwrap();
        assert_eq!(restored.exported_at, payload.exported_at);
        assert_eq!(restored.settings.get("theme").unwrap(), "dark");
        assert_eq!(restored.tables.get("accounts").unwrap().len(), 1);
    }

    #[test]
    fn backup_bytes_decrypt_fails_with_wrong_password() {
        let bytes = encrypt_backup_bytes(&minimal_payload(), SYNC_PW).unwrap();
        let err = decrypt_backup_bytes(&bytes, "wrong").unwrap_err();
        assert!(err.to_string().contains("wrong password"), "got: {err}");
    }

    #[test]
    fn backup_bytes_rejects_bad_magic() {
        let mut bytes = encrypt_backup_bytes(&minimal_payload(), SYNC_PW).unwrap();
        bytes[0] = b'X';
        assert!(decrypt_backup_bytes(&bytes, SYNC_PW).is_err());
    }

    #[test]
    fn backup_bytes_rejects_unsupported_version() {
        let mut bytes = encrypt_backup_bytes(&minimal_payload(), SYNC_PW).unwrap();
        bytes[4] = 99;
        let err = decrypt_backup_bytes(&bytes, SYNC_PW).unwrap_err();
        assert!(err.to_string().contains("unsupported backup version"), "got: {err}");
    }

    #[test]
    fn backup_bytes_rejects_truncated_file() {
        assert!(decrypt_backup_bytes(&[0u8; HEADER_LEN - 1], SYNC_PW).is_err());
    }

    #[test]
    fn backup_bytes_rejects_tampered_ciphertext() {
        let mut bytes = encrypt_backup_bytes(&minimal_payload(), SYNC_PW).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        assert!(decrypt_backup_bytes(&bytes, SYNC_PW).is_err(), "AES-GCM must detect tampering");
    }

    // ── Value conversions ──

    #[test]
    fn json_to_sql_value_maps_all_types() {
        use rusqlite::types::Value;
        assert_eq!(json_to_sql_value(&serde_json::Value::Null), Value::Null);
        assert_eq!(json_to_sql_value(&serde_json::json!(true)), Value::Integer(1));
        assert_eq!(json_to_sql_value(&serde_json::json!(42)), Value::Integer(42));
        assert_eq!(json_to_sql_value(&serde_json::json!(1.5)), Value::Real(1.5));
        assert_eq!(
            json_to_sql_value(&serde_json::json!("hi")),
            Value::Text("hi".to_string())
        );
    }

    // ── Payload build/apply against real migrated SQLCipher DBs ──

    fn seed_source_db(db: &crate::db::DbPool) {
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts (name, type) VALUES (?1, ?2)",
            params!["Source Bank", "bank"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO equity_holdings (account_id, isin, symbol, exchange, name, quantity, avg_buy_price)
             VALUES (1, 'INE002A01018', 'RELIANCE', 'NSE', 'Reliance Industries', 10.0, 2400.5)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('theme', 'dark')
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .unwrap();
        // Session token — must NOT leak into the sync payload
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('zerodha_access_token', 'secret-token')
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .unwrap();
    }

    #[test]
    fn build_payload_includes_data_and_allowed_settings_only() {
        let (_dir, db) = test_db_pool();
        seed_source_db(&db);

        let conn = db.get().unwrap();
        let (payload, total_rows) = build_backup_payload(&conn).unwrap();

        // Migrations pre-seed some tables (e.g. default milestones), so
        // check bookkeeping against the payload itself, not a fixed count.
        let sum: usize = payload.tables.values().map(|rows| rows.len()).sum();
        assert_eq!(total_rows, sum, "total_rows must match payload contents");
        assert_eq!(payload.tables.get("accounts").unwrap().len(), 1);
        assert_eq!(payload.tables.get("equity_holdings").unwrap().len(), 1);
        assert_eq!(payload.settings.get("theme").unwrap(), "dark");
        assert!(
            !payload.settings.contains_key("zerodha_access_token"),
            "session tokens must not be exported"
        );
        assert!(!payload.exported_at.is_empty());
    }

    /// Full sync round-trip between two real DBs: device A exports, device B
    /// imports — B's pre-existing data is replaced, not merged.
    #[test]
    fn payload_roundtrip_replaces_target_db_contents() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        // Device B has its own stale data that must be replaced
        {
            let conn = db_b.get().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, type) VALUES ('Stale Account', 'manual')",
                [],
            )
            .unwrap();
        }

        // Export → encrypt → decrypt → apply, same path as a real sync minus file IO
        let (payload, total_rows) = {
            let conn = db_a.get().unwrap();
            build_backup_payload(&conn).unwrap()
        };
        let bytes = encrypt_backup_bytes(&payload, SYNC_PW).unwrap();
        let restored = decrypt_backup_bytes(&bytes, SYNC_PW).unwrap();
        let summary = apply_backup_payload(&db_b, &restored).unwrap();

        assert_eq!(summary.rows_imported, total_rows, "every exported row must be imported");
        assert!(summary.tables_imported >= 2, "at least accounts + equity_holdings");

        let conn = db_b.get().unwrap();
        let names: Vec<String> = conn
            .prepare("SELECT name FROM accounts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(names, vec!["Source Bank"], "stale account replaced, not merged");

        let (symbol, qty): (String, f64) = conn
            .query_row(
                "SELECT symbol, quantity FROM equity_holdings",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(symbol, "RELIANCE");
        assert_eq!(qty, 10.0);

        let theme: String = conn
            .query_row("SELECT value FROM app_settings WHERE key='theme'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(theme, "dark");
    }

    #[test]
    fn apply_payload_restores_fk_enforcement_after_import() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        let (payload, _) = {
            let conn = db_a.get().unwrap();
            build_backup_payload(&conn).unwrap()
        };
        apply_backup_payload(&db_b, &payload).unwrap();

        let conn = db_b.get().unwrap();
        let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 1, "foreign_keys must be re-enabled after import");
    }
}
