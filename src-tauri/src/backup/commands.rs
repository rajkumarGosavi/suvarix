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
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{AndroidFsExt, Entry, FsUri};

use crate::constants::APP_NAME;
use crate::db::migrations::SYNC_TABLES;
use crate::db::{DbPool, DbState};
use crate::error::{AppError, Result};

// ── File format ───────────────────────────────────────────────

const MAGIC: &[u8; 4] = b"FFBK";
// v1: wholesale table replace (see `apply_backup_payload_replace`) — kept for
// reading files exported by a not-yet-updated device during a version transition.
// v2: per-row merge keyed by `sync_id`, with FK id-remapping and tombstones (see
// `apply_backup_payload_merge`). New exports always write v2.
const FORMAT_VERSION: u8 = 2;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 33

/// Filename the auto-sync scheduler writes/reads inside the user's chosen
/// sync folder (desktop) or SAF tree (Android). Shared with `backup::scheduler`.
pub(crate) const SYNC_FILENAME: &str = "suvarix-sync.svbak";

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

/// A `(table, sync_id)` that was deleted on some device, with when — carried in
/// the payload so an importing device can tell "deleted elsewhere" apart from
/// "never had this row" during merge. See `migration_019_sync_infra`.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[cfg_attr(test, derive(Debug))]
struct TombstoneRow {
    table_name: String,
    sync_id: String,
    deleted_at: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(Debug))]
