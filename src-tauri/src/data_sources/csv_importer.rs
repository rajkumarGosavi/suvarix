use csv::ReaderBuilder;
use rusqlite::Connection;
use crate::error::{AppError, Result};
use crate::models::common::ImportResult;
use super::commands::BrokerCsvRow;

// ── Shared helpers ────────────────────────────────────────────────────────────

fn normalize_header(h: &str) -> String {
    let mut out = String::with_capacity(h.len());
    let mut prev_under = false;
    for c in h.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
            prev_under = false;
        } else if !prev_under {
            out.push('_');
            prev_under = true;
        }
    }
    out.trim_end_matches('_').to_string()
}

fn find_col(headers: &[String], aliases: &[&str]) -> Option<usize> {
    aliases.iter().find_map(|a| headers.iter().position(|h| h == a))
}

fn cell(record: &csv::StringRecord, idx: usize) -> &str {
    record.get(idx).unwrap_or("").trim()
}

fn cell_opt(record: &csv::StringRecord, idx: Option<usize>) -> Option<String> {
    idx.and_then(|i| {
        let s = record.get(i).unwrap_or("").trim();
        if s.is_empty() { None } else { Some(s.to_string()) }
    })
}

fn parse_f64(s: &str) -> f64 {
    s.replace(',', "").parse::<f64>().unwrap_or(0.0)
}

fn parse_f64_opt(s: &str) -> Option<f64> {
    let v = s.replace(',', "").parse::<f64>().ok()?;
    if v == 0.0 { None } else { Some(v) }
}

fn parse_bool(s: &str) -> bool {
    matches!(s.to_lowercase().trim(), "true" | "yes" | "1" | "direct" | "growth")
}

fn parse_bool_int(s: &str) -> i64 {
    if parse_bool(s) { 1 } else { 0 }
}

// ── 1a. Equity CSV parser ─────────────────────────────────────────────────────

pub fn parse_equity_csv(broker: &str, content: &str) -> Result<Vec<BrokerCsvRow>> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    type Aliases = (&'static [&'static str], &'static [&'static str], &'static [&'static str], &'static [&'static str], &'static [&'static str], &'static [&'static str]);
    let (symbol_al, qty_al, avg_al, ltp_al, exch_al, isin_al): Aliases = match broker {
        "zerodha" => (
            &["instrument", "symbol", "scrip", "stock_symbol"],
            &["qty", "quantity", "net_qty"],
            &["avg_cost", "avg_price", "average_price", "buy_price", "average_cost"],
            &["ltp", "last_price", "cmp", "close_price"],
            &["exchange", "exch", "segment"],
            &["isin"],
        ),
        "upstox" => (
            &["instrument_name", "trading_symbol", "symbol", "scrip_name", "stock"],
            &["quantity", "net_qty", "qty"],
            &["avg_cost_price", "avg__cost_price", "buy_avg_price", "average_price", "avg_price"],
            &["ltp", "current_price", "last_price", "close_price"],
            &["exchange", "exch"],
            &["isin"],
        ),
        "angel_one" => (
            &["symbol", "instrument", "stock"],
            &["net_qty", "qty", "quantity"],
            &["avg_buy_price", "average_price", "avg_price"],
            &["ltp", "close_price"],
            &["exchange"],
            &["isin"],
        ),
        "groww" => (
            &["symbol", "ticker", "scrip"],
            &["qty", "quantity"],
            &["avg_price", "avg_cost", "average_price"],
            &["ltp", "current_price"],
            &["exchange"],
            &["isin"],
        ),
        _ => (
            &["symbol", "instrument", "scrip"],
            &["qty", "quantity", "net_qty"],
            &["avg_price", "avg_cost", "average_price", "avg_buy_price"],
            &["ltp", "last_price", "current_price"],
            &["exchange", "exch"],
            &["isin"],
        ),
    };

    let symbol_idx = find_col(&headers, symbol_al)
        .ok_or_else(|| AppError::Parse("Symbol column not found in CSV".into()))?;
    let qty_idx = find_col(&headers, qty_al)
        .ok_or_else(|| AppError::Parse("Quantity column not found in CSV".into()))?;
    let avg_idx = find_col(&headers, avg_al)
        .ok_or_else(|| AppError::Parse("Average price column not found in CSV".into()))?;
    let ltp_idx = find_col(&headers, ltp_al);
    let exch_idx = find_col(&headers, exch_al);
    let isin_idx = find_col(&headers, isin_al);

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;
        let symbol = cell(&record, symbol_idx).to_string();
        let quantity = parse_f64(cell(&record, qty_idx));
        let avg_price = parse_f64(cell(&record, avg_idx));
        let ltp = ltp_idx.and_then(|i| parse_f64_opt(cell(&record, i)));
        let exchange = cell_opt(&record, exch_idx);
        let isin = cell_opt(&record, isin_idx);

        if !symbol.is_empty() && quantity > 0.0 && avg_price > 0.0 {
            rows.push(BrokerCsvRow { symbol, isin, quantity, avg_price, ltp, exchange });
        }
    }
    Ok(rows)
}

