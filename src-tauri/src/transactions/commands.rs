use rusqlite::Connection;
use tauri::State;
use crate::db::DbState;
use crate::error::Result;
use crate::models::transaction::{AddTransactionPayload, Transaction, TransactionFilter};

/// Builds the shared "WHERE ..." clause + bound params for a TransactionFilter,
/// used by both `list_transactions` and `count_transactions` so the two stay in sync.
fn filter_where_clause(filter: &TransactionFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
    let mut sql = String::from(" WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
    if let Some(v) = filter.r#type.clone() { sql.push_str(" AND type=?"); params.push(Box::new(v)); }
    if let Some(v) = filter.asset_class.clone() { sql.push_str(" AND asset_class=?"); params.push(Box::new(v)); }
    if let Some(v) = filter.account_id { sql.push_str(" AND account_id=?"); params.push(Box::new(v)); }
    if let Some(v) = filter.category.clone() { sql.push_str(" AND category=?"); params.push(Box::new(v)); }
    if let Some(v) = filter.date_from.clone() { sql.push_str(" AND date>=?"); params.push(Box::new(v)); }
    if let Some(v) = filter.date_to.clone() {
        sql.push_str(" AND date<=?");
        // date_to arrives as "YYYY-MM-DD" (date only) but the date column may hold a
        // full datetime — widen to end-of-day so same-day transactions aren't excluded.
        let bound = if v.contains(' ') { v } else { format!("{v} 23:59:59") };
        params.push(Box::new(bound));
    }
    if let Some(v) = filter.search.clone().filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND (description LIKE ? ESCAPE '\\' OR category LIKE ? ESCAPE '\\' OR tag LIKE ? ESCAPE '\\')");
        let pattern = format!("%{}%", v.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }
    (sql, params)
}

/// Whitelisted ORDER BY clause — never interpolate the filter's sort fields directly into SQL.
fn order_by_clause(filter: &TransactionFilter) -> &'static str {
    let desc = filter.sort_dir.as_deref() != Some("asc");
    match (filter.sort_by.as_deref(), desc) {
        (Some("amount"), true)  => " ORDER BY amount DESC",
        (Some("amount"), false) => " ORDER BY amount ASC",
        (_, true)               => " ORDER BY date DESC",
        (_, false)              => " ORDER BY date ASC",
    }
}

