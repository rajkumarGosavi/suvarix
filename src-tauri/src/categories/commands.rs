use rusqlite::Connection;
use tauri::State;
use crate::db::DbState;
use crate::error::{AppError, Result};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPayload {
    pub name: String,
}

fn list_categories_impl(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare("SELECT id, name, created_at FROM categories ORDER BY name ASC")?;
    let rows = stmt.query_map([], |r| Ok(Category {
        id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn add_category_impl(conn: &Connection, name: &str) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Category name cannot be empty".into()));
    }
    conn.execute("INSERT INTO categories (name) VALUES (?1)", rusqlite::params![name])
        .map_err(|e| map_unique_violation(e, name))?;
    Ok(conn.last_insert_rowid())
}

fn update_category_impl(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Category name cannot be empty".into()));
    }
    conn.execute("UPDATE categories SET name=?1 WHERE id=?2", rusqlite::params![name, id])
        .map_err(|e| map_unique_violation(e, name))?;
    Ok(())
}

fn delete_category_impl(conn: &Connection, id: i64) -> Result<()> {
    let name: Option<String> = conn
        .query_row("SELECT name FROM categories WHERE id=?1", [id], |r| r.get(0))
        .ok();
    let Some(name) = name else { return Ok(()) };

    let in_use: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM transactions WHERE category = ?1)
              + (SELECT COUNT(*) FROM budgets WHERE category = ?1)
              + (SELECT COUNT(*) FROM recurring_transactions WHERE category = ?1)",
        rusqlite::params![name],
        |r| r.get(0),
    )?;
    if in_use > 0 {
        return Err(AppError::Validation(format!(
            "Category \"{name}\" is used by {in_use} existing record(s) and can't be deleted. Remove or recategorize them first."
        )));
    }

    conn.execute("DELETE FROM categories WHERE id=?1", [id])?;
    Ok(())
}

/// Maps a SQLite UNIQUE constraint violation on `categories.name` to a friendly
/// validation error instead of surfacing the raw SQLite error to the UI.
fn map_unique_violation(e: rusqlite::Error, name: &str) -> AppError {
    if let rusqlite::Error::SqliteFailure(ref sqlite_err, _) = e {
        if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation {
            return AppError::Validation(format!("Category \"{name}\" already exists"));
        }
    }
    AppError::from(e)
}

#[tauri::command]
pub fn list_categories(state: State<DbState>) -> Result<Vec<Category>> {
    let conn = state.0.get()?;
    list_categories_impl(&conn)
}

#[tauri::command]
pub fn add_category(payload: CategoryPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    add_category_impl(&conn, &payload.name)
}

#[tauri::command]
pub fn update_category(id: i64, payload: CategoryPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    update_category_impl(&conn, id, &payload.name)
}

#[tauri::command]
pub fn delete_category(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    delete_category_impl(&conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    #[test]
    fn add_category_success() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_category_impl(&conn, "Groceries").unwrap();
        let cats = list_categories_impl(&conn).unwrap();
        assert!(cats.iter().any(|c| c.id == id && c.name == "Groceries"));
    }

    #[test]
    fn add_category_duplicate_name_returns_error() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        add_category_impl(&conn, "Groceries").unwrap();
        let err = add_category_impl(&conn, "Groceries").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "expected a friendly validation error, got {err:?}");
    }

    #[test]
    fn list_categories_returns_seeded_defaults() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let cats = list_categories_impl(&conn).unwrap();
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        for expected in ["Food", "Rent", "EMI", "Other"] {
            assert!(names.contains(&expected), "expected seeded default '{expected}' to be present, got {names:?}");
        }
    }

    #[test]
    fn update_category_renames() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_category_impl(&conn, "Groceries").unwrap();
        update_category_impl(&conn, id, "Household").unwrap();

        let cats = list_categories_impl(&conn).unwrap();
        assert!(cats.iter().any(|c| c.id == id && c.name == "Household"));
        assert!(!cats.iter().any(|c| c.name == "Groceries"));
    }

    #[test]
    fn update_category_duplicate_name_returns_error() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        add_category_impl(&conn, "Groceries").unwrap();
        let id2 = add_category_impl(&conn, "Household").unwrap();

        let err = update_category_impl(&conn, id2, "Groceries").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn delete_category_unused_succeeds() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_category_impl(&conn, "Groceries").unwrap();
        delete_category_impl(&conn, id).unwrap();

        let cats = list_categories_impl(&conn).unwrap();
        assert!(!cats.iter().any(|c| c.id == id));
    }

    #[test]
    fn delete_category_referenced_by_transaction_blocked() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_category_impl(&conn, "Groceries").unwrap();
        conn.execute(
            "INSERT INTO transactions (date, type, amount, category) VALUES ('2026-01-01','expense',50,'Groceries')",
            [],
        ).unwrap();

        let err = delete_category_impl(&conn, id).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));

        let cats = list_categories_impl(&conn).unwrap();
        assert!(cats.iter().any(|c| c.id == id), "category should still exist after blocked delete");
    }

    #[test]
    fn delete_category_referenced_by_budget_blocked() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_category_impl(&conn, "Groceries").unwrap();
        conn.execute(
            "INSERT INTO budgets (category, monthly_limit) VALUES ('Groceries', 5000)",
            [],
        ).unwrap();

        let err = delete_category_impl(&conn, id).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));

        let cats = list_categories_impl(&conn).unwrap();
        assert!(cats.iter().any(|c| c.id == id), "category should still exist after blocked delete");
    }
}
