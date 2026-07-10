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
// `apply_backup_payload_merge`).
// v3: adds the `sync_hlc` column (rows) and `hlc` column (tombstones) — see
// `migration_021_hlc_state_and_triggers`. Bumped because `update_row_by_id` /
// `insert_row_new_id` build their SQL from whatever keys are present in the
// remote JSON row, so an older binary applying a row with a `sync_hlc` key it
// has no local column for would hit "no such column" mid-transaction instead
// of failing cleanly — this must be caught by the version check below
// (`decrypt_backup_bytes`) before merge ever runs. New exports always write v3.
// Bump this again for any future change to a `SYNC_TABLES` table's columns.
pub(crate) const FORMAT_VERSION: u8 = 3;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 33

/// Filename the auto-sync scheduler writes/reads inside the user's chosen
/// sync folder (desktop) or SAF tree (Android). Shared with `backup::scheduler`.
pub(crate) const SYNC_FILENAME: &str = "suvarix-sync.svbak";

/// Set by `scheduler::run_tick` when an import hits `AppError::UnsupportedBackupVersion`
/// (a peer device wrote a newer `.svbak` format than this app build reads) — surfaced to
/// the frontend via `get_sync_block_status` as a persistent "update the app" banner
/// rather than a one-off toast, since retrying next tick won't fix it on its own.
pub(crate) const SETTING_SYNC_BLOCKED: &str = "sync_blocked_version_mismatch";

