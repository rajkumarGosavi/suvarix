use tauri::State;
use serde::{Deserialize, Serialize};
use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::loan::{AddCreditCardPayload, AddLoanPayload, CreditCard, Loan};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmiRow {
    pub month: i64,
    pub payment: f64,
    pub principal: f64,
    pub interest: f64,
    pub balance: f64,
}

#[tauri::command]
pub fn list_loans(state: State<DbState>) -> Result<Vec<Loan>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, loan_type, lender_name, account_number, principal, outstanding,
                interest_rate, emi_amount, tenure_months, disbursement_date, next_emi_date,
                created_at, updated_at FROM loans ORDER BY loan_type"
    )?;
    let rows = stmt.query_map([], |r| Ok(Loan {
        id: r.get(0)?, loan_type: r.get(1)?, lender_name: r.get(2)?, account_number: r.get(3)?,
        principal: r.get(4)?, outstanding: r.get(5)?, interest_rate: r.get(6)?,
        emi_amount: r.get(7)?, tenure_months: r.get(8)?, disbursement_date: r.get(9)?,
        next_emi_date: r.get(10)?, created_at: r.get(11)?, updated_at: r.get(12)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_loan(payload: AddLoanPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO loans (loan_type, lender_name, account_number, principal, outstanding,
         interest_rate, emi_amount, tenure_months, disbursement_date, next_emi_date)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params![payload.loan_type, payload.lender_name, payload.account_number,
            payload.principal, payload.outstanding, payload.interest_rate, payload.emi_amount,
            payload.tenure_months, payload.disbursement_date, payload.next_emi_date],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_loan(id: i64, payload: AddLoanPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE loans SET loan_type=?1, lender_name=?2, account_number=?3, principal=?4,
         outstanding=?5, interest_rate=?6, emi_amount=?7, tenure_months=?8,
         disbursement_date=?9, next_emi_date=?10, updated_at=datetime('now') WHERE id=?11",
        rusqlite::params![
            payload.loan_type, payload.lender_name, payload.account_number,
            payload.principal, payload.outstanding, payload.interest_rate,
            payload.emi_amount, payload.tenure_months,
            payload.disbursement_date, payload.next_emi_date, id
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_loan(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM loans WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn get_amortization_schedule(loan_id: i64, state: State<DbState>) -> Result<Vec<EmiRow>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let loan: Loan = conn.query_row(
        "SELECT id, loan_type, lender_name, account_number, principal, outstanding,
                interest_rate, emi_amount, tenure_months, disbursement_date, next_emi_date,
                created_at, updated_at FROM loans WHERE id=?1",
        [loan_id],
        |r| Ok(Loan {
            id: r.get(0)?, loan_type: r.get(1)?, lender_name: r.get(2)?, account_number: r.get(3)?,
            principal: r.get(4)?, outstanding: r.get(5)?, interest_rate: r.get(6)?,
            emi_amount: r.get(7)?, tenure_months: r.get(8)?, disbursement_date: r.get(9)?,
            next_emi_date: r.get(10)?, created_at: r.get(11)?, updated_at: r.get(12)?,
        }),
    ).map_err(|_| AppError::NotFound("loan".into()))?;

    let monthly_rate = loan.interest_rate / 100.0 / 12.0;
    let mut balance = loan.outstanding;
    let mut schedule = vec![];

    for month in 1..=loan.tenure_months {
        let interest = balance * monthly_rate;
        let principal = (loan.emi_amount - interest).min(balance);
        balance -= principal;
        if balance < 0.0 { balance = 0.0; }
        schedule.push(EmiRow { month, payment: loan.emi_amount, principal, interest, balance });
        if balance == 0.0 { break; }
    }
    Ok(schedule)
}

#[tauri::command]
pub fn list_credit_cards(state: State<DbState>) -> Result<Vec<CreditCard>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, bank_name, card_name, last_four, credit_limit, current_balance,
                due_date, min_payment, updated_at FROM credit_cards ORDER BY bank_name"
    )?;
    let rows = stmt.query_map([], |r| Ok(CreditCard {
        id: r.get(0)?, bank_name: r.get(1)?, card_name: r.get(2)?, last_four: r.get(3)?,
        credit_limit: r.get(4)?, current_balance: r.get(5)?, due_date: r.get(6)?,
        min_payment: r.get(7)?, updated_at: r.get(8)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_credit_card(payload: AddCreditCardPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO credit_cards (bank_name, card_name, last_four, credit_limit,
         current_balance, due_date, min_payment) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![payload.bank_name, payload.card_name, payload.last_four,
            payload.credit_limit, payload.current_balance, payload.due_date, payload.min_payment],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_credit_card(id: i64, payload: AddCreditCardPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE credit_cards SET bank_name=?1, card_name=?2, last_four=?3,
         credit_limit=?4, current_balance=?5, due_date=?6, min_payment=?7,
         updated_at=datetime('now') WHERE id=?8",
        rusqlite::params![
            payload.bank_name, payload.card_name, payload.last_four,
            payload.credit_limit, payload.current_balance,
            payload.due_date, payload.min_payment, id
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_credit_card(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM credit_cards WHERE id=?1", [id])?;
    Ok(())
}
