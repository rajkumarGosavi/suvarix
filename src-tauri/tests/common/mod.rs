//! Shared helper for building a real, migrated, temp-file SQLCipher DbState
//! used by command-level integration tests. Tests call the same SQL the
//! `#[tauri::command]` wrappers run, directly against `DbState`, instead of
//! going through `tauri::State` (constructing that requires a live/mock Tauri
//! `App`, and `tauri::test`'s MockRuntime is not usable in this environment —
//! see git history / conversation notes for the STATUS_ENTRYPOINT_NOT_FOUND
//! repro).
use suvarix_lib::db::{DbPool, DbState};

pub const TEST_PASSWORD: &str = "test-password-123";

/// Keep the returned `TempDir` alive for the test's duration — dropping it
/// deletes the underlying db file.
pub fn setup_db_state() -> (tempfile::TempDir, DbState) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("test.db");
    let pool = DbPool::new(db_path.to_string_lossy().into_owned());
    pool.initialize(TEST_PASSWORD).expect("initialize test db");
    (dir, DbState(pool))
}
