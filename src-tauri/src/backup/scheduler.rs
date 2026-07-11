// Background auto-sync scheduler — periodically pushes/pulls an encrypted
// `.svbak` snapshot (see `backup::commands`) to/from a folder the user syncs
// themselves (Dropbox/Drive/OneDrive/etc.). No app-hosted backend involved:
// the cloud provider's own client propagates the file between devices, this
// loop just decides when to read/write it. Started on unlock, stopped on
// lock/quit — same lifecycle as `notifications::scheduler::SchedulerState`.

#[cfg(not(target_os = "android"))]
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::backup::commands::{
    decrypt_sync_password, dedupe_duplicate_rows_impl, ImportSummary, SETTING_SYNC_BLOCKED,
};
#[cfg(not(target_os = "android"))]
use crate::backup::commands::{export_sync_backup_impl, import_sync_backup_impl, SYNC_FILENAME};
#[cfg(target_os = "android")]
use crate::backup::commands::{android_export_sync_backup, android_import_sync_backup, android_peek_exported_at};
use crate::db::DbPool;
use crate::error::{AppError, Result};

const SETTING_ENABLED: &str = "auto_sync_enabled";
const SETTING_FOLDER: &str = "sync_folder_path";
const SETTING_INTERVAL_MIN: &str = "auto_sync_interval_minutes";
const SETTING_PASSWORD_ENC: &str = "sync_password_encrypted";
const SETTING_LAST_EXPORTED_AT: &str = "last_sync_exported_at";
const DEFAULT_INTERVAL_MIN: u64 = 30;
const MIN_INTERVAL_MIN: u64 = 5;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub ran: bool,
    pub imported: bool,
    pub exported_at: Option<String>,
    /// True when this tick's import was skipped because the remote file is a
    /// newer backup format than this app build reads (see `SETTING_SYNC_BLOCKED`) —
    /// the export still ran (this device's own writes are always safe to push,
    /// see `FORMAT_VERSION`'s doc comment), just nothing was pulled in.
    pub blocked_version_mismatch: bool,
}

/// Holds the running background task so it can be cancelled on lock/quit.
/// Uses Tauri's own runtime handle, not raw `tokio::spawn` — see the same
/// note in `notifications::scheduler::SchedulerState`.
#[derive(Default)]
pub struct SyncSchedulerState(Mutex<Option<tauri::async_runtime::JoinHandle<()>>>);

impl SyncSchedulerState {
    /// Starts the periodic sync loop, replacing any previously running one —
    /// safe to call again on re-unlock without an intervening lock.
    pub fn start(&self, app: AppHandle, db: Arc<DbPool>) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(old) = guard.take() {
                old.abort();
            }
            *guard = Some(spawn_sync_loop(app, db));
        }
    }

    /// Cancels the running loop, if any. Called on lock() and on app quit.
    pub fn stop(&self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

const SYNC_IMPORTED_EVENT: &str = "auto-sync-imported";

fn spawn_sync_loop(app: AppHandle, db: Arc<DbPool>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            match run_tick(&app, &db) {
                // Only notify the UI when this tick actually pulled newer data
                // from another device — a routine push-only tick (no diff)
                // shouldn't interrupt the user.
                Ok(outcome) if outcome.imported => {
                    let _ = app.emit(SYNC_IMPORTED_EVENT, &outcome);
                }
                Ok(_) => {}
                Err(e) => {
                    // A locked DB (AuthRequired) shows up here too — expected when
                    // the user locks mid-interval; just skip this tick and keep looping.
                    tracing::warn!("auto-sync tick skipped: {e}");
                }
            }

            let interval_min = read_interval_minutes(&db).unwrap_or(DEFAULT_INTERVAL_MIN);
            tokio::time::sleep(Duration::from_secs(jittered_interval_secs(interval_min))).await;
        }
    })
}

/// Adds up to 20% random jitter to the configured interval, so two devices on
/// the same interval don't keep landing their ticks in the same narrow window
/// tick after tick (which would repeatedly hit the same remote-file-in-use
/// races instead of spreading them out).
fn jittered_interval_secs(interval_min: u64) -> u64 {
    let base_secs = interval_min.max(MIN_INTERVAL_MIN) * 60;
    let jitter_secs = (base_secs as f64 * rand::random::<f64>() * 0.2) as u64;
    base_secs + jitter_secs
}

fn read_interval_minutes(db: &DbPool) -> Result<u64> {
    let conn = db.get()?;
    let raw = get_setting(&conn, SETTING_INTERVAL_MIN)?;
    Ok(raw.and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_INTERVAL_MIN))
}

fn get_setting(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM app_settings WHERE key=?1", [key], |r| r.get(0))
        .ok())
}

