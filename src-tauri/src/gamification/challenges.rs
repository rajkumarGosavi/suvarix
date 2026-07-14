//! Opt-in, time-boxed savings challenges. Progress is derived on demand from the
//! transaction ledger / budgets — only status transitions and XP awards are written
//! back. Three kinds:
//!   - `save_amount`  save ≥ target rupees over the window
//!   - `no_spend`     accumulate ≥ target zero-spend days in the window
//!   - `budget_hold`  finish the window without breaching any budget
//!
//! All gamification-gated (compiled only with `--features gamification`).

use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use super::commands::award_xp_internal;
use crate::db::DbState;
use crate::error::{AppError, Result};

// ─── Catalog ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeTemplate {
    pub kind: String,
    pub title: String,
    pub icon: String,
    pub description: String,
    pub default_target: f64,
    pub duration_days: i64,
    pub xp_reward: i64,
    /// save_amount / no_spend let the user pick a target; budget_hold does not.
    pub target_editable: bool,
    /// "₹" (currency), "days", or "" — how the frontend should render target/progress.
    pub unit: String,
}

fn catalog() -> Vec<ChallengeTemplate> {
    vec![
        ChallengeTemplate {
            kind: "save_amount".into(),
            title: "Monthly Saver".into(),
            icon: "🐷".into(),
            description: "Save a target amount over the next 30 days.".into(),
            default_target: 10_000.0,
            duration_days: 30,
            xp_reward: 60,
            target_editable: true,
            unit: "₹".into(),
        },
        ChallengeTemplate {
            kind: "no_spend".into(),
            title: "No-Spend Days".into(),
            icon: "🚫".into(),
            description: "Rack up zero-spend days this week.".into(),
            default_target: 3.0,
            duration_days: 7,
            xp_reward: 40,
            target_editable: true,
            unit: "days".into(),
        },
        ChallengeTemplate {
            kind: "budget_hold".into(),
            title: "Budget Boss".into(),
            icon: "🛡️".into(),
            description: "Stay within every budget for 30 days straight.".into(),
            default_target: 0.0,
            duration_days: 30,
            xp_reward: 50,
            target_editable: false,
            unit: "".into(),
        },
    ]
}

fn template_for(kind: &str) -> Option<ChallengeTemplate> {
    catalog().into_iter().find(|t| t.kind == kind)
}

fn unit_for(kind: &str) -> String {
    template_for(kind).map(|t| t.unit).unwrap_or_default()
}

// ─── View ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeView {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub icon: String,
    pub target: f64,
    pub start_date: String,
    pub end_date: String,
    pub xp_reward: i64,
    pub status: String,
    /// 0–100.
    pub progress_pct: f64,
    /// Raw current value (rupees saved / no-spend days / elapsed days) for the
    /// frontend to format (currency vs count).
    pub progress_value: f64,
    pub unit: String,
    pub days_left: i64,
}

// A stored challenge row, before progress is layered on.
struct ChallengeRow {
    id: i64,
    kind: String,
    title: String,
    icon: String,
    target: f64,
    start_date: String,
    end_date: String,
    xp_reward: i64,
    status: String,
}

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<ChallengeRow> {
    Ok(ChallengeRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        icon: r.get(3)?,
        target: r.get(4)?,
        start_date: r.get(5)?,
        end_date: r.get(6)?,
        xp_reward: r.get(7)?,
        status: r.get(8)?,
    })
}

const SELECT_COLS: &str =
    "id, kind, title, icon, target, start_date, end_date, xp_reward, status";

// ─── Progress engine ──────────────────────────────────────────────────────────

// Outcome of evaluating a row against the ledger *today*.
struct Progress {
    pct: f64,
    value: f64,
    done: bool,   // target met
    failed: bool, // window ended unmet, or a budget was breached
}

fn sum_amount(conn: &Connection, types: &str, start: &str, end: &str) -> f64 {
    conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(amount),0) FROM transactions \
             WHERE type IN ({types}) AND date(date) BETWEEN ?1 AND ?2"
        ),
        [start, end],
        |r| r.get::<_, f64>(0),
    )
    .unwrap_or(0.0)
}