// ── 1b. MF CSV parser ─────────────────────────────────────────────────────────

pub struct MfCsvRow {
    pub scheme_name: String,
    pub isin: String,
    pub folio_number: String,
    pub units: f64,
    pub avg_nav: f64,
    pub current_nav: Option<f64>,
    pub is_direct: bool,
    pub is_growth: bool,
    pub amc_name: String,
}

pub fn parse_mf_csv(content: &str) -> Result<Vec<MfCsvRow>> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    let scheme_idx = find_col(&headers, &["scheme_name", "fund_name", "scheme"])
        .ok_or_else(|| AppError::Parse("scheme_name column not found".into()))?;
    let isin_idx = find_col(&headers, &["isin"])
        .ok_or_else(|| AppError::Parse("isin column not found".into()))?;
    let folio_idx = find_col(&headers, &["folio_number", "folio"])
        .ok_or_else(|| AppError::Parse("folio_number column not found".into()))?;
    let units_idx = find_col(&headers, &["units", "nav_units"])
        .ok_or_else(|| AppError::Parse("units column not found".into()))?;
    let avg_nav_idx = find_col(&headers, &["avg_nav", "average_nav", "purchase_nav", "cost_nav"])
        .ok_or_else(|| AppError::Parse("avg_nav column not found".into()))?;
    let current_nav_idx = find_col(&headers, &["current_nav", "nav", "lnav"]);
    let is_direct_idx = find_col(&headers, &["is_direct", "direct"]);
    let is_growth_idx = find_col(&headers, &["is_growth", "growth"]);
    let amc_idx = find_col(&headers, &["amc_name", "amc", "fund_house"]);

    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;

        let scheme_name = cell(&record, scheme_idx).to_string();
        let isin = cell(&record, isin_idx).to_string();
        let folio_number = cell(&record, folio_idx).to_string();
        let units = parse_f64(cell(&record, units_idx));
        let avg_nav = parse_f64(cell(&record, avg_nav_idx));

        if scheme_name.is_empty() || isin.is_empty() || folio_number.is_empty()
            || units <= 0.0 || avg_nav <= 0.0
        {
            continue;
        }

        let current_nav = current_nav_idx.and_then(|i| parse_f64_opt(cell(&record, i)));
        let is_direct = is_direct_idx.map(|i| parse_bool(cell(&record, i))).unwrap_or(false);
        let is_growth = is_growth_idx.map(|i| parse_bool(cell(&record, i))).unwrap_or(true);
        let amc_name = cell_opt(&record, amc_idx).unwrap_or_else(|| "Unknown".to_string());

        rows.push(MfCsvRow { scheme_name, isin, folio_number, units, avg_nav, current_nav, is_direct, is_growth, amc_name });
    }
    Ok(rows)
}

// ── 1c. FD CSV importer ───────────────────────────────────────────────────────

