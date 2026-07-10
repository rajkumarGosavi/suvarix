use tauri::{AppHandle, Manager, State};
use rusqlite::params;
use crate::db::DbState;
use crate::error::{AppError, Result};

/// Financial data tables `wipe_all_data` clears with a plain `DELETE FROM`.
/// `categories` and `milestones` are deliberately not here — both carry
/// default rows seeded once at migration time that a wipe must put back
/// (see `wipe_all_data`'s reseed step below), unlike everything in this list.
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
    "recurring_transactions",
    "bills",
    "goals",
    "import_log",
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
    restore_database_impl(&src_path, &state.0)
}

/// Shared by the `restore_database` command and its tests — `State<DbState>`
/// can't be constructed outside a running Tauri app (see `tests/common/mod.rs`),
/// so the real logic takes `&DbPool` directly, same split as `backup::commands`'
/// `*_impl` functions.
pub(crate) fn restore_database_impl(src_path: &str, db: &crate::db::DbPool) -> Result<()> {
    let password = db.current_password()?;
    let db_path = db.db_path().to_string();
    let temp_path = std::path::Path::new(&db_path).with_extension("db.restoretmp");
    let temp_path_str = temp_path.to_string_lossy().into_owned();

    if temp_path.exists() {
        std::fs::remove_file(&temp_path)?;
    }

    let export_result = (|| -> Result<()> {
        let src = rusqlite::Connection::open(src_path)?;
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
    db.lock();

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
    db.unlock(&password)?;

    // The just-restored file carries whatever device_id was live wherever it
    // was backed up from — never reuse it (see `db::regenerate_device_id`'s
    // doc comment for why this doesn't bother comparing old vs. restored first).
    let regen_result = (|| -> Result<()> {
        let conn = db.get()?;
        crate::db::regenerate_device_id(&conn)
    })();

    swap_result?;
    regen_result?;
    Ok(())
}

pub(crate) fn wipe_all_data_impl(db: &crate::db::DbPool) -> Result<()> {
    let conn = db.get()?;
    for table in DATA_TABLES {
        conn.execute(&format!("DELETE FROM {}", table), [])
            .map_err(|e| AppError::Database(e.to_string()))?;
    }

    // categories/milestones hold default rows seeded once at migration time
    // (INSERT OR IGNORE, never re-run outside a fresh migration) plus
    // user-added/custom rows and, for milestones, achieved-progress state —
    // a wipe must clear all of that *and* put the defaults back, or the
    // category dropdown and milestone ladder are just gone afterward.
    conn.execute("DELETE FROM categories", [])
        .map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(crate::db::migrations::DEFAULT_CATEGORIES_SEED)
        .map_err(|e| AppError::Database(e.to_string()))?;

    conn.execute("DELETE FROM milestones", [])
        .map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(crate::db::migrations::DEFAULT_MILESTONES_SEED)
        .map_err(|e| AppError::Database(e.to_string()))?;

    // Every DELETE above fired that table's `trg_<table>_tombstone` trigger,
    // so at this point `sync_tombstones` holds a "deleted right now" entry
    // for every row this device ever had — which would silently block the
    // very next import or sync from restoring any of it (the merge treats a
    // local tombstone newer than an incoming row's timestamp as "don't
    // resurrect," and a wipe's tombstones are always newer). A wipe means
    // starting over, including this device's memory of what it deleted.
    conn.execute("DELETE FROM sync_tombstones", [])
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(())
}

#[tauri::command]
pub fn wipe_all_data(state: State<DbState>) -> Result<()> {
    wipe_all_data_impl(&state.0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    fn row_count(conn: &rusqlite::Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    }

    /// Seeds one row in every table `wipe_all_data` is responsible for
    /// clearing, including the ones that were missing from `DATA_TABLES`
    /// before this fix (recurring_transactions, bills, goals, import_log)
    /// and the two that need a reseed rather than a plain delete
    /// (categories, milestones).
    fn seed_one_row_per_wipeable_table(conn: &rusqlite::Connection) {
        conn.execute("INSERT INTO accounts (name, type) VALUES ('Bank', 'bank')", []).unwrap();
        conn.execute(
            "INSERT INTO transactions (type, amount, category, date) VALUES ('expense', 100, 'Food', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO recurring_transactions (name, type, amount, category, next_due_date)
             VALUES ('Netflix', 'expense', 500, 'Entertainment', '2026-02-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bills (name, category, amount, next_due_date) VALUES ('Electricity', 'utilities', 2000, '2026-02-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO goals (name, target_amount, target_date) VALUES ('Retirement', 10000000, '2050-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO import_log (source, status) VALUES ('csv', 'success')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories (name) VALUES ('My Custom Category')", []).unwrap();
        conn.execute(
            "UPDATE milestones SET achieved_at = datetime('now') WHERE amount = 100000",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO milestones (amount, label, is_custom) VALUES (999999999, 'My Custom Goal', 1)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn wipe_clears_every_previously_missing_table() {
        let (_dir, db) = test_db_pool();
        {
            let conn = db.get().unwrap();
            seed_one_row_per_wipeable_table(&conn);
        }

        wipe_all_data_impl(&db).unwrap();

        let conn = db.get().unwrap();
        for table in [
            "accounts",
            "transactions",
            "recurring_transactions",
            "bills",
            "goals",
            "import_log",
        ] {
            assert_eq!(row_count(&conn, table), 0, "{table} must be empty after wipe");
        }
    }

    #[test]
    fn wipe_reseeds_default_categories_but_drops_custom_ones() {
        let (_dir, db) = test_db_pool();
        {
            let conn = db.get().unwrap();
            seed_one_row_per_wipeable_table(&conn);
        }

        wipe_all_data_impl(&db).unwrap();

        let conn = db.get().unwrap();
        let names: Vec<String> = conn
            .prepare("SELECT name FROM categories ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(names.len(), 13, "the 13 default categories must be restored");
        assert!(!names.contains(&"My Custom Category".to_string()), "custom categories must not survive a wipe");
        assert!(names.contains(&"Food".to_string()));
    }

    #[test]
    fn wipe_reseeds_default_milestones_resets_progress_drops_custom_ones() {
        let (_dir, db) = test_db_pool();
        {
            let conn = db.get().unwrap();
            seed_one_row_per_wipeable_table(&conn);
        }

        wipe_all_data_impl(&db).unwrap();

        let conn = db.get().unwrap();
        assert_eq!(row_count(&conn, "milestones"), 9, "the 9 default milestones must be restored, custom one dropped");
        let achieved: i64 =
            conn.query_row("SELECT count(*) FROM milestones WHERE achieved_at IS NOT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(achieved, 0, "achieved-at progress must be reset, not carried over");
    }

    /// `sync_hlc_state` (the HLC counter) must never go *backward* on a wipe —
    /// resetting it to zero/small would let reseeded milestone/category rows
    /// get an artificially low `sync_hlc` that could lose every future merge
    /// comparison against a remote peer's already-higher counter. Note wipe's
    /// own `DELETE`s legitimately bump the counter *forward* (each fires
    /// `trg_<table>_tombstone`, which stamps a real HLC) — the invariant under
    /// test is "never reset to below where it was," not "frozen in place."
    /// See the "Wipe All Data interaction" note in the sync HLC upgrade plan.
    #[test]
    fn wipe_does_not_reset_sync_hlc_state() {
        let (_dir, db) = test_db_pool();
        let ahead_ms = chrono::Utc::now().timestamp_millis() + 3 * 60 * 60 * 1000;
        {
            let conn = db.get().unwrap();
            conn.execute(
                "UPDATE sync_hlc_state SET last_physical_ms = ?1, last_logical = 7 WHERE id = 1",
                [ahead_ms],
            )
            .unwrap();
        }

        wipe_all_data_impl(&db).unwrap();

        let conn = db.get().unwrap();
        let state: (i64, i64) = conn
            .query_row("SELECT last_physical_ms, last_logical FROM sync_hlc_state WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert!(state.0 >= ahead_ms, "wipe must never reset the HLC counter backward");

        // Reseeded milestones must still pick up a real (non-null) sync_hlc via
        // the insert trigger, using that same un-reset counter.
        let milestone_hlc: Option<String> =
            conn.query_row("SELECT sync_hlc FROM milestones LIMIT 1", [], |r| r.get(0)).unwrap();
        assert!(milestone_hlc.is_some(), "reseeded milestones must still get a real sync_hlc stamped");
    }

    /// `device_id` lives in `app_settings`, which wipe never touches — a wiped
    /// device must keep its own identity (it's the same physical device).
    #[test]
    fn wipe_does_not_touch_device_id() {
        let (_dir, db) = test_db_pool();
        let before: String =
            db.get().unwrap().query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap();

        wipe_all_data_impl(&db).unwrap();

        let after: String =
            db.get().unwrap().query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap();
        assert_eq!(before, after, "wipe must not change device_id");
    }

    #[test]
    fn wipe_is_idempotent() {
        let (_dir, db) = test_db_pool();
        wipe_all_data_impl(&db).unwrap();
        // Second call on an already-wiped (but freshly-migrated) DB must not
        // error — e.g. re-seeding categories/milestones must tolerate rows
        // that are already there from the first call.
        wipe_all_data_impl(&db).unwrap();

        let conn = db.get().unwrap();
        assert_eq!(row_count(&conn, "categories"), 13);
        assert_eq!(row_count(&conn, "milestones"), 9);
    }

    // ── restore_database_impl ──

    /// Forces `db`'s live `device_id` to a known value, then exports it into a
    /// fresh sibling `.svbak`-unrelated backup file (same `ATTACH ... KEY ...;
    /// sqlcipher_export(...)` pattern `backup_database` itself uses). Returns
    /// the backup file's `TempDir` (keep it alive) and path.
    fn make_backup_with_device_id(db: &crate::db::DbPool, device_id: &str) -> (tempfile::TempDir, String) {
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('device_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [device_id],
        )
        .unwrap();
        let password = db.current_password().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let backup_path = dir.path().join("backup.db");
        let backup_path_str = backup_path.to_string_lossy().into_owned();
        conn.execute("ATTACH DATABASE ?1 AS backup_db KEY ?2", params![backup_path_str, password]).unwrap();
        conn.query_row("SELECT sqlcipher_export('backup_db')", [], |_| Ok(())).unwrap();
        conn.execute("DETACH DATABASE backup_db", []).unwrap();
        (dir, backup_path_str)
    }

    fn device_id_of(db: &crate::db::DbPool) -> String {
        db.get().unwrap().query_row("SELECT value FROM app_settings WHERE key='device_id'", [], |r| r.get(0)).unwrap()
    }

    /// The core fix: restoring must never leave this device with whatever
    /// `device_id` the backup file happened to carry, regardless of whether
    /// that id differs from what this device currently has.
    #[test]
    fn restore_never_reuses_the_backups_device_id() {
        let (_dir, db) = test_db_pool();
        let (_backup_dir, backup_path) = make_backup_with_device_id(&db, "backup-source-device-id");

        restore_database_impl(&backup_path, &db).unwrap();

        assert_ne!(
            device_id_of(&db),
            "backup-source-device-id",
            "restore must never keep the backup file's own device_id"
        );
    }

    /// Same fix, but covering the case where the backup's device_id happens to
    /// already match what this device currently has — still must regenerate,
    /// not treat a match as proof of safety (see `db::regenerate_device_id`'s
    /// doc comment for why comparing first wouldn't actually help here).
    #[test]
    fn restore_regenerates_device_id_even_when_backup_matches_current() {
        let (_dir, db) = test_db_pool();
        let current = device_id_of(&db);
        let (_backup_dir, backup_path) = make_backup_with_device_id(&db, &current);

        restore_database_impl(&backup_path, &db).unwrap();

        assert_ne!(
            device_id_of(&db),
            current,
            "restore must regenerate even when the backup's device_id already matched current"
        );
    }

    /// End-to-end through the real command-backing function (not a hand-rolled
    /// copy of the ATTACH/export/swap logic, unlike the separate integration
    /// test in tests/settings_backup_restore.rs) — data actually comes back,
    /// and device_id is regenerated in the same operation.
    #[test]
    fn restore_recovers_data_and_regenerates_device_id_together() {
        let (_dir, db) = test_db_pool();
        db.get().unwrap().execute("INSERT INTO accounts (name, type) VALUES ('Test Savings', 'bank')", []).unwrap();
        let (_backup_dir, backup_path) = make_backup_with_device_id(&db, "irrelevant-device-id");
        let device_id_before_restore = device_id_of(&db);

        // Simulate data loss before restoring.
        db.get().unwrap().execute("DELETE FROM accounts", []).unwrap();
        assert_eq!(row_count(&db.get().unwrap(), "accounts"), 0, "precondition: data gone before restore");

        restore_database_impl(&backup_path, &db).unwrap();

        let conn = db.get().unwrap();
        let name: String = conn.query_row("SELECT name FROM accounts LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "Test Savings", "restore must bring the data back");
        assert_ne!(device_id_of(&db), device_id_before_restore, "restore must still regenerate device_id");
    }
}