/// Whether auto-sync is currently paused because a peer wrote a newer backup
/// format than this app build understands (see `SETTING_SYNC_BLOCKED`). Local
/// (non-sync) app usage is unaffected either way.
#[tauri::command]
pub fn get_sync_block_status(state: State<DbState>) -> Result<bool> {
    let conn = state.0.get()?;
    Ok(conn
        .query_row("SELECT value FROM app_settings WHERE key=?1", [SETTING_SYNC_BLOCKED], |r| {
            r.get::<_, String>(0)
        })
        .map(|v| v == "true")
        .unwrap_or(false))
}

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
    // Absent in v2-and-earlier payloads (from a peer that hasn't updated, or an
    // older local export from before this device updated) — falls back to
    // `deleted_at` string comparison at merge time, same as a row missing `sync_hlc`.
    #[serde(default)]
    hlc: Option<String>,
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
            .prepare("SELECT table_name, sync_id, deleted_at, hlc FROM sync_tombstones")
            .map_err(|e| AppError::Database(format!("prepare tombstones: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TombstoneRow {
                    table_name: r.get(0)?,
                    sync_id: r.get(1)?,
                    deleted_at: r.get(2)?,
                    hlc: r.get(3)?,
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
// (`payload.tombstones`) are applied *first*, before any row insert/update,
// deleting local rows an at-least-as-recent remote delete targets — both so a
// row deleted on one device doesn't get silently resurrected by the union, and
// so a still-live not-yet-deduped duplicate is actually gone before some other
// row's FK remap tries to reuse the UNIQUE slot it's sitting on (see
// `apply_backup_payload_merge`'s comment on that ordering).

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

/// True if side `a` should win over side `b` in a merge comparison (which one
/// is "remote"/"local", or "row"/"tombstone", depends on the call site).
/// Prefers comparing `sync_hlc` (millisecond resolution + device tiebreak,
/// immune to the seconds-resolution ties `sync_updated_at` alone can hit)
/// whenever both sides have one; falls back to the legacy `sync_updated_at`
/// string compare when either side lacks it (a legacy row untouched since
/// before this device's HLC upgrade, or a payload from a peer that hasn't
/// updated yet). `or_equal` selects the tombstone-vs-row comparisons' `>=` (a
/// delete at least as recent as the row it targets wins, so a delete is never
/// silently lost to a concurrent no-op re-sync of the same row) vs. plain
/// row-update's strict `>` (a tie leaves the local copy alone rather than
/// churning it).
fn a_wins(a_hlc: Option<&str>, a_ts: &str, b_hlc: Option<&str>, b_ts: &str, or_equal: bool) -> bool {
    match (a_hlc, b_hlc) {
        (Some(ah), Some(bh)) if !ah.is_empty() && !bh.is_empty() => {
            if or_equal { ah >= bh } else { ah > bh }
        }
        _ => {
            if or_equal { a_ts >= b_ts } else { a_ts > b_ts }
        }
    }
}

/// HLC "receive" rule: after merging in a peer's rows/tombstones, this
/// device's own `sync_hlc_state` must be pulled forward to at least the
/// highest HLC just observed — otherwise a local edit made shortly after this
/// import could still stamp a lower HLC than the row it just imported (this
/// device's system clock has no reason to already be caught up with whatever
/// the peer's was), silently undoing the entire point of using HLC over a
/// plain wall-clock compare the next time these two devices' edits conflict.
/// Parses `"{physical_ms}:{logical}:{device_id}"` (see
/// `migrations::migration_021_hlc_state_and_triggers`); malformed input (should
/// be unreachable — only ever produced by this app's own trigger) is ignored
/// rather than failing the whole merge over a clock nicety.
/// `TombstoneRow.table_name` comes straight from an imported (untrusted —
/// corrupted file, or a malicious/compromised peer with write access to the
/// sync folder) payload and is interpolated directly into SQL as an
/// identifier below (table names can't be bound as query parameters), rather
/// than passed as a value. Standard practice for any dynamic SQL identifier
/// is to validate against a known-good allow-list before it ever touches a
/// query string, regardless of whether a specific exploit chain is provably
/// constructible against today's exact query shapes and rusqlite's
/// single-statement `execute`/`query_row` — the alternative is re-auditing
/// every call site by hand every time either changes.
fn is_valid_sync_table(table: &str) -> bool {
    SYNC_TABLES.contains(&table)
}

const MAX_CLOCK_SKEW_MS: i64 = 24 * 60 * 60 * 1000; // 1 day

/// Format + plausibility check for a `sync_hlc` from an untrusted payload
/// (remote row or tombstone) before it's ever compared against a local value
/// or fed into `apply_hlc_receive`. Without this, a corrupted or malicious
/// `.svbak` could plant a `sync_hlc` with an absurd `physical_ms` (say, the
/// year 9999) that wins every merge forever and — worse — permanently drags
/// this device's own clock into the future via the HLC "receive" rule,
/// poisoning every sync this device does afterward, not just the one row.
/// Malformed or implausible input is treated as absent, i.e. falls back to
/// the legacy `sync_updated_at` compare, exactly like a legacy peer's row
/// that never had a `sync_hlc` at all.
fn sanitize_hlc(hlc: Option<String>) -> Option<String> {
    let hlc = hlc?;
    let parts: Vec<&str> = hlc.split(':').collect();
    let [phys_str, logical_str, device_str] = parts.as_slice() else { return None };
    if phys_str.len() != 13 || !phys_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if logical_str.len() != 9 || !logical_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if device_str.len() != 32 || !device_str.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let physical_ms: i64 = phys_str.parse().ok()?;
    if physical_ms > chrono::Utc::now().timestamp_millis() + MAX_CLOCK_SKEW_MS {
        return None;
    }
    Some(hlc)
}

/// Loose but effective sanity check for the legacy wall-clock timestamp
/// (`datetime('now')`-style `"YYYY-MM-DD HH:MM:SS"`, no timezone) from an
/// untrusted payload. Same motivation as `sanitize_hlc`: a corrupted or
/// malicious payload could otherwise set `sync_updated_at`/`deleted_at`
/// arbitrarily far in the future to win every merge comparison forever.
/// Anything unparseable or implausibly ahead of this device's own clock is
/// treated as the empty string, which `a_wins`'s comparisons already handle
/// as "never wins" (same as a row that never had this field set).
fn sanitize_legacy_ts(ts: String) -> String {
    let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S") else {
        return String::new();
    };
    let max_allowed = chrono::Utc::now().naive_utc() + chrono::Duration::milliseconds(MAX_CLOCK_SKEW_MS);
    if parsed > max_allowed {
        return String::new();
    }
    ts
}

fn apply_hlc_receive(tx: &rusqlite::Transaction<'_>, remote_hlc: &str) -> Result<()> {
    let mut parts = remote_hlc.splitn(3, ':');
    let (Some(phys_str), Some(logical_str)) = (parts.next(), parts.next()) else { return Ok(()) };
    let (Ok(remote_phys), Ok(remote_logical)) = (phys_str.parse::<i64>(), logical_str.parse::<i64>()) else {
        return Ok(());
    };
    tx.execute(
        "UPDATE sync_hlc_state SET
            last_logical = CASE
                WHEN ?1 > last_physical_ms THEN ?2
                WHEN ?1 = last_physical_ms THEN MAX(last_logical, ?2)
                ELSE last_logical
            END,
            last_physical_ms = MAX(last_physical_ms, ?1)
         WHERE id = 1",
        rusqlite::params![remote_phys, remote_logical],
    )
    .map_err(|e| AppError::Database(format!("apply hlc receive: {e}")))?;
    Ok(())
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
        // Highest `sync_hlc` seen anywhere in this payload (rows or tombstones).
        // Fixed-width zero-padded encoding means plain string `>` already gives
        // the numerically-correct max, so no parsing needed to track it here.
        // Applied to this device's own `sync_hlc_state` once, after the loop
        // below, via `apply_hlc_receive` — the HLC "receive" rule: observing a
        // peer's clock must pull this device's own clock forward too, or a
        // local edit made shortly after this import could still stamp a lower
        // HLC than the row it just imported, undoing the whole point of using
        // HLC over a plain wall-clock compare.
        let mut max_remote_hlc: Option<String> = None;

        // Tombstones first, before any row insert/update: a delete on device A
        // for a row that a *different*, still-live duplicate on this device
        // (B) currently occupies a UNIQUE slot for (e.g. two devices' own
        // independently-seeded copies of the same holding, each pointing at a
        // different not-yet-deduped account) must actually be gone before an
        // incoming FK remap tries to point another row at that same slot —
        // otherwise the insert/update below can hit a UNIQUE constraint
        // violation against a sibling this same payload was about to delete
        // moments later anyway. Applying deletes up front, not last, avoids
        // that ordering hazard; nothing here depends on `id_maps` (tombstones
        // are matched purely by `sync_id`, no FK columns involved), so moving
        // it earlier changes only ordering, not behavior, for every other case.
        for t in &payload.tombstones {
            // `t.table_name` is interpolated into SQL as an identifier below —
            // never do that for a value from an untrusted payload without
            // checking it against the known table set first (see
            // `is_valid_sync_table`'s doc comment).
            if !is_valid_sync_table(&t.table_name) {
                continue;
            }
            let t_deleted_at = sanitize_legacy_ts(t.deleted_at.clone());
            let t_hlc = sanitize_hlc(t.hlc.clone());

            if let Some(th) = &t_hlc {
                if max_remote_hlc.as_deref().map(|m| th.as_str() > m).unwrap_or(true) {
                    max_remote_hlc = Some(th.clone());
                }
            }

            let existing_row: Option<(i64, String, Option<String>)> = tx
                .query_row(
                    &format!("SELECT id, sync_updated_at, sync_hlc FROM \"{}\" WHERE sync_id = ?1", t.table_name),
                    [&t.sync_id],
                    |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get(2)?)),
                )
                .ok();
            if let Some((local_id, local_updated, local_hlc)) = existing_row {
                if a_wins(t_hlc.as_deref(), &t_deleted_at, local_hlc.as_deref(), &local_updated, true) {
                    tx.execute(&format!("DELETE FROM \"{}\" WHERE id = ?1", t.table_name), [local_id])
                        .map_err(|e| AppError::Database(format!("apply tombstone: {e}")))?;
                }
            }

            let existing_tombstone: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT deleted_at, hlc FROM sync_tombstones WHERE table_name = ?1 AND sync_id = ?2",
                    rusqlite::params![t.table_name, t.sync_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            let incoming_wins = match &existing_tombstone {
                Some((ex_deleted_at, ex_hlc)) => {
                    a_wins(t_hlc.as_deref(), &t_deleted_at, ex_hlc.as_deref(), ex_deleted_at, true)
                }
                None => true,
            };
            if incoming_wins {
                tx.execute(
                    "INSERT INTO sync_tombstones (table_name, sync_id, deleted_at, hlc) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(table_name, sync_id) DO UPDATE SET deleted_at = excluded.deleted_at, hlc = excluded.hlc",
                    rusqlite::params![t.table_name, t.sync_id, t_deleted_at, t_hlc],
                )
                .map_err(|e| AppError::Database(format!("merge tombstone: {e}")))?;
            }
        }

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

            let mut tombstoned: HashMap<String, (String, Option<String>)> = HashMap::new();
            {
                let mut stmt = tx
                    .prepare("SELECT sync_id, deleted_at, hlc FROM sync_tombstones WHERE table_name = ?1")
                    .map_err(|e| AppError::Database(e.to_string()))?;
                let rows = stmt
                    .query_map([table], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
                    })
                    .map_err(|e| AppError::Database(e.to_string()))?;
                for row in rows.flatten() {
                    tombstoned.insert(row.0, (row.1, row.2));
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
                let remote_updated = sanitize_legacy_ts(
                    obj.get("sync_updated_at").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                );
                let remote_hlc =
                    sanitize_hlc(obj.get("sync_hlc").and_then(|v| v.as_str()).map(str::to_string));
                if let Some(rh) = &remote_hlc {
                    if max_remote_hlc.as_deref().map(|m| rh.as_str() > m).unwrap_or(true) {
                        max_remote_hlc = Some(rh.clone());
                    }
                }

                if !remap_row_fks(table, &mut obj, &id_maps) {
                    continue; // required parent not present on this device — skip
                }
                obj.insert("sync_id".to_string(), serde_json::Value::String(sync_id.clone()));

                if let Some((local_id, local_row)) = local_by_sync_id.get(&sync_id) {
                    let local_id = *local_id;
                    id_maps.get_mut(table).unwrap().insert(remote_id, local_id);
                    let local_updated =
                        local_row.get("sync_updated_at").and_then(|v| v.as_str()).unwrap_or_default();
                    let local_hlc = local_row.get("sync_hlc").and_then(|v| v.as_str());
                    if a_wins(remote_hlc.as_deref(), &remote_updated, local_hlc, local_updated, false) {
                        update_row_by_id(&tx, table, local_id, &obj)?;
                        touched_this_table = true;
                        total_rows += 1;
                    }
                    continue;
                }

                if let Some((deleted_at, tombstone_hlc)) = tombstoned.get(&sync_id) {
                    if a_wins(tombstone_hlc.as_deref(), deleted_at, remote_hlc.as_deref(), &remote_updated, true) {
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

        if let Some(hlc) = &max_remote_hlc {
            apply_hlc_receive(&tx, hlc)?;
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

// ── One-time duplicate cleanup ───────────────────────────────────
// Devices that had pre-existing rows when MIGRATION_019 backfilled `sync_id`
// each minted their own random ids for the *same* logical rows (see the
// migration's backfill UPDATE). The very first merge sync between such
// devices therefore imported every one of those rows as "new" — nothing
// recognized them as the same row, since the merge only matches by
// `sync_id` — doubling every table with no UNIQUE constraint to catch it
// (the way default milestones are). This is a one-time cleanup for damage
// already done, not a fix to the ongoing merge path — `apply_backup_payload_merge`
// itself is correct once every device's rows carry a stable, matching `sync_id`.

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupeSummary {
    pub tables_affected: usize,
    pub rows_removed: usize,
}

/// Content fingerprint for duplicate detection — every column except the
/// ones expected to legitimately differ between two rows that are otherwise
/// the same logical entity: `id` (per-device local key), `sync_id`
/// (independently minted per device at backfill time — the whole reason this
/// cleanup exists), and `sync_updated_at`/`sync_hlc` (merge clocks, not row
/// content — `sync_hlc` in particular always differs between devices since it
/// embeds `device_id`, so it must be excluded here just like the others or
/// two devices' independently-seeded copies of the same row would never
/// fingerprint as duplicates).
fn fingerprint_row(obj: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut pairs: Vec<(&String, &serde_json::Value)> = obj
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "id" | "sync_id" | "sync_updated_at" | "sync_hlc"))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    serde_json::to_string(&pairs).unwrap_or_default()
}

/// Rewrites `row`'s FK columns using `id_maps` built by dedup passes over
/// earlier (parent) tables so far — unlike `remap_row_fks` (used during
/// import merge), an id absent from the map means "this parent wasn't
/// deduped," not "this parent is missing," so the column is left untouched
/// rather than nulled out or treated as a required-FK failure.
fn remap_fk_if_deduped(
    table: &str,
    row: &mut serde_json::Map<String, serde_json::Value>,
    id_maps: &HashMap<&'static str, HashMap<i64, i64>>,
) -> bool {
    let mut changed = false;
    for &(col, ref_table, _required) in fk_columns(table) {
        let Some(id) = row.get(col).and_then(|v| v.as_i64()) else { continue };
        if let Some(&new_id) = id_maps.get(ref_table).and_then(|m| m.get(&id)) {
            row.insert(col.to_string(), serde_json::json!(new_id));
            changed = true;
        }
    }
    if table == "transactions" {
        if let Some(id) = row.get("holding_id").and_then(|v| v.as_i64()) {
            let asset_class = row.get("asset_class").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(&new_id) = holding_table_for_asset_class(asset_class)
                .and_then(|t| id_maps.get(t))
                .and_then(|m| m.get(&id))
            {
                row.insert("holding_id".to_string(), serde_json::json!(new_id));
                changed = true;
            }
        }
    }
    changed
}

/// One-time cleanup for the migration-backfill duplicate bug: walks
/// `SYNC_TABLES` in FK-safe (parent-first) order, and within each table
/// groups rows by content fingerprint. A group with more than one row is a
/// duplicate set — keeps the row with the lexicographically smallest
/// `sync_id` (a deterministic, content-only rule so independent runs on two
/// different devices converge on the *same* surviving row rather than each
/// keeping its own), and deletes the rest. Deletion goes through a normal
/// `DELETE`, so the existing `trg_<table>_tombstone` trigger fires and the
/// removal propagates to other devices on the next sync exactly like any
/// other delete — no special-casing needed here.
///
/// Child tables get their FK columns rewritten (via `remap_fk_if_deduped`)
/// before fingerprinting, using the id map built from parent tables' own
/// dedup pass — so e.g. two duplicate transactions that each point at a
/// different (but now-deduped) duplicate account row are recognized as the
/// same content instead of missed because of a stale FK.
pub(crate) fn dedupe_duplicate_rows_impl(db: &DbPool) -> Result<DedupeSummary> {
    let mut conn = db.get()?;
    conn.execute_batch("PRAGMA foreign_keys = OFF").map_err(|e| AppError::Database(e.to_string()))?;

    let result = (|| -> Result<DedupeSummary> {
        let tx = conn.transaction()?;
        let mut id_maps: HashMap<&'static str, HashMap<i64, i64>> = HashMap::new();
        let mut tables_affected = 0usize;
        let mut rows_removed = 0usize;

        for &table in SYNC_TABLES {
            id_maps.entry(table).or_default();
            let rows = dump_table(&tx, table)?;
            if rows.is_empty() {
                continue;
            }

            let objs: Vec<serde_json::Map<String, serde_json::Value>> =
                rows.into_iter().filter_map(|r| r.as_object().cloned()).collect();

            // FK remap is computed in memory only, never written yet — a table
            // can carry its own UNIQUE constraint (e.g. equity_holdings on
            // `account_id, isin`), so writing the remapped value to a
            // still-live duplicate before it's deleted can collide with the
            // duplicate that hasn't been removed yet.
            let remapped: Vec<(bool, serde_json::Map<String, serde_json::Value>)> = objs
                .into_iter()
                .map(|obj| {
                    let mut copy = obj.clone();
                    let changed = remap_fk_if_deduped(table, &mut copy, &id_maps);
                    (changed, copy)
                })
                .collect();

            let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
            for (i, (_, obj)) in remapped.iter().enumerate() {
                groups.entry(fingerprint_row(obj)).or_default().push(i);
            }

            let mut touched = false;
            let mut survivors: Vec<usize> = Vec::new();
            for (_, mut idxs) in groups {
                if idxs.len() < 2 {
                    survivors.push(idxs[0]);
                    continue;
                }
                idxs.sort_by(|&a, &b| {
                    let sa = remapped[a].1.get("sync_id").and_then(|v| v.as_str()).unwrap_or_default();
                    let sb = remapped[b].1.get("sync_id").and_then(|v| v.as_str()).unwrap_or_default();
                    sa.cmp(sb)
                });
                let keep_idx = idxs[0];
                let keep_id = remapped[keep_idx].1.get("id").and_then(|v| v.as_i64()).unwrap();
                survivors.push(keep_idx);
                for &dup_idx in &idxs[1..] {
                    let dup_id = remapped[dup_idx].1.get("id").and_then(|v| v.as_i64()).unwrap();
                    tx.execute(&format!("DELETE FROM \"{table}\" WHERE id = ?1"), [dup_id])
                        .map_err(|e| AppError::Database(format!("dedupe delete {table}: {e}")))?;
                    id_maps.get_mut(table).unwrap().insert(dup_id, keep_id);
                    rows_removed += 1;
                    touched = true;
                }
            }

            // Now that duplicates are gone, it's safe to persist the FK remap
            // for whichever row of each group survived.
            for &i in &survivors {
                if remapped[i].0 {
                    let id = remapped[i].1.get("id").and_then(|v| v.as_i64()).unwrap();
                    update_row_by_id(&tx, table, id, &remapped[i].1)?;
                }
            }

            if touched {
                tables_affected += 1;
            }
        }

        tx.commit()?;
        Ok(DedupeSummary { tables_affected, rows_removed })
    })();

    let _ = conn.execute_batch("PRAGMA foreign_keys = ON");
    result
}

/// One-time cleanup command for the migration-backfill duplicate bug — see
/// `dedupe_duplicate_rows_impl`. Safe to run more than once (a second run
/// finds no duplicate groups and is a no-op) and safe to run independently
/// on each device (the keep-rule is deterministic by content, so both sides
/// converge on the same surviving row).
#[tauri::command]
pub fn dedupe_duplicate_rows(state: State<DbState>) -> Result<DedupeSummary> {
    dedupe_duplicate_rows_impl(&state.0)
}

const SETTING_DEDUPE_V1_APPLIED: &str = "dedupe_v1_applied";

/// Runs `dedupe_duplicate_rows_impl` automatically, once ever per device,
/// right after unlock — no user action, no UI, no manual command. Gated by
/// an `app_settings` flag (same idempotent-flag pattern as MIGRATION_017's
/// `notified_reminder_ids`) so it's a no-op on every unlock after the first.
/// Runs before the sync scheduler starts so the very next outgoing sync
/// already reflects the cleaned-up local state instead of re-exporting
/// duplicates that were about to be deleted.
pub(crate) fn run_dedupe_once_on_unlock(db: &DbPool) -> Result<Option<DedupeSummary>> {
    {
        let conn = db.get()?;
        let already: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [SETTING_DEDUPE_V1_APPLIED],
                |r| r.get(0),
            )
            .ok();
        if already.as_deref() == Some("true") {
            return Ok(None);
        }
    }
    let summary = dedupe_duplicate_rows_impl(db)?;
    {
        let conn = db.get()?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, 'true')
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [SETTING_DEDUPE_V1_APPLIED],
        )?;
    }
    Ok(Some(summary))
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

/// Real filesystem paths (desktop) get an atomic write — write to a sibling
/// `.tmp` file then `rename` over the target, so a crash/power-loss mid-write
/// can never leave a torn file the next import fails to decrypt. `file://`/
/// Android `content://` URIs have no equivalent atomic rename across schemes
/// and keep the old in-place truncate+write (unchanged, known limitation).
fn write_via_fs(app: &AppHandle, path: &str, bytes: &[u8]) -> std::io::Result<()> {
    let file_path = path.parse::<tauri_plugin_fs::FilePath>().unwrap();

    if let Some(real_path) = file_path.as_path() {
        let file_name = real_path.file_name().expect("sync backup path must have a filename");
        let tmp_path = real_path.with_file_name(format!("{}.tmp", file_name.to_string_lossy()));
        std::fs::write(&tmp_path, bytes)?;
        std::fs::rename(&tmp_path, real_path)?;
        return Ok(());
    }

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
        return Err(AppError::UnsupportedBackupVersion(data[4], FORMAT_VERSION));
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
    let rows = stmt
        .query_map([], |row| {
            let mut map = serde_json::Map::new();
            for (i, col) in col_names.iter().enumerate() {
                let val = match row.get_ref(i) {
                    Ok(vref) => value_ref_to_json(vref),
                    Err(_) => serde_json::Value::Null,
                };
                map.insert(col.clone(), val);
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
        assert!(
            matches!(err, AppError::UnsupportedBackupVersion(99, v) if v == FORMAT_VERSION),
            "got: {err:?}"
        );
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
    /// the one with the newer `sync_updated_at` must win — the legacy
    /// (pre-HLC, or peer-hasn't-upgraded) comparison path. `sync_hlc` is
    /// stripped from A's payload here since it now takes priority whenever
    /// both sides have one — this test is specifically about the
    /// `sync_updated_at` fallback, covered on its own in
    /// `merge_falls_back_to_sync_updated_at_when_remote_hlc_missing` too.
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

        let mut payload_2 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        for rows in payload_2.tables.values_mut() {
            for row in rows {
                if let Some(obj) = row.as_object_mut() {
                    obj.remove("sync_hlc");
                }
            }
        }
        apply_backup_payload(&db_b, &payload_2).unwrap();

        let name: String =
            db_b.get().unwrap().query_row("SELECT name FROM accounts", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "A Edited", "the strictly-newer edit must win");
    }

    // ── Hybrid Logical Clock (sync_hlc) ──

    fn sync_hlc_of(db: &crate::db::DbPool, table: &str) -> Option<String> {
        db.get()
            .unwrap()
            .query_row(&format!("SELECT sync_hlc FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    /// The trigger's logical counter must advance instead of physical time
    /// when the clock hasn't moved forward — forcing `sync_hlc_state` into the
    /// future (as if a previous write already claimed that millisecond) proves
    /// this deterministically, without depending on two real writes landing in
    /// the same millisecond by chance.
    #[test]
    fn hlc_logical_counter_bumps_when_physical_clock_does_not_advance() {
        let (_dir, db) = test_db_pool();
        let conn = db.get().unwrap();
        let future_ms = 9_999_999_999_999i64; // far past any real "now" this test will run at
        conn.execute(
            "UPDATE sync_hlc_state SET last_physical_ms = ?1, last_logical = 5 WHERE id = 1",
            [future_ms],
        )
        .unwrap();

        conn.execute("INSERT INTO accounts (name, type) VALUES ('A', 'bank')", []).unwrap();
        let hlc_1 = sync_hlc_of(&db, "accounts").unwrap();
        assert_eq!(hlc_1, format!("{future_ms:013}:{:09}:{}", 6, device_id_of(&db)));

        conn.execute("INSERT INTO accounts (name, type) VALUES ('B', 'bank')", []).unwrap();
        let state: (i64, i64) = conn
            .query_row("SELECT last_physical_ms, last_logical FROM sync_hlc_state WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(state, (future_ms, 7), "logical counter keeps advancing while physical clock is stuck");
    }

    fn device_id_of(db: &crate::db::DbPool) -> String {
        db.get()
            .unwrap()
            .query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn hlc_values_are_lexicographically_ordered_by_write_order() {
        let (_dir, db) = test_db_pool();
        let conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts (name, type) VALUES ('A', 'bank')", []).unwrap();
        let first = sync_hlc_of(&db, "accounts").unwrap();
        conn.execute("UPDATE accounts SET name = 'A2' WHERE name = 'A'", []).unwrap();
        let second: String = conn.query_row("SELECT sync_hlc FROM accounts", [], |r| r.get(0)).unwrap();
        assert!(second.as_str() > first.as_str(), "a later write's sync_hlc must sort after an earlier one");
    }

    /// The whole point of upgrading past a plain wall-clock compare: a device
    /// whose `sync_updated_at` is forged/rolled-back (simulating clock skew or
    /// a buggy remote export) must not win the merge if its `sync_hlc` — set by
    /// this same device's own trigger at the time of the real edit — correctly
    /// reflects that the edit happened earlier than usual would suggest.
    #[test]
    fn hlc_wins_over_forged_sync_updated_at() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        let payload_1 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload_1).unwrap();

        // B edits second (later real sync_hlc than A's original), but A's export
        // below claims a `sync_updated_at` far in the future — a forged/skewed
        // wall clock is exactly the failure mode `sync_hlc` must not fall for.
        db_b.get().unwrap().execute("UPDATE accounts SET name = 'B Edited' WHERE name = 'Source Bank'", []).unwrap();

        let payload_2 = {
            let conn = db_a.get().unwrap();
            conn.execute(
                "UPDATE accounts SET sync_updated_at = datetime('now', '+10 years') WHERE name = 'Source Bank'",
                [],
            )
            .unwrap();
            build_backup_payload(&conn).unwrap().0
        };
        apply_backup_payload(&db_b, &payload_2).unwrap();

        let name: String =
            db_b.get().unwrap().query_row("SELECT name FROM accounts", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "B Edited", "sync_hlc must win over a forged/skewed sync_updated_at");
    }

    /// A row from a peer that hasn't upgraded yet (or a legacy row untouched
    /// since before this device's own HLC upgrade) has no `sync_hlc` at all —
    /// merge must fall back to the legacy `sync_updated_at` compare exactly as
    /// it worked before, not treat the missing value as "always loses".
    #[test]
    fn merge_falls_back_to_sync_updated_at_when_remote_hlc_missing() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        let payload_1 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload_1).unwrap();

        db_b.get().unwrap().execute("UPDATE accounts SET name = 'B Edited' WHERE name = 'Source Bank'", []).unwrap();

        let mut payload_2 = {
            let conn = db_a.get().unwrap();
            conn.execute("UPDATE accounts SET name = 'A Edited' WHERE name = 'Source Bank'", []).unwrap();
            conn.execute(
                "UPDATE accounts SET sync_updated_at = datetime('now', '+1 hour') WHERE name = 'A Edited'",
                [],
            )
            .unwrap();
            build_backup_payload(&conn).unwrap().0
        };
        // Simulate a pre-HLC-upgrade peer's export: strip `sync_hlc` from every row.
        for rows in payload_2.tables.values_mut() {
            for row in rows {
                if let Some(obj) = row.as_object_mut() {
                    obj.remove("sync_hlc");
                }
            }
        }

        apply_backup_payload(&db_b, &payload_2).unwrap();

        let name: String =
            db_b.get().unwrap().query_row("SELECT name FROM accounts", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "A Edited", "must fall back to sync_updated_at when remote has no sync_hlc");
    }

    /// `device_id` must never travel through a settings import — it's not in
    /// `SYNC_SETTINGS_KEYS` (an allow-list), so even a payload that includes it
    /// (a buggy or malicious peer) must not overwrite this device's own id.
    #[test]
    fn device_id_never_imported_from_peer_settings() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);
        let own_device_id = device_id_of(&db_b);

        let mut payload = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        payload.settings.insert("device_id".to_string(), "malicious-peer-device-id".to_string());

        apply_backup_payload(&db_b, &payload).unwrap();

        assert_eq!(device_id_of(&db_b), own_device_id, "device_id must never be overwritten by an import");
    }

    /// The HLC "receive" rule: after importing a peer's higher HLC, this
    /// device's own `sync_hlc_state` must be pulled forward, or a local edit
    /// made right after this import could still stamp a lower HLC than the row
    /// it just imported. Uses a realistic few-hours clock-ahead skew (not an
    /// absurd far-future value) since `sanitize_hlc` now rejects implausible
    /// ones outright — this is testing the receive rule, not the sanitizer.
    #[test]
    fn merge_pulls_local_hlc_state_forward_to_match_imported_peer() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        // Give A's clock a realistic head start (a few hours fast) before it exports.
        let ahead_ms = chrono::Utc::now().timestamp_millis() + 3 * 60 * 60 * 1000;
        db_a.get()
            .unwrap()
            .execute(
                "UPDATE sync_hlc_state SET last_physical_ms = ?1, last_logical = 3 WHERE id = 1",
                [ahead_ms],
            )
            .unwrap();
        db_a.get().unwrap().execute("UPDATE accounts SET name = 'A Edited' WHERE name = 'Source Bank'", []).unwrap();

        let payload = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload).unwrap();

        let b_state: (i64, i64) = db_b
            .get()
            .unwrap()
            .query_row("SELECT last_physical_ms, last_logical FROM sync_hlc_state WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert!(b_state.0 >= ahead_ms, "B's own clock must be pulled forward to at least A's");
    }

    // ── Untrusted-payload validation (corrupted file / malicious peer) ──

    /// A tombstone naming a table outside `SYNC_TABLES` — whether garbage
    /// from a corrupted file or a crafted value probing for SQL-identifier
    /// injection — must be rejected by the allow-list before it ever reaches
    /// a query string, and must not error out or otherwise disrupt the rest
    /// of the import.
    #[test]
    fn tombstone_with_invalid_table_name_is_rejected_not_used_in_sql() {
        let (_dir_a, db_a) = test_db_pool();
        seed_source_db(&db_a);
        let goals_before = row_count(&db_a, "goals");

        let mut payload = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        for table_name in ["not_a_real_table", "accounts\" UNION SELECT 1,2,3 --"] {
            payload.tombstones.push(TombstoneRow {
                table_name: table_name.to_string(),
                sync_id: "deadbeef00000000000000000000000".to_string(),
                deleted_at: "2026-01-01 00:00:00".to_string(),
                hlc: None,
            });
        }

        apply_backup_payload(&db_a, &payload).unwrap();

        assert_eq!(row_count(&db_a, "goals"), goals_before, "unrelated table must be untouched");
    }

    #[test]
    fn sanitize_hlc_rejects_wrong_shape() {
        assert_eq!(sanitize_hlc(None), None);
        assert_eq!(sanitize_hlc(Some("garbage".to_string())), None);
        assert_eq!(sanitize_hlc(Some("123:456:789".to_string())), None); // wrong widths
        assert_eq!(
            sanitize_hlc(Some("1234567890123:000000001:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_string())),
            None,
            "device id must be hex"
        );
    }

    #[test]
    fn sanitize_hlc_rejects_implausible_future_physical_ms() {
        // 13 nines — centuries past any real "now" this app will run at.
        let forged = "9999999999999:000000001:00000000000000000000000000000000".to_string();
        assert_eq!(sanitize_hlc(Some(forged)), None);
    }

    #[test]
    fn sanitize_hlc_accepts_well_formed_current_value() {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let valid = format!("{now_ms:013}:000000001:00000000000000000000000000000000");
        assert_eq!(sanitize_hlc(Some(valid.clone())), Some(valid));
    }

    #[test]
    fn sanitize_legacy_ts_rejects_unparseable_and_implausible_future() {
        assert_eq!(sanitize_legacy_ts("not-a-date".to_string()), "");
        assert_eq!(sanitize_legacy_ts("2099-01-01 00:00:00".to_string()), "");
    }

    #[test]
    fn sanitize_legacy_ts_accepts_ordinary_clock_drift() {
        // A few minutes of real clock drift between devices must not be
        // treated as suspicious — only implausible (multi-day+) jumps are.
        let near_future = (chrono::Utc::now() + chrono::Duration::minutes(5))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(sanitize_legacy_ts(near_future.clone()), near_future);
    }

    /// End-to-end: a row claiming a forged far-future `sync_hlc` must not
    /// permanently win the merge, and must not drag this device's own HLC
    /// clock into the future via the "receive" rule either — that would
    /// poison every sync this device does afterward, not just this one row.
    #[test]
    fn forged_future_hlc_does_not_win_and_does_not_poison_local_clock() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        seed_source_db(&db_a);

        let payload_1 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload_1).unwrap();
        db_b.get().unwrap().execute("UPDATE accounts SET name = 'B Edited' WHERE name = 'Source Bank'", []).unwrap();

        let mut payload_2 = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        for row in payload_2.tables.get_mut("accounts").unwrap() {
            if let Some(obj) = row.as_object_mut() {
                obj.insert(
                    "sync_hlc".to_string(),
                    serde_json::Value::String(
                        "9999999999999:000000001:00000000000000000000000000000000".to_string(),
                    ),
                );
            }
        }

        apply_backup_payload(&db_b, &payload_2).unwrap();

        let name: String =
            db_b.get().unwrap().query_row("SELECT name FROM accounts", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "B Edited", "forged future HLC must not win over B's real edit");

        let b_physical_ms: i64 = db_b
            .get()
            .unwrap()
            .query_row("SELECT last_physical_ms FROM sync_hlc_state WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert!(b_physical_ms < 9_999_999_999_999, "forged HLC must not poison this device's own clock state");
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

    // ── One-time duplicate cleanup (dedupe_duplicate_rows_impl) ──
    // Reproduces the migration-backfill bug: two devices independently seed
    // identical content (their own `sync_id`s minted at backfill time), then
    // sync in both directions — the exact sequence that doubled every
    // non-UNIQUE-constrained table on first sync.

    /// Pre-existing test flakiness, unrelated to sync_hlc: `seed_source_db`
    /// lets `created_at`/`updated_at` default to real `datetime('now')` at
    /// insert time, which only has whole-second resolution — two back-to-back
    /// calls (one per device) landing on opposite sides of a real second
    /// boundary would give the "same" seeded content a different fingerprint
    /// (`fingerprint_row` includes these two columns, correctly, since a real
    /// independently-created duplicate legitimately can have different
    /// business timestamps too) and `dedupe_duplicate_rows_impl` would never
    /// recognize them as duplicates. Forcing identical values here makes the
    /// *test fixture's* intent — "these two devices seeded the same logical
    /// content" — deterministic, without changing `fingerprint_row` itself
    /// (which is correctly timestamp-sensitive for real, non-simulated data).
    fn duplicate_via_first_sync(db_a: &crate::db::DbPool, db_b: &crate::db::DbPool) {
        seed_source_db(db_a);
        seed_source_db(db_b);
        for db in [db_a, db_b] {
            let conn = db.get().unwrap();
            conn.execute("UPDATE accounts SET created_at = '2026-01-01 00:00:00', updated_at = '2026-01-01 00:00:00'", []).unwrap();
            conn.execute("UPDATE equity_holdings SET created_at = '2026-01-01 00:00:00', updated_at = '2026-01-01 00:00:00'", []).unwrap();
        }
        let payload_a = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(db_b, &payload_a).unwrap();
        let payload_b = { build_backup_payload(&db_b.get().unwrap()).unwrap().0 };
        apply_backup_payload(db_a, &payload_b).unwrap();
    }

    fn row_count(db: &crate::db::DbPool, table: &str) -> i64 {
        db.get().unwrap().query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn dedupe_collapses_duplicate_rows_from_first_sync() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        assert_eq!(row_count(&db_a, "accounts"), 2, "precondition: doubled by the first sync");
        assert_eq!(row_count(&db_a, "equity_holdings"), 2, "precondition: doubled by the first sync");

        let summary = dedupe_duplicate_rows_impl(&db_a).unwrap();
        assert_eq!(summary.rows_removed, 2, "one duplicate account + one duplicate holding");
        assert_eq!(summary.tables_affected, 2);
        assert_eq!(row_count(&db_a, "accounts"), 1);
        assert_eq!(row_count(&db_a, "equity_holdings"), 1);
    }

    #[test]
    fn dedupe_remaps_child_fk_to_surviving_parent() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        dedupe_duplicate_rows_impl(&db_a).unwrap();

        let conn = db_a.get().unwrap();
        let account_id: i64 = conn.query_row("SELECT id FROM accounts", [], |r| r.get(0)).unwrap();
        let holding_account_id: i64 =
            conn.query_row("SELECT account_id FROM equity_holdings", [], |r| r.get(0)).unwrap();
        assert_eq!(
            holding_account_id, account_id,
            "the surviving holding must point at the surviving account, not a deleted duplicate"
        );
    }

    #[test]
    fn dedupe_is_idempotent() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        dedupe_duplicate_rows_impl(&db_a).unwrap();
        let second = dedupe_duplicate_rows_impl(&db_a).unwrap();
        assert_eq!(second.rows_removed, 0, "second run must find nothing left to dedupe");
        assert_eq!(second.tables_affected, 0);
    }

    #[test]
    fn dedupe_deletes_propagate_to_other_device_via_tombstone() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        dedupe_duplicate_rows_impl(&db_a).unwrap();
        assert_eq!(row_count(&db_b, "accounts"), 2, "precondition: B still has its own duplicate");

        // A re-exports — now carrying a tombstone for the row it just deleted
        // via the ordinary `trg_accounts_tombstone` trigger — and B applies it.
        let payload = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };
        apply_backup_payload(&db_b, &payload).unwrap();

        assert_eq!(row_count(&db_b, "accounts"), 1, "B's duplicate must be removed by the propagated tombstone");
    }

    #[test]
    fn dedupe_run_independently_on_both_devices_converges_on_same_row() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        // Both devices dedupe entirely on their own, with no further sync
        // between them — the deterministic tie-break (smallest sync_id) must
        // still pick the same surviving row on each side, so a real user
        // running this on two offline devices doesn't end up with divergent
        // "kept" copies.
        dedupe_duplicate_rows_impl(&db_a).unwrap();
        dedupe_duplicate_rows_impl(&db_b).unwrap();

        let sync_id_of_account = |db: &crate::db::DbPool| -> String {
            db.get().unwrap().query_row("SELECT sync_id FROM accounts", [], |r| r.get(0)).unwrap()
        };
        assert_eq!(
            sync_id_of_account(&db_a),
            sync_id_of_account(&db_b),
            "independent dedupe on both devices must keep the same row"
        );
    }

    #[test]
    fn dedupe_leaves_genuinely_distinct_rows_alone() {
        let (_dir, db) = test_db_pool();
        {
            let conn = db.get().unwrap();
            conn.execute("INSERT INTO accounts (name, type) VALUES ('Alpha', 'bank')", []).unwrap();
            conn.execute("INSERT INTO accounts (name, type) VALUES ('Beta', 'bank')", []).unwrap();
        }

        let summary = dedupe_duplicate_rows_impl(&db).unwrap();
        assert_eq!(summary.rows_removed, 0, "distinct content must never be treated as a duplicate");
        assert_eq!(row_count(&db, "accounts"), 2);
    }

    #[test]
    fn dedupe_no_op_on_fresh_db_with_no_duplicates() {
        let (_dir, db) = test_db_pool();
        seed_source_db(&db);

        let summary = dedupe_duplicate_rows_impl(&db).unwrap();
        assert_eq!(summary.rows_removed, 0);
        assert_eq!(summary.tables_affected, 0);
    }

    // ── One-time-on-unlock gating (run_dedupe_once_on_unlock) ──

    #[test]
    fn run_dedupe_once_on_unlock_runs_the_first_time() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        let outcome = run_dedupe_once_on_unlock(&db_a).unwrap();
        let summary = outcome.expect("must run on first-ever call");
        assert_eq!(summary.rows_removed, 2);
        assert_eq!(row_count(&db_a, "accounts"), 1);
    }

    #[test]
    fn run_dedupe_once_on_unlock_is_a_no_op_on_later_unlocks() {
        let (_dir_a, db_a) = test_db_pool();
        let (_dir_b, db_b) = test_db_pool();
        duplicate_via_first_sync(&db_a, &db_b);

        run_dedupe_once_on_unlock(&db_a).unwrap();
        assert_eq!(row_count(&db_a, "accounts"), 1, "precondition: first run already cleaned up");

        // A *different* pair of duplicate-content rows appears later (e.g. a
        // second, unrelated pre-existing-data collision this cleanup never
        // saw) — unlike the tombstoned pair from the first run, there's
        // nothing here to stop `dedupe_duplicate_rows_impl` from collapsing
        // it if it ran. The once-only flag must still suppress it: only the
        // explicit manual command should touch data after the first unlock.
        db_a.get()
            .unwrap()
            .execute("INSERT INTO accounts (name, type) VALUES ('Alpha', 'bank')", [])
            .unwrap();
        db_a.get()
            .unwrap()
            .execute("INSERT INTO accounts (name, type) VALUES ('Alpha', 'bank')", [])
            .unwrap();
        assert_eq!(row_count(&db_a, "accounts"), 3, "precondition: a new duplicate pair exists");

        let outcome = run_dedupe_once_on_unlock(&db_a).unwrap();
        assert!(outcome.is_none(), "must not run again after the flag is set");
        assert_eq!(row_count(&db_a, "accounts"), 3, "the flag gate must leave later duplicates untouched");
    }

    // ── Wipe All Data must not block a subsequent restore (sync_tombstones) ──
    // Every `DELETE` in `wipe_all_data_impl` fires the row's tombstone trigger,
    // so without also clearing `sync_tombstones`, importing a backup taken
    // *before* the wipe looks — to the merge's own newer-tombstone check — like
    // resurrecting something this device deliberately deleted, and every row
    // is silently skipped. This is the exact bug behind "0 records restored."

    #[test]
    fn wipe_then_import_backup_actually_restores_data() {
        let (_dir_a, db_a) = test_db_pool();
        seed_source_db(&db_a);

        // The backup a user would export before wiping.
        let payload = { build_backup_payload(&db_a.get().unwrap()).unwrap().0 };

        crate::settings::commands::wipe_all_data_impl(&db_a).unwrap();
        assert_eq!(row_count(&db_a, "accounts"), 0, "precondition: wipe cleared everything");

        let summary = apply_backup_payload(&db_a, &payload).unwrap();
        assert!(summary.rows_imported > 0, "importing a pre-wipe backup must actually restore rows, not no-op");
        assert_eq!(row_count(&db_a, "accounts"), 1, "the account must come back");
        assert_eq!(row_count(&db_a, "equity_holdings"), 1, "the equity holding must come back");
    }

    #[test]
    fn wipe_all_data_clears_sync_tombstones() {
        let (_dir_a, db_a) = test_db_pool();
        seed_source_db(&db_a);

        crate::settings::commands::wipe_all_data_impl(&db_a).unwrap();

        let tombstones: i64 =
            db_a.get().unwrap().query_row("SELECT count(*) FROM sync_tombstones", [], |r| r.get(0)).unwrap();
        assert_eq!(tombstones, 0, "a wipe must not leave tombstones behind to block a future restore");
    }
}