pub fn import_fd_from_csv(content: &str, conn: &Connection) -> Result<ImportResult> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    let bank_idx = find_col(&headers, &["bank_name", "bank"])
        .ok_or_else(|| AppError::Parse("bank_name column not found".into()))?;
    let acct_idx = find_col(&headers, &["account_number", "account_no", "acct_number"]);
    let principal_idx = find_col(&headers, &["principal", "deposit_amount", "amount"])
        .ok_or_else(|| AppError::Parse("principal column not found".into()))?;
    let rate_idx = find_col(&headers, &["interest_rate", "rate", "roi"])
        .ok_or_else(|| AppError::Parse("interest_rate column not found".into()))?;
    let compound_idx = find_col(&headers, &["compounding", "compound_frequency"]);
    let tenure_idx = find_col(&headers, &["tenure_months", "tenure", "months"])
        .ok_or_else(|| AppError::Parse("tenure_months column not found".into()))?;
    let start_idx = find_col(&headers, &["start_date", "open_date", "issue_date"])
        .ok_or_else(|| AppError::Parse("start_date column not found".into()))?;
    let maturity_idx = find_col(&headers, &["maturity_date", "maturity"])
        .ok_or_else(|| AppError::Parse("maturity_date column not found".into()))?;
    let mat_amt_idx = find_col(&headers, &["maturity_amount", "maturity_value"]);
    let cumul_idx = find_col(&headers, &["is_cumulative", "cumulative"]);

    let mut imported = 0i64;
    let mut skipped = 0i64;

    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;

        let bank_name = cell(&record, bank_idx).to_string();
        let principal = parse_f64(cell(&record, principal_idx));
        let interest_rate = parse_f64(cell(&record, rate_idx));
        let tenure_months = parse_f64(cell(&record, tenure_idx)) as i64;
        let start_date = cell(&record, start_idx).to_string();
        let maturity_date = cell(&record, maturity_idx).to_string();

        if bank_name.is_empty() || principal <= 0.0 || interest_rate <= 0.0
            || tenure_months <= 0 || start_date.is_empty() || maturity_date.is_empty()
        {
            skipped += 1;
            continue;
        }

        let account_number = cell_opt(&record, acct_idx);
        let compounding = cell_opt(&record, compound_idx)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "quarterly".to_string());
        let maturity_amount = mat_amt_idx.and_then(|i| parse_f64_opt(cell(&record, i)));
        let is_cumulative = cumul_idx
            .map(|i| parse_bool_int(cell(&record, i)))
            .unwrap_or(1);

        let res = conn.execute(
            "INSERT INTO fd_holdings
               (account_id, bank_name, account_number, principal, interest_rate,
                compounding, tenure_months, start_date, maturity_date, maturity_amount, is_cumulative)
             VALUES (NULL,?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                bank_name, account_number, principal, interest_rate,
                compounding, tenure_months, start_date, maturity_date,
                maturity_amount, is_cumulative,
            ],
        );
        match res {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ImportResult { imported, skipped })
}

// ── 1d. Gold CSV importer ─────────────────────────────────────────────────────

const GOLD_TYPES: &[&str] = &["physical", "digital", "etf", "sgb"];

pub fn import_gold_from_csv(content: &str, conn: &Connection) -> Result<ImportResult> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    let type_idx = find_col(&headers, &["gold_type", "type"])
        .ok_or_else(|| AppError::Parse("gold_type column not found".into()))?;
    let name_idx = find_col(&headers, &["name", "fund_name", "scheme_name"]);
    let weight_idx = find_col(&headers, &["weight_grams", "weight", "grams"]);
    let purity_idx = find_col(&headers, &["purity"]);
    let units_idx = find_col(&headers, &["units", "quantity", "qty"]);
    let price_idx = find_col(&headers, &["avg_buy_price", "avg_price", "purchase_price", "buy_price"])
        .ok_or_else(|| AppError::Parse("avg_buy_price column not found".into()))?;

    let mut imported = 0i64;
    let mut skipped = 0i64;

    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;

        let gold_type = cell(&record, type_idx).to_lowercase();
        let avg_buy_price = parse_f64(cell(&record, price_idx));

        if gold_type.is_empty() || !GOLD_TYPES.contains(&gold_type.as_str()) || avg_buy_price <= 0.0 {
            skipped += 1;
            continue;
        }

        let name = cell_opt(&record, name_idx);
        let weight_grams = weight_idx.and_then(|i| parse_f64_opt(cell(&record, i)));
        let purity = cell_opt(&record, purity_idx);
        let units = units_idx.and_then(|i| parse_f64_opt(cell(&record, i)));

        let res = conn.execute(
            "INSERT INTO gold_holdings
               (gold_type, name, weight_grams, purity, units, avg_buy_price, account_id, maturity_date)
             VALUES (?1,?2,?3,?4,?5,?6,NULL,NULL)",
            rusqlite::params![gold_type, name, weight_grams, purity, units, avg_buy_price],
        );
        match res {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ImportResult { imported, skipped })
}

