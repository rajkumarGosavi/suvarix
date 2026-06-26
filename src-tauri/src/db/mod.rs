use rusqlite::Connection;
use std::sync::Mutex;

use crate::error::{AppError, Result};

pub mod migrations;

pub struct DbState(pub Mutex<Connection>);

impl DbState {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .map_err(|e| AppError::Database(e.to_string()))?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;",
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        migrations::run_migrations(&conn)?;

        Ok(DbState(Mutex::new(conn)))
    }
}
