use tauri::State;
use crate::db::DbState;
use crate::error::{AppError, Result};

const DUMMY_TAG: &str = "__dummy__";

#[tauri::command]
pub fn is_dev_build() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub fn is_dummy_data_seeded(state: State<DbState>) -> Result<bool> {
    let conn = state.0.get()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts WHERE provider = ?1",
        [DUMMY_TAG],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

#[tauri::command]
pub fn seed_dummy_data(state: State<DbState>) -> Result<()> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Validation("Not available in release builds".into()));
    }
    let conn = state.0.get()?;

    let already: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts WHERE provider = ?1",
        [DUMMY_TAG],
        |r| r.get(0),
    )?;
    if already > 0 {
        return Ok(());
    }

    // ── Accounts ────────────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO accounts (name, type, provider) VALUES ('Zerodha (Demo)', 'broker', ?1)",
        [DUMMY_TAG],
    )?;
    let broker_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO accounts (name, type, provider) VALUES ('SBI Savings (Demo)', 'bank', ?1)",
        [DUMMY_TAG],
    )?;
    let bank_id = conn.last_insert_rowid();

    // ── Equity ──────────────────────────────────────────────────────────────
    let equities: &[(&str, &str, &str, &str, f64, f64, f64)] = &[
        ("INE002A01018", "RELIANCE",  "NSE", "Reliance Industries Ltd",             25.0, 2450.0, 2891.0),
        ("INE009A01021", "INFY",      "NSE", "Infosys Ltd",                         50.0, 1480.0, 1642.0),
        ("INE467B01029", "TCS",       "NSE", "Tata Consultancy Services Ltd",       15.0, 3200.0, 3850.0),
        ("INE040A01034", "HDFCBANK",  "NSE", "HDFC Bank Ltd",                       40.0, 1580.0, 1730.0),
        ("INE090A01021", "ICICIBANK", "NSE", "ICICI Bank Ltd",                      60.0,  920.0, 1250.0),
    ];
    for (isin, symbol, exchange, name, qty, avg, curr) in equities {
        conn.execute(
            "INSERT OR IGNORE INTO equity_holdings
             (account_id, isin, symbol, exchange, name, quantity, avg_buy_price, current_price)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![broker_id, isin, symbol, exchange, name, qty, avg, curr],
        )?;
    }

    // ── Mutual Funds ────────────────────────────────────────────────────────
    let mfs: &[(&str, &str, &str, &str, f64, f64, f64)] = &[
        ("119551", "Mirae Asset Large Cap Fund - Direct Growth",          "Mirae Asset MF",      "12345001", 250.0,  72.50,  89.30),
        ("120503", "Axis Bluechip Fund - Direct Growth",                  "Axis Mutual Fund",    "12345002", 500.0,  38.20,  52.10),
        ("125497", "Parag Parikh Flexi Cap Fund - Direct Growth",         "PPFAS Mutual Fund",   "12345003", 180.0,  48.00,  71.50),
        ("118989", "SBI Small Cap Fund - Direct Growth",                  "SBI Funds Management","12345004", 300.0,  95.00, 142.80),
    ];
    for (scheme_code, scheme_name, amc, folio, units, avg_nav, curr_nav) in mfs {
        conn.execute(
            "INSERT OR IGNORE INTO mf_holdings
             (account_id, scheme_code, scheme_name, amc_name, folio_number, units, avg_nav, current_nav, is_direct, is_growth)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 1)",
            rusqlite::params![broker_id, scheme_code, scheme_name, amc, folio, units, avg_nav, curr_nav],
        )?;
    }

    // ── Fixed Deposits ───────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO fd_holdings (account_id, bank_name, account_number, principal, interest_rate, compounding, tenure_months, start_date, maturity_date, maturity_amount, is_cumulative)
         VALUES (?1, 'HDFC Bank', ?2, 200000.0, 7.10, 'quarterly', 24, '2023-06-15', '2025-06-15', 230142.0, 1)",
        rusqlite::params![bank_id, DUMMY_TAG],
    )?;
    conn.execute(
        "INSERT INTO fd_holdings (account_id, bank_name, account_number, principal, interest_rate, compounding, tenure_months, start_date, maturity_date, maturity_amount, is_cumulative)
         VALUES (?1, 'SBI', ?2, 500000.0, 6.80, 'quarterly', 36, '2023-01-10', '2026-01-10', 610221.0, 1)",
        rusqlite::params![bank_id, DUMMY_TAG],
    )?;

    // ── PPF / EPF ────────────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO ppf_epf_holdings (account_type, account_number, balance, interest_rate, financial_year)
         VALUES ('PPF', ?1, 850000.0, 7.10, '2024-25')",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "INSERT INTO ppf_epf_holdings (account_type, account_number, balance, interest_rate, financial_year, employer_contrib, employee_contrib)
         VALUES ('EPF', ?1, 1250000.0, 8.25, '2024-25', 24000.0, 24000.0)",
        [DUMMY_TAG],
    )?;

    // ── Real Estate ──────────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO real_estate_holdings (property_name, property_type, location, purchase_price, purchase_date, current_value, rental_income, has_mortgage)
         VALUES ('Flat 4B, Skyline Residency', 'residential', ?1, 5500000.0, '2019-03-20', 8200000.0, 22000.0, 1)",
        [DUMMY_TAG],
    )?;

    // ── Gold ─────────────────────────────────────────────────────────────────
    // Physical gold — linked to broker account so DELETE CASCADE works
    conn.execute(
        "INSERT INTO gold_holdings (gold_type, name, weight_grams, purity, avg_buy_price, current_price, account_id)
         VALUES ('physical', ?1, 50.0, '22K', 4800.0, 7200.0, ?2)",
        rusqlite::params![DUMMY_TAG, broker_id],
    )?;
    // SGB — no account_id; tagged by name
    conn.execute(
        "INSERT INTO gold_holdings (gold_type, name, units, avg_buy_price, current_price, maturity_date)
         VALUES ('sgb', ?1, 8.0, 5800.0, 7200.0, '2030-11-28')",
        [DUMMY_TAG],
    )?;

    // ── Crypto ───────────────────────────────────────────────────────────────
    conn.execute(
        "INSERT OR IGNORE INTO crypto_holdings (account_id, exchange_name, coin_symbol, quantity, avg_buy_price, current_price)
         VALUES (?1, 'CoinDCX', 'BTC', 0.05, 2800000.0, 5800000.0)",
        rusqlite::params![broker_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO crypto_holdings (account_id, exchange_name, coin_symbol, quantity, avg_buy_price, current_price)
         VALUES (?1, 'CoinDCX', 'ETH', 1.5, 175000.0, 320000.0)",
        rusqlite::params![broker_id],
    )?;

    // ── Insurance ────────────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO insurance_holdings (insurance_type, provider, policy_number, premium_amount, premium_freq, coverage_amount, start_date, end_date, next_due_date)
         VALUES ('term', 'HDFC Life', ?1, 15000.0, 'annual', 10000000.0, '2022-04-01', '2052-04-01', '2026-04-01')",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "INSERT INTO insurance_holdings (insurance_type, provider, policy_number, premium_amount, premium_freq, coverage_amount, start_date, next_due_date)
         VALUES ('health', 'Star Health', ?1, 18000.0, 'annual', 1000000.0, '2023-07-15', '2026-07-15')",
        [DUMMY_TAG],
    )?;

    // ── Loans ────────────────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO loans (loan_type, lender_name, account_number, principal, outstanding, interest_rate, emi_amount, tenure_months, disbursement_date, next_emi_date)
         VALUES ('home', 'SBI Home Loans', ?1, 4000000.0, 3250000.0, 8.50, 35000.0, 240, '2020-06-01', '2025-07-01')",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "INSERT INTO loans (loan_type, lender_name, account_number, principal, outstanding, interest_rate, emi_amount, tenure_months, disbursement_date, next_emi_date)
         VALUES ('car', 'HDFC Bank', ?1, 800000.0, 320000.0, 9.00, 15000.0, 60, '2022-04-15', '2025-07-15')",
        [DUMMY_TAG],
    )?;

    // ── Credit Card ──────────────────────────────────────────────────────────
    // last_four = 'DEMO' used as sentinel for clear
    conn.execute(
        "INSERT INTO credit_cards (bank_name, card_name, last_four, credit_limit, current_balance, due_date, min_payment)
         VALUES ('HDFC Bank', 'Regalia Credit Card', 'DEMO', 300000.0, 45000.0, 15, 4500.0)",
        [],
    )?;

    // ── Transactions ────────────────────────────────────────────────────────
    // Dated relative to today so "This Month" filters still show data after the seed ages.
    use chrono::{Datelike, Local};
    let now = Local::now();
    let ym = |day: u32| format!("{}-{:02}-{:02}", now.year(), now.month(), day);
    let txns: &[(String, &str, &str, &str, f64, &str)] = &[
        (ym(1),  "income",   "cash",   "Monthly Salary",              150000.0, "salary"),
        (ym(5),  "expense",  "cash",   "Grocery - BigBasket",           8500.0, "food"),
        (ym(8),  "expense",  "cash",   "Electricity Bill",              2200.0, "utilities"),
        (ym(10), "buy",      "equity", "Bought RELIANCE × 10",        122500.0, "investment"),
        (ym(12), "expense",  "cash",   "Restaurant - Swiggy",           1800.0, "food"),
        (ym(15), "emi",      "loan",   "Home Loan EMI",                35000.0, "housing"),
        (ym(16), "sip",      "mf",     "SIP – Parag Parikh FCF",        5000.0, "investment"),
        (ym(20), "expense",  "cash",   "Fuel – Petrol",                 4500.0, "transport"),
        (ym(22), "dividend", "equity", "INFY Interim Dividend",         1250.0, "investment"),
        (ym(25), "expense",  "cash",   "Mobile Recharge",                599.0, "utilities"),
    ];
    for (date, txn_type, asset_class, desc, amount, category) in txns {
        conn.execute(
            "INSERT INTO transactions (date, type, asset_class, account_id, amount, description, category, source, external_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'manual', ?8)",
            rusqlite::params![date, txn_type, asset_class, bank_id, amount, desc, category, DUMMY_TAG],
        )?;
    }

    // ── Goals ────────────────────────────────────────────────────────────────
    conn.execute(
        "INSERT INTO goals (name, category, target_amount, target_date, notes)
         VALUES ('Retirement Corpus', 'retirement', 50000000.0, '2045-03-31', ?1)",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "INSERT INTO goals (name, category, target_amount, target_date, notes)
         VALUES ('Second Home Down Payment', 'home', 2000000.0, '2027-12-31', ?1)",
        [DUMMY_TAG],
    )?;

    // ── Bonds ────────────────────────────────────────────────────────────────
    // (isin, issuer, bond_type, face_value, qty, buy_price, curr_price, coupon_rate, coupon_freq, purchase_date, maturity_date)
    type BondSeed<'a> = (&'a str, &'a str, &'a str, f64, f64, f64, f64, f64, &'a str, &'a str, &'a str);
    let bonds: &[BondSeed] = &[
        ("IN0020220178", "Government of India",          "government", 1000.0, 50.0,  982.0,  1010.0, 7.26, "semi_annual", "2023-03-15", "2033-03-15"),
        ("INE001A07BS8", "HDFC Ltd NCD",                 "ncd",        1000.0, 25.0,  995.0,  1020.0, 8.45, "quarterly",   "2022-09-01", "2027-09-01"),
        ("INE062A08330", "Indian Railway Finance Corp",  "tax_free",   1000.0, 100.0, 1005.0, 1030.0, 5.13, "annual",      "2021-11-20", "2031-11-20"),
        ("INE414G07BI5", "Tata Capital Financial Svcs", "corporate",  1000.0, 30.0,  988.0,   999.0, 8.75, "semi_annual", "2024-01-10", "2027-01-10"),
    ];
    for (isin, issuer, bond_type, face_value, qty, buy_price, curr_price, coupon_rate, coupon_freq, purchase_date, maturity_date) in bonds {
        conn.execute(
            "INSERT INTO bond_holdings
             (account_id, isin, issuer_name, bond_type, face_value, quantity, purchase_price, current_price, coupon_rate, coupon_frequency, purchase_date, maturity_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![broker_id, isin, issuer, bond_type, face_value, qty, buy_price, curr_price, coupon_rate, coupon_freq, purchase_date, maturity_date],
        )?;
    }

    // ── Transactions — multi-month history for trend chart ──────────────────
    // Jan–May 2025 (June already seeded above); all linked to bank_id → cleared with account delete
    let monthly_txns: &[(&str, &str, &str, f64, &str)] = &[
        // Jan 2025
        ("2025-01-01", "income",  "Salary – January",     150000.0, "salary"),
        ("2025-01-06", "expense", "Grocery",                 8200.0, "food"),
        ("2025-01-08", "expense", "Electricity Bill",        2100.0, "utilities"),
        ("2025-01-10", "expense", "Rent",                   18000.0, "rent"),
        ("2025-01-15", "emi",     "Home Loan EMI",          35000.0, "housing"),
        ("2025-01-18", "expense", "Fuel",                    3800.0, "transport"),
        ("2025-01-22", "expense", "OTT Subscriptions",       1248.0, "entertainment"),
        ("2025-01-25", "expense", "Mobile Recharge",          599.0, "utilities"),
        // Feb 2025
        ("2025-02-01", "income",  "Salary – February",    150000.0, "salary"),
        ("2025-02-05", "expense", "Grocery",                 9100.0, "food"),
        ("2025-02-08", "expense", "Electricity Bill",        1980.0, "utilities"),
        ("2025-02-10", "expense", "Rent",                   18000.0, "rent"),
        ("2025-02-15", "emi",     "Home Loan EMI",          35000.0, "housing"),
        ("2025-02-17", "expense", "Medical – Pharmacy",      3200.0, "medical"),
        ("2025-02-20", "expense", "Restaurant",              2400.0, "food"),
        ("2025-02-25", "expense", "Mobile Recharge",          599.0, "utilities"),
        // Mar 2025
        ("2025-03-01", "income",  "Salary – March",       155000.0, "salary"),
        ("2025-03-05", "expense", "Grocery",                 8800.0, "food"),
        ("2025-03-08", "expense", "Electricity Bill",        2300.0, "utilities"),
        ("2025-03-10", "expense", "Rent",                   18000.0, "rent"),
        ("2025-03-12", "income",  "Freelance Project",      25000.0, "freelance"),
        ("2025-03-15", "emi",     "Home Loan EMI",          35000.0, "housing"),
        ("2025-03-20", "expense", "Fuel",                    4100.0, "transport"),
        ("2025-03-28", "expense", "Shopping",               12500.0, "shopping"),
        // Apr 2025
        ("2025-04-01", "income",  "Salary – April",       155000.0, "salary"),
        ("2025-04-05", "expense", "Grocery",                 7900.0, "food"),
        ("2025-04-08", "expense", "Electricity Bill",        2050.0, "utilities"),
        ("2025-04-10", "expense", "Rent",                   18000.0, "rent"),
        ("2025-04-15", "emi",     "Home Loan EMI",          35000.0, "housing"),
        ("2025-04-18", "expense", "Travel – Goa Trip",      28000.0, "travel"),
        ("2025-04-25", "expense", "Mobile Recharge",          599.0, "utilities"),
        ("2025-04-28", "dividend","INFY Dividend",            1500.0, "investment"),
        // May 2025
        ("2025-05-01", "income",  "Salary – May",         155000.0, "salary"),
        ("2025-05-06", "expense", "Grocery",                 8600.0, "food"),
        ("2025-05-08", "expense", "Electricity Bill",        2450.0, "utilities"),
        ("2025-05-10", "expense", "Rent",                   18000.0, "rent"),
        ("2025-05-15", "emi",     "Home Loan EMI",          35000.0, "housing"),
        ("2025-05-16", "sip",     "SIP – Parag Parikh FCF",  5000.0, "investment"),
        ("2025-05-20", "expense", "Fuel",                    4200.0, "transport"),
        ("2025-05-22", "income",  "Interest – SBI FD",       2830.0, "interest"),
    ];
    for (date, txn_type, desc, amount, category) in monthly_txns {
        conn.execute(
            "INSERT INTO transactions (date, type, asset_class, account_id, amount, description, category, source, external_ref)
             VALUES (?1, ?2, 'cash', ?3, ?4, ?5, ?6, 'manual', ?7)",
            rusqlite::params![date, txn_type, bank_id, amount, desc, category, DUMMY_TAG],
        )?;
    }

    // ── Bills ────────────────────────────────────────────────────────────────
    // notes = DUMMY_TAG used as sentinel for clear
    let bills: &[(&str, &str, f64, &str, &str)] = &[
        // (name, category, amount, frequency, next_due_date)
        ("MSEDCL Electricity",         "utilities",   2500.0, "monthly",  "2026-07-15"),
        ("Jio Fiber Broadband",        "utilities",    999.0, "monthly",  "2026-07-18"),
        ("Airtel Postpaid – Mobile",   "utilities",    599.0, "monthly",  "2026-07-20"),
        ("Netflix Subscription",       "subscription", 649.0, "monthly",  "2026-07-10"),
        ("Amazon Prime",               "subscription", 299.0, "monthly",  "2026-07-12"),
        ("SBI Home Loan EMI",          "emi",        35000.0, "monthly",  "2026-07-05"),
        ("HDFC Car Loan EMI",          "emi",        15000.0, "monthly",  "2026-07-05"),
        ("HDFC Life Term Insurance",   "insurance",  15000.0, "yearly",   "2026-04-01"),
    ];
    for (name, category, amount, frequency, next_due_date) in bills {
        conn.execute(
            "INSERT INTO bills (name, category, amount, frequency, next_due_date, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![name, category, amount, frequency, next_due_date, DUMMY_TAG],
        )?;
    }

    // ── Recurring Transactions ───────────────────────────────────────────────
    // notes = DUMMY_TAG used as sentinel for clear
    let recurring: &[(&str, &str, f64, &str, &str, &str, &str)] = &[
        // (name, type, amount, category, asset_class, frequency, next_due_date)
        ("Monthly Salary",           "income",   150000.0, "salary",        "cash",   "monthly", "2026-07-01"),
        ("SIP – Parag Parikh FCF",   "sip",        5000.0, "investment",    "mf",     "monthly", "2026-07-16"),
        ("SIP – Axis Bluechip",      "sip",        3000.0, "investment",    "mf",     "monthly", "2026-07-10"),
        ("Rent Payment",             "expense",   18000.0, "rent",          "cash",   "monthly", "2026-07-10"),
        ("Netflix & OTT",            "expense",    1200.0, "entertainment", "cash",   "monthly", "2026-07-10"),
        ("Gym Membership",           "expense",    2000.0, "entertainment", "cash",   "monthly", "2026-07-07"),
        ("SBI Home Loan EMI",        "emi",       35000.0, "housing",       "loan",   "monthly", "2026-07-05"),
        ("HDFC Car Loan EMI",        "emi",       15000.0, "housing",       "loan",   "monthly", "2026-07-05"),
    ];
    for (name, txn_type, amount, category, asset_class, frequency, next_due_date) in recurring {
        conn.execute(
            "INSERT INTO recurring_transactions (name, type, amount, category, asset_class, frequency, next_due_date, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![name, txn_type, amount, category, asset_class, frequency, next_due_date, DUMMY_TAG],
        )?;
    }

    // ── Budgets ──────────────────────────────────────────────────────────────
    // INSERT OR IGNORE — no sentinel column; cleared by hardcoded category list in clear_dummy_data
    let budgets: &[(&str, f64)] = &[
        ("food", 12000.0), ("rent", 20000.0), ("utilities", 6000.0),
        ("transport", 8000.0), ("entertainment", 5000.0), ("shopping", 10000.0),
        ("medical", 4000.0), ("travel", 20000.0),
    ];
    for (category, limit) in budgets {
        conn.execute(
            "INSERT OR IGNORE INTO budgets (category, monthly_limit, period, is_active)
             VALUES (?1, ?2, 'monthly', 1)",
            rusqlite::params![category, limit],
        )?;
    }

    // ── Net worth snapshot ───────────────────────────────────────────────────
    conn.execute(
        "INSERT OR IGNORE INTO net_worth_snapshots (snapshot_date, total_assets, total_liabilities, net_worth, breakdown_json)
         VALUES ('2025-06-01', 28500000.0, 3570000.0, 24930000.0, '{\"_dummy\":true}')",
        [],
    )?;

    Ok(())
}