// ── 1e. Crypto CSV importer ───────────────────────────────────────────────────

pub fn import_crypto_from_csv(content: &str, conn: &Connection) -> Result<ImportResult> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    let exch_idx = find_col(&headers, &["exchange", "exchange_name", "platform"])
        .ok_or_else(|| AppError::Parse("exchange column not found".into()))?;
    let coin_idx = find_col(&headers, &["coin_symbol", "symbol", "coin", "ticker"])
        .ok_or_else(|| AppError::Parse("coin_symbol column not found".into()))?;
    let qty_idx = find_col(&headers, &["quantity", "qty", "amount"])
        .ok_or_else(|| AppError::Parse("quantity column not found".into()))?;
    let price_idx = find_col(&headers, &["avg_buy_price", "avg_price", "purchase_price", "buy_price"])
        .ok_or_else(|| AppError::Parse("avg_buy_price column not found".into()))?;

    let mut imported = 0i64;
    let mut skipped = 0i64;

    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;

        let exchange_name = cell(&record, exch_idx).to_string();
        let coin_symbol = cell(&record, coin_idx).to_uppercase();
        let quantity = parse_f64(cell(&record, qty_idx));
        let avg_buy_price = parse_f64(cell(&record, price_idx));

        if exchange_name.is_empty() || coin_symbol.is_empty() || quantity <= 0.0 || avg_buy_price <= 0.0 {
            skipped += 1;
            continue;
        }

        let res = conn.execute(
            "INSERT OR IGNORE INTO crypto_holdings
               (account_id, exchange_name, coin_symbol, quantity, avg_buy_price)
             VALUES (NULL,?1,?2,?3,?4)",
            rusqlite::params![exchange_name, coin_symbol, quantity, avg_buy_price],
        );
        match res {
            Ok(n) if n > 0 => imported += 1,
            Ok(_) => skipped += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ImportResult { imported, skipped })
}

// ── 1f. Bond CSV importer ─────────────────────────────────────────────────────

const BOND_TYPES: &[&str] = &["government", "corporate", "tax_free", "sgb", "ncd", "treasury_bill"];
const COUPON_FREQS: &[&str] = &["annual", "semi_annual", "quarterly", "monthly", "zero_coupon"];

