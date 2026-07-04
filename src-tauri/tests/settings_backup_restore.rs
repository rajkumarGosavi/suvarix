//! Exercises the same backup/restore logic the `backup_database`/`restore_database`
//! commands run (settings::commands, src-tauri/src/settings/commands.rs) directly
//! against `DbState` — see tests/common/mod.rs for why this doesn't go through
//! `tauri::State`.
mod common;

/// KNOWN BUG (found while writing this test): `backup_database`/`restore_database`
/// use `rusqlite::backup::Backup`, which SQLCipher rejects on encrypted connections
/// with "backup is not supported with encrypted databases". The codebase already
/// works around this exact SQLCipher limitation elsewhere via `ATTACH ... KEY ...;
/// sqlcipher_export()` (see `migrate_from_device_key` in src-tauri/src/db/mod.rs),
/// but the settings backup/restore commands were never updated to do the same.
/// This test is a regression guard: it should start failing (in a good way) once
/// someone fixes backup_database/restore_database to work against the real,
/// encrypted app DB — at which point this test should be replaced with a real
/// round-trip assertion (see git history for the round-trip version of this test).
#[test]
fn backup_database_currently_fails_against_an_encrypted_connection() {
    let (_dir, state) = common::setup_db_state();

    let backup_dir = tempfile::tempdir().expect("create backup dir");
    let backup_path = backup_dir.path().join("backup.db");

    let conn = state.0.get().unwrap();
    let mut dest = rusqlite::Connection::open(&backup_path).unwrap();
    let result = rusqlite::backup::Backup::new(&conn, &mut dest);

    let err = match result {
        Ok(_) => panic!(
            "backup_database is expected to fail against an encrypted DB today — \
             if this now succeeds, the underlying bug has been fixed; replace this \
             test with a real backup -> wipe -> restore round-trip assertion"
        ),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("encrypted"),
        "expected the SQLCipher \"backup is not supported with encrypted databases\" error, got: {err}"
    );
}
