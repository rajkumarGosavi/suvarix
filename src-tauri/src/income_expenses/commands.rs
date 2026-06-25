use tauri::State;
use serde::{Deserialize, Serialize};
use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::budget::{Budget, BudgetStatus};

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

#[tauri::command]
pub fn get_category_summary(period: String, state: State<DbState>) -> Result<Vec<CategorySummary>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let (start, end) = period_bounds(&period);
    let mut stmt = conn.prepare(
        "SELECT category, type, SUM(amount) as total, COUNT(*) as cnt
         FROM transactions
         WHERE type IN ('expense','income') AND date>=?1 AND date<=?2
           AND category IS NOT NULL
         GROUP BY category, type ORDER BY total DESC"
    )?;
    let rows = stmt.query_map([&start, &end], |r| Ok(CategorySummary {
        category: r.get(0)?, tx_type: r.get(1)?, total: r.get(2)?, count: r.get(3)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn get_budget_status(state: State<DbState>) -> Result<Vec<BudgetStatus>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let (start, end) = current_month_bounds();
    let mut stmt = conn.prepare("SELECT id, category, monthly_limit, period, is_active FROM budgets WHERE is_active=1")?;
    let budgets: Vec<Budget> = stmt.query_map([], |r| Ok(Budget {
        id: r.get(0)?, category: r.get(1)?, monthly_limit: r.get(2)?,
        period: r.get(3)?, is_active: r.get::<_, i64>(4)? != 0,
    }))?.filter_map(|r| r.ok()).collect();

    let mut result = vec![];
    for b in budgets {
        let spent: f64 = conn.query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM transactions
             WHERE type='expense' AND category=?1 AND date>=?2 AND date<=?3",
            rusqlite::params![&b.category, &start, &end],
            |r| r.get(0),
        ).unwrap_or(0.0);

        let remaining = (b.monthly_limit - spent).max(0.0);
        let percent_used = if b.monthly_limit > 0.0 {
            ((spent / b.monthly_limit) * 10000.0).round() / 100.0
        } else { 0.0 };
        result.push(BudgetStatus { category: b.category, monthly_limit: b.monthly_limit, spent, remaining, percent_used });
    }
    Ok(result)
}

#[tauri::command]
pub fn set_budget(payload: SetBudgetPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO budgets (category, monthly_limit) VALUES (?1, ?2)
         ON CONFLICT(category, period) DO UPDATE SET monthly_limit=excluded.monthly_limit",
        rusqlite::params![payload.category, payload.monthly_limit],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_monthly_trend(months: i64, state: State<DbState>) -> Result<Vec<MonthlyTrend>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', date) as month,
                SUM(CASE WHEN type='income' THEN amount ELSE 0 END) as income,
                SUM(CASE WHEN type='expense' THEN amount ELSE 0 END) as expense
         FROM transactions
         WHERE date >= date('now', '-' || ?1 || ' months')
         GROUP BY month ORDER BY month DESC"
    )?;
    let rows = stmt.query_map([months], |r| Ok(MonthlyTrend {
        month: r.get(0)?, income: r.get(1)?, expense: r.get(2)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn period_bounds(period: &str) -> (String, String) {
    match period {
        "this_month" => current_month_bounds(),
        "last_month" => last_month_bounds(),
        _ => ("1900-01-01".into(), "2099-12-31".into()),
    }
}

fn last_month_bounds() -> (String, String) {
    use chrono::{Datelike, Local, NaiveDate};
    let now = Local::now();
    let first_of_this = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
    let last_of_prev = first_of_this.pred_opt().unwrap();
    let first_of_prev = NaiveDate::from_ymd_opt(last_of_prev.year(), last_of_prev.month(), 1).unwrap();
    (first_of_prev.to_string(), last_of_prev.to_string())
}

fn current_month_bounds() -> (String, String) {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let start = format!("{}-{:02}-01", now.year(), now.month());
    let end = format!("{}-{:02}-31", now.year(), now.month());
    (start, end)
}
