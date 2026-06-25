use tauri::State;
use crate::db::DbState;
use crate::error::{AppError, Result};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub target_amount: f64,
    pub target_date: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoalPayload {
    pub name: String,
    pub category: String,
    pub target_amount: f64,
    pub target_date: String,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn list_goals(state: State<DbState>) -> Result<Vec<Goal>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, name, category, target_amount, target_date, notes, created_at, updated_at
         FROM goals ORDER BY target_date ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Goal {
            id: r.get(0)?,
            name: r.get(1)?,
            category: r.get(2)?,
            target_amount: r.get(3)?,
            target_date: r.get(4)?,
            notes: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_goal(payload: GoalPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO goals (name, category, target_amount, target_date, notes)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            payload.name,
            payload.category,
            payload.target_amount,
            payload.target_date,
            payload.notes,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_goal(id: i64, payload: GoalPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE goals SET name=?1, category=?2, target_amount=?3, target_date=?4,
         notes=?5, updated_at=datetime('now') WHERE id=?6",
        rusqlite::params![
            payload.name,
            payload.category,
            payload.target_amount,
            payload.target_date,
            payload.notes,
            id,
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_goal(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM goals WHERE id=?1", [id])?;
    Ok(())
}
