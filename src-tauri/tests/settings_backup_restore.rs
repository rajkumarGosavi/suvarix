//! Exercises the same backup/restore logic the `backup_database`/`restore_database`
//! commands run (settings::commands, src-tauri/src/settings/commands.rs) directly
//! against `DbState` — see tests/common/mod.rs for why this doesn't go through
//! `tauri::State`.
//!
//! `backup_database`/`restore_database` used to call `rusqlite::backup::Backup`,
//! which SQLCipher rejects on encrypted connections ("backup is not supported
//! with encrypted databases"). They now go through the same
//! `ATTACH ... KEY ...; SELECT sqlcipher_export(...)` workaround used by
//! `migrate_from_device_key` in src-tauri/src/db/mod.rs (restore additionally
//! swaps the exported file in for the live DB, since `sqlcipher_export` can't
//! merge into a `main` connection that already has the app's own schema).
//! This test is a real round-trip: seed data, back up, wipe, restore, assert
//! the data reappears.
mod common;

use rusqlite::params;

#[test]
fn backup_then_wipe_then_restore_recovers_data() {
    let (_dir, state) = common::setup_db_state();
    let password = state.0.current_password().unwrap();
    let db_path = state.0.db_path().to_string();

    {
        let conn = state.0.get().unwrap();
        conn.execute(
            "INSERT INTO accounts (name, type) VALUES (?1, ?2)",
            params!["Test Savings", "bank"],
        )
        .unwrap();
    }

    // ── Backup: export the live db into a fresh sibling file ──
    let backup_dir = tempfile::tempdir().expect("create backup dir");
    let backup_path = backup_dir.path().join("backup.db");
    let backup_path_str = backup_path.to_string_lossy().into_owned();

    {
        let conn = state.0.get().unwrap();
        conn.execute(
            "ATTACH DATABASE ?1 AS backup_db KEY ?2",
            params![backup_path_str, password],
        )
        .unwrap();
        conn.query_row("SELECT sqlcipher_export('backup_db')", [], |_| Ok(()))
            .unwrap();
        conn.execute("DETACH DATABASE backup_db", []).unwrap();
    }
    assert!(backup_path.exists(), "backup file should have been created");

    // Wipe live data (mirrors wipe_all_data).
    {
        let conn = state.0.get().unwrap();
        conn.execute("DELETE FROM accounts", []).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "accounts should be empty after wipe");
    }

    // ── Restore: export the backup into a fresh temp file, then swap it in ──
    let temp_path = std::path::Path::new(&db_path).with_extension("db.restoretmp");
    {
        let src = rusqlite::Connection::open(&backup_path).unwrap();
        src.execute_batch(&format!("PRAGMA key = '{password}';")).unwrap();
        src.execute(
            "ATTACH DATABASE ?1 AS restored KEY ?2",
            params![temp_path.to_string_lossy().into_owned(), password],
        )
        .unwrap();
        src.query_row("SELECT sqlcipher_export('restored')", [], |_| Ok(()))
            .unwrap();
        src.execute("DETACH DATABASE restored", []).unwrap();
    }

    state.0.lock();
    std::fs::rename(&temp_path, &db_path).unwrap();
    for ext in ["-wal", "-shm"] {
        let sidecar = format!("{db_path}{ext}");
        let _ = std::fs::remove_file(&sidecar);
    }
    assert!(
        state.0.unlock(&password).unwrap(),
        "should re-unlock with the same password after restore"
    );

    let conn = state.0.get().unwrap();
    let name: String = conn
        .query_row("SELECT name FROM accounts LIMIT 1", [], |r| r.get(0))
        .expect("restored account should exist");
    assert_eq!(name, "Test Savings");
}
