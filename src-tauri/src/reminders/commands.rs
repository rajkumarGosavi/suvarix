use chrono::{Datelike, Duration, Local, NaiveDate};
use tauri::State;
use serde::{Deserialize, Serialize};
use crate::db::{DbPool, DbState};
use crate::error::{AppError, Result};

// ── Bill structs ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bill {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub amount: f64,
    pub frequency: String,
    pub next_due_date: String,
    pub notes: Option<String>,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillPayload {
    pub name: String,
    pub category: String,
    pub amount: f64,
    pub frequency: String,
    pub next_due_date: String,
    pub notes: Option<String>,
}

// ── Recurring transaction structs ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecurringTx {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: f64,
    pub category: String,
    pub asset_class: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub frequency: String,
    pub next_due_date: String,
    pub last_run_date: Option<String>,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringTxPayload {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: f64,
    pub category: String,
    pub asset_class: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub frequency: String,
    pub next_due_date: String,
}

// ── Upcoming reminder aggregate ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingReminder {
    pub source: String,      // "bill" | "loan" | "credit_card"
    pub source_id: i64,
    pub name: String,
    pub amount: f64,
    pub due_date: String,    // YYYY-MM-DD
    pub category: String,
    pub days_until_due: i64,
}

// ── Maturity alert aggregate ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaturityAlert {
    pub source: String,               // "fd" | "bond"
    pub source_id: i64,
    pub name: String,
    pub principal: f64,               // principal (FD) or face_value * quantity (bond)
    pub maturity_date: String,        // YYYY-MM-DD
    pub maturity_amount: Option<f64>, // FD only; None for bonds
    pub days_until_maturity: i64,     // negative = already matured
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn advance_date(date: NaiveDate, frequency: &str) -> NaiveDate {
    match frequency {
        "daily"   => date + Duration::days(1),
        "weekly"  => date + Duration::weeks(1),
        "yearly"  => date.with_year(date.year() + 1).unwrap_or(date + Duration::days(365)),
        // monthly — add 1 month, clamping to last day of target month
        _ => {
            let (y, m) = if date.month() == 12 {
                (date.year() + 1, 1u32)
            } else {
                (date.year(), date.month() + 1)
            };
            let last_day = days_in_month(y, m);
            NaiveDate::from_ymd_opt(y, m, date.day().min(last_day)).unwrap_or(date)
        }
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    match (next, NaiveDate::from_ymd_opt(year, month, 1)) {
        (Some(n), Some(f)) => n.signed_duration_since(f).num_days() as u32,
        _ => 30,
    }
}

// ── Bill commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_bills(state: State<DbState>) -> Result<Vec<Bill>> {
    let conn = state.0.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, category, amount, frequency, next_due_date, notes, is_active, created_at
         FROM bills ORDER BY next_due_date ASC"
    )?;
    let rows = stmt.query_map([], |r| Ok(Bill {
        id: r.get(0)?, name: r.get(1)?, category: r.get(2)?, amount: r.get(3)?,
        frequency: r.get(4)?, next_due_date: r.get(5)?, notes: r.get(6)?,
        is_active: r.get(7)?, created_at: r.get(8)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_bill(payload: BillPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO bills (name, category, amount, frequency, next_due_date, notes)
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![payload.name, payload.category, payload.amount,
            payload.frequency, payload.next_due_date, payload.notes],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_bill(id: i64, payload: BillPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "UPDATE bills SET name=?1, category=?2, amount=?3, frequency=?4,
         next_due_date=?5, notes=?6, updated_at=datetime('now') WHERE id=?7",
        rusqlite::params![payload.name, payload.category, payload.amount,
            payload.frequency, payload.next_due_date, payload.notes, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_bill(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute("DELETE FROM bills WHERE id=?1", [id])?;
    Ok(())
}

// ── Aggregate reminders ───────────────────────────────────────────────────────

#[tauri::command]
pub fn get_upcoming_reminders(days: i32, state: State<DbState>) -> Result<Vec<UpcomingReminder>> {
    upcoming_reminders(&state.0, days)
}

/// Plain-Rust entry point (no Tauri invoke context) so the background
/// reminder scheduler can call this directly from a tokio task.
pub fn upcoming_reminders(pool: &DbPool, days: i32) -> Result<Vec<UpcomingReminder>> {
    let conn = pool.get()?;
    let today = Local::now().date_naive();
    let mut reminders: Vec<UpcomingReminder> = vec![];

    // Custom bills
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, amount, next_due_date, category FROM bills WHERE is_active=1"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?,
                r.get::<_, String>(3)?, r.get::<_, String>(4)?))
        })?;
        for row in rows.flatten() {
            let (id, name, amount, due_str, category) = row;
            if let Ok(due) = NaiveDate::parse_from_str(&due_str, "%Y-%m-%d") {
                let diff = (due - today).num_days();
                if diff <= days as i64 {
                    reminders.push(UpcomingReminder {
                        source: "bill".into(), source_id: id, name, amount,
                        due_date: due_str, category, days_until_due: diff,
                    });
                }
            }
        }
    }

    // Loans — use next_emi_date directly
    {
        let mut stmt = conn.prepare(
            "SELECT id, lender_name, emi_amount, next_emi_date FROM loans WHERE next_emi_date IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?,
                r.get::<_, String>(3)?))
        })?;
        for row in rows.flatten() {
            let (id, lender, emi, due_str) = row;
            if let Ok(due) = NaiveDate::parse_from_str(&due_str, "%Y-%m-%d") {
                let diff = (due - today).num_days();
                if diff <= days as i64 {
                    reminders.push(UpcomingReminder {
                        source: "loan".into(), source_id: id,
                        name: format!("{} EMI", lender), amount: emi,
                        due_date: due_str, category: "EMI".into(), days_until_due: diff,
                    });
                }
            }
        }
    }

    // Credit cards — due_date is stored as day-of-month integer
    {
        let mut stmt = conn.prepare(
            "SELECT id, bank_name, card_name, min_payment, due_date FROM credit_cards WHERE due_date IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<f64>>(3)?, r.get::<_, Option<i64>>(4)?))
        })?;
        for row in rows.flatten() {
            let (id, bank, card_name, min_pay, due_day_opt) = row;
            let Some(due_day) = due_day_opt else { continue };
            if due_day < 1 || due_day > 31 { continue; }
            let due_day = due_day as u32;

            // Compute next occurrence of this day-of-month
            let this_month_due = NaiveDate::from_ymd_opt(today.year(), today.month(), due_day.min(days_in_month(today.year(), today.month())));
            let due = if let Some(d) = this_month_due {
                if d >= today { d }
                else {
                    let (ny, nm) = if today.month() == 12 { (today.year() + 1, 1) } else { (today.year(), today.month() + 1) };
                    let clamped = due_day.min(days_in_month(ny, nm));
                    NaiveDate::from_ymd_opt(ny, nm, clamped).unwrap_or(d)
                }
            } else { continue };

            let diff = (due - today).num_days();
            if diff <= days as i64 {
                let label = card_name.map(|c| format!("{} {}", bank, c)).unwrap_or(bank);
                reminders.push(UpcomingReminder {
                    source: "credit_card".into(), source_id: id,
                    name: format!("{} Payment", label), amount: min_pay.unwrap_or(0.0),
                    due_date: due.format("%Y-%m-%d").to_string(),
                    category: "Credit Card".into(), days_until_due: diff,
                });
            }
        }
    }

    reminders.sort_by(|a, b| a.due_date.cmp(&b.due_date));
    Ok(reminders)
}

