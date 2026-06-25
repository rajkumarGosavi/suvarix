use rusqlite::Connection;
use crate::error::{AppError, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(MIGRATION_001).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_002).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_003).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_004).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_005).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_006).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_007).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_008).map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

const MIGRATION_001: &str = "
CREATE TABLE IF NOT EXISTS accounts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    type        TEXT NOT NULL CHECK(type IN ('broker','bank','wallet','manual')),
    provider    TEXT,
    external_id TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS equity_holdings (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    isin             TEXT NOT NULL,
    symbol           TEXT NOT NULL,
    exchange         TEXT NOT NULL CHECK(exchange IN ('NSE','BSE')),
    name             TEXT NOT NULL,
    quantity         REAL NOT NULL,
    avg_buy_price    REAL NOT NULL,
    current_price    REAL,
    price_updated_at TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(account_id, isin)
);
CREATE INDEX IF NOT EXISTS idx_equity_isin ON equity_holdings(isin);

CREATE TABLE IF NOT EXISTS mf_holdings (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    scheme_code   TEXT NOT NULL,
    scheme_name   TEXT NOT NULL,
    amc_name      TEXT NOT NULL,
    folio_number  TEXT NOT NULL,
    units         REAL NOT NULL,
    avg_nav       REAL NOT NULL,
    current_nav   REAL,
    nav_date      TEXT,
    is_direct     INTEGER NOT NULL DEFAULT 0,
    is_growth     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(account_id, folio_number, scheme_code)
);
CREATE INDEX IF NOT EXISTS idx_mf_scheme ON mf_holdings(scheme_code);