#[tauri::command]
pub fn clear_dummy_data(state: State<DbState>) -> Result<()> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Validation("Not available in release builds".into()));
    }
    let conn = state.0.get()?;

    // Remove all tables that reference accounts(id) WITHOUT ON DELETE CASCADE,
    // before deleting the accounts row (FK enforcement is ON).
    conn.execute(
        "DELETE FROM transactions WHERE account_id IN (SELECT id FROM accounts WHERE provider = ?1)",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "DELETE FROM gold_holdings WHERE account_id IN (SELECT id FROM accounts WHERE provider = ?1) OR name = ?1",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "DELETE FROM sip_schedules WHERE account_id IN (SELECT id FROM accounts WHERE provider = ?1)",
        [DUMMY_TAG],
    )?;
    conn.execute(
        "DELETE FROM bond_holdings WHERE account_id IN (SELECT id FROM accounts WHERE provider = ?1)
             OR isin IN ('IN0020220178', 'INE001A07BS8', 'INE062A08330', 'INE414G07BI5')",
        [DUMMY_TAG],
    )?;

    // Deleting accounts cascades: equity, mf, fd, crypto holdings
    conn.execute("DELETE FROM accounts WHERE provider = ?1", [DUMMY_TAG])?;

    // Standalone tables tagged by sentinel column
    conn.execute("DELETE FROM ppf_epf_holdings WHERE account_number = ?1", [DUMMY_TAG])?;
    conn.execute("DELETE FROM real_estate_holdings WHERE location = ?1", [DUMMY_TAG])?;
    conn.execute("DELETE FROM insurance_holdings WHERE policy_number = ?1", [DUMMY_TAG])?;
    conn.execute("DELETE FROM loans WHERE account_number = ?1", [DUMMY_TAG])?;
    conn.execute("DELETE FROM credit_cards WHERE last_four = 'DEMO'", [])?;
    conn.execute("DELETE FROM bills WHERE notes = ?1", [DUMMY_TAG])?;
    conn.execute("DELETE FROM recurring_transactions WHERE notes = ?1", [DUMMY_TAG])?;
    conn.execute("DELETE FROM goals WHERE notes = ?1", [DUMMY_TAG])?;
    conn.execute(
        "DELETE FROM budgets WHERE category IN
             ('food','rent','utilities','transport','entertainment','shopping','medical','travel')",
        [],
    )?;
    conn.execute("DELETE FROM net_worth_snapshots WHERE breakdown_json LIKE '%\"_dummy\"%'", [])?;

    Ok(())
}
