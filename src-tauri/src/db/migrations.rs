use rusqlite::Connection;
use crate::error::{AppError, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
    tracing::debug!("running migrations");
    conn.execute_batch(MIGRATION_001).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_002).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_003).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_004).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_005).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_006).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_007).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_008).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(DEFAULT_MILESTONES_SEED).map_err(|e| AppError::Database(e.to_string()))?;
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
    // MIGRATION_015: user-managed categories table, seeded with legacy hardcoded defaults
    // and backfilled from any category text already present (e.g. from CSV imports) —
    // idempotent (CREATE TABLE IF NOT EXISTS / INSERT OR IGNORE throughout).
    conn.execute_batch(MIGRATION_015).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(DEFAULT_CATEGORIES_SEED).map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute_batch(MIGRATION_015_BACKFILL).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_016 uses ALTER TABLE which is not idempotent — ignore "duplicate column" errors
    let _ = conn.execute_batch(MIGRATION_016);
    // MIGRATION_017: reminder-scheduler dedup state — INSERT OR IGNORE is idempotent
    conn.execute_batch(MIGRATION_017).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_018 uses ALTER TABLE which is not idempotent — ignore "duplicate column" errors
    let _ = conn.execute_batch(&migration_018_add_sync_columns());
    // MIGRATION_019: sync_tombstones table + backfill + triggers — CREATE ... IF NOT EXISTS
    // and UPDATE ... WHERE col IS NULL throughout, safe to re-run.
    conn.execute_batch(&migration_019_sync_infra()).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_020 uses ALTER TABLE which is not idempotent — ignore "duplicate column" errors
    let _ = conn.execute_batch(&migration_020_add_hlc_columns());
    // MIGRATION_021: HLC counter state + DROP/CREATE of the sync triggers to also stamp
    // `sync_hlc` — DROP TRIGGER IF EXISTS + CREATE TRIGGER (no IF NOT EXISTS) is safe to re-run.
    conn.execute_batch(&migration_021_hlc_state_and_triggers())
        .map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_022: financial-health score history — CREATE TABLE IF NOT EXISTS, idempotent.
    conn.execute_batch(MIGRATION_022).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_023: financial-health badges — only when gamification is compiled in
    // (the `badges` table itself only exists under that feature). INSERT OR IGNORE, idempotent.
    #[cfg(feature = "gamification")]
    conn.execute_batch(MIGRATION_023).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_024: outcome-bound wealth badges — gamification-only, INSERT OR IGNORE, idempotent.
    #[cfg(feature = "gamification")]
    conn.execute_batch(MIGRATION_024).map_err(|e| AppError::Database(e.to_string()))?;
    // MIGRATION_025: opt-in savings challenges — gamification-only, CREATE ... IF NOT EXISTS, idempotent.
    #[cfg(feature = "gamification")]
    conn.execute_batch(MIGRATION_025).map_err(|e| AppError::Database(e.to_string()))?;
    tracing::debug!("migrations complete");
    Ok(())
}