pub fn import_bond_from_csv(content: &str, conn: &Connection) -> Result<ImportResult> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    let isin_idx = find_col(&headers, &["isin"]);
    let issuer_idx = find_col(&headers, &["issuer_name", "issuer", "company"])
        .ok_or_else(|| AppError::Parse("issuer_name column not found".into()))?;
    let type_idx = find_col(&headers, &["bond_type", "type"]);
    let face_idx = find_col(&headers, &["face_value", "face"]);
    let qty_idx = find_col(&headers, &["quantity", "qty", "units"])
        .ok_or_else(|| AppError::Parse("quantity column not found".into()))?;
    let price_idx = find_col(&headers, &["purchase_price", "buy_price", "avg_price"])
        .ok_or_else(|| AppError::Parse("purchase_price column not found".into()))?;
    let coupon_rate_idx = find_col(&headers, &["coupon_rate", "coupon", "rate"]);
    let coupon_freq_idx = find_col(&headers, &["coupon_frequency", "coupon_freq", "frequency"]);
    let purchase_date_idx = find_col(&headers, &["purchase_date", "buy_date", "issue_date"])
        .ok_or_else(|| AppError::Parse("purchase_date column not found".into()))?;
    let maturity_idx = find_col(&headers, &["maturity_date", "maturity"]);
    let rating_idx = find_col(&headers, &["credit_rating", "rating"]);

    let mut imported = 0i64;
    let mut skipped = 0i64;

    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;

        let issuer_name = cell(&record, issuer_idx).to_string();
        let quantity = parse_f64(cell(&record, qty_idx));
        let purchase_price = parse_f64(cell(&record, price_idx));
        let purchase_date = cell(&record, purchase_date_idx).to_string();

        if issuer_name.is_empty() || quantity <= 0.0 || purchase_price <= 0.0 || purchase_date.is_empty() {
            skipped += 1;
            continue;
        }

        let isin = cell_opt(&record, isin_idx);
        let bond_type = type_idx.map(|i| {
            let v = cell(&record, i).to_lowercase();
            if BOND_TYPES.contains(&v.as_str()) { v } else { "corporate".to_string() }
        }).unwrap_or_else(|| "corporate".to_string());
        let face_value = face_idx.map(|i| parse_f64(cell(&record, i))).filter(|v| *v > 0.0).unwrap_or(1000.0);
        let coupon_rate = coupon_rate_idx.map(|i| parse_f64(cell(&record, i))).unwrap_or(0.0);
        let coupon_frequency = coupon_freq_idx.map(|i| {
            let v = cell(&record, i).to_lowercase();
            if COUPON_FREQS.contains(&v.as_str()) { v } else { "semi_annual".to_string() }
        }).unwrap_or_else(|| "semi_annual".to_string());
        let maturity_date = cell_opt(&record, maturity_idx);
        let credit_rating = cell_opt(&record, rating_idx);

        let res = conn.execute(
            "INSERT INTO bond_holdings
               (account_id, isin, issuer_name, bond_type, face_value, quantity,
                purchase_price, current_price, coupon_rate, coupon_frequency,
                purchase_date, maturity_date, credit_rating)
             VALUES (NULL,?1,?2,?3,?4,?5,?6,NULL,?7,?8,?9,?10,?11)",
            rusqlite::params![
                isin, issuer_name, bond_type, face_value, quantity,
                purchase_price, coupon_rate, coupon_frequency,
                purchase_date, maturity_date, credit_rating,
            ],
        );
        match res {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ImportResult { imported, skipped })
}

// ── 1g. PPF/EPF CSV importer ──────────────────────────────────────────────────

const PPF_EPF_TYPES: &[&str] = &["PPF", "EPF", "NPS", "VPF"];

pub fn import_ppf_epf_from_csv(content: &str, conn: &Connection) -> Result<ImportResult> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let raw_headers = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .clone();
    let headers: Vec<String> = raw_headers.iter().map(normalize_header).collect();

    let acct_type_idx = find_col(&headers, &["account_type", "type"])
        .ok_or_else(|| AppError::Parse("account_type column not found".into()))?;
    let acct_num_idx = find_col(&headers, &["account_number", "account_no", "member_id"]);
    let balance_idx = find_col(&headers, &["balance", "corpus", "current_balance"])
        .ok_or_else(|| AppError::Parse("balance column not found".into()))?;
    let rate_idx = find_col(&headers, &["interest_rate", "rate", "roi"])
        .ok_or_else(|| AppError::Parse("interest_rate column not found".into()))?;
    let fy_idx = find_col(&headers, &["financial_year", "fy", "fiscal_year"]);
    let employer_idx = find_col(&headers, &["employer_contrib", "employer_contribution"]);
    let employee_idx = find_col(&headers, &["employee_contrib", "employee_contribution"]);

    let mut imported = 0i64;
    let mut skipped = 0i64;

    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Parse(e.to_string()))?;

        let account_type = cell(&record, acct_type_idx).to_uppercase();
        let balance = parse_f64(cell(&record, balance_idx));
        let interest_rate = parse_f64(cell(&record, rate_idx));

        if !PPF_EPF_TYPES.contains(&account_type.as_str()) || balance < 0.0 || interest_rate <= 0.0 {
            skipped += 1;
            continue;
        }

        let account_number = cell_opt(&record, acct_num_idx);
        let financial_year = cell_opt(&record, fy_idx);
        let employer_contrib = employer_idx.and_then(|i| parse_f64_opt(cell(&record, i)));
        let employee_contrib = employee_idx.and_then(|i| parse_f64_opt(cell(&record, i)));

        let res = conn.execute(
            "INSERT INTO ppf_epf_holdings
               (account_type, account_number, balance, interest_rate,
                financial_year, employer_contrib, employee_contrib)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![
                account_type, account_number, balance, interest_rate,
                financial_year, employer_contrib, employee_contrib,
            ],
        );
        match res {
            Ok(_) => imported += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ImportResult { imported, skipped })
}