#[tauri::command]
pub fn mark_reminder_paid(
    source: String,
    source_id: i64,
    amount: f64,
    date: String,
    notes: Option<String>,
    state: State<DbState>,
) -> Result<()> {
    let mut conn = state.0.get()?;

    match source.as_str() {
        "loan" => {
            let lender: String = conn.query_row(
                "SELECT lender_name FROM loans WHERE id=?1", [source_id], |r| r.get(0),
            ).map_err(|_| AppError::NotFound("loan".into()))?;
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO transactions (date, type, category, description, amount, notes)
                 VALUES (?1,'emi','EMI',?2,?3,?4)",
                rusqlite::params![date, lender, amount, notes],
            )?;
            tx.execute(
                "UPDATE loans SET next_emi_date = date(next_emi_date, '+1 month') WHERE id=?1",
                [source_id],
            )?;
            tx.commit()?;
        }
        "credit_card" => {
            let (bank, card): (String, Option<String>) = conn.query_row(
                "SELECT bank_name, card_name FROM credit_cards WHERE id=?1", [source_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).map_err(|_| AppError::NotFound("credit card".into()))?;
            let label = card.map(|c| format!("{} {}", bank, c)).unwrap_or(bank);
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO transactions (date, type, category, description, amount, notes)
                 VALUES (?1,'expense','Credit Card',?2,?3,?4)",
                rusqlite::params![date, label, amount, notes],
            )?;
            tx.commit()?;
        }
        "bill" => {
            let (name, freq, category, current_due): (String, String, String, String) = conn.query_row(
                "SELECT name, frequency, category, next_due_date FROM bills WHERE id=?1",
                [source_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            ).map_err(|_| AppError::NotFound("bill".into()))?;
            let next_due = if freq != "one_time" {
                NaiveDate::parse_from_str(&current_due, "%Y-%m-%d")
                    .ok()
                    .map(|d| advance_date(d, &freq).format("%Y-%m-%d").to_string())
            } else {
                None
            };
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO transactions (date, type, category, description, amount, notes)
                 VALUES (?1,'expense',?2,?3,?4,?5)",
                rusqlite::params![date, category, name, amount, notes],
            )?;
            if let Some(next) = next_due {
                tx.execute(
                    "UPDATE bills SET next_due_date=?1, updated_at=datetime('now') WHERE id=?2",
                    rusqlite::params![next, source_id],
                )?;
            }
            tx.commit()?;
        }
        _ => return Err(AppError::Database("unknown reminder source".into())),
    }
    Ok(())
}

