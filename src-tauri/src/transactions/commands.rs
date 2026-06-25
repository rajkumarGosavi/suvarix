use tauri::State;
use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::transaction::{AddTransactionPayload, Transaction, TransactionFilter};

#[tauri::command]
pub fn list_transactions(filter: TransactionFilter, state: State<DbState>) -> Result<Vec<Transaction>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let limit = filter.limit.unwrap_or(100);
    let offset = filter.offset.unwrap_or(0);

    let mut sql = String::from(
        "SELECT id, date, type, asset_class, account_id, holding_id, amount, quantity,
                price, category, description, notes, source, external_ref, created_at, updated_at
         FROM transactions WHERE 1=1"
    );
    if filter.asset_class.is_some() { sql.push_str(" AND asset_class=?"); }
    if filter.account_id.is_some() { sql.push_str(" AND account_id=?"); }
    if filter.category.is_some() { sql.push_str(" AND category=?"); }
    if filter.date_from.is_some() { sql.push_str(" AND date>=?"); }
    if filter.date_to.is_some() { sql.push_str(" AND date<=?"); }
    sql.push_str(" ORDER BY date DESC LIMIT ? OFFSET ?");

    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
    if let Some(v) = filter.asset_class { params.push(Box::new(v)); }
    if let Some(v) = filter.account_id { params.push(Box::new(v)); }
    if let Some(v) = filter.category { params.push(Box::new(v)); }
    if let Some(v) = filter.date_from { params.push(Box::new(v)); }
    if let Some(v) = filter.date_to { params.push(Box::new(v)); }
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |r| Ok(Transaction {
        id: r.get(0)?, date: r.get(1)?, r#type: r.get(2)?, asset_class: r.get(3)?,
        account_id: r.get(4)?, holding_id: r.get(5)?, amount: r.get(6)?,
        quantity: r.get(7)?, price: r.get(8)?, category: r.get(9)?,
        description: r.get(10)?, notes: r.get(11)?, source: r.get(12)?,
        external_ref: r.get(13)?, created_at: r.get(14)?, updated_at: r.get(15)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_transaction(payload: AddTransactionPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO transactions (date, type, asset_class, account_id, holding_id,
         amount, quantity, price, category, description, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        rusqlite::params![payload.date, payload.r#type, payload.asset_class, payload.account_id,
            payload.holding_id, payload.amount, payload.quantity, payload.price,
            payload.category, payload.description, payload.notes],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_transaction(id: i64, payload: AddTransactionPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE transactions SET date=?1, type=?2, asset_class=?3, amount=?4,
         quantity=?5, price=?6, category=?7, description=?8, notes=?9,
         updated_at=datetime('now') WHERE id=?10",
        rusqlite::params![payload.date, payload.r#type, payload.asset_class, payload.amount,
            payload.quantity, payload.price, payload.category, payload.description,
            payload.notes, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_transaction(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM transactions WHERE id=?1", [id])?;
    Ok(())
}