// ── Property-based tests (proptest) ────────────────────────────
// Off by default — gated behind the `property-tests` feature (see
// Cargo.toml's doc comment) and run via `make test-property`, not part of
// the everyday `make test` loop, since each generated case spins up a real
// temp-file SQLCipher DB. The hand-written scenario tests above check
// specific, hand-picked cases; these check properties across many randomly
// generated ones — the two are complementary, not a replacement for each
// other. Scoped to the `accounts` table (no FK columns, simplest to model)
// rather than all 21 `SYNC_TABLES`, since the point is exercising the merge
// *decision* machinery (sync_id matching, HLC/timestamp comparison,
// tombstones, validation), not re-deriving every table's own dedicated tests.
#[cfg(all(test, feature = "property-tests"))]
mod proptests {
    use super::*;
    use crate::test_utils::test_db_pool;
    use proptest::prelude::*;

    fn account_name_strategy() -> impl Strategy<Value = String> {
        "[A-Za-z]{1,12}"
    }

    #[derive(Debug, Clone)]
    enum AccountOp {
        Insert(String),
        Rename(usize, String),
        Delete(usize),
    }

    fn account_op_strategy() -> impl Strategy<Value = AccountOp> {
        prop_oneof![
            account_name_strategy().prop_map(AccountOp::Insert),
            (any::<usize>(), account_name_strategy()).prop_map(|(i, n)| AccountOp::Rename(i, n)),
            any::<usize>().prop_map(AccountOp::Delete),
        ]
    }