// ── Recurring transaction commands ────────────────────────────────────────────

#[tauri::command]
pub fn list_recurring(state: State<DbState>) -> Result<Vec<RecurringTx>> {
    let conn = state.0.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, type, amount, category, asset_class, description, notes,
                frequency, next_due_date, last_run_date, is_active, created_at
         FROM recurring_transactions ORDER BY next_due_date ASC"
    )?;
    let rows = stmt.query_map([], |r| Ok(RecurringTx {
        id: r.get(0)?, name: r.get(1)?, type_: r.get(2)?, amount: r.get(3)?,
        category: r.get(4)?, asset_class: r.get(5)?, description: r.get(6)?,
        notes: r.get(7)?, frequency: r.get(8)?, next_due_date: r.get(9)?,
        last_run_date: r.get(10)?, is_active: r.get(11)?, created_at: r.get(12)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_recurring(payload: RecurringTxPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO recurring_transactions
         (name, type, amount, category, asset_class, description, notes, frequency, next_due_date)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![payload.name, payload.type_, payload.amount, payload.category,
            payload.asset_class, payload.description, payload.notes,
            payload.frequency, payload.next_due_date],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_recurring(id: i64, payload: RecurringTxPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "UPDATE recurring_transactions SET name=?1, type=?2, amount=?3, category=?4,
         asset_class=?5, description=?6, notes=?7, frequency=?8, next_due_date=?9,
         updated_at=datetime('now') WHERE id=?10",
        rusqlite::params![payload.name, payload.type_, payload.amount, payload.category,
            payload.asset_class, payload.description, payload.notes,
            payload.frequency, payload.next_due_date, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_recurring(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute("DELETE FROM recurring_transactions WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn toggle_recurring(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute(
        "UPDATE recurring_transactions SET is_active = 1 - is_active, updated_at=datetime('now') WHERE id=?1",
        [id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_due_recurring(state: State<DbState>) -> Result<Vec<RecurringTx>> {
    let conn = state.0.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, type, amount, category, asset_class, description, notes,
                frequency, next_due_date, last_run_date, is_active, created_at
         FROM recurring_transactions
         WHERE next_due_date <= date('now') AND is_active=1
         ORDER BY next_due_date ASC"
    )?;
    let rows = stmt.query_map([], |r| Ok(RecurringTx {
        id: r.get(0)?, name: r.get(1)?, type_: r.get(2)?, amount: r.get(3)?,
        category: r.get(4)?, asset_class: r.get(5)?, description: r.get(6)?,
        notes: r.get(7)?, frequency: r.get(8)?, next_due_date: r.get(9)?,
        last_run_date: r.get(10)?, is_active: r.get(11)?, created_at: r.get(12)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn apply_recurring(ids: Vec<i64>, state: State<DbState>) -> Result<i32> {
    let conn = state.0.get()?;
    let today = Local::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    let mut applied = 0i32;

    for id in ids {
        let row: std::result::Result<(String, String, f64, String, Option<String>, Option<String>, Option<String>, String, String), _> =
            conn.query_row(
                "SELECT name, type, amount, category, asset_class, description, notes, frequency, next_due_date
                 FROM recurring_transactions WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                         r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
            );
        let Ok((name, type_, amount, category, asset_class, description, notes, frequency, due_str)) = row else { continue };

        conn.execute(
            "INSERT INTO transactions (date, type, category, asset_class, description, amount, notes)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![today_str, type_, category, asset_class,
                description.unwrap_or_else(|| name.clone()), amount, notes],
        )?;

        let next_due = if let Ok(due) = NaiveDate::parse_from_str(&due_str, "%Y-%m-%d") {
            // Advance until future
            let mut next = advance_date(due, &frequency);
            while next <= today { next = advance_date(next, &frequency); }
            next.format("%Y-%m-%d").to_string()
        } else {
            today_str.clone()
        };

        conn.execute(
            "UPDATE recurring_transactions SET next_due_date=?1, last_run_date=?2, updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![next_due, today_str, id],
        )?;
        applied += 1;
    }
    Ok(applied)
}

// ── Milestone commands ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub id: i64,
    pub amount: f64,
    pub label: String,
    pub is_custom: i64,
    pub achieved_at: Option<String>,
    pub created_at: String,
}

fn map_milestone(r: &rusqlite::Row) -> rusqlite::Result<Milestone> {
    Ok(Milestone {
        id: r.get(0)?, amount: r.get(1)?, label: r.get(2)?,
        is_custom: r.get(3)?, achieved_at: r.get(4)?, created_at: r.get(5)?,
    })
}

/// Check net_worth against unachieved milestones. Marks crossed ones as achieved
/// and returns only the newly crossed milestones so the caller can notify.
#[tauri::command]
pub fn check_milestones(net_worth: f64, state: State<DbState>) -> Result<Vec<Milestone>> {
    let conn = state.0.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, amount, label, is_custom, achieved_at, created_at
         FROM milestones WHERE amount <= ?1 AND achieved_at IS NULL ORDER BY amount ASC"
    )?;
    let newly: Vec<Milestone> = stmt.query_map([net_worth], map_milestone)?
        .filter_map(|r| r.ok())
        .collect();
    if !newly.is_empty() {
        conn.execute(
            "UPDATE milestones SET achieved_at = date('now') WHERE amount <= ?1 AND achieved_at IS NULL",
            [net_worth],
        )?;
    }
    Ok(newly)
}

#[tauri::command]
pub fn list_milestones(state: State<DbState>) -> Result<Vec<Milestone>> {
    let conn = state.0.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, amount, label, is_custom, achieved_at, created_at
         FROM milestones ORDER BY amount ASC"
    )?;
    let rows = stmt.query_map([], map_milestone)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_milestone(amount: f64, label: String, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO milestones (amount, label, is_custom) VALUES (?1,?2,1)",
        rusqlite::params![amount, label],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_milestone(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    conn.execute("DELETE FROM milestones WHERE id=?1 AND is_custom=1", [id])?;
    Ok(())
}

// ── Calendar events ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub date: String,        // YYYY-MM-DD
    pub event_type: String,  // "loan" | "credit_card" | "bill" | "goal" | "recurring"
    pub source_id: i64,
    pub name: String,
    pub amount: f64,
    pub category: String,
    pub is_overdue: bool,
}

fn last_day_of_month_date(year: i32, month: u32) -> NaiveDate {
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1).unwrap() - Duration::days(1)
}

/// Returns all financial events (loans, credit cards, bills, goals, recurring) for the given month.
#[tauri::command]
pub fn get_calendar_events(year: i32, month: u32, state: State<DbState>) -> Result<Vec<CalendarEvent>> {
    let conn = state.0.get()?;
    let today = Local::now().date_naive();
    let first_day = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::Database("invalid year/month".into()))?;
    let last_day = last_day_of_month_date(year, month);
    let mut events: Vec<CalendarEvent> = Vec::new();

    // ── Loans: emit if next_emi_date falls in [first_day, last_day] ──────────
    {
        let mut stmt = conn.prepare(
            "SELECT id, lender_name, loan_type, emi_amount, next_emi_date
             FROM loans WHERE next_emi_date IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?, r.get::<_, String>(4)?))
        })?;
        for row in rows.flatten() {
            let (id, lender, loan_type, emi, due_str) = row;
            if let Ok(due) = NaiveDate::parse_from_str(&due_str, "%Y-%m-%d") {
                if due >= first_day && due <= last_day {
                    events.push(CalendarEvent {
                        date: due_str,
                        event_type: "loan".into(),
                        source_id: id,
                        name: format!("{} EMI", lender),
                        amount: emi,
                        category: loan_type,
                        is_overdue: due < today,
                    });
                }
            }
        }
    }

    // ── Credit Cards: compute due date from day-of-month for this month ───────
    {
        let mut stmt = conn.prepare(
            "SELECT id, bank_name, card_name, min_payment, current_balance, due_date
             FROM credit_cards WHERE due_date IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<f64>>(3)?, r.get::<_, f64>(4)?, r.get::<_, Option<i64>>(5)?))
        })?;
        for row in rows.flatten() {
            let (id, bank, card_name, min_pay, balance, due_day_opt) = row;
            let Some(due_day) = due_day_opt else { continue };
            if due_day < 1 || due_day > 31 { continue; }
            let due_day = due_day as u32;
            let clamped = due_day.min(days_in_month(year, month));
            if let Some(due) = NaiveDate::from_ymd_opt(year, month, clamped) {
                let label = card_name.map(|c| format!("{} {}", bank, c)).unwrap_or(bank);
                events.push(CalendarEvent {
                    date: due.format("%Y-%m-%d").to_string(),
                    event_type: "credit_card".into(),
                    source_id: id,
                    name: format!("{} Payment", label),
                    amount: min_pay.unwrap_or(balance),
                    category: "Credit Card".into(),
                    is_overdue: due < today,
                });
            }
        }
    }

    // ── Bills: generate all occurrences of active bills within the month ──────
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, category, amount, frequency, next_due_date
             FROM bills WHERE is_active=1"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?))
        })?;
        for row in rows.flatten() {
            let (id, name, category, amount, frequency, due_str) = row;
            let Ok(mut date) = NaiveDate::parse_from_str(&due_str, "%Y-%m-%d") else { continue };
            // Advance to first occurrence >= first_day
            while date < first_day {
                if frequency == "one_time" { break; }
                date = advance_date(date, &frequency);
            }
            // Emit all occurrences in [first_day, last_day]
            loop {
                if date > last_day { break; }
                if date < first_day { break; } // one_time in past
                events.push(CalendarEvent {
                    date: date.format("%Y-%m-%d").to_string(),
                    event_type: "bill".into(),
                    source_id: id,
                    name: name.clone(),
                    amount,
                    category: category.clone(),
                    is_overdue: date < today,
                });
                if frequency == "one_time" { break; }
                date = advance_date(date, &frequency);
            }
        }
    }

    // ── Goals: emit if target_date falls in [first_day, last_day] ────────────
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, category, target_amount, target_date FROM goals"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?, r.get::<_, String>(4)?))
        })?;
        for row in rows.flatten() {
            let (id, name, category, target_amount, date_str) = row;
            if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                if date >= first_day && date <= last_day {
                    events.push(CalendarEvent {
                        date: date_str,
                        event_type: "goal".into(),
                        source_id: id,
                        name,
                        amount: target_amount,
                        category,
                        is_overdue: date < today,
                    });
                }
            }
        }
    }

    // ── Recurring transactions: same iteration logic as bills ─────────────────
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, type, category, amount, frequency, next_due_date
             FROM recurring_transactions WHERE is_active=1"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                r.get::<_, String>(3)?, r.get::<_, f64>(4)?,
                r.get::<_, String>(5)?, r.get::<_, String>(6)?))
        })?;
        for row in rows.flatten() {
            let (id, name, type_, category, amount, frequency, due_str) = row;
            let Ok(mut date) = NaiveDate::parse_from_str(&due_str, "%Y-%m-%d") else { continue };
            while date < first_day {
                date = advance_date(date, &frequency);
            }
            loop {
                if date > last_day { break; }
                events.push(CalendarEvent {
                    date: date.format("%Y-%m-%d").to_string(),
                    event_type: "recurring".into(),
                    source_id: id,
                    name: name.clone(),
                    amount,
                    category: format!("{} — {}", type_, category),
                    is_overdue: date < today,
                });
                date = advance_date(date, &frequency);
            }
        }
    }

    // ── FD Holdings: emit maturity dates in [first_day, last_day] ────────────
    {
        let mut stmt = conn.prepare(
            "SELECT id, bank_name, principal, maturity_date, maturity_amount
             FROM fd_holdings
             WHERE maturity_date >= ?1 AND maturity_date <= ?2",
        )?;
        for row in stmt.query_map(
            [first_day.format("%Y-%m-%d").to_string(), last_day.format("%Y-%m-%d").to_string()],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?,
                    r.get::<_, String>(3)?, r.get::<_, Option<f64>>(4)?)),
        )?.flatten() {
            let (id, bank_name, principal, maturity_date, maturity_amount) = row;
            if let Ok(date) = NaiveDate::parse_from_str(&maturity_date, "%Y-%m-%d") {
                events.push(CalendarEvent {
                    date: maturity_date,
                    event_type: "fd_maturity".into(),
                    source_id: id,
                    name: format!("{} FD Maturity", bank_name),
                    amount: maturity_amount.unwrap_or(principal),
                    category: "Fixed Deposit".into(),
                    is_overdue: date < today,
                });
            }
        }
    }

    // ── Bond Holdings: emit maturity dates in [first_day, last_day] ──────────
    {
        let mut stmt = conn.prepare(
            "SELECT id, issuer_name, face_value * quantity, maturity_date
             FROM bond_holdings
             WHERE maturity_date IS NOT NULL
               AND maturity_date >= ?1 AND maturity_date <= ?2",
        )?;
        for row in stmt.query_map(
            [first_day.format("%Y-%m-%d").to_string(), last_day.format("%Y-%m-%d").to_string()],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?, r.get::<_, String>(3)?)),
        )?.flatten() {
            let (id, issuer_name, investment, maturity_date) = row;
            if let Ok(date) = NaiveDate::parse_from_str(&maturity_date, "%Y-%m-%d") {
                events.push(CalendarEvent {
                    date: maturity_date,
                    event_type: "bond_maturity".into(),
                    source_id: id,
                    name: format!("{} Bond Maturity", issuer_name),
                    amount: investment,
                    category: "Bond".into(),
                    is_overdue: date < today,
                });
            }
        }
    }

    events.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(events)
}

