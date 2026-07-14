use rusqlite::Connection;
use tauri::State;
use serde::{Deserialize, Serialize};
use crate::db::DbState;
use crate::error::Result;
use crate::models::budget::BudgetStatus;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub category: String,
    pub tx_type: String,
    pub total: f64,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyTrend {
    pub month: String,
    pub income: f64,
    pub expense: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBudgetPayload {
    pub category: String,
    pub monthly_limit: f64,
}

fn category_summary_impl(conn: &Connection, start: &str, end: &str) -> Result<Vec<CategorySummary>> {
    let mut stmt = conn.prepare(
        "SELECT category, type, SUM(amount) as total, COUNT(*) as cnt
         FROM transactions
         WHERE type IN ('expense','income') AND date>=?1 AND date<=?2
           AND category IS NOT NULL
         GROUP BY category, type ORDER BY total DESC"
    )?;
    let rows = stmt.query_map([start, end], |r| Ok(CategorySummary {
        category: r.get(0)?, tx_type: r.get(1)?, total: r.get(2)?, count: r.get(3)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub(crate) fn budget_status_impl(conn: &Connection, start: &str, end: &str) -> Result<Vec<BudgetStatus>> {
    let mut stmt = conn.prepare(
        "SELECT b.category, b.monthly_limit,
                COALESCE(SUM(t.amount), 0.0) AS spent
         FROM budgets b
         LEFT JOIN transactions t
           ON  t.category = b.category
           AND t.type     = 'expense'
           AND t.date    >= ?1
           AND t.date    <= ?2
         WHERE b.is_active = 1
         GROUP BY b.id, b.category, b.monthly_limit",
    )?;
    let rows = stmt.query_map(rusqlite::params![start, end], |r| {
        let category: String = r.get(0)?;
        let monthly_limit: f64 = r.get(1)?;
        let spent: f64 = r.get(2)?;
        Ok((category, monthly_limit, spent))
    })?;
    let result = rows.filter_map(|r| r.ok()).map(|(category, monthly_limit, spent)| {
        let remaining = (monthly_limit - spent).max(0.0);
        let percent_used = if monthly_limit > 0.0 {
            ((spent / monthly_limit) * 10000.0).round() / 100.0
        } else { 0.0 };
        BudgetStatus { category, monthly_limit, spent, remaining, percent_used }
    }).collect();
    Ok(result)
}

fn set_budget_impl(conn: &Connection, payload: &SetBudgetPayload) -> Result<()> {
    conn.execute(
        "INSERT INTO budgets (category, monthly_limit) VALUES (?1, ?2)
         ON CONFLICT(category, period) DO UPDATE SET monthly_limit=excluded.monthly_limit",
        rusqlite::params![payload.category, payload.monthly_limit],
    )?;
    Ok(())
}

fn delete_budget_impl(conn: &Connection, category: &str) -> Result<()> {
    conn.execute("DELETE FROM budgets WHERE category = ?1", [category])?;
    Ok(())
}

fn monthly_trend_impl(conn: &Connection, months: i64) -> Result<Vec<MonthlyTrend>> {
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', date) as month,
                SUM(CASE WHEN type='income' THEN amount ELSE 0 END) as income,
                SUM(CASE WHEN type='expense' THEN amount ELSE 0 END) as expense
         FROM transactions
         WHERE ?1 <= 0 OR date >= date('now', '-' || ?1 || ' months')
         GROUP BY month ORDER BY month DESC"
    )?;
    let rows = stmt.query_map([months], |r| Ok(MonthlyTrend {
        month: r.get(0)?, income: r.get(1)?, expense: r.get(2)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn get_category_summary(
    period: String,
    custom_start: Option<String>,
    custom_end: Option<String>,
    state: State<DbState>,
) -> Result<Vec<CategorySummary>> {
    let conn = state.0.get()?;
    let (start, end) = period_bounds(&period, custom_start, custom_end);
    category_summary_impl(&conn, &start, &end)
}

#[tauri::command]
pub fn get_budget_status(state: State<DbState>) -> Result<Vec<BudgetStatus>> {
    let conn = state.0.get()?;
    let (start, end) = current_month_bounds();
    budget_status_impl(&conn, &start, &end)
}

#[tauri::command]
pub fn set_budget(payload: SetBudgetPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    set_budget_impl(&conn, &payload)
}

#[tauri::command]
pub fn delete_budget(category: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    delete_budget_impl(&conn, &category)
}

#[tauri::command]
pub fn get_monthly_trend(months: i64, state: State<DbState>) -> Result<Vec<MonthlyTrend>> {
    let conn = state.0.get()?;
    monthly_trend_impl(&conn, months)
}

fn period_bounds(period: &str, custom_start: Option<String>, custom_end: Option<String>) -> (String, String) {
    match period {
        "this_month" => current_month_bounds(),
        "last_month" => last_month_bounds(),
        "custom" => match (custom_start, custom_end) {
            (Some(start), Some(end)) => (start, format!("{end} 23:59:59")),
            _ => ("1900-01-01".into(), "2099-12-31".into()),
        },
        _ => ("1900-01-01".into(), "2099-12-31".into()),
    }
}

fn last_month_bounds() -> (String, String) {
    use chrono::{Datelike, Local, NaiveDate};
    let now = Local::now();
    let first_of_this = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(now.year(), 1, 1).expect("Jan 1 is always valid"));
    let last_of_prev = first_of_this.pred_opt().unwrap_or(first_of_this);
    let first_of_prev = NaiveDate::from_ymd_opt(last_of_prev.year(), last_of_prev.month(), 1)
        .unwrap_or(last_of_prev);
    (first_of_prev.to_string(), format!("{} 23:59:59", last_of_prev))
}

pub(crate) fn current_month_bounds() -> (String, String) {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let start = format!("{}-{:02}-01", now.year(), now.month());
    let end = format!("{}-{:02}-31 23:59:59", now.year(), now.month());
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    fn insert_txn(conn: &Connection, date: &str, tx_type: &str, amount: f64, category: &str) {
        conn.execute(
            "INSERT INTO transactions (date, type, amount, category) VALUES (?1,?2,?3,?4)",
            rusqlite::params![date, tx_type, amount, category],
        )
        .unwrap();
    }

    #[test]
    fn category_summary_groups_by_category_and_type_within_window() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        insert_txn(&conn, "2026-06-05", "expense", 500.0, "Food");
        insert_txn(&conn, "2026-06-20", "expense", 300.0, "Food");
        insert_txn(&conn, "2026-06-10", "income", 50_000.0, "Salary");
        insert_txn(&conn, "2026-05-01", "expense", 999.0, "Food"); // outside window
        insert_txn(&conn, "2026-06-15", "buy", 1_000.0, "Food");   // wrong type

        let summary = category_summary_impl(&conn, "2026-06-01", "2026-06-30 23:59:59").unwrap();
        assert_eq!(summary.len(), 2);

        let food = summary.iter().find(|s| s.category == "Food").unwrap();
        assert_eq!(food.tx_type, "expense");
        assert_eq!(food.total, 800.0);
        assert_eq!(food.count, 2);

        let salary = summary.iter().find(|s| s.category == "Salary").unwrap();
        assert_eq!(salary.tx_type, "income");
        assert_eq!(salary.total, 50_000.0);
    }

    #[test]
    fn budget_status_computes_spent_remaining_and_percent() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        set_budget_impl(&conn, &SetBudgetPayload { category: "Food".into(), monthly_limit: 1_000.0 }).unwrap();
        insert_txn(&conn, "2026-06-05", "expense", 250.0, "Food");

        let status = budget_status_impl(&conn, "2026-06-01", "2026-06-30 23:59:59").unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].spent, 250.0);
        assert_eq!(status[0].remaining, 750.0);
        assert_eq!(status[0].percent_used, 25.0);
    }

    #[test]
    fn budget_status_overspend_clamps_remaining_to_zero() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        set_budget_impl(&conn, &SetBudgetPayload { category: "Food".into(), monthly_limit: 1_000.0 }).unwrap();
        insert_txn(&conn, "2026-06-05", "expense", 1_500.0, "Food");

        let status = budget_status_impl(&conn, "2026-06-01", "2026-06-30 23:59:59").unwrap();
        assert_eq!(status[0].remaining, 0.0);
        assert_eq!(status[0].percent_used, 150.0);
    }

    #[test]
    fn set_budget_upserts_existing_category_limit() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        set_budget_impl(&conn, &SetBudgetPayload { category: "Food".into(), monthly_limit: 1_000.0 }).unwrap();
        set_budget_impl(&conn, &SetBudgetPayload { category: "Food".into(), monthly_limit: 2_000.0 }).unwrap();

        let status = budget_status_impl(&conn, "2026-06-01", "2026-06-30").unwrap();
        assert_eq!(status.len(), 1, "upsert must not create a duplicate budget row");
        assert_eq!(status[0].monthly_limit, 2_000.0);
    }

    #[test]
    fn delete_budget_removes_row() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        set_budget_impl(&conn, &SetBudgetPayload { category: "Food".into(), monthly_limit: 1_000.0 }).unwrap();
        delete_budget_impl(&conn, "Food").unwrap();

        let status = budget_status_impl(&conn, "2026-06-01", "2026-06-30").unwrap();
        assert!(status.is_empty());
    }

    #[test]
    fn monthly_trend_groups_income_and_expense_per_month() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        insert_txn(&conn, "2026-05-10", "income", 50_000.0, "Salary");
        insert_txn(&conn, "2026-05-15", "expense", 8_000.0, "Rent");
        insert_txn(&conn, "2026-06-10", "income", 50_000.0, "Salary");

        // months <= 0 → all history, newest month first
        let trend = monthly_trend_impl(&conn, 0).unwrap();
        assert_eq!(trend.len(), 2);
        assert_eq!(trend[0].month, "2026-06");
        assert_eq!(trend[0].income, 50_000.0);
        assert_eq!(trend[0].expense, 0.0);
        assert_eq!(trend[1].month, "2026-05");
        assert_eq!(trend[1].income, 50_000.0);
        assert_eq!(trend[1].expense, 8_000.0);
    }

    #[test]
    fn period_bounds_custom_widens_end_to_end_of_day() {
        let (start, end) = period_bounds("custom", Some("2026-01-01".into()), Some("2026-01-31".into()));
        assert_eq!(start, "2026-01-01");
        assert_eq!(end, "2026-01-31 23:59:59");
    }

    #[test]
    fn period_bounds_custom_without_dates_falls_back_to_all_time() {
        let (start, end) = period_bounds("custom", None, None);
        assert_eq!(start, "1900-01-01");
        assert_eq!(end, "2099-12-31");
    }
}
