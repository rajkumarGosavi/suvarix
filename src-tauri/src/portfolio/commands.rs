use tauri::State;
use serde::{Deserialize, Serialize};
use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::{equity::*, mf::*, fd::*, ppf_epf::*, real_estate::*, gold::*, crypto::*, insurance::*};
use super::calculator::{self, AllocationItem, NetWorthSummary};

// ─── NET WORTH & ALLOCATION ──────────────────────────────────────────────────

#[tauri::command]
pub fn get_net_worth(state: State<DbState>) -> Result<NetWorthSummary> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    calculator::calc_net_worth(&conn)
}

#[tauri::command]
pub fn get_allocation_breakdown(state: State<DbState>) -> Result<Vec<AllocationItem>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    calculator::calc_allocation(&conn)
}

// ─── EQUITY ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_equity(state: State<DbState>) -> Result<Vec<EquityHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, isin, symbol, exchange, name, quantity, avg_buy_price,
                current_price, price_updated_at, created_at, updated_at
         FROM equity_holdings ORDER BY name"
    )?;
    let rows = stmt.query_map([], |r| Ok(EquityHolding {
        id: r.get(0)?, account_id: r.get(1)?, isin: r.get(2)?, symbol: r.get(3)?,
        exchange: r.get(4)?, name: r.get(5)?, quantity: r.get(6)?, avg_buy_price: r.get(7)?,
        current_price: r.get(8)?, price_updated_at: r.get(9)?,
        created_at: r.get(10)?, updated_at: r.get(11)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_equity(payload: AddEquityPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO equity_holdings (account_id, isin, symbol, exchange, name, quantity, avg_buy_price)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![payload.account_id, payload.isin, payload.symbol, payload.exchange,
            payload.name, payload.quantity, payload.avg_buy_price],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_equity(id: i64, payload: AddEquityPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE equity_holdings SET symbol=?1, exchange=?2, name=?3, quantity=?4,
         avg_buy_price=?5, updated_at=datetime('now') WHERE id=?6",
        rusqlite::params![payload.symbol, payload.exchange, payload.name,
            payload.quantity, payload.avg_buy_price, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_equity(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM equity_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── MUTUAL FUNDS ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_mf(state: State<DbState>) -> Result<Vec<MfHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, scheme_code, scheme_name, amc_name, folio_number,
                units, avg_nav, current_nav, nav_date, is_direct, is_growth, created_at, updated_at
         FROM mf_holdings ORDER BY scheme_name"
    )?;
    let rows = stmt.query_map([], |r| Ok(MfHolding {
        id: r.get(0)?, account_id: r.get(1)?, scheme_code: r.get(2)?, scheme_name: r.get(3)?,
        amc_name: r.get(4)?, folio_number: r.get(5)?, units: r.get(6)?, avg_nav: r.get(7)?,
        current_nav: r.get(8)?, nav_date: r.get(9)?,
        is_direct: r.get::<_, i64>(10)? != 0, is_growth: r.get::<_, i64>(11)? != 0,
        created_at: r.get(12)?, updated_at: r.get(13)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_mf(payload: AddMfPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO mf_holdings (account_id, scheme_code, scheme_name, amc_name, folio_number,
         units, avg_nav, is_direct, is_growth) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![payload.account_id, payload.scheme_code, payload.scheme_name,
            payload.amc_name, payload.folio_number, payload.units, payload.avg_nav,
            payload.is_direct as i64, payload.is_growth as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_mf(id: i64, payload: AddMfPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE mf_holdings SET units=?1, avg_nav=?2, is_direct=?3, is_growth=?4,
         updated_at=datetime('now') WHERE id=?5",
        rusqlite::params![payload.units, payload.avg_nav, payload.is_direct as i64,
            payload.is_growth as i64, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_mf(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM mf_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── FIXED DEPOSITS ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_fd(state: State<DbState>) -> Result<Vec<FdHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, bank_name, account_number, principal, interest_rate,
                compounding, tenure_months, start_date, maturity_date, maturity_amount,
                is_cumulative, created_at FROM fd_holdings ORDER BY maturity_date"
    )?;
    let rows = stmt.query_map([], |r| Ok(FdHolding {
        id: r.get(0)?, account_id: r.get(1)?, bank_name: r.get(2)?, account_number: r.get(3)?,
        principal: r.get(4)?, interest_rate: r.get(5)?, compounding: r.get(6)?,
        tenure_months: r.get(7)?, start_date: r.get(8)?, maturity_date: r.get(9)?,
        maturity_amount: r.get(10)?, is_cumulative: r.get::<_, i64>(11)? != 0,
        created_at: r.get(12)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_fd(payload: AddFdPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO fd_holdings (account_id, bank_name, account_number, principal, interest_rate,
         compounding, tenure_months, start_date, maturity_date, maturity_amount, is_cumulative)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        rusqlite::params![payload.account_id, payload.bank_name, payload.account_number,
            payload.principal, payload.interest_rate, payload.compounding, payload.tenure_months,
            payload.start_date, payload.maturity_date, payload.maturity_amount,
            payload.is_cumulative as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_fd(id: i64, payload: AddFdPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE fd_holdings SET bank_name=?1, principal=?2, interest_rate=?3,
         tenure_months=?4, maturity_date=?5, maturity_amount=?6 WHERE id=?7",
        rusqlite::params![payload.bank_name, payload.principal, payload.interest_rate,
            payload.tenure_months, payload.maturity_date, payload.maturity_amount, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_fd(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM fd_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── PPF / EPF ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_ppf_epf(state: State<DbState>) -> Result<Vec<PpfEpfHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_type, account_number, balance, interest_rate,
                financial_year, employer_contrib, employee_contrib, updated_at
         FROM ppf_epf_holdings ORDER BY account_type"
    )?;
    let rows = stmt.query_map([], |r| Ok(PpfEpfHolding {
        id: r.get(0)?, account_type: r.get(1)?, account_number: r.get(2)?,
        balance: r.get(3)?, interest_rate: r.get(4)?, financial_year: r.get(5)?,
        employer_contrib: r.get(6)?, employee_contrib: r.get(7)?, updated_at: r.get(8)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_ppf_epf(payload: AddPpfEpfPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO ppf_epf_holdings (account_type, account_number, balance, interest_rate,
         financial_year, employer_contrib, employee_contrib)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![payload.account_type, payload.account_number, payload.balance,
            payload.interest_rate, payload.financial_year, payload.employer_contrib,
            payload.employee_contrib],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_ppf_epf(id: i64, payload: AddPpfEpfPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE ppf_epf_holdings SET balance=?1, interest_rate=?2, employer_contrib=?3,
         employee_contrib=?4, updated_at=datetime('now') WHERE id=?5",
        rusqlite::params![payload.balance, payload.interest_rate,
            payload.employer_contrib, payload.employee_contrib, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_ppf_epf(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM ppf_epf_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── REAL ESTATE ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_real_estate(state: State<DbState>) -> Result<Vec<RealEstateHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, property_name, property_type, location, purchase_price, purchase_date,
                current_value, rental_income, has_mortgage, created_at
         FROM real_estate_holdings ORDER BY property_name"
    )?;
    let rows = stmt.query_map([], |r| Ok(RealEstateHolding {
        id: r.get(0)?, property_name: r.get(1)?, property_type: r.get(2)?,
        location: r.get(3)?, purchase_price: r.get(4)?, purchase_date: r.get(5)?,
        current_value: r.get(6)?, rental_income: r.get(7)?,
        has_mortgage: r.get::<_, i64>(8)? != 0, created_at: r.get(9)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_real_estate(payload: AddRealEstatePayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO real_estate_holdings (property_name, property_type, location, purchase_price,
         purchase_date, current_value, rental_income, has_mortgage)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![payload.property_name, payload.property_type, payload.location,
            payload.purchase_price, payload.purchase_date, payload.current_value,
            payload.rental_income, payload.has_mortgage as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_real_estate(id: i64, payload: AddRealEstatePayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE real_estate_holdings SET property_name=?1, current_value=?2,
         rental_income=?3, has_mortgage=?4 WHERE id=?5",
        rusqlite::params![payload.property_name, payload.current_value,
            payload.rental_income, payload.has_mortgage as i64, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_real_estate(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM real_estate_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── GOLD ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_gold(state: State<DbState>) -> Result<Vec<GoldHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, gold_type, name, weight_grams, purity, units, avg_buy_price,
                current_price, account_id, maturity_date, created_at
         FROM gold_holdings ORDER BY gold_type"
    )?;
    let rows = stmt.query_map([], |r| Ok(GoldHolding {
        id: r.get(0)?, gold_type: r.get(1)?, name: r.get(2)?, weight_grams: r.get(3)?,
        purity: r.get(4)?, units: r.get(5)?, avg_buy_price: r.get(6)?,
        current_price: r.get(7)?, account_id: r.get(8)?, maturity_date: r.get(9)?,
        created_at: r.get(10)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_gold(payload: AddGoldPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO gold_holdings (gold_type, name, weight_grams, purity, units, avg_buy_price,
         account_id, maturity_date) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![payload.gold_type, payload.name, payload.weight_grams, payload.purity,
            payload.units, payload.avg_buy_price, payload.account_id, payload.maturity_date],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_gold(id: i64, payload: AddGoldPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE gold_holdings SET weight_grams=?1, units=?2, avg_buy_price=?3 WHERE id=?4",
        rusqlite::params![payload.weight_grams, payload.units, payload.avg_buy_price, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_gold(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM gold_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── CRYPTO ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_crypto(state: State<DbState>) -> Result<Vec<CryptoHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, exchange_name, coin_symbol, quantity, avg_buy_price, current_price, created_at
         FROM crypto_holdings ORDER BY coin_symbol"
    )?;
    let rows = stmt.query_map([], |r| Ok(CryptoHolding {
        id: r.get(0)?, account_id: r.get(1)?, exchange_name: r.get(2)?, coin_symbol: r.get(3)?,
        quantity: r.get(4)?, avg_buy_price: r.get(5)?, current_price: r.get(6)?,
        created_at: r.get(7)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_crypto(payload: AddCryptoPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO crypto_holdings (account_id, exchange_name, coin_symbol, quantity, avg_buy_price)
         VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![payload.account_id, payload.exchange_name, payload.coin_symbol,
            payload.quantity, payload.avg_buy_price],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_crypto(id: i64, payload: AddCryptoPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE crypto_holdings SET quantity=?1, avg_buy_price=?2 WHERE id=?3",
        rusqlite::params![payload.quantity, payload.avg_buy_price, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_crypto(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM crypto_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── INSURANCE ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_insurance(state: State<DbState>) -> Result<Vec<InsuranceHolding>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, insurance_type, provider, policy_number, premium_amount, premium_freq,
                coverage_amount, maturity_value, start_date, end_date, next_due_date, created_at
         FROM insurance_holdings ORDER BY provider"
    )?;
    let rows = stmt.query_map([], |r| Ok(InsuranceHolding {
        id: r.get(0)?, insurance_type: r.get(1)?, provider: r.get(2)?, policy_number: r.get(3)?,
        premium_amount: r.get(4)?, premium_freq: r.get(5)?, coverage_amount: r.get(6)?,
        maturity_value: r.get(7)?, start_date: r.get(8)?, end_date: r.get(9)?,
        next_due_date: r.get(10)?, created_at: r.get(11)?,
    }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn add_insurance(payload: AddInsurancePayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO insurance_holdings (insurance_type, provider, policy_number, premium_amount,
         premium_freq, coverage_amount, maturity_value, start_date, end_date, next_due_date)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params![payload.insurance_type, payload.provider, payload.policy_number,
            payload.premium_amount, payload.premium_freq, payload.coverage_amount,
            payload.maturity_value, payload.start_date, payload.end_date, payload.next_due_date],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_insurance(id: i64, payload: AddInsurancePayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE insurance_holdings SET premium_amount=?1, coverage_amount=?2,
         maturity_value=?3, next_due_date=?4 WHERE id=?5",
        rusqlite::params![payload.premium_amount, payload.coverage_amount,
            payload.maturity_value, payload.next_due_date, id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_insurance(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM insurance_holdings WHERE id=?1", [id])?;
    Ok(())
}

// ─── SIP SCHEDULES ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SipSchedule {
    pub id: i64,
    pub mf_holding_id: Option<i64>,
    pub scheme_code: String,
    pub scheme_name: Option<String>,
    pub amount: f64,
    pub frequency: String,
    pub debit_day: i64,
    pub start_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSipPayload {
    pub mf_holding_id: Option<i64>,
    pub scheme_code: String,
    pub amount: f64,
    pub frequency: String,
    pub debit_day: i64,
    pub start_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
}

#[tauri::command]
pub fn list_sip_schedules(state: State<DbState>) -> Result<Vec<SipSchedule>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.mf_holding_id, s.scheme_code, m.scheme_name,
                s.amount, s.frequency, s.debit_day, s.start_date, s.end_date, s.is_active
         FROM sip_schedules s
         LEFT JOIN mf_holdings m ON s.mf_holding_id = m.id
         ORDER BY s.is_active DESC, s.debit_day ASC"
    )?;
    let rows: Vec<_> = stmt.query_map([], |r| Ok(SipSchedule {
        id: r.get(0)?,
        mf_holding_id: r.get(1)?,
        scheme_code: r.get(2)?,
        scheme_name: r.get(3)?,
        amount: r.get(4)?,
        frequency: r.get(5)?,
        debit_day: r.get(6)?,
        start_date: r.get(7)?,
        end_date: r.get(8)?,
        is_active: r.get::<_, i64>(9)? != 0,
    }))?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}

#[tauri::command]
pub fn add_sip_schedule(payload: AddSipPayload, state: State<DbState>) -> Result<i64> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO sip_schedules (mf_holding_id, scheme_code, amount, frequency,
         debit_day, start_date, end_date, is_active)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            payload.mf_holding_id, payload.scheme_code, payload.amount,
            payload.frequency, payload.debit_day, payload.start_date,
            payload.end_date, payload.is_active as i64
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_sip_schedule(id: i64, payload: AddSipPayload, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "UPDATE sip_schedules SET mf_holding_id=?1, scheme_code=?2, amount=?3,
         frequency=?4, debit_day=?5, start_date=?6, end_date=?7, is_active=?8
         WHERE id=?9",
        rusqlite::params![
            payload.mf_holding_id, payload.scheme_code, payload.amount,
            payload.frequency, payload.debit_day, payload.start_date,
            payload.end_date, payload.is_active as i64, id
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_sip_schedule(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM sip_schedules WHERE id=?1", [id])?;
    Ok(())
}