#[tauri::command]
pub fn get_maturity_alerts(days: i64, state: State<DbState>) -> Result<Vec<MaturityAlert>> {
    maturity_alerts(&state.0, days)
}

/// Plain-Rust entry point (no Tauri invoke context) so the background
/// reminder scheduler can call this directly from a tokio task.
pub fn maturity_alerts(pool: &DbPool, days: i64) -> Result<Vec<MaturityAlert>> {
    let conn = pool.get()?;
    let mut alerts: Vec<MaturityAlert> = vec![];

    // FDs — maturity_date NOT NULL (indexed via idx_fd_maturity)
    let mut stmt = conn.prepare(
        "SELECT id, bank_name, principal, maturity_date, maturity_amount
         FROM fd_holdings
         WHERE julianday(maturity_date) - julianday('now') BETWEEN -30 AND ?1
         ORDER BY maturity_date ASC",
    )?;
    for row in stmt.query_map([days], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?,
            r.get::<_, String>(3)?, r.get::<_, Option<f64>>(4)?))
    })?.flatten() {
        let (id, name, principal, maturity_date, maturity_amount) = row;
        let days_until = NaiveDate::parse_from_str(&maturity_date, "%Y-%m-%d")
            .map(|d| (d - Local::now().date_naive()).num_days())
            .unwrap_or(0);
        alerts.push(MaturityAlert {
            source: "fd".into(), source_id: id, name, principal,
            maturity_date, maturity_amount, days_until_maturity: days_until,
        });
    }

    // Bonds — maturity_date nullable (perpetual bonds have NULL)
    let mut stmt = conn.prepare(
        "SELECT id, issuer_name, face_value * quantity, maturity_date
         FROM bond_holdings
         WHERE maturity_date IS NOT NULL
           AND julianday(maturity_date) - julianday('now') BETWEEN -30 AND ?1
         ORDER BY maturity_date ASC",
    )?;
    for row in stmt.query_map([days], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?, r.get::<_, String>(3)?))
    })?.flatten() {
        let (id, name, principal, maturity_date) = row;
        let days_until = NaiveDate::parse_from_str(&maturity_date, "%Y-%m-%d")
            .map(|d| (d - Local::now().date_naive()).num_days())
            .unwrap_or(0);
        alerts.push(MaturityAlert {
            source: "bond".into(), source_id: id, name, principal,
            maturity_date, maturity_amount: None, days_until_maturity: days_until,
        });
    }

    alerts.sort_by(|a, b| a.maturity_date.cmp(&b.maturity_date));
    Ok(alerts)
}
