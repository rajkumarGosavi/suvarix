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

#[tauri::command]
pub fn get_category_summary(
    period: String,
    custom_start: Option<String>,
    custom_end: Option<String>,
    state: State<DbState>,
) -> Result<Vec<CategorySummary>> {
    let conn = state.0.get()?;
    let (start, end) = period_bounds(&period, custom_start, custom_end);
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
    let conn = state.0.get()?;
    let (start, end) = current_month_bounds();
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
    let rows = stmt.query_map(rusqlite::params![&start, &end], |r| {
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

#[tauri::command]
pub fn set_budget(payload: SetBudgetPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO budgets (category, monthly_limit) VALUES (?1, ?2)
         ON CONFLICT(category, period) DO UPDATE SET monthly_limit=excluded.monthly_limit",
        rusqlite::params![payload.category, payload.monthly_limit],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_budget(category: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute("DELETE FROM budgets WHERE category = ?1", [category])?;
    Ok(())
}

#[tauri::command]
pub fn get_monthly_trend(months: i64, state: State<DbState>) -> Result<Vec<MonthlyTrend>> {
    let conn = state.0.get()?;
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

fn current_month_bounds() -> (String, String) {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let start = format!("{}-{:02}-01", now.year(), now.month());
    let end = format!("{}-{:02}-31 23:59:59", now.year(), now.month());
    (start, end)
}
