use rusqlite::Connection;
use tauri::State;
use crate::db::DbState;
use crate::error::Result;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub target_amount: f64,
    pub target_date: String,
    pub notes: Option<String>,
    pub achieved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoalAchievement {
    pub id: i64,
    pub name: String,
    pub target_amount: f64,
    pub category: String,
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

fn list_goals_impl(conn: &Connection) -> Result<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, category, target_amount, target_date, notes, achieved_at, created_at, updated_at
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
            achieved_at: r.get(6)?,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn add_goal_impl(conn: &Connection, payload: &GoalPayload) -> Result<i64> {
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

fn update_goal_impl(conn: &Connection, id: i64, payload: &GoalPayload) -> Result<()> {
    conn.execute(
        "UPDATE goals SET name=?1, category=?2, target_amount=?3, target_date=?4,
         notes=?5, achieved_at=NULL, updated_at=datetime('now') WHERE id=?6",
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

fn delete_goal_impl(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM goals WHERE id=?1", [id])?;
    Ok(())
}

fn mark_goal_achieved_impl(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE goals SET achieved_at = date('now'), updated_at = datetime('now') WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

fn check_goal_achievements_impl(conn: &Connection, total_assets: f64) -> Result<Vec<GoalAchievement>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, target_amount, category FROM goals
         WHERE target_amount <= ?1 AND (achieved_at IS NULL)"
    )?;
    let newly: Vec<GoalAchievement> = stmt
        .query_map([total_assets], |r| Ok(GoalAchievement {
            id: r.get(0)?,
            name: r.get(1)?,
            target_amount: r.get(2)?,
            category: r.get(3)?,
        }))?
        .filter_map(|r| r.ok())
        .collect();
    if !newly.is_empty() {
        conn.execute(
            "UPDATE goals SET achieved_at = date('now') WHERE target_amount <= ?1 AND achieved_at IS NULL",
            [total_assets],
        )?;
    }
    Ok(newly)
}

#[tauri::command]
pub fn list_goals(state: State<DbState>) -> Result<Vec<Goal>> {
    let conn = state.0.get()?;
    list_goals_impl(&conn)
}

#[tauri::command]
pub fn add_goal(payload: GoalPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    add_goal_impl(&conn, &payload)
}

#[tauri::command]
pub fn update_goal(id: i64, payload: GoalPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    update_goal_impl(&conn, id, &payload)
}

#[tauri::command]
pub fn delete_goal(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    delete_goal_impl(&conn, id)
}

/// Manually mark a goal as achieved regardless of current net worth.
#[tauri::command]
pub fn mark_goal_achieved(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    mark_goal_achieved_impl(&conn, id)
}

/// Check total_assets against unachieved goals. Marks newly reached ones and
/// returns only the newly achieved goals so the caller can notify.
#[tauri::command]
pub fn check_goal_achievements(total_assets: f64, state: State<DbState>) -> Result<Vec<GoalAchievement>> {
    let conn = state.0.get()?;
    check_goal_achievements_impl(&conn, total_assets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    fn goal(name: &str, target_amount: f64, target_date: &str) -> GoalPayload {
        GoalPayload {
            name: name.into(),
            category: "other".into(),
            target_amount,
            target_date: target_date.into(),
            notes: None,
        }
    }

    #[test]
    fn add_and_list_goals_ordered_by_target_date() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        add_goal_impl(&conn, &goal("Retirement", 10_000_000.0, "2045-01-01")).unwrap();
        add_goal_impl(&conn, &goal("Car", 800_000.0, "2027-06-01")).unwrap();

        let goals = list_goals_impl(&conn).unwrap();
        assert_eq!(goals.len(), 2);
        assert_eq!(goals[0].name, "Car");
        assert_eq!(goals[1].name, "Retirement");
        assert!(goals[0].achieved_at.is_none());
    }

    #[test]
    fn update_goal_edits_fields_and_resets_achievement() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_goal_impl(&conn, &goal("Car", 800_000.0, "2027-06-01")).unwrap();
        mark_goal_achieved_impl(&conn, id).unwrap();

        update_goal_impl(&conn, id, &goal("Bigger Car", 1_200_000.0, "2028-06-01")).unwrap();

        let goals = list_goals_impl(&conn).unwrap();
        assert_eq!(goals[0].name, "Bigger Car");
        assert_eq!(goals[0].target_amount, 1_200_000.0);
        assert!(goals[0].achieved_at.is_none(), "editing a goal must reset achieved_at");
    }

    #[test]
    fn delete_goal_removes_it() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_goal_impl(&conn, &goal("Car", 800_000.0, "2027-06-01")).unwrap();
        delete_goal_impl(&conn, id).unwrap();
        assert!(list_goals_impl(&conn).unwrap().is_empty());
    }

    #[test]
    fn mark_goal_achieved_sets_achieved_at() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        let id = add_goal_impl(&conn, &goal("Car", 800_000.0, "2027-06-01")).unwrap();
        mark_goal_achieved_impl(&conn, id).unwrap();

        let goals = list_goals_impl(&conn).unwrap();
        assert!(goals[0].achieved_at.is_some());
    }

    #[test]
    fn check_achievements_marks_reached_goals_and_returns_them_once() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        add_goal_impl(&conn, &goal("Emergency", 100_000.0, "2026-12-01")).unwrap();
        add_goal_impl(&conn, &goal("House", 5_000_000.0, "2035-01-01")).unwrap();

        let newly = check_goal_achievements_impl(&conn, 150_000.0).unwrap();
        assert_eq!(newly.len(), 1);
        assert_eq!(newly[0].name, "Emergency");

        // Second call with same net worth: already marked, nothing new
        let again = check_goal_achievements_impl(&conn, 150_000.0).unwrap();
        assert!(again.is_empty());

        // Goal above threshold stays unachieved
        let goals = list_goals_impl(&conn).unwrap();
        let house = goals.iter().find(|g| g.name == "House").unwrap();
        assert!(house.achieved_at.is_none());
    }
}
