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
use crate::db::DbState;
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
    let conn = state.0.get()?;

    // Serialize all data tables
    let mut tables: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    let mut total_rows = 0usize;
    for &table in DATA_TABLES {
        let rows = dump_table(&conn, table)?;
        total_rows += rows.len();
        tables.insert(table.to_string(), rows);
    }

    // Serialize allowed settings
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

    let exported_at = chrono::Local::now().to_rfc3339();
    let payload = BackupPayload { version: FORMAT_VERSION, exported_at: exported_at.clone(), tables, settings };
    let json_bytes = serde_json::to_vec(&payload)
        .map_err(|e| AppError::Parse(format!("serialize backup: {e}")))?;

    // Derive key + encrypt
    let salt: [u8; SALT_LEN] = random();
    let nonce_bytes: [u8; NONCE_LEN] = random();
    let key = derive_key(&password, &salt);
    let ciphertext = aes_encrypt(&key, &nonce_bytes, &json_bytes)?;

    // Build file bytes: MAGIC + version + salt + nonce + ciphertext
    let mut file = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    file.extend_from_slice(MAGIC);
    file.push(FORMAT_VERSION);
    file.extend_from_slice(&salt);
    file.extend_from_slice(&nonce_bytes);
    file.extend_from_slice(&ciphertext);

    write_via_fs(&app, &dest_path, &file)
        .map_err(|e| AppError::Database(format!("write backup: {e}")))?;

    Ok(ExportSummary { rows_exported: total_rows, exported_at })
}

#[tauri::command]
pub fn import_sync_backup(
    app: AppHandle,
    src_path: String,
    password: String,
    state: State<DbState>,
) -> Result<ImportSummary> {
    // Read + validate header
    let data = app
        .fs()
        .read(src_path.parse::<tauri_plugin_fs::FilePath>().unwrap())
        .map_err(|e| AppError::Database(format!("read backup: {e}")))?;

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

    let key = derive_key(&password, salt);
    let json_bytes = aes_decrypt(&key, nonce_bytes, ciphertext)?;

    let payload: BackupPayload = serde_json::from_slice(&json_bytes)
        .map_err(|e| AppError::Parse(format!("parse backup: {e}")))?;

    // Write to DB
    let mut conn = state.0.get()?;

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