// Daily record of the computed Financial Health Score, so the app can show a trend
// and award improvement-only XP by comparing today against the last recorded day.
const MIGRATION_022: &str = "
CREATE TABLE IF NOT EXISTS health_score_history (
    snapshot_date TEXT PRIMARY KEY,
    score         REAL NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
";

// Health-milestone badges. Awarded from the frontend health-check when thresholds
// are crossed (see gamification::check_and_award_badges health flags).
#[cfg(feature = "gamification")]
const MIGRATION_023: &str = "
INSERT OR IGNORE INTO badges (id, name, description, icon, xp_reward) VALUES
    ('health_a',        'Financially Fit',   'Reached a grade-A financial health score',   '🩺', 30),
    ('health_aplus',    'Peak Health',       'Reached a grade-A+ financial health score',  '🏆', 50),
    ('emergency_ready', 'Safety Net',        'Built 6 months of emergency-fund cover',     '🛟', 30),
    ('debt_light',      'Debt Light',        'Kept EMIs under 20% of income',              '🪶', 30);
";

// Outcome-bound wealth badges — awarded when a real financial outcome is reached
// (verified backend-side in gamification::check_and_award_badges), not for activity.
#[cfg(feature = "gamification")]
const MIGRATION_024: &str = "
INSERT OR IGNORE INTO badges (id, name, description, icon, xp_reward) VALUES
    ('first_lakh',   'First Lakh',      'Grew net worth past ₹1 lakh',                  '🌱', 20),
    ('ten_lakh',     'Ten Lakh Club',   'Grew net worth past ₹10 lakh',                 '💎', 30),
    ('savings_star', 'Savings Star',    'Saved over 50% of income across 90 days',      '⭐', 30);
";

// Opt-in, time-boxed savings challenges. Progress is computed on demand from the
// transaction ledger / budgets (no per-tick writes); only status transitions and
// XP awards are persisted. Gamification-gated like the rest of the XP system.
#[cfg(feature = "gamification")]
const MIGRATION_025: &str = "
CREATE TABLE IF NOT EXISTS user_challenges (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kind         TEXT NOT NULL,          -- 'save_amount' | 'no_spend' | 'budget_hold'
    title        TEXT NOT NULL,
    icon         TEXT NOT NULL,
    target       REAL NOT NULL,          -- rupees (save_amount) or days (no_spend); unused for budget_hold
    start_date   TEXT NOT NULL,          -- YYYY-MM-DD inclusive
    end_date     TEXT NOT NULL,          -- YYYY-MM-DD inclusive
    xp_reward    INTEGER NOT NULL,
    status       TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'completed' | 'failed'
    completed_at TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_user_challenges_status ON user_challenges(status);
";

/// Tables that participate in cross-device sync (see `backup::commands`). Single
/// source of truth for both the sync-column migration below and the merge logic
/// in `backup::commands`, which re-exports this list rather than duplicating it.
pub(crate) const SYNC_TABLES: &[&str] = &[
    "accounts",
    "equity_holdings",
    "mf_holdings",
    "sip_schedules",
    "fd_holdings",
    "ppf_epf_holdings",
    "real_estate_holdings",
    "gold_holdings",
    "crypto_holdings",
    "insurance_holdings",
    "bond_holdings",
    "loans",
    "credit_cards",
    "transactions",
    "budgets",
    "net_worth_snapshots",
    "import_log",
    "goals",
    "bills",
    "recurring_transactions",
    "milestones",
];

/// Adds the two bookkeeping columns every synced table needs for merge (as opposed
/// to wholesale-replace) sync: `sync_id` is a globally-unique row identity that
/// survives across devices (unlike the local `INTEGER PRIMARY KEY`, which two
/// devices can assign to unrelated rows); `sync_updated_at` is a merge-only
/// last-write-wins clock maintained purely by triggers (see `migration_019_sync_infra`),
/// independent of whichever app-level `updated_at` column a table may or may not have.
fn migration_018_add_sync_columns() -> String {
    let mut sql = String::new();
    for table in SYNC_TABLES {
        sql.push_str(&format!("ALTER TABLE {table} ADD COLUMN sync_id TEXT;\n"));
        sql.push_str(&format!("ALTER TABLE {table} ADD COLUMN sync_updated_at TEXT;\n"));
    }
    sql
}

/// Best-effort existing timestamp to seed `sync_updated_at` from at backfill time,
/// per table (tables vary in which of `updated_at`/`created_at`/neither they have).
fn sync_backfill_source_expr(table: &str) -> &'static str {
    match table {
        "sip_schedules" | "budgets" | "net_worth_snapshots" => "datetime('now')",
        "fd_holdings" | "real_estate_holdings" | "gold_holdings" | "crypto_holdings"
        | "insurance_holdings" | "milestones" => "COALESCE(created_at, datetime('now'))",
        "import_log" => "COALESCE(imported_at, datetime('now'))",
        _ => "COALESCE(updated_at, datetime('now'))",
    }
}

/// `sync_tombstones` records a `(table, sync_id, deleted_at)` for every row ever
/// deleted from a synced table, so a delete on one device can be told apart from
/// "this device just never had that row" when merging with another device's
/// export — without this, a merge-by-union would resurrect deleted rows forever.
/// Populated automatically by the `trg_<table>_tombstone` trigger below, so
/// deletes — including ON DELETE CASCADE — need no app-code changes to be tracked.
/// Tombstones are kept indefinitely (no retention purge): correctness over file size
/// at this app's personal single/dual-device scale.
///
/// `sync_updated_at` is bumped by `trg_<table>_sync_update` only when the app's own
/// UPDATE didn't already set it — which lets the merge/import path set an explicit
/// `sync_updated_at` (copied from the remote row) without the trigger clobbering it.
/// Same COALESCE trick on insert lets import preserve a remote row's original
/// `sync_id`/`sync_updated_at` instead of minting fresh ones.
fn migration_019_sync_infra() -> String {
    let mut sql = String::from(
        "
CREATE TABLE IF NOT EXISTS sync_tombstones (
    table_name TEXT NOT NULL,
    sync_id    TEXT NOT NULL,
    deleted_at TEXT NOT NULL,
    PRIMARY KEY (table_name, sync_id)
);
",
    );
    for table in SYNC_TABLES {
        let expr = sync_backfill_source_expr(table);
        sql.push_str(&format!(
            "UPDATE {table} SET sync_id = lower(hex(randomblob(16))) WHERE sync_id IS NULL;\n"
        ));
        sql.push_str(&format!(
            "UPDATE {table} SET sync_updated_at = {expr} WHERE sync_updated_at IS NULL;\n"
        ));
        sql.push_str(&format!(
            "CREATE TRIGGER IF NOT EXISTS trg_{table}_sync_insert AFTER INSERT ON {table} BEGIN \
             UPDATE {table} SET sync_id = COALESCE(NEW.sync_id, lower(hex(randomblob(16)))), \
             sync_updated_at = COALESCE(NEW.sync_updated_at, datetime('now')) WHERE rowid = NEW.rowid; \
             END;\n"
        ));
        sql.push_str(&format!(
            "CREATE TRIGGER IF NOT EXISTS trg_{table}_sync_update AFTER UPDATE ON {table} \
             WHEN NEW.sync_updated_at IS OLD.sync_updated_at BEGIN \
             UPDATE {table} SET sync_updated_at = datetime('now') WHERE rowid = NEW.rowid; \
             END;\n"
        ));
        sql.push_str(&format!(
            "CREATE TRIGGER IF NOT EXISTS trg_{table}_tombstone AFTER DELETE ON {table} BEGIN \
             INSERT INTO sync_tombstones (table_name, sync_id, deleted_at) \
             VALUES ('{table}', OLD.sync_id, datetime('now')) \
             ON CONFLICT(table_name, sync_id) DO UPDATE SET deleted_at = excluded.deleted_at; \
             END;\n"
        ));
    }
    sql
}

/// Adds a per-row Hybrid Logical Clock column, additive and nullable exactly like
/// `migration_018_add_sync_columns` — legacy rows (and rows from a peer device that
/// hasn't updated yet) simply keep `sync_hlc = NULL` and fall back to the existing
/// `sync_updated_at` wall-clock comparison at merge time (see
/// `backup::commands::apply_backup_payload_merge`). No forced backfill.
fn migration_020_add_hlc_columns() -> String {
    let mut sql = String::new();
    for table in SYNC_TABLES {
        sql.push_str(&format!("ALTER TABLE {table} ADD COLUMN sync_hlc TEXT;\n"));
    }
    sql.push_str("ALTER TABLE sync_tombstones ADD COLUMN hlc TEXT;\n");
    sql
}

/// `sync_hlc` values are `"{physical_ms:013}:{logical:09}:{device_id}"` — zero-padded
/// so plain string comparison (`>`/`>=`, same operators the existing merge logic
/// already uses on `sync_updated_at`) sorts chronologically. `physical_ms` is
/// epoch milliseconds (finer resolution than `sync_updated_at`'s whole-second
/// `datetime('now')`); `logical` is a counter that advances instead of `physical_ms`
/// when two writes land in the same millisecond or the system clock hasn't moved
/// forward — the standard HLC bump rule. Centralizing this bump in `sync_hlc_state`
/// (one row, shared by every table's triggers) rather than scattering "compute and
/// stamp the next HLC" across every command handler avoids the exact class of
/// per-call-site drift risk `DATA_TABLES` had (see INCIDENT_REPORT.md) — one state
/// table and one trigger-body change per table is the whole surface area.
fn migration_021_hlc_state_and_triggers() -> String {
    let mut sql = String::from(
        "
CREATE TABLE IF NOT EXISTS sync_hlc_state (
    id               INTEGER PRIMARY KEY CHECK(id = 1),
    last_physical_ms INTEGER NOT NULL DEFAULT 0,
    last_logical     INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO sync_hlc_state (id, last_physical_ms, last_logical) VALUES (1, 0, 0);
",
    );
    for table in SYNC_TABLES {
        sql.push_str(&format!("DROP TRIGGER IF EXISTS trg_{table}_sync_insert;\n"));
        sql.push_str(&format!(
            "CREATE TRIGGER trg_{table}_sync_insert AFTER INSERT ON {table} BEGIN \
             UPDATE sync_hlc_state SET \
               last_logical = CASE WHEN {NOW_MS} <= last_physical_ms THEN last_logical + 1 ELSE 0 END, \
               last_physical_ms = MAX({NOW_MS}, last_physical_ms) \
             WHERE id = 1; \
             UPDATE {table} SET \
               sync_id = COALESCE(NEW.sync_id, lower(hex(randomblob(16)))), \
               sync_updated_at = COALESCE(NEW.sync_updated_at, datetime('now')), \
               sync_hlc = COALESCE(NEW.sync_hlc, {HLC_EXPR}) \
             WHERE rowid = NEW.rowid; \
             END;\n",
            NOW_MS = NOW_MS_EXPR,
            HLC_EXPR = hlc_expr(),
        ));

        sql.push_str(&format!("DROP TRIGGER IF EXISTS trg_{table}_sync_update;\n"));
        sql.push_str(&format!(
            "CREATE TRIGGER trg_{table}_sync_update AFTER UPDATE ON {table} \
             WHEN NEW.sync_updated_at IS OLD.sync_updated_at BEGIN \
             UPDATE sync_hlc_state SET \
               last_logical = CASE WHEN {NOW_MS} <= last_physical_ms THEN last_logical + 1 ELSE 0 END, \
               last_physical_ms = MAX({NOW_MS}, last_physical_ms) \
             WHERE id = 1; \
             UPDATE {table} SET \
               sync_updated_at = datetime('now'), \
               sync_hlc = {HLC_EXPR} \
             WHERE rowid = NEW.rowid; \
             END;\n",
            NOW_MS = NOW_MS_EXPR,
            HLC_EXPR = hlc_expr(),
        ));

        sql.push_str(&format!("DROP TRIGGER IF EXISTS trg_{table}_tombstone;\n"));
        sql.push_str(&format!(
            "CREATE TRIGGER trg_{table}_tombstone AFTER DELETE ON {table} BEGIN \
             UPDATE sync_hlc_state SET \
               last_logical = CASE WHEN {NOW_MS} <= last_physical_ms THEN last_logical + 1 ELSE 0 END, \
               last_physical_ms = MAX({NOW_MS}, last_physical_ms) \
             WHERE id = 1; \
             INSERT INTO sync_tombstones (table_name, sync_id, deleted_at, hlc) \
             VALUES ('{table}', OLD.sync_id, datetime('now'), {HLC_EXPR}) \
             ON CONFLICT(table_name, sync_id) DO UPDATE SET \
               deleted_at = excluded.deleted_at, hlc = excluded.hlc; \
             END;\n",
            NOW_MS = NOW_MS_EXPR,
            HLC_EXPR = hlc_expr(),
        ));
    }
    sql
}

/// SQLite epoch-milliseconds expression (no native `unixepoch(..., 'subsec')` in the
/// bundled SQLCipher version here, so this is the julianday-based equivalent).
const NOW_MS_EXPR: &str = "(CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))";

/// Reads `sync_hlc_state` (just bumped by the same trigger body, above) and this
/// device's `device_id` (see `db::ensure_device_id`) to build one `sync_hlc` value.
fn hlc_expr() -> String {
    "(SELECT printf('%013d:%09d:%s', last_physical_ms, last_logical, \
      (SELECT value FROM app_settings WHERE key = 'device_id')) \
      FROM sync_hlc_state WHERE id = 1)"
        .to_string()
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

/// The 9 default net-worth milestones — shared with `settings::commands::wipe_all_data`,
/// which re-seeds them after a wipe (the table also holds user-added custom
/// milestones and `achieved_at` progress, so a wipe means a real `DELETE`,
/// not something a migration's `IF NOT EXISTS` would ever re-run).
pub(crate) const DEFAULT_MILESTONES_SEED: &str = "
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

const MIGRATION_008: &str = "
CREATE TABLE IF NOT EXISTS milestones (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    amount      REAL NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    is_custom   INTEGER NOT NULL DEFAULT 0,
    achieved_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
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

// Shared, user-manageable category list for transactions/budgets/recurring transactions.
// Seeded with the legacy hardcoded defaults, then backfilled with any category text already
// present in existing data (e.g. imported from another app) so nothing is lost or hidden.
/// The 13 default category names — shared with `settings::commands::wipe_all_data`,
/// which re-seeds them after a wipe (unlike a fresh migration, a wipe leaves
/// no transaction/budget data behind to backfill from, so only this part of
/// MIGRATION_015 applies).
pub(crate) const DEFAULT_CATEGORIES_SEED: &str = "
INSERT OR IGNORE INTO categories (name) VALUES
    ('Food'), ('Rent'), ('EMI'), ('Travel'), ('Medical'), ('Utilities'),
    ('Entertainment'), ('Education'), ('Shopping'), ('Dividend'), ('Interest'),
    ('Salary'), ('Other');
";

const MIGRATION_015: &str = "
CREATE TABLE IF NOT EXISTS categories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT UNIQUE NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const MIGRATION_015_BACKFILL: &str = "
INSERT OR IGNORE INTO categories (name)
    SELECT DISTINCT category FROM transactions WHERE category IS NOT NULL AND category != '';
INSERT OR IGNORE INTO categories (name)
    SELECT DISTINCT category FROM budgets WHERE category IS NOT NULL AND category != '';
INSERT OR IGNORE INTO categories (name)
    SELECT DISTINCT category FROM recurring_transactions WHERE category IS NOT NULL AND category != '';
";

// ALTER TABLE is not idempotent — this migration is run with error ignored in run_migrations
const MIGRATION_016: &str = "
ALTER TABLE transactions ADD COLUMN tag TEXT;
";

// Dedup state for the background reminder scheduler (notifications/scheduler.rs) —
// a JSON array of "source:source_id:date" keys already notified, so a bill/maturity
// isn't re-notified on every interval tick before its due date rolls over.
const MIGRATION_017: &str = "
INSERT OR IGNORE INTO app_settings (key, value) VALUES ('notified_reminder_ids', '[]');
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

    #[test]
    fn migration_015_seeds_defaults_and_backfills_existing_data() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        // Simulate a pre-existing/imported transaction using a category NOT in the
        // hardcoded default list — this must survive migration as a real category row.
        conn.execute(
            "INSERT INTO transactions (date, type, amount, category) VALUES ('2026-01-01','expense',10,'Consulting Fees')",
            [],
        ).unwrap();

        run_migrations(&conn).expect("re-running migrations after seeding data should succeed");

        let default_count: i64 = conn.query_row(
            "SELECT count(*) FROM categories WHERE name = 'Food'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(default_count, 1, "hardcoded default categories should be seeded");

        let backfilled_count: i64 = conn.query_row(
            "SELECT count(*) FROM categories WHERE name = 'Consulting Fees'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(backfilled_count, 1, "pre-existing/imported category text must be backfilled, not dropped");
    }

    #[test]
    fn migration_016_adds_tag_column_idempotently() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        // First run already happened via test_db_pool(); running again must not error
        // even though ALTER TABLE ADD COLUMN fails on a column that already exists.
        run_migrations(&conn).expect("second run_migrations call should be a no-op, not an error");
        run_migrations(&conn).expect("third run_migrations call should also be a no-op");

        conn.execute(
            "INSERT INTO transactions (date, type, amount, tag) VALUES ('2026-01-01','expense',10,'House')",
            [],
        ).expect("tag column should exist and accept values");
    }
}