fn compute_progress(conn: &Connection, row: &ChallengeRow, today: NaiveDate) -> Progress {
    let start = NaiveDate::parse_from_str(&row.start_date, "%Y-%m-%d").unwrap_or(today);
    let end = NaiveDate::parse_from_str(&row.end_date, "%Y-%m-%d").unwrap_or(today);
    // Progress is measured up to today, never past the window's end.
    let eff_end = today.min(end);
    let eff_end_s = eff_end.to_string();
    let window_over = today > end;

    match row.kind.as_str() {
        "save_amount" => {
            let income = sum_amount(conn, "'income'", &row.start_date, &eff_end_s);
            let spent = sum_amount(conn, "'expense','emi'", &row.start_date, &eff_end_s);
            let saved = income - spent;
            let done = saved >= row.target && row.target > 0.0;
            let pct = if row.target > 0.0 {
                (saved / row.target * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            Progress { pct, value: saved.max(0.0), done, failed: window_over && !done }
        }
        "no_spend" => {
            let elapsed = ((eff_end - start).num_days() + 1).max(0);
            let days_with_spend: i64 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT date(date)) FROM transactions \
                     WHERE type IN ('expense','emi') AND date(date) BETWEEN ?1 AND ?2",
                    [&row.start_date, &eff_end_s],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let no_spend = (elapsed - days_with_spend).max(0);
            let done = (no_spend as f64) >= row.target && row.target > 0.0;
            let pct = if row.target > 0.0 {
                (no_spend as f64 / row.target * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            Progress { pct, value: no_spend as f64, done, failed: window_over && !done }
        }
        "budget_hold" => {
            // Any active budget whose in-window expense exceeds its monthly limit = breach.
            let breaches: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ( \
                        SELECT b.id FROM budgets b \
                        JOIN transactions t ON t.category = b.category \
                          AND t.type = 'expense' \
                          AND date(t.date) BETWEEN ?1 AND ?2 \
                        WHERE b.is_active = 1 \
                        GROUP BY b.id, b.monthly_limit \
                        HAVING SUM(t.amount) > b.monthly_limit )",
                    [&row.start_date, &eff_end_s],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let breached = breaches > 0;
            let total_days = ((end - start).num_days() + 1).max(1);
            let elapsed = ((eff_end - start).num_days() + 1).clamp(0, total_days);
            let done = window_over && !breached;
            let pct = if breached {
                0.0
            } else {
                (elapsed as f64 / total_days as f64 * 100.0).clamp(0.0, 100.0)
            };
            Progress { pct, value: elapsed as f64, done, failed: breached }
        }
        _ => Progress { pct: 0.0, value: 0.0, done: false, failed: false },
    }
}

fn view_from(conn: &Connection, row: &ChallengeRow, today: NaiveDate) -> ChallengeView {
    let end = NaiveDate::parse_from_str(&row.end_date, "%Y-%m-%d").unwrap_or(today);
    let days_left = (end - today).num_days().max(0);

    // Terminal rows keep their recorded status; active rows show live progress.
    let (status, pct, value) = match row.status.as_str() {
        "completed" => ("completed".to_string(), 100.0, row.target),
        "failed" => ("failed".to_string(), 0.0, 0.0),
        _ => {
            let p = compute_progress(conn, row, today);
            ("active".to_string(), p.pct, p.value)
        }
    };

    ChallengeView {
        id: row.id,
        kind: row.kind.clone(),
        title: row.title.clone(),
        icon: row.icon.clone(),
        target: row.target,
        start_date: row.start_date.clone(),
        end_date: row.end_date.clone(),
        xp_reward: row.xp_reward,
        status,
        progress_pct: (pct * 10.0).round() / 10.0,
        progress_value: value,
        unit: unit_for(&row.kind),
        days_left,
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Catalog of joinable challenges, minus any kind the user already has active
/// (one live instance per kind keeps the feed focused).
#[tauri::command]
pub fn list_challenge_templates(state: State<DbState>) -> Result<Vec<ChallengeTemplate>> {
    let conn = state.0.get()?;
    let mut active_kinds = std::collections::HashSet::new();
    {
        let mut stmt = conn
            .prepare("SELECT DISTINCT kind FROM user_challenges WHERE status = 'active'")
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        for k in rows.flatten() {
            active_kinds.insert(k);
        }
    }
    Ok(catalog()
        .into_iter()
        .filter(|t| !active_kinds.contains(&t.kind))
        .collect())
}

/// Joins a challenge, creating an active instance dated [today, today+duration-1].
/// `target` overrides the template default for editable kinds.
#[tauri::command]
pub fn join_challenge(kind: String, target: Option<f64>, state: State<DbState>) -> Result<()> {
    let tmpl = template_for(&kind)
        .ok_or_else(|| AppError::Validation(format!("unknown challenge kind: {kind}")))?;
    let conn = state.0.get()?;

    // One active instance per kind.
    let existing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_challenges WHERE kind = ?1 AND status = 'active'",
            [&kind],
            |r| r.get(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    if existing > 0 {
        return Err(AppError::Validation("challenge already active".into()));
    }

    let chosen = if tmpl.target_editable {
        let t = target.unwrap_or(tmpl.default_target);
        if !(t.is_finite() && t > 0.0) {
            return Err(AppError::Validation("target must be positive".into()));
        }
        t
    } else {
        tmpl.default_target
    };

    let today = Local::now().date_naive();
    let end = today + chrono::Duration::days(tmpl.duration_days - 1);
    conn.execute(
        "INSERT INTO user_challenges \
         (kind, title, icon, target, start_date, end_date, xp_reward, status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active')",
        rusqlite::params![
            tmpl.kind,
            tmpl.title,
            tmpl.icon,
            chosen,
            today.to_string(),
            end.to_string(),
            tmpl.xp_reward,
        ],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Active challenges plus those that ended in the last 14 days (for the "done"
/// afterglow), each with live progress. Pure read — no status transitions.
#[tauri::command]
pub fn get_challenges(state: State<DbState>) -> Result<Vec<ChallengeView>> {
    let conn = state.0.get()?;
    let today = Local::now().date_naive();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLS} FROM user_challenges \
             WHERE status = 'active' \
                OR (status != 'active' AND date(completed_at) >= date('now','-14 days')) \
                OR (status != 'active' AND completed_at IS NULL AND date(end_date) >= date('now','-14 days')) \
             ORDER BY status = 'active' DESC, end_date ASC"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], row_from)
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(rows.flatten().map(|row| view_from(&conn, &row, today)).collect())
}

/// Transitions active challenges: awards XP for newly met targets (→ completed)
/// and fails those whose window ended unmet or whose budget was breached. Returns
/// the challenges that *just* completed, so the frontend can celebrate them.
#[tauri::command]
pub fn evaluate_challenges(state: State<DbState>) -> Result<Vec<ChallengeView>> {
    let conn = state.0.get()?;
    let today = Local::now().date_naive();

    let active: Vec<ChallengeRow> = {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {SELECT_COLS} FROM user_challenges WHERE status = 'active'"
            ))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], row_from)
            .map_err(|e| AppError::Database(e.to_string()))?;
        rows.flatten().collect()
    };

    let mut just_completed = Vec::new();
    for row in &active {
        let p = compute_progress(&conn, row, today);
        if p.done {
            conn.execute(
                "UPDATE user_challenges SET status = 'completed', completed_at = datetime('now') \
                 WHERE id = ?1 AND status = 'active'",
                [row.id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
            award_xp_internal(&conn, row.xp_reward as i32)?;
            let mut v = view_from(&conn, row, today);
            v.status = "completed".into();
            v.progress_pct = 100.0;
            just_completed.push(v);
        } else if p.failed {
            conn.execute(
                "UPDATE user_challenges SET status = 'failed', completed_at = datetime('now') \
                 WHERE id = ?1 AND status = 'active'",
                [row.id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(just_completed)
}

/// Drops an active challenge (user opt-out). No penalty.
#[tauri::command]
pub fn abandon_challenge(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "DELETE FROM user_challenges WHERE id = ?1 AND status = 'active'",
        [id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        run_migrations(&c).unwrap();
        c
    }

    fn insert_challenge(c: &Connection, kind: &str, target: f64, start: &str, end: &str) -> i64 {
        c.execute(
            "INSERT INTO user_challenges (kind, title, icon, target, start_date, end_date, xp_reward, status) \
             VALUES (?1, 'T', '🎯', ?2, ?3, ?4, 50, 'active')",
            rusqlite::params![kind, target, start, end],
        )
        .unwrap();
        c.last_insert_rowid()
    }

    #[test]
    fn save_amount_progress_and_completion() {
        let c = conn();
        let today = Local::now().date_naive();
        let start = (today - chrono::Duration::days(2)).to_string();
        let end = (today + chrono::Duration::days(10)).to_string();
        let id = insert_challenge(&c, "save_amount", 10_000.0, &start, &end);

        c.execute(
            "INSERT INTO transactions (type, amount, date) VALUES ('income', 30000, ?1)",
            [&start],
        )
        .unwrap();
        c.execute(
            "INSERT INTO transactions (type, amount, date) VALUES ('expense', 18000, ?1)",
            [&start],
        )
        .unwrap();

        let row = ChallengeRow {
            id,
            kind: "save_amount".into(),
            title: "T".into(),
            icon: "🎯".into(),
            target: 10_000.0,
            start_date: start,
            end_date: end,
            xp_reward: 50,
            status: "active".into(),
        };
        let p = compute_progress(&c, &row, today);
        assert!(p.done, "saved 12k >= 10k target");
        assert_eq!(p.value, 12_000.0);
        assert!(!p.failed);
    }

    #[test]
    fn evaluate_completes_and_awards_xp() {
        let c = conn();
        let today = Local::now().date_naive();
        let start = today.to_string();
        let end = (today + chrono::Duration::days(6)).to_string();
        insert_challenge(&c, "save_amount", 1000.0, &start, &end);
        c.execute(
            "INSERT INTO transactions (type, amount, date) VALUES ('income', 5000, ?1)",
            [&start],
        )
        .unwrap();

        // Directly exercise the transition + XP award logic.
        let xp_before: i64 = c
            .query_row("SELECT total_xp FROM user_xp WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);
        let active: Vec<ChallengeRow> = {
            let mut stmt = c
                .prepare(&format!(
                    "SELECT {SELECT_COLS} FROM user_challenges WHERE status = 'active'"
                ))
                .unwrap();
            let rows = stmt.query_map([], row_from).unwrap();
            rows.flatten().collect()
        };
        for row in &active {
            let p = compute_progress(&c, row, today);
            if p.done {
                c.execute(
                    "UPDATE user_challenges SET status='completed', completed_at=datetime('now') WHERE id=?1",
                    [row.id],
                )
                .unwrap();
                award_xp_internal(&c, row.xp_reward as i32).unwrap();
            }
        }
        let status: String = c
            .query_row("SELECT status FROM user_challenges LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "completed");
        let xp_after: i64 = c
            .query_row("SELECT total_xp FROM user_xp WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert!(xp_after > xp_before, "XP should be awarded on completion");
    }

    #[test]
    fn no_spend_counts_zero_expense_days() {
        let c = conn();
        let today = Local::now().date_naive();
        let start = (today - chrono::Duration::days(2)).to_string(); // 3-day window so far
        let end = (today + chrono::Duration::days(4)).to_string();
        let id = insert_challenge(&c, "no_spend", 3.0, &start, &end);
        // One expense on the start day only → 2 no-spend days elapsed.
        c.execute(
            "INSERT INTO transactions (type, amount, date) VALUES ('expense', 500, ?1)",
            [&start],
        )
        .unwrap();
        let row = ChallengeRow {
            id, kind: "no_spend".into(), title: "T".into(), icon: "🎯".into(),
            target: 3.0, start_date: start, end_date: end, xp_reward: 50, status: "active".into(),
        };
        let p = compute_progress(&c, &row, today);
        assert_eq!(p.value, 2.0, "3 elapsed days minus 1 spend day");
        assert!(!p.done);
    }
}