    /// Applies each op to `db`'s `accounts` table, in order. `Rename`/`Delete`
    /// target the `idx`-th existing row (mod current row count) so arbitrary
    /// `usize` values from the generator always land on a real row instead of
    /// needing a separate "valid index" strategy — a no-op when the table's
    /// currently empty.
    fn apply_ops(db: &crate::db::DbPool, ops: &[AccountOp]) {
        let conn = db.get().unwrap();
        for op in ops {
            let count: i64 = conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0)).unwrap();
            match op {
                AccountOp::Insert(name) => {
                    conn.execute("INSERT INTO accounts (name, type) VALUES (?1, 'bank')", [name]).unwrap();
                }
                AccountOp::Rename(idx, new_name) if count > 0 => {
                    let id: i64 = conn
                        .query_row(
                            &format!("SELECT id FROM accounts LIMIT 1 OFFSET {}", (*idx as i64) % count),
                            [],
                            |r| r.get(0),
                        )
                        .unwrap();
                    conn.execute("UPDATE accounts SET name = ?1 WHERE id = ?2", rusqlite::params![new_name, id])
                        .unwrap();
                }
                AccountOp::Delete(idx) if count > 0 => {
                    let id: i64 = conn
                        .query_row(
                            &format!("SELECT id FROM accounts LIMIT 1 OFFSET {}", (*idx as i64) % count),
                            [],
                            |r| r.get(0),
                        )
                        .unwrap();
                    conn.execute("DELETE FROM accounts WHERE id = ?1", [id]).unwrap();
                }
                _ => {} // Rename/Delete with nothing to target yet — no-op.
            }
        }
    }

    fn account_names(db: &crate::db::DbPool) -> Vec<String> {
        let conn = db.get().unwrap();
        let mut names: Vec<String> = conn
            .prepare("SELECT name FROM accounts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        names.sort();
        names
    }

    fn sync_a_to_b(a: &crate::db::DbPool, b: &crate::db::DbPool) {
        let payload = build_backup_payload(&a.get().unwrap()).unwrap().0;
        apply_backup_payload(b, &payload).unwrap();
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

        /// A CRDT merge must be idempotent: replaying an unchanged remote
        /// payload (e.g. two sync ticks in a row before either side changed
        /// anything) must never alter the result of the first apply.
        #[test]
        fn merge_is_idempotent(ops in proptest::collection::vec(account_op_strategy(), 0..12)) {
            let (_dir_a, db_a) = test_db_pool();
            let (_dir_b, db_b) = test_db_pool();
            apply_ops(&db_a, &ops);

            let payload = build_backup_payload(&db_a.get().unwrap()).unwrap().0;
            apply_backup_payload(&db_b, &payload).unwrap();
            let after_first = account_names(&db_b);

            apply_backup_payload(&db_b, &payload).unwrap();
            let after_second = account_names(&db_b);

            prop_assert_eq!(after_first, after_second);
        }

        /// Two devices independently mutating their own copy, then syncing
        /// with each other, must converge on the same final content —
        /// regardless of which side happens to pull first. Two full rounds
        /// (not just one A->B-then-B->A pass) since a single round can leave
        /// whichever side went second with content the other hasn't seen back
        /// yet — real devices converge over repeated ticks, not necessarily
        /// in exactly one round trip.
        #[test]
        fn two_devices_converge_regardless_of_sync_order(
            ops_a in proptest::collection::vec(account_op_strategy(), 0..8),
            ops_b in proptest::collection::vec(account_op_strategy(), 0..8),
            a_first in any::<bool>(),
        ) {
            let (_dir_a, db_a) = test_db_pool();
            let (_dir_b, db_b) = test_db_pool();
            apply_ops(&db_a, &ops_a);
            apply_ops(&db_b, &ops_b);

            if a_first {
                sync_a_to_b(&db_a, &db_b);
                sync_a_to_b(&db_b, &db_a);
            } else {
                sync_a_to_b(&db_b, &db_a);
                sync_a_to_b(&db_a, &db_b);
            }
            sync_a_to_b(&db_a, &db_b);
            sync_a_to_b(&db_b, &db_a);

            prop_assert_eq!(account_names(&db_a), account_names(&db_b));
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

        /// Arbitrary/garbage row and tombstone shapes — not just the specific
        /// malformed cases #2's hand-written tests picked — must never panic
        /// the merge, only ever return `Ok` or a clean `Err`. This is the
        /// proptest counterpart to `sanitize_hlc`/`sanitize_legacy_ts`/
        /// `is_valid_sync_table`'s unit tests: same validation layer, random
        /// inputs instead of hand-picked ones.
        #[test]
        fn merge_never_panics_on_arbitrary_row_and_tombstone_shapes(
            garbage_name in any::<Option<String>>(),
            garbage_sync_id in any::<Option<String>>(),
            garbage_hlc in any::<Option<String>>(),
            garbage_table_name in any::<String>(),
            garbage_deleted_at in any::<String>(),
        ) {
            let (_dir, db) = test_db_pool();

            let mut tables = HashMap::new();
            tables.insert(
                "accounts".to_string(),
                vec![serde_json::json!({
                    "id": 1,
                    "name": garbage_name,
                    "type": "bank",
                    "provider": null, "external_id": null, "is_active": 1,
                    "created_at": "2026-01-01 00:00:00", "updated_at": "2026-01-01 00:00:00",
                    "sync_id": garbage_sync_id,
                    "sync_updated_at": "2026-01-01 00:00:00",
                    "sync_hlc": garbage_hlc,
                })],
            );
            let payload = BackupPayload {
                version: FORMAT_VERSION,
                exported_at: "2026-01-01T00:00:00+00:00".to_string(),
                tables,
                settings: HashMap::new(),
                tombstones: vec![TombstoneRow {
                    table_name: garbage_table_name,
                    sync_id: "deadbeef00000000000000000000000".to_string(),
                    deleted_at: garbage_deleted_at,
                    hlc: None,
                }],
            };

            // Must never panic — Ok or Err are both fine, a crash is not.
            let _ = apply_backup_payload(&db, &payload);
        }
    }
}
