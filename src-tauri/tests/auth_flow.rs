use suvarix_lib::db::DbPool;
use suvarix_lib::error::AppError;

fn temp_db_path() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("auth-test.db").to_string_lossy().into_owned();
    (dir, path)
}

#[test]
fn full_setup_unlock_rekey_lock_lifecycle() {
    let (_dir, db_path) = temp_db_path();

    let pool = DbPool::new(db_path.clone());
    assert!(!pool.is_password_set(), "fresh path should have no password set");

    pool.initialize("correct-horse-battery").expect("initialize should succeed");
    assert!(pool.is_password_set());

    // A fresh DbPool bound to the same path simulates app restart before unlock.
    let pool = DbPool::new(db_path.clone());
    assert!(!pool.unlock("wrong-password").unwrap(), "wrong password must not unlock");
    assert!(pool.unlock("correct-horse-battery").unwrap(), "correct password must unlock");
    assert!(pool.verify_password("correct-horse-battery").unwrap());
    assert!(!pool.verify_password("wrong-password").unwrap());

    pool.rekey("new-passphrase").expect("rekey should succeed");

    // Verify the on-disk key actually changed by reopening from scratch.
    let pool = DbPool::new(db_path.clone());
    assert!(!pool.unlock("correct-horse-battery").unwrap(), "old password must fail after rekey");
    assert!(pool.unlock("new-passphrase").unwrap(), "new password must unlock after rekey");

    pool.lock();
    let err = pool.get().expect_err("locked pool must reject checkout");
    assert!(matches!(err, AppError::AuthRequired));
}