struct BackupPayload {
    version: u8,
    exported_at: String,
    tables: HashMap<String, Vec<serde_json::Value>>,
    settings: HashMap<String, String>,
    // Missing entirely in v1 files (pre-merge-support exports) — default to empty.
    #[serde(default)]
    tombstones: Vec<TombstoneRow>,
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
    for &table in SYNC_TABLES {
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

    let tombstones = {
        let mut stmt = conn
            .prepare("SELECT table_name, sync_id, deleted_at FROM sync_tombstones")
            .map_err(|e| AppError::Database(format!("prepare tombstones: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TombstoneRow {
                    table_name: r.get(0)?,
                    sync_id: r.get(1)?,
                    deleted_at: r.get(2)?,
                })
            })
            .map_err(|e| AppError::Database(format!("dump tombstones: {e}")))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // UTC so timestamps from devices in different offsets compare correctly.
    let exported_at = chrono::Utc::now().to_rfc3339();
    Ok((
        BackupPayload { version: FORMAT_VERSION, exported_at, tables, settings, tombstones },
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

/// Applies an imported payload, branching on the format version it was
/// exported with: pre-merge-support (v1) files still get the old wholesale
/// replace (they can't be merged — the exporting device never wrote `sync_id`s
/// or tombstones); anything else goes through the per-row merge.
fn apply_backup_payload(db: &DbPool, payload: &BackupPayload) -> Result<ImportSummary> {
    if payload.version < 2 {
        apply_backup_payload_replace(db, payload)
    } else {
        apply_backup_payload_merge(db, payload)
    }
}

/// Replaces all synced tables + settings with the payload's contents. Kept
/// only for reading v1 (`FORMAT_VERSION` before merge support) exports from a
/// device that hasn't updated yet — new exports never use this path since it
/// wipes anything the importing device added that the exporter never saw.
fn apply_backup_payload_replace(db: &DbPool, payload: &BackupPayload) -> Result<ImportSummary> {
    let mut conn = db.get()?;

    // FK checks off during bulk replace so delete/insert order doesn't matter
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let result = (|| -> Result<ImportSummary> {
        let tx = conn.transaction()?;

        for &table in SYNC_TABLES {
            tx.execute(&format!("DELETE FROM \"{table}\""), [])?;
        }

        let mut total_rows = 0usize;
        let mut tables_imported = 0usize;
        for &table in SYNC_TABLES {
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

// ── Merge (v2) ────────────────────────────────────────────────
//
// Union, not replace: for each table (in `SYNC_TABLES`'s FK-safe order), match
// remote rows to local rows by `sync_id` (stable across devices, unlike the
// local `INTEGER PRIMARY KEY`), keep whichever side has the newer
// `sync_updated_at`, and insert whatever the other device has that this one
// doesn't. FK columns (`account_id`, `mf_holding_id`, `transactions.holding_id`)
// hold the *exporting* device's local ids, so every row is rewritten through an
// `remote_id -> this_device_local_id` map built table-by-table as we go —
// `SYNC_TABLES` orders parents before children so every FK a row can carry
// already has a complete map by the time that row is processed. Tombstones
// (`payload.tombstones`) are applied last, deleting local rows an
// at-least-as-recent remote delete targets, so a row deleted on one device
// doesn't get silently resurrected by the next union.

/// FK columns each table's rows carry, as `(column, referenced synced table,
/// required)` — used to translate a remote device's local ids into this
/// device's local ids during merge. `required` rows are skipped entirely (not
/// written with a dangling reference) if the referenced parent isn't present on
/// this device (e.g. tombstoned here and not resurrected); optional ones just
/// get set to `NULL`. `transactions.holding_id` is polymorphic (depends on the
/// row's `asset_class`) and is handled separately in `remap_row_fks`.
fn fk_columns(table: &str) -> &'static [(&'static str, &'static str, bool)] {
    match table {
        "equity_holdings" | "mf_holdings" => &[("account_id", "accounts", true)],
        "sip_schedules" => {
            &[("account_id", "accounts", true), ("mf_holding_id", "mf_holdings", false)]
        }
        "fd_holdings" | "gold_holdings" | "crypto_holdings" | "bond_holdings" => {
            &[("account_id", "accounts", false)]
        }
        "transactions" => &[("account_id", "accounts", false)],
        _ => &[],
    }
}

/// Which synced table `transactions.holding_id` points into, based on the
/// row's `asset_class` — that FK is polymorphic, unlike every other one.
fn holding_table_for_asset_class(asset_class: &str) -> Option<&'static str> {
    match asset_class {
        "equity" => Some("equity_holdings"),
        "mf" => Some("mf_holdings"),
        "fd" => Some("fd_holdings"),
        "ppf_epf" => Some("ppf_epf_holdings"),
        "real_estate" => Some("real_estate_holdings"),
        "gold" => Some("gold_holdings"),
        "crypto" => Some("crypto_holdings"),
        "insurance" => Some("insurance_holdings"),
        "loan" => Some("loans"),
        "credit_card" => Some("credit_cards"),
        _ => None,
    }
}

/// Rewrites every FK column in `row` (a remote-exported row, about to be
/// inserted/updated locally) from the remote device's local ids to this
/// device's, using the `remote_id -> local_id` maps built so far. Returns
/// `false` if a *required* FK's parent hasn't been merged on this device —
/// the row must be skipped, not written with a dangling reference.
fn remap_row_fks(
    table: &str,
    row: &mut serde_json::Map<String, serde_json::Value>,
    id_maps: &HashMap<&'static str, HashMap<i64, i64>>,
) -> bool {
    for &(col, ref_table, required) in fk_columns(table) {
        let Some(remote_id) = row.get(col).and_then(|v| v.as_i64()) else { continue };
        match id_maps.get(ref_table).and_then(|m| m.get(&remote_id)).copied() {
            Some(local_id) => {
                row.insert(col.to_string(), serde_json::json!(local_id));
            }
            None if required => return false,
            None => {
                row.insert(col.to_string(), serde_json::Value::Null);
            }
        }
    }
    if table == "transactions" {
        if let Some(remote_holding_id) = row.get("holding_id").and_then(|v| v.as_i64()) {
            let asset_class = row.get("asset_class").and_then(|v| v.as_str()).unwrap_or("");
            let mapped = holding_table_for_asset_class(asset_class)
                .and_then(|t| id_maps.get(t))
                .and_then(|m| m.get(&remote_holding_id))
                .copied();
            row.insert(
                "holding_id".to_string(),
                mapped.map(|id| serde_json::json!(id)).unwrap_or(serde_json::Value::Null),
            );
        }
    }
    true
}

fn random_sync_id() -> String {
    let bytes: [u8; 16] = random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `UPDATE table SET col=?, ... WHERE id = ?` over every column in `row` except
/// `id` — mirrors `restore_table`'s generic column handling but for a targeted
/// single-row update instead of a bulk insert.
fn update_row_by_id(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    id: i64,
    row: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let cols: Vec<&String> = row.keys().filter(|k| k.as_str() != "id").collect();
    if cols.is_empty() {
        return Ok(());
    }
    let set_clause = cols
        .iter()
        .enumerate()
        .map(|(i, c)| format!("\"{c}\" = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("UPDATE \"{table}\" SET {set_clause} WHERE id = ?{}", cols.len() + 1);
    let mut params: Vec<rusqlite::types::Value> =
        cols.iter().map(|c| json_to_sql_value(row.get(*c).unwrap())).collect();
    params.push(rusqlite::types::Value::Integer(id));
    tx.execute(&sql, rusqlite::params_from_iter(params))
        .map_err(|e| AppError::Database(format!("update {table}: {e}")))?;
    Ok(())
}

/// `INSERT INTO table (...) VALUES (...)` over every column in `row` except
/// `id` (left for AUTOINCREMENT to assign), returning the new local id — or
/// `None` if the insert hit a UNIQUE constraint. That happens when both
/// devices independently seeded identical content under different `sync_id`s
/// (e.g. the default milestone rows, unique on `amount`, backfilled with a
/// fresh random `sync_id` per device at migration time) — the row already
/// exists in substance, just not by `sync_id`, so skipping it is correct
/// rather than failing the whole merge.
fn insert_row_new_id(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    row: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<i64>> {
    let cols: Vec<&String> = row.keys().filter(|k| k.as_str() != "id").collect();
    let col_list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let placeholders = (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
    let sql = format!("INSERT INTO \"{table}\" ({col_list}) VALUES ({placeholders})");
    let params: Vec<rusqlite::types::Value> =
        cols.iter().map(|c| json_to_sql_value(row.get(*c).unwrap())).collect();
    match tx.execute(&sql, rusqlite::params_from_iter(params)) {
        Ok(_) => Ok(Some(tx.last_insert_rowid())),
        Err(rusqlite::Error::SqliteFailure(_, Some(ref msg)))
            if msg.contains("UNIQUE constraint failed") =>
        {
            Ok(None)
        }
        Err(e) => Err(AppError::Database(format!("insert into {table}: {e}"))),
    }
}

fn apply_backup_payload_merge(db: &DbPool, payload: &BackupPayload) -> Result<ImportSummary> {
    let mut conn = db.get()?;
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let result = (|| -> Result<ImportSummary> {
        let tx = conn.transaction()?;
        let mut id_maps: HashMap<&'static str, HashMap<i64, i64>> = HashMap::new();
        let mut total_rows = 0usize;
        let mut tables_touched = 0usize;

        for &table in SYNC_TABLES {
            id_maps.entry(table).or_default();
            let remote_rows = payload.tables.get(table).cloned().unwrap_or_default();
            if remote_rows.is_empty() {
                continue;
            }

            let local_rows = dump_table(&tx, table)?;
            let mut local_by_sync_id: HashMap<String, (i64, serde_json::Value)> = HashMap::new();
            for row in local_rows {
                if let (Some(sid), Some(id)) = (
                    row.get("sync_id").and_then(|v| v.as_str()),
                    row.get("id").and_then(|v| v.as_i64()),
                ) {
                    local_by_sync_id.insert(sid.to_string(), (id, row));
                }
            }

            let mut tombstoned: HashMap<String, String> = HashMap::new();
            {
                let mut stmt = tx
                    .prepare("SELECT sync_id, deleted_at FROM sync_tombstones WHERE table_name = ?1")
                    .map_err(|e| AppError::Database(e.to_string()))?;
                let rows = stmt
                    .query_map([table], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                    .map_err(|e| AppError::Database(e.to_string()))?;
                for row in rows.flatten() {
                    tombstoned.insert(row.0, row.1);
                }
            }

            let mut touched_this_table = false;

            for remote_row in remote_rows {
                let Some(mut obj) = remote_row.as_object().cloned() else { continue };
                let Some(remote_id) = obj.get("id").and_then(|v| v.as_i64()) else { continue };
                let sync_id = obj
                    .get("sync_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(random_sync_id);
                let remote_updated =
                    obj.get("sync_updated_at").and_then(|v| v.as_str()).unwrap_or_default().to_string();

                if !remap_row_fks(table, &mut obj, &id_maps) {
                    continue; // required parent not present on this device — skip
                }
                obj.insert("sync_id".to_string(), serde_json::Value::String(sync_id.clone()));

                if let Some((local_id, local_row)) = local_by_sync_id.get(&sync_id) {
                    let local_id = *local_id;
                    id_maps.get_mut(table).unwrap().insert(remote_id, local_id);
                    let local_updated =
                        local_row.get("sync_updated_at").and_then(|v| v.as_str()).unwrap_or_default();
                    if remote_updated.as_str() > local_updated {
                        update_row_by_id(&tx, table, local_id, &obj)?;
                        touched_this_table = true;
                        total_rows += 1;
                    }
                    continue;
                }

                if let Some(deleted_at) = tombstoned.get(&sync_id) {
                    if deleted_at.as_str() >= remote_updated.as_str() {
                        continue; // deleted here at least as recently — don't resurrect
                    }
                }

                if let Some(new_id) = insert_row_new_id(&tx, table, &obj)? {
                    id_maps.get_mut(table).unwrap().insert(remote_id, new_id);
                    touched_this_table = true;
                    total_rows += 1;
                }
            }

            if touched_this_table {
                tables_touched += 1;
            }
        }

        // Tombstones last: delete local rows an at-least-as-recent remote delete
        // targets (fires this table's own tombstone trigger, upserting a local
        // record so a future export tells other devices too), then fold the
        // remote tombstone list into ours either way, keeping the newer
        // `deleted_at` if we already knew about this delete.
        for t in &payload.tombstones {
            let existing: Option<(i64, String)> = tx
                .query_row(
                    &format!("SELECT id, sync_updated_at FROM \"{}\" WHERE sync_id = ?1", t.table_name),
                    [&t.sync_id],
                    |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())),
                )
                .ok();
            if let Some((local_id, local_updated)) = existing {
                if t.deleted_at.as_str() >= local_updated.as_str() {
                    tx.execute(&format!("DELETE FROM \"{}\" WHERE id = ?1", t.table_name), [local_id])
                        .map_err(|e| AppError::Database(format!("apply tombstone: {e}")))?;
                }
            }
            tx.execute(
                "INSERT INTO sync_tombstones (table_name, sync_id, deleted_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(table_name, sync_id) DO UPDATE SET deleted_at = excluded.deleted_at
                 WHERE excluded.deleted_at > sync_tombstones.deleted_at",
                rusqlite::params![t.table_name, t.sync_id, t.deleted_at],
            )
            .map_err(|e| AppError::Database(format!("merge tombstone: {e}")))?;
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
        Ok(ImportSummary { rows_imported: total_rows, tables_imported: tables_touched })
    })();

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

// ── Android auto-sync folder (SAF) ───────────────────────────────
// tauri-plugin-dialog has no directory picker on Android (desktop-only), so
// auto-sync's folder picker uses tauri-plugin-android-fs there instead. That
// plugin also persists the SAF grant across app restarts — required because
// the background scheduler ticks even after Android kills and relaunches the
// process, long after the picker dialog is gone. The folder is stored in the
// `sync_folder_path` setting as `FsUri::to_json_string()`, not a plain path.

#[cfg(target_os = "android")]
#[tauri::command]
pub fn pick_sync_folder_android(app: AppHandle) -> Result<Option<String>> {
    let api = app.android_fs();
    let dir = api
        .picker()
        .pick_dir(None, false)
        .map_err(|e| AppError::Database(format!("pick sync folder: {e}")))?;
    let Some(dir) = dir else { return Ok(None) };
    api.picker()
        .persist_uri_permission(&dir)
        .map_err(|e| AppError::Database(format!("persist sync folder permission: {e}")))?;
    let json = dir
        .to_json_string()
        .map_err(|e| AppError::Database(format!("serialize sync folder: {e}")))?;
    Ok(Some(json))
}

/// Finds `SYNC_FILENAME` inside the picked folder, creating it if this is
/// the first sync from this device.
#[cfg(target_os = "android")]
fn android_resolve_sync_file(app: &AppHandle, dir_json: &str) -> Result<FsUri> {
    let dir = FsUri::from_json_str(dir_json)
        .map_err(|e| AppError::Database(format!("bad sync folder: {e}")))?;
    let api = app.android_fs();
    let existing = api
        .read_dir(&dir)
        .map_err(|e| AppError::Database(format!("list sync folder: {e}")))?
        .into_iter()
        .find_map(|entry| match entry {
            Entry::File { uri, name, .. } if name == SYNC_FILENAME => Some(uri),
            _ => None,
        });
    if let Some(uri) = existing {
        return Ok(uri);
    }
    api.create_new_file(&dir, SYNC_FILENAME, Some("application/octet-stream"))
        .map_err(|e| AppError::Database(format!("create sync file: {e}")))
}

/// Android counterpart to `export_sync_backup_impl` — same payload build +
/// encrypt, but writes via the SAF-aware `android_fs` API instead of
/// `tauri_plugin_fs`, since the folder is a persisted content:// tree URI.
#[cfg(target_os = "android")]
pub(crate) fn android_export_sync_backup(
    app: &AppHandle,
    dir_json: &str,
    password: &str,
    db: &DbPool,
) -> Result<ExportSummary> {
    let (payload, total_rows) = {
        let conn = db.get()?;
        build_backup_payload(&conn)?
    };
    let exported_at = payload.exported_at.clone();
    let bytes = encrypt_backup_bytes(&payload, password)?;
    let file_uri = android_resolve_sync_file(app, dir_json)?;
    app.android_fs()
        .write(&file_uri, &bytes)
        .map_err(|e| AppError::Database(format!("write sync file: {e}")))?;
    Ok(ExportSummary { rows_exported: total_rows, exported_at })
}

/// Android counterpart to `peek_exported_at`. Returns `None` when no remote
/// file exists yet (first sync from this device), matching desktop's
/// `Path::exists()` check in the scheduler.
#[cfg(target_os = "android")]
pub(crate) fn android_peek_exported_at(
    app: &AppHandle,
    dir_json: &str,
    password: &str,
) -> Result<Option<String>> {
    let dir = FsUri::from_json_str(dir_json)
        .map_err(|e| AppError::Database(format!("bad sync folder: {e}")))?;
    let api = app.android_fs();
    let existing = api
        .read_dir(&dir)
        .map_err(|e| AppError::Database(format!("list sync folder: {e}")))?
        .into_iter()
        .find_map(|entry| match entry {
            Entry::File { uri, name, .. } if name == SYNC_FILENAME => Some(uri),
            _ => None,
        });
    let Some(file_uri) = existing else { return Ok(None) };
    let bytes = api
        .read(&file_uri)
        .map_err(|e| AppError::Database(format!("read sync file: {e}")))?;
    Ok(Some(decrypt_backup_bytes(&bytes, password)?.exported_at))
}

/// Android counterpart to `import_sync_backup_impl`.
#[cfg(target_os = "android")]
pub(crate) fn android_import_sync_backup(
    app: &AppHandle,
    dir_json: &str,
    password: &str,
    db: &DbPool,
) -> Result<ImportSummary> {
    let file_uri = android_resolve_sync_file(app, dir_json)?;
    let bytes = app
        .android_fs()
        .read(&file_uri)
        .map_err(|e| AppError::Database(format!("read sync file: {e}")))?;
    let payload = decrypt_backup_bytes(&bytes, password)?;
    apply_backup_payload(db, &payload)
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
    // Accept older formats (e.g. v1 wholesale-replace, from a not-yet-updated
    // device) alongside the current one — only reject a version newer than this
    // binary understands. `apply_backup_payload` branches on `payload.version`.
    if data[4] > FORMAT_VERSION {
        return Err(AppError::Validation(format!("unsupported backup version {}", data[4])));
    }

    let salt = &data[5..5 + SALT_LEN];
    let nonce_bytes = &data[5 + SALT_LEN..HEADER_LEN];
    let ciphertext = &data[HEADER_LEN..];

    let key = derive_key(password, salt);
    let json_bytes = aes_decrypt(&key, nonce_bytes, ciphertext)?;

    serde_json::from_slice(&json_bytes).map_err(|e| AppError::Parse(format!("parse backup: {e}")))
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
            tombstones: Vec::new(),
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
    /// imports — B's own unrelated data survives (union, not replace), and A's
    /// account gets a *new* local id on B (since B already had an account with
    /// the same integer id) with `equity_holdings.account_id` correctly
    /// remapped to point at it rather than at B's unrelated row.
    #[test]
    fn payload_roundtrip_merges_by_sync_id_with_fk_remap() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        // Device B has its own unrelated account, which also gets local id 1 —
        // the exact id collision that broke wholesale-replace merges.
        {
            let conn = db_b.get().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, type) VALUES ('B''s Own Account', 'manual')",
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

        // total_rows includes the 9 default milestone rows, which already exist
        // on B (seeded identically, unique on `amount`) under a different
        // sync_id — those are correctly skipped as duplicates-by-content, not
        // inserted again. Only the account + equity holding are genuinely new.
        assert_eq!(summary.rows_imported, total_rows - 9, "account + equity holding must be inserted as new");

        let conn = db_b.get().unwrap();
        let mut names: Vec<String> = conn
            .prepare("SELECT name FROM accounts ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        names.sort();
        assert_eq!(
            names,
            vec!["B's Own Account".to_string(), "Source Bank".to_string()],
            "both devices' accounts must survive — union, not replace"
        );

        // The imported equity holding's account_id must point at whichever local
        // id "Source Bank" actually landed on for B, not at B's own account.
        let source_bank_id: i64 = conn
            .query_row("SELECT id FROM accounts WHERE name = 'Source Bank'", [], |r| r.get(0))
            .unwrap();
        let (symbol, qty, account_id): (String, f64, i64) = conn
            .query_row(
                "SELECT symbol, quantity, account_id FROM equity_holdings",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(symbol, "RELIANCE");
        assert_eq!(qty, 10.0);
        assert_eq!(account_id, source_bank_id, "FK must be remapped to B's local id for the row");

        let theme: String = conn
            .query_row("SELECT value FROM app_settings WHERE key='theme'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(theme, "dark");
    }

    /// A v1 (pre-merge-support) payload must still go through the old
    /// wholesale-replace path — it has no `sync_id`s to merge by.
    #[test]
    fn v1_payload_still_replaces_wholesale() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);
        {
            let conn = db_b.get().unwrap();
            conn.execute(
                "INSERT INTO accounts (name, type) VALUES ('Stale Account', 'manual')",
                [],
            )
            .unwrap();
        }

        let mut payload = {
            let conn = db_a.get().unwrap();
            build_backup_payload(&conn).unwrap().0
        };
        payload.version = 1;

        apply_backup_payload(&db_b, &payload).unwrap();

        let conn = db_b.get().unwrap();
        let names: Vec<String> = conn
            .prepare("SELECT name FROM accounts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(names, vec!["Source Bank"], "v1 payload replaces wholesale, not merges");
    }

    /// A row deleted on device A (after both devices have already synced once)
    /// must be deleted on device B too on the next sync, not resurrected by the
    /// union — this is the tombstone mechanism.
    #[test]
    fn delete_on_one_device_propagates_via_tombstone() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        // First sync: A -> B, so B has a copy of the account.
        let payload_1 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload_1).unwrap();
        {
            let conn = db_b.get().unwrap();
            let count: i64 =
                conn.query_row("SELECT count(*) FROM accounts WHERE name = 'Source Bank'", [], |r| r.get(0)).unwrap();
            assert_eq!(count, 1, "precondition: B has the account after first sync");
        }

        // A deletes its account (cascades to equity_holdings), then re-exports.
        db_a.get().unwrap().execute("DELETE FROM accounts WHERE name = 'Source Bank'", []).unwrap();
        let payload_2 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        assert!(!payload_2.tombstones.is_empty(), "delete must produce a tombstone");

        apply_backup_payload(&db_b, &payload_2).unwrap();

        let conn = db_b.get().unwrap();
        let count: i64 =
            conn.query_row("SELECT count(*) FROM accounts WHERE name = 'Source Bank'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0, "delete must propagate, not be resurrected by the union");
    }

    /// When both devices edited the same row (same `sync_id`) independently,
    /// the one with the newer `sync_updated_at` must win.
    #[test]
    fn both_sides_edited_newer_sync_updated_at_wins() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        // First sync so both devices share the same sync_id for the account.
        let payload_1 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload_1).unwrap();

        // B edits the account name — bumps B's local sync_updated_at.
        db_b.get().unwrap().execute("UPDATE accounts SET name = 'B Edited' WHERE name = 'Source Bank'", []).unwrap();

        // A edits it too, but "later" — force sync_updated_at strictly ahead of B's.
        {
            let conn = db_a.get().unwrap();
            conn.execute("UPDATE accounts SET name = 'A Edited' WHERE name = 'Source Bank'", []).unwrap();
            conn.execute(
                "UPDATE accounts SET sync_updated_at = datetime('now', '+1 hour') WHERE name = 'A Edited'",
                [],
            )
            .unwrap();
        }

        let payload_2 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload_2).unwrap();

        let name: String =
            db_b.get().unwrap().query_row("SELECT name FROM accounts", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "A Edited", "the strictly-newer edit must win");
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
