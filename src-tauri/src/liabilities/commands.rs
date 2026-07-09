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
    let conn = state.0.get()?;
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
    let conn = state.0.get()?;
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
    let conn = state.0.get()?;
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
    let conn = state.0.get()?;
    conn.execute("DELETE FROM loans WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn get_amortization_schedule(loan_id: i64, state: State<DbState>) -> Result<Vec<EmiRow>> {
    let conn = state.0.get()?;
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

    Ok(amortization_schedule(loan.outstanding, loan.interest_rate, loan.emi_amount, loan.tenure_months))
}

fn amortization_schedule(outstanding: f64, annual_rate_pct: f64, emi: f64, tenure_months: i64) -> Vec<EmiRow> {
    let monthly_rate = annual_rate_pct / 100.0 / 12.0;
    let mut balance = outstanding;
    let mut schedule = vec![];

    for month in 1..=tenure_months {
        let interest = balance * monthly_rate;
        let principal = (emi - interest).min(balance);
        balance -= principal;
        if balance < 0.0 { balance = 0.0; }
        schedule.push(EmiRow { month, payment: emi, principal, interest, balance });
        if balance == 0.0 { break; }
    }
    schedule
}

#[tauri::command]
pub fn list_credit_cards(state: State<DbState>) -> Result<Vec<CreditCard>> {
    let conn = state.0.get()?;
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
    let conn = state.0.get()?;
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
    let conn = state.0.get()?;
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
    let conn = state.0.get()?;
    conn.execute("DELETE FROM credit_cards WHERE id=?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_month_interest_is_monthly_rate_on_outstanding() {
        // ₹1,20,000 @ 12% p.a. → 1%/month → ₹1,200 interest in month 1
        let schedule = amortization_schedule(120_000.0, 12.0, 5_000.0, 36);
        let first = &schedule[0];
        assert!((first.interest - 1_200.0).abs() < 1e-9);
        assert!((first.principal - 3_800.0).abs() < 1e-9);
        assert!((first.balance - 116_200.0).abs() < 1e-9);
    }

    #[test]
    fn principal_plus_interest_equals_emi_until_final_month() {
        let schedule = amortization_schedule(100_000.0, 10.0, 8_800.0, 12);
        for row in &schedule[..schedule.len() - 1] {
            assert!(
                (row.principal + row.interest - 8_800.0).abs() < 1e-9,
                "month {} split doesn't add up to EMI",
                row.month
            );
        }
    }

    #[test]
    fn balance_reaches_zero_and_schedule_stops_early_on_payoff() {
        // Zero-interest loan: 1000 outstanding, 100 EMI → paid off in 10 of 24 months
        let schedule = amortization_schedule(1_000.0, 0.0, 100.0, 24);
        assert_eq!(schedule.len(), 10);
        assert_eq!(schedule.last().unwrap().balance, 0.0);
        assert_eq!(schedule.last().unwrap().month, 10);
    }

    #[test]
    fn final_month_principal_clamped_to_remaining_balance() {
        // 250 outstanding, 100 EMI, no interest → last row pays only 50
        let schedule = amortization_schedule(250.0, 0.0, 100.0, 12);
        assert_eq!(schedule.len(), 3);
        assert!((schedule[2].principal - 50.0).abs() < 1e-9);
        assert_eq!(schedule[2].balance, 0.0);
    }

    #[test]
    fn emi_below_interest_never_amortizes_but_schedule_is_bounded() {
        // EMI smaller than monthly interest → balance grows; loop must stop at tenure
        let schedule = amortization_schedule(1_000_000.0, 24.0, 1_000.0, 12);
        assert_eq!(schedule.len(), 12);
        assert!(schedule.last().unwrap().balance > 1_000_000.0);
    }
}