fn set_setting(db: &DbPool, key: &str, value: &str) -> Result<()> {
    let conn = db.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Classifies an import outcome into `(imported_rows, blocked_version_mismatch)`.
/// An `UnsupportedBackupVersion` error (this app build is older than whatever
/// wrote the remote file, see `FORMAT_VERSION`'s doc comment in
/// `backup::commands`) sets a persistent flag instead of just logging and
/// retrying forever next tick — that failure mode won't fix itself without an
/// app update. Any other import error (e.g. a transiently torn remote file) is
/// logged and swallowed here rather than propagated, so it never blocks this
/// device's own export later in the same tick (see the comment at the call site).
fn classify_import(result: Result<ImportSummary>, db: &DbPool) -> Result<(bool, bool)> {
    match result {
        Ok(summary) => {
            set_setting(db, SETTING_SYNC_BLOCKED, "false")?;
            Ok((summary.rows_imported > 0 || summary.tables_imported > 0, false))
        }
        Err(AppError::UnsupportedBackupVersion(remote_v, our_v)) => {
            tracing::warn!(
                "auto-sync import blocked: remote backup is format v{remote_v}, \
                 this app build only reads up to v{our_v} — update the app to resume syncing"
            );
            set_setting(db, SETTING_SYNC_BLOCKED, "true")?;
            Ok((false, true))
        }
        Err(e) => {
            tracing::warn!("auto-sync import failed, will retry next tick: {e}");
            Ok((false, false))
        }
    }
}

/// One pull-then-push sync cycle. Shared by the background loop and the
/// manual "Sync now" command — same logic either way, only the caller and
/// cadence differ.
///
/// Import always runs when a remote file exists: `import_sync_backup_impl`
/// merges per-row by `sync_id` (see `backup::commands`) rather than replacing
/// the whole DB, so re-importing an unchanged or stale remote snapshot is a
/// safe no-op — there's no longer a reason to gate it on a file-level
/// "is the remote newer" timestamp check the way wholesale-replace needed.
pub(crate) fn run_tick(app: &AppHandle, db: &DbPool) -> Result<SyncOutcome> {
    let (enabled, folder, password_enc) = {
        let conn = db.get()?;
        (
            get_setting(&conn, SETTING_ENABLED)?.unwrap_or_default(),
            get_setting(&conn, SETTING_FOLDER)?.unwrap_or_default(),
            get_setting(&conn, SETTING_PASSWORD_ENC)?.unwrap_or_default(),
        )
    };

    if enabled != "true" || folder.is_empty() || password_enc.is_empty() {
        return Ok(SyncOutcome {
            ran: false,
            imported: false,
            exported_at: None,
            blocked_version_mismatch: false,
        });
    }

    let master_password = db.current_password()?;
    let sync_password = decrypt_sync_password(&password_enc, &master_password)?;

    // On Android, `folder` is a persisted SAF tree URI (JSON-encoded FsUri),
    // not a real filesystem path — std::path::Path can't join/exists() it.
    #[cfg(target_os = "android")]
    {
        let mut imported = false;
        let mut blocked = false;
        let remote_peek = android_peek_exported_at(app, &folder, &sync_password)?;
        tracing::debug!(remote_exported_at = ?remote_peek, "android tick: peeked remote file");
        if remote_peek.is_some() {
            let result = android_import_sync_backup(app, &folder, &sync_password, db);
            (imported, blocked) = classify_import(result, db)?;
        }

        // A merge matches incoming rows by `sync_id` only, so two devices that
        // minted different `sync_id`s for the same pre-existing content (fresh
        // install, or a device that predates the sync_id backfill) still end
        // up with duplicate rows after this import. Cleaning up right here,
        // every tick, catches that the moment it happens instead of relying
        // solely on the one-time post-unlock cleanup, which can no-op on a
        // device's very first unlock (nothing to dedupe yet) before this tick
        // ever runs — permanently skipping the cleanup it was meant to catch.
        if imported {
            dedupe_duplicate_rows_impl(db)?;
        }

        // Always runs, even when the import above was blocked or failed: this
        // device's own export is independent of whatever's wrong with the
        // remote file, and (per `FORMAT_VERSION`'s doc comment) always writes
        // in a format any peer — old or new — can read safely.
        let summary = android_export_sync_backup(app, &folder, &sync_password, db)?;
        set_setting(db, SETTING_LAST_EXPORTED_AT, &summary.exported_at)?;
        tracing::debug!(imported, blocked, exported_at = %summary.exported_at, "android tick complete");

        return Ok(SyncOutcome {
            ran: true,
            imported,
            exported_at: Some(summary.exported_at),
            blocked_version_mismatch: blocked,
        });
    }

    #[cfg(not(target_os = "android"))]
    {
        let file_path = Path::new(&folder).join(SYNC_FILENAME).to_string_lossy().into_owned();

        let mut imported = false;
        let mut blocked = false;
        let remote_exists = Path::new(&file_path).exists();
        tracing::debug!(file_path = %file_path, remote_exists, "desktop tick: checked remote file");
        if remote_exists {
            let result = import_sync_backup_impl(app, &file_path, &sync_password, db);
            (imported, blocked) = classify_import(result, db)?;
        }

        // See the matching comment in the Android branch above: dedupe right
        // after every import that actually merged rows in, not just once on
        // unlock, so first-sync duplicates get caught the same tick they're
        // created.
        if imported {
            dedupe_duplicate_rows_impl(db)?;
        }

        // Always runs — see the matching comment in the Android branch above.
        let summary = export_sync_backup_impl(app, &file_path, &sync_password, db)?;
        set_setting(db, SETTING_LAST_EXPORTED_AT, &summary.exported_at)?;
        tracing::debug!(imported, blocked, exported_at = %summary.exported_at, "desktop tick complete");

        Ok(SyncOutcome {
            ran: true,
            imported,
            exported_at: Some(summary.exported_at),
            blocked_version_mismatch: blocked,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────
// `run_tick`/`spawn_sync_loop` need an `AppHandle` (file IO + event emit),
// which can't be constructed in tests — see tests/common/mod.rs. Covered
// here: settings plumbing, and `classify_import` (the part of `run_tick` that
// decides whether an import failure blocks this device's own export — see the
// INCIDENT_REPORT.md-flagged bug this guards against). The staleness/merge
// decision now lives per-row in `backup::commands::apply_backup_payload_merge`,
// tested there.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    // ── classify_import: import failures must never block this device's own export ──

    #[test]
    fn classify_import_ok_with_rows_reports_imported_and_clears_block_flag() {
        let (_dir, db) = test_db_pool();
        set_setting(&db, SETTING_SYNC_BLOCKED, "true").unwrap(); // simulate a previously-blocked state
        let result = Ok(ImportSummary { rows_imported: 3, tables_imported: 1 });

        let (imported, blocked) = classify_import(result, &db).unwrap();

        assert!(imported);
        assert!(!blocked);
        let conn = db.get().unwrap();
        assert_eq!(get_setting(&conn, SETTING_SYNC_BLOCKED).unwrap(), Some("false".to_string()));
    }

    #[test]
    fn classify_import_ok_with_no_rows_reports_not_imported() {
        let (_dir, db) = test_db_pool();
        let result = Ok(ImportSummary { rows_imported: 0, tables_imported: 0 });
        let (imported, blocked) = classify_import(result, &db).unwrap();
        assert!(!imported);
        assert!(!blocked);
    }

    /// The exact bug class from INCIDENT_REPORT.md's fix: a torn/corrupt remote
    /// file must not stop this device's own export from running later in the
    /// same tick — `classify_import` must swallow the error (log it, not
    /// propagate), not return `Err`.
    #[test]
    fn classify_import_generic_error_is_swallowed_not_propagated() {
        let (_dir, db) = test_db_pool();
        let result: Result<ImportSummary> = Err(AppError::Validation("corrupt file".into()));
        let (imported, blocked) = classify_import(result, &db).unwrap();
        assert!(!imported);
        assert!(!blocked);
    }

    #[test]
    fn classify_import_version_mismatch_sets_block_flag_without_erroring() {
        let (_dir, db) = test_db_pool();
        let result: Result<ImportSummary> = Err(AppError::UnsupportedBackupVersion(4, 3));

        let (imported, blocked) = classify_import(result, &db).unwrap();

        assert!(!imported);
        assert!(blocked);
        let conn = db.get().unwrap();
        assert_eq!(get_setting(&conn, SETTING_SYNC_BLOCKED).unwrap(), Some("true".to_string()));
    }

    // ── Settings plumbing ──

    #[test]
    fn interval_defaults_when_unset_or_garbage() {
        let (_dir, db) = test_db_pool();
        assert_eq!(read_interval_minutes(&db).unwrap(), DEFAULT_INTERVAL_MIN);

        set_setting(&db, SETTING_INTERVAL_MIN, "not-a-number").unwrap();
        assert_eq!(read_interval_minutes(&db).unwrap(), DEFAULT_INTERVAL_MIN);
    }

    #[test]
    fn interval_reads_configured_value() {
        let (_dir, db) = test_db_pool();
        set_setting(&db, SETTING_INTERVAL_MIN, "45").unwrap();
        assert_eq!(read_interval_minutes(&db).unwrap(), 45);
    }

    #[test]
    fn set_setting_upserts() {
        let (_dir, db) = test_db_pool();
        set_setting(&db, SETTING_LAST_EXPORTED_AT, "first").unwrap();
        set_setting(&db, SETTING_LAST_EXPORTED_AT, "second").unwrap();
        let conn = db.get().unwrap();
        assert_eq!(
            get_setting(&conn, SETTING_LAST_EXPORTED_AT).unwrap(),
            Some("second".to_string())
        );
    }

    #[test]
    fn get_setting_returns_none_for_missing_key() {
        let (_dir, db) = test_db_pool();
        let conn = db.get().unwrap();
        assert_eq!(get_setting(&conn, "no-such-key").unwrap(), None);
    }

    #[test]
    fn jittered_interval_never_goes_below_base_and_stays_within_20_percent() {
        let base_secs = 30 * 60;
        for _ in 0..50 {
            let jittered = jittered_interval_secs(30);
            assert!(jittered >= base_secs, "jitter must never shorten the interval");
            assert!(jittered <= base_secs + base_secs / 5, "jitter must stay within +20%");
        }
    }
}
