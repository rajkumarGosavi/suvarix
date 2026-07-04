//! Shared helpers for building an isolated, migrated SQLCipher DB per test.
#![cfg(test)]

use crate::db::{DbPool, DbState};

pub const TEST_PASSWORD: &str = "test-password-123";

/// Creates a fresh temp-file SQLCipher DB, initialized and migrated.
/// Keep the returned `TempDir` alive for the test's duration — dropping it
/// deletes the underlying db file.
pub fn test_db_pool() -> (tempfile::TempDir, DbPool) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let db_path = dir.path().join("test.db");
    let pool = DbPool::new(db_path.to_string_lossy().into_owned());
    pool.initialize(TEST_PASSWORD).expect("initialize test db");
    (dir, pool)
}

pub fn test_db_state() -> (tempfile::TempDir, DbState) {
    let (dir, pool) = test_db_pool();
    (dir, DbState(pool))
}

#[cfg(test)]
mod smoke {
    use super::*;

    #[test]
    fn test_db_pool_is_migrated_and_usable() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().expect("checkout pooled connection");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN \
                 ('accounts','equity_holdings','mf_holdings','transactions','loans')",
                [],
                |r| r.get(0),
            )
            .expect("query sqlite_master");
        assert_eq!(count, 5, "expected core tables to exist after migrations");
    }
}
