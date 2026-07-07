// Background auto-sync scheduler — periodically pushes/pulls an encrypted
// `.svbak` snapshot (see `backup::commands`) to/from a folder the user syncs
// themselves (Dropbox/Drive/OneDrive/etc.). No app-hosted backend involved:
// the cloud provider's own client propagates the file between devices, this
// loop just decides when to read/write it. Started on unlock, stopped on
// lock/quit — same lifecycle as `notifications::scheduler::SchedulerState`.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::backup::commands::{
    decrypt_sync_password, export_sync_backup_impl, import_sync_backup_impl, peek_exported_at,
};
use crate::db::DbPool;
use crate::error::Result;

const SYNC_FILENAME: &str = "suvarix-sync.svbak";
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
            let interval_min = read_interval_minutes(&db).unwrap_or(DEFAULT_INTERVAL_MIN);
            tokio::time::sleep(Duration::from_secs(interval_min.max(MIN_INTERVAL_MIN) * 60)).await;
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
        }
    })
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

/// One pull-then-push sync cycle. Shared by the background loop and the
/// manual "Sync now" command — same logic either way, only the caller and
/// cadence differ.
///
/// Import only runs if the remote file's embedded `exported_at` is newer
/// than what this device last applied/produced — otherwise an unattended
/// tick could clobber a fresher local copy that hasn't finished propagating
/// to this device with an older remote one still sitting in the folder.
pub(crate) fn run_tick(app: &AppHandle, db: &DbPool) -> Result<SyncOutcome> {
    let (enabled, folder, password_enc, last_applied) = {
        let conn = db.get()?;
        (
            get_setting(&conn, SETTING_ENABLED)?.unwrap_or_default(),
            get_setting(&conn, SETTING_FOLDER)?.unwrap_or_default(),
            get_setting(&conn, SETTING_PASSWORD_ENC)?.unwrap_or_default(),
            get_setting(&conn, SETTING_LAST_EXPORTED_AT)?.unwrap_or_default(),
        )
    };

    if enabled != "true" || folder.is_empty() || password_enc.is_empty() {
        return Ok(SyncOutcome { ran: false, imported: false, exported_at: None });
    }

    let master_password = db.current_password()?;
    let sync_password = decrypt_sync_password(&password_enc, &master_password)?;
    let file_path = Path::new(&folder).join(SYNC_FILENAME).to_string_lossy().into_owned();

    let mut imported = false;
    if Path::new(&file_path).exists() {
        if let Ok(remote_exported_at) = peek_exported_at(app, &file_path, &sync_password) {
            if is_remote_newer(&remote_exported_at, &last_applied) {
                import_sync_backup_impl(app, &file_path, &sync_password, db)?;
                set_setting(db, SETTING_LAST_EXPORTED_AT, &remote_exported_at)?;
                imported = true;
            }
        }
    }

    let summary = export_sync_backup_impl(app, &file_path, &sync_password, db)?;
    set_setting(db, SETTING_LAST_EXPORTED_AT, &summary.exported_at)?;

    Ok(SyncOutcome { ran: true, imported, exported_at: Some(summary.exported_at) })
}

/// Last-write-wins staleness check: import only if the remote snapshot's
/// `exported_at` (RFC 3339) is strictly newer than what this device last
/// applied/produced. Compared as parsed instants, not strings — devices in
/// different UTC offsets (old snapshots used local time) would misorder
/// lexicographically. Falls back to string compare if either side doesn't
/// parse. An empty `last_applied` means this device has never synced —
/// always import.
fn is_remote_newer(remote_exported_at: &str, last_applied: &str) -> bool {
    if last_applied.is_empty() {
        return true;
    }
    match (
        chrono::DateTime::parse_from_rfc3339(remote_exported_at),
        chrono::DateTime::parse_from_rfc3339(last_applied),
    ) {
        (Ok(remote), Ok(applied)) => remote > applied,
        _ => remote_exported_at > last_applied,
    }
}

// ── Tests ─────────────────────────────────────────────────────
// `run_tick`/`spawn_sync_loop` need an `AppHandle` (file IO + event emit),
// which can't be constructed in tests — see tests/common/mod.rs. Covered
// here: settings plumbing and the staleness decision the tick hinges on.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    // ── Staleness check ──

    #[test]
    fn never_synced_device_always_imports() {
        assert!(is_remote_newer("2026-07-07T10:00:00+05:30", ""));
    }

    #[test]
    fn newer_remote_imports_older_or_equal_does_not() {
        let applied = "2026-07-07T10:00:00+05:30";
        assert!(is_remote_newer("2026-07-07T10:00:01+05:30", applied));
        assert!(!is_remote_newer(applied, applied), "same snapshot must not re-import");
        assert!(
            !is_remote_newer("2026-07-07T09:59:59+05:30", applied),
            "older remote must not clobber newer local"
        );
    }

    #[test]
    fn cross_offset_timestamps_compare_as_instants() {
        // 06:00 UTC == 11:30 IST — newer than 10:00 IST, but lexicographically
        // smaller ("+00:00" file vs "+05:30"). Must still import.
        assert!(is_remote_newer("2026-07-07T06:00:00+00:00", "2026-07-07T10:00:00+05:30"));
        // Reverse: 10:00 IST (04:30 UTC) is older than 06:00 UTC despite
        // comparing lexicographically greater. Must not import.
        assert!(!is_remote_newer("2026-07-07T10:00:00+05:30", "2026-07-07T06:00:00+00:00"));
        // Same instant, different offsets — must not re-import.
        assert!(!is_remote_newer("2026-07-07T11:30:00+05:30", "2026-07-07T06:00:00+00:00"));
    }

    #[test]
    fn unparseable_timestamp_falls_back_to_string_compare() {
        assert!(is_remote_newer("not-a-date-b", "not-a-date-a"));
        assert!(!is_remote_newer("not-a-date-a", "not-a-date-b"));
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
}
