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
    conn.execute_batch(MIGRATION_009).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_010 uses ALTER TABLE which is not idempotent — ignore "duplicate column" errors
    let _ = conn.execute_batch(MIGRATION_010);
    // MIGRATION_011 uses IF NOT EXISTS throughout — safe to re-run
    conn.execute_batch(MIGRATION_011).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_012: relax exchange CHECK constraint — only runs if old CHECK still present
    let schema_sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='equity_holdings'",
        [],
        |r| r.get(0),
    ).unwrap_or_default();
    if schema_sql.contains("CHECK(exchange") || schema_sql.contains("CHECK (exchange") {
        conn.execute_batch(MIGRATION_012).map_err(|e| AppError::Database(e.to_string()))?;
    }
    // MIGRATION_013: onboarding_complete flag — INSERT OR IGNORE is idempotent;
    // UPDATE flips to 'true' for existing users who already have a password set
    conn.execute_batch(MIGRATION_013).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_014: gamification tables — only when feature is enabled
    #[cfg(feature = "gamification")]
    conn.execute_batch(MIGRATION_014).map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    #[test]
    fn run_migrations_is_idempotent_when_rerun_on_same_connection() {
        // test_db_pool() already runs migrations once via DbPool::initialize.
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        // Re-running against the same already-migrated connection must not error,
        // despite MIGRATION_010's non-idempotent ALTER TABLE (swallowed internally)
        // and MIGRATION_012's conditional CHECK-constraint relaxation.
        run_migrations(&conn).expect("second run_migrations call should be a no-op, not an error");

        let table_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN \
             ('accounts','equity_holdings','mf_holdings','transactions','loans','app_settings')",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(table_count, 6, "core tables should still exist after a second migration run");

        // app_settings seed rows must not be duplicated (INSERT OR IGNORE / PRIMARY KEY on `key`)
        let settings_count: i64 = conn.query_row(
            "SELECT count(*) FROM app_settings WHERE key = 'currency'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(settings_count, 1);
    }
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

const MIGRATION_009: &str = "
CREATE TABLE IF NOT EXISTS bond_holdings (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       INTEGER,
    isin             TEXT,
    issuer_name      TEXT NOT NULL,
    bond_type        TEXT NOT NULL DEFAULT 'corporate'
                         CHECK(bond_type IN ('government','corporate','tax_free','sgb','ncd','treasury_bill')),
    face_value       REAL NOT NULL DEFAULT 1000,
    quantity         REAL NOT NULL,
    purchase_price   REAL NOT NULL,
    current_price    REAL,
    coupon_rate      REAL NOT NULL DEFAULT 0,
    coupon_frequency TEXT NOT NULL DEFAULT 'semi_annual'
                         CHECK(coupon_frequency IN ('annual','semi_annual','quarterly','monthly','zero_coupon')),
    purchase_date    TEXT NOT NULL,
    maturity_date    TEXT,
    credit_rating    TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
";

// ALTER TABLE is not idempotent — this migration is run with error ignored in run_migrations
const MIGRATION_010: &str = "
ALTER TABLE goals ADD COLUMN achieved_at TEXT;
";

const MIGRATION_012: &str = "
CREATE TABLE equity_holdings_new (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    isin             TEXT NOT NULL,
    symbol           TEXT NOT NULL,
    exchange         TEXT NOT NULL,
    name             TEXT NOT NULL,
    quantity         REAL NOT NULL,
    avg_buy_price    REAL NOT NULL,
    current_price    REAL,
    price_updated_at TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(account_id, isin)
);
INSERT INTO equity_holdings_new SELECT * FROM equity_holdings;
DROP TABLE equity_holdings;
ALTER TABLE equity_holdings_new RENAME TO equity_holdings;
CREATE INDEX IF NOT EXISTS idx_equity_isin ON equity_holdings(isin);
CREATE INDEX IF NOT EXISTS idx_equity_account ON equity_holdings(account_id);
";

const MIGRATION_013: &str = "
INSERT OR IGNORE INTO app_settings (key, value)
    VALUES ('onboarding_complete', 'false');

UPDATE app_settings
SET value = 'true'
WHERE key = 'onboarding_complete'
  AND (SELECT value FROM app_settings WHERE key = 'password_hash') != '';
";

#[cfg(feature = "gamification")]
const MIGRATION_014: &str = "
CREATE TABLE IF NOT EXISTS user_xp (
    id          INTEGER PRIMARY KEY DEFAULT 1,
    total_xp    INTEGER NOT NULL DEFAULT 0,
    level       TEXT NOT NULL DEFAULT 'Rookie',
    updated_at  TEXT DEFAULT (datetime('now'))
);
INSERT OR IGNORE INTO user_xp (id, total_xp, level) VALUES (1, 0, 'Rookie');

CREATE TABLE IF NOT EXISTS badges (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL,
    icon        TEXT NOT NULL,
    xp_reward   INTEGER NOT NULL DEFAULT 20
);

CREATE TABLE IF NOT EXISTS user_badges (
    badge_id    TEXT PRIMARY KEY REFERENCES badges(id),
    earned_at   TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS streaks (
    streak_type         TEXT PRIMARY KEY,
    current_count       INTEGER NOT NULL DEFAULT 0,
    best_count          INTEGER NOT NULL DEFAULT 0,
    last_activity_date  TEXT,
    updated_at          TEXT DEFAULT (datetime('now'))
);
INSERT OR IGNORE INTO streaks (streak_type, current_count, best_count) VALUES ('transaction', 0, 0);

INSERT OR IGNORE INTO badges (id, name, description, icon, xp_reward) VALUES
    ('first_investment',     'First Investment',       'Added your first holding',                         '🌱', 20),
    ('goal_getter',          'Goal Getter',            'Achieved your first financial goal',               '🎯', 20),
    ('milestone_hunter',     'Milestone Hunter',       'Crossed your first net worth milestone',           '🏔️', 20),
    ('diversified_investor', 'Diversified Investor',   'Holdings across 5 or more asset classes',         '🌍', 20),
    ('debt_destroyer',       'Debt Destroyer',         'Paid off a liability or loan',                     '⚔️', 20),
    ('crore_club',           'Crore Club',             'Net worth crossed Rs 1 Crore',                     '💎', 20),
    ('centurion',            'Centurion',              'Earned 100 XP total',                              '💯', 20);
";

const MIGRATION_011: &str = "
CREATE INDEX IF NOT EXISTS idx_recurring_txn_due
    ON recurring_transactions(next_due_date, is_active);

CREATE INDEX IF NOT EXISTS idx_bills_due
    ON bills(next_due_date, is_active);

CREATE INDEX IF NOT EXISTS idx_accounts_type
    ON accounts(type);

CREATE INDEX IF NOT EXISTS idx_equity_account
    ON equity_holdings(account_id);

CREATE INDEX IF NOT EXISTS idx_mf_account
    ON mf_holdings(account_id);

CREATE INDEX IF NOT EXISTS idx_txn_account_date
    ON transactions(account_id, date);

CREATE INDEX IF NOT EXISTS idx_sip_active
    ON sip_schedules(is_active, account_id);
";