CREATE TABLE IF NOT EXISTS sip_schedules (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    mf_holding_id INTEGER REFERENCES mf_holdings(id) ON DELETE SET NULL,
    scheme_code   TEXT NOT NULL,
    account_id    INTEGER NOT NULL REFERENCES accounts(id),
    amount        REAL NOT NULL,
    frequency     TEXT NOT NULL CHECK(frequency IN ('monthly','quarterly','weekly')),
    debit_day     INTEGER NOT NULL,
    start_date    TEXT NOT NULL,
    end_date      TEXT,
    is_active     INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS fd_holdings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
    bank_name       TEXT NOT NULL,
    account_number  TEXT,
    principal       REAL NOT NULL,
    interest_rate   REAL NOT NULL,
    compounding     TEXT NOT NULL DEFAULT 'quarterly',
    tenure_months   INTEGER NOT NULL,
    start_date      TEXT NOT NULL,
    maturity_date   TEXT NOT NULL,
    maturity_amount REAL,
    is_cumulative   INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_fd_maturity ON fd_holdings(maturity_date);

CREATE TABLE IF NOT EXISTS ppf_epf_holdings (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    account_type     TEXT NOT NULL CHECK(account_type IN ('PPF','EPF','NPS','VPF')),
    account_number   TEXT,
    balance          REAL NOT NULL DEFAULT 0,
    interest_rate    REAL NOT NULL,
    financial_year   TEXT,
    employer_contrib REAL DEFAULT 0,
    employee_contrib REAL DEFAULT 0,
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS real_estate_holdings (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    property_name  TEXT NOT NULL,
    property_type  TEXT NOT NULL CHECK(property_type IN ('residential','commercial','land','agricultural')),
    location       TEXT,
    purchase_price REAL NOT NULL,
    purchase_date  TEXT NOT NULL,
    current_value  REAL,
    rental_income  REAL DEFAULT 0,
    has_mortgage   INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS gold_holdings (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    gold_type     TEXT NOT NULL CHECK(gold_type IN ('physical','digital','etf','sgb')),
    name          TEXT,
    weight_grams  REAL,
    purity        TEXT,
    units         REAL,
    avg_buy_price REAL NOT NULL,
    current_price REAL,
    account_id    INTEGER REFERENCES accounts(id),
    maturity_date TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS crypto_holdings (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id    INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
    exchange_name TEXT NOT NULL,
    coin_symbol   TEXT NOT NULL,
    quantity      REAL NOT NULL,
    avg_buy_price REAL NOT NULL,
    current_price REAL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(account_id, coin_symbol)
);

CREATE TABLE IF NOT EXISTS insurance_holdings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    insurance_type  TEXT NOT NULL CHECK(insurance_type IN ('life','health','term','ulip','vehicle','home')),
    provider        TEXT NOT NULL,
    policy_number   TEXT,
    premium_amount  REAL NOT NULL,
    premium_freq    TEXT NOT NULL CHECK(premium_freq IN ('monthly','quarterly','annual')),
    coverage_amount REAL,
    maturity_value  REAL,
    start_date      TEXT NOT NULL,
    end_date        TEXT,
    next_due_date   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_insurance_due ON insurance_holdings(next_due_date);
";

const MIGRATION_002: &str = "
CREATE TABLE IF NOT EXISTS loans (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    loan_type         TEXT NOT NULL CHECK(loan_type IN ('home','car','personal','education','gold')),
    lender_name       TEXT NOT NULL,
    account_number    TEXT,
    principal         REAL NOT NULL,
    outstanding       REAL NOT NULL,
    interest_rate     REAL NOT NULL,
    emi_amount        REAL NOT NULL,
    tenure_months     INTEGER NOT NULL,
    disbursement_date TEXT NOT NULL,
    next_emi_date     TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS credit_cards (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    bank_name       TEXT NOT NULL,
    card_name       TEXT,
    last_four       TEXT,
    credit_limit    REAL NOT NULL,
    current_balance REAL NOT NULL DEFAULT 0,
    due_date        INTEGER,
    min_payment     REAL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const MIGRATION_003: &str = "
CREATE TABLE IF NOT EXISTS transactions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    date         TEXT NOT NULL,
    type         TEXT NOT NULL CHECK(type IN (
                   'buy','sell','dividend','interest','sip','redemption',
                   'deposit','withdrawal','expense','income','emi','transfer')),
    asset_class  TEXT CHECK(asset_class IN (
                   'equity','mf','fd','ppf_epf','real_estate',
                   'gold','crypto','insurance','cash','loan','credit_card')),
    account_id   INTEGER REFERENCES accounts(id),
    holding_id   INTEGER,
    amount       REAL NOT NULL,
    quantity     REAL,
    price        REAL,
    category     TEXT,
    description  TEXT,
    notes        TEXT,
    source       TEXT DEFAULT 'manual',
    external_ref TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_txn_date     ON transactions(date DESC);
CREATE INDEX IF NOT EXISTS idx_txn_account  ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_txn_asset    ON transactions(asset_class);
CREATE INDEX IF NOT EXISTS idx_txn_type     ON transactions(type);
CREATE INDEX IF NOT EXISTS idx_txn_ext_ref  ON transactions(external_ref) WHERE external_ref IS NOT NULL;
";

const MIGRATION_004: &str = "
CREATE TABLE IF NOT EXISTS budgets (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    category      TEXT NOT NULL,
    monthly_limit REAL NOT NULL,
    period        TEXT NOT NULL DEFAULT 'monthly',
    is_active     INTEGER NOT NULL DEFAULT 1,
    UNIQUE(category, period)
);

CREATE TABLE IF NOT EXISTS net_worth_snapshots (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_date     TEXT NOT NULL UNIQUE,
    total_assets      REAL NOT NULL,
    total_liabilities REAL NOT NULL,
    net_worth         REAL NOT NULL,
    breakdown_json    TEXT
);
CREATE INDEX IF NOT EXISTS idx_nw_date ON net_worth_snapshots(snapshot_date);

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO app_settings VALUES ('currency', 'INR');
INSERT OR IGNORE INTO app_settings VALUES ('auto_lock_minutes', '15');
INSERT OR IGNORE INTO app_settings VALUES ('theme', 'system');
INSERT OR IGNORE INTO app_settings VALUES ('password_hash', '');
INSERT OR IGNORE INTO app_settings VALUES ('password_salt', '');

CREATE TABLE IF NOT EXISTS import_log (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    source           TEXT NOT NULL,
    filename         TEXT,
    records_imported INTEGER NOT NULL DEFAULT 0,
    records_skipped  INTEGER NOT NULL DEFAULT 0,
    status           TEXT NOT NULL CHECK(status IN ('success','partial','failed')),
    error_message    TEXT,
    imported_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const MIGRATION_005: &str = "
CREATE TABLE IF NOT EXISTS goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    category      TEXT NOT NULL DEFAULT 'other'
                      CHECK(category IN ('home','vehicle','education','retirement','travel','emergency','other')),
    target_amount REAL NOT NULL,
    target_date   TEXT NOT NULL,
    notes         TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const MIGRATION_006: &str = "
CREATE TABLE IF NOT EXISTS app_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    event_name TEXT NOT NULL,
    properties TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_events_name ON app_events(event_name);

CREATE TABLE IF NOT EXISTS app_errors (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    error_type TEXT NOT NULL,
    message    TEXT NOT NULL,
    stack      TEXT,
    context    TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS perf_metrics (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_name TEXT NOT NULL,
    value_ms    REAL NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_perf_name ON perf_metrics(metric_name);
";

const MIGRATION_007: &str = "
CREATE TABLE IF NOT EXISTS bills (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    category      TEXT NOT NULL DEFAULT 'utilities'
                      CHECK(category IN ('utilities','rent','subscription','insurance','emi','tax','other')),
    amount        REAL NOT NULL,
    frequency     TEXT NOT NULL DEFAULT 'monthly'
                      CHECK(frequency IN ('weekly','monthly','quarterly','yearly','one_time')),
    next_due_date TEXT NOT NULL,
    notes         TEXT,
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS recurring_transactions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    type          TEXT NOT NULL,
    amount        REAL NOT NULL,
    category      TEXT NOT NULL,
    asset_class   TEXT,
    description   TEXT,
    notes         TEXT,
    frequency     TEXT NOT NULL DEFAULT 'monthly'
                      CHECK(frequency IN ('daily','weekly','monthly','yearly')),
    next_due_date TEXT NOT NULL,
    last_run_date TEXT,
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const MIGRATION_008: &str = "
CREATE TABLE IF NOT EXISTS milestones (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    amount      REAL NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    is_custom   INTEGER NOT NULL DEFAULT 0,
    achieved_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO milestones (amount, label, is_custom) VALUES
    (100000,    '₹1 Lakh',     0),
    (500000,    '₹5 Lakh',     0),
    (1000000,   '₹10 Lakh',    0),
    (2500000,   '₹25 Lakh',    0),
    (5000000,   '₹50 Lakh',    0),
    (10000000,  '₹1 Crore',    0),
    (25000000,  '₹2.5 Crore',  0),
    (50000000,  '₹5 Crore',    0),
    (100000000, '₹10 Crore',   0);
";