fn list_transactions_impl(conn: &Connection, filter: &TransactionFilter) -> Result<Vec<Transaction>> {
    let limit = filter.limit.unwrap_or(100);
    let offset = filter.offset.unwrap_or(0);

    let (where_clause, mut params) = filter_where_clause(filter);
    let sql = format!(
        "SELECT id, date, type, asset_class, account_id, holding_id, amount, quantity,
                price, category, tag, description, notes, source, external_ref, created_at, updated_at
         FROM transactions{}{} LIMIT ? OFFSET ?",
        where_clause,
        order_by_clause(filter),
    );
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |r| Ok(Transaction {
        id: r.get(0)?, date: r.get(1)?, r#type: r.get(2)?, asset_class: r.get(3)?,
        account_id: r.get(4)?, holding_id: r.get(5)?, amount: r.get(6)?,
        quantity: r.get(7)?, price: r.get(8)?, category: r.get(9)?, tag: r.get(10)?,
        description: r.get(11)?, notes: r.get(12)?, source: r.get(13)?,
        external_ref: r.get(14)?, created_at: r.get(15)?, updated_at: r.get(16)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn count_transactions_impl(conn: &Connection, filter: &TransactionFilter) -> Result<i64> {
    let (where_clause, params) = filter_where_clause(filter);
    let sql = format!("SELECT COUNT(*) FROM transactions{}", where_clause);
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let count: i64 = conn.query_row(&sql, param_refs.as_slice(), |r| r.get(0))?;
    Ok(count)
}

fn add_transaction_impl(conn: &Connection, payload: &AddTransactionPayload) -> Result<i64> {
    conn.execute(
        "INSERT INTO transactions (date, type, asset_class, account_id, holding_id,
         amount, quantity, price, category, tag, description, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        rusqlite::params![payload.date, payload.r#type, payload.asset_class, payload.account_id,
            payload.holding_id, payload.amount, payload.quantity, payload.price,
            payload.category, payload.tag, payload.description, payload.notes],
    )?;
    Ok(conn.last_insert_rowid())
}

fn update_transaction_impl(conn: &Connection, id: i64, payload: &AddTransactionPayload) -> Result<()> {
    conn.execute(
        "UPDATE transactions SET date=?1, type=?2, asset_class=?3, amount=?4,
         quantity=?5, price=?6, category=?7, tag=?8, description=?9, notes=?10,
         updated_at=datetime('now') WHERE id=?11",
        rusqlite::params![payload.date, payload.r#type, payload.asset_class, payload.amount,
            payload.quantity, payload.price, payload.category, payload.tag, payload.description,
            payload.notes, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_transactions(filter: TransactionFilter, state: State<DbState>) -> Result<Vec<Transaction>> {
    let conn = state.0.get()?;
    list_transactions_impl(&conn, &filter)
}

/// Total count of transactions matching the filter (ignoring limit/offset) — used by the
/// frontend's lazy DataTable paginator to know how many pages exist.
#[tauri::command]
pub fn count_transactions(filter: TransactionFilter, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    count_transactions_impl(&conn, &filter)
}

#[tauri::command]
pub fn add_transaction(payload: AddTransactionPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    add_transaction_impl(&conn, &payload)
}

#[tauri::command]
pub fn update_transaction(id: i64, payload: AddTransactionPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    update_transaction_impl(&conn, id, &payload)
}

#[tauri::command]
pub fn delete_transaction(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute("DELETE FROM transactions WHERE id=?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    fn sample_payload(category: Option<&str>, tag: Option<&str>) -> AddTransactionPayload {
        AddTransactionPayload {
            date: "2026-01-01 00:00:00".into(),
            r#type: "expense".into(),
            asset_class: None,
            account_id: None,
            holding_id: None,
            amount: 100.0,
            quantity: None,
            price: None,
            category: category.map(String::from),
            tag: tag.map(String::from),
            description: Some("test".into()),
            notes: None,
        }
    }

    #[test]
    fn add_transaction_with_tag_persists_tag() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_transaction_impl(&conn, &sample_payload(Some("Food"), Some("House"))).unwrap();

        let txns = list_transactions_impl(&conn, &TransactionFilter {
            r#type: None, asset_class: None, account_id: None, category: None,
            date_from: None, date_to: None, search: None, sort_by: None, sort_dir: None,
            limit: None, offset: None,
        }).unwrap();
        let saved = txns.iter().find(|t| t.id == id).expect("just-inserted transaction should be listed");
        assert_eq!(saved.tag, Some("House".to_string()));
    }

    #[test]
    fn update_transaction_tag() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_transaction_impl(&conn, &sample_payload(Some("Food"), Some("House"))).unwrap();
        update_transaction_impl(&conn, id, &sample_payload(Some("Food"), Some("Personal"))).unwrap();

        let tag: Option<String> = conn.query_row(
            "SELECT tag FROM transactions WHERE id=?1", [id], |r| r.get(0),
        ).unwrap();
        assert_eq!(tag, Some("Personal".to_string()));
    }

    #[test]
    fn search_filter_matches_tag() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        add_transaction_impl(&conn, &sample_payload(Some("Food"), Some("House"))).unwrap();
        add_transaction_impl(&conn, &sample_payload(Some("Food"), Some("Personal"))).unwrap();

        let filter = TransactionFilter {
            r#type: None, asset_class: None, account_id: None, category: None,
            date_from: None, date_to: None, search: Some("House".into()),
            sort_by: None, sort_dir: None, limit: None, offset: None,
        };
        let txns = list_transactions_impl(&conn, &filter).unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].tag, Some("House".to_string()));
    }
}
