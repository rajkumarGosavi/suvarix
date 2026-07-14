//! Financial Health Score — a single 0–100 gauge of the user's money situation,
//! built from six behaviour-first pillars scored against Indian personal-finance
//! rules of thumb. Pure read over data already in the DB (no new data collection).
//!
//! Each pillar is scored 0–100, then combined by a fixed weight. Pillars whose
//! inputs are missing (e.g. no income logged yet) return `None` and are excluded
//! with the remaining weights re-normalised, so a new user isn't unfairly tanked
//! for data they simply haven't entered.
//!
//! The score is *core* (works with gamification off). Gamification only rewards
//! *moving it up* — see `record_health_snapshot` + the frontend health-check.

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::portfolio::calculator::{calc_allocation, calc_net_worth};

// ─── Structs ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pillar {
    /// Stable machine key (used by the frontend to pick icons / badge flags).
    pub key: String,
    pub label: String,
    /// 0–100, or `None` when inputs are missing (excluded from the overall).
    pub score: Option<f64>,
    /// Fixed weight in the overall (behaviour-first mix; see `PILLAR_WEIGHTS`).
    pub weight: f64,
    /// Plain-language current state, e.g. "You save 12% of income".
    pub status: String,
    /// The single highest-leverage action to improve this pillar (empty if maxed / N/A).
    pub top_fix: String,
    /// Up to how many points the *overall* score could rise if this pillar hit 100,
    /// after weight re-normalisation. Honest ceiling, rounded.
    pub potential_gain: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancialHealthScore {
    /// Weighted 0–100 across non-null pillars. 0 when nothing is computable yet.
    pub overall: f64,
    /// Letter band: A+ / A / B / C / D.
    pub grade: String,
    pub pillars: Vec<Pillar>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSnapshotResult {
    /// The most recent score recorded on an *earlier* day (None on first-ever run).
    pub previous_score: Option<f64>,
    /// The score stored for today.
    pub today_score: f64,
    /// True if today's row already existed before this call (so XP was likely
    /// already awarded today — the frontend uses this to stay improvement-only).
    pub already_recorded_today: bool,
}

// Behaviour-first weights (must sum to 100). Savings / emergency fund / debt are
// the things the user most directly controls, so they carry the most weight.
const W_SAVINGS: f64 = 20.0;
const W_EMERGENCY: f64 = 20.0;
const W_DEBT: f64 = 20.0;
const W_DIVERSIFICATION: f64 = 15.0;
const W_PROTECTION: f64 = 10.0;
const W_NETWORTH: f64 = 15.0;

// ─── Scoring helpers ─────────────────────────────────────────────────────────

/// Linear score in [0,100]: `at0` maps to 0, `at100` maps to 100, clamped.
/// Works in either direction (`at100` may be below `at0` for "lower is better").
fn scale(value: f64, at0: f64, at100: f64) -> f64 {
    if (at100 - at0).abs() < f64::EPSILON {
        return 0.0;
    }
    let t = (value - at0) / (at100 - at0);
    (t * 100.0).clamp(0.0, 100.0)
}

fn grade_for(overall: f64) -> &'static str {
    match overall {
        x if x >= 85.0 => "A+",
        x if x >= 70.0 => "A",
        x if x >= 55.0 => "B",
        x if x >= 40.0 => "C",
        _ => "D",
    }
}

fn sum_query(conn: &Connection, sql: &str) -> f64 {
    conn.query_row(sql, [], |r| r.get::<_, f64>(0)).unwrap_or(0.0)
}

// Read a positive numeric app_setting, falling back to `default` if missing,
// unparseable, or non-positive.
fn setting_f64(conn: &Connection, key: &str, default: f64) -> f64 {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.trim().parse::<f64>().ok())
    .filter(|v| v.is_finite() && *v > 0.0)
    .unwrap_or(default)
}

// Trailing-90-day income / expense. `date` is stored ISO-ish, so `date(date)` is safe.
fn income_90(conn: &Connection) -> f64 {
    sum_query(
        conn,
        "SELECT COALESCE(SUM(amount),0) FROM transactions \
         WHERE type = 'income' AND date(date) >= date('now','-90 days')",
    )
}

fn expense_90(conn: &Connection) -> f64 {
    sum_query(
        conn,
        "SELECT COALESCE(SUM(amount),0) FROM transactions \
         WHERE type IN ('expense','emi') AND date(date) >= date('now','-90 days')",
    )
}

// Liquid buffer for emergency-fund maths: positive cash balance (savings, from the
// net transaction ledger) + FDs (near-liquid) + liquid/overnight mutual funds.
// Liquid-MF detection is a name heuristic — no fund-category column exists, so we
// match scheme names containing "liquid" or "overnight" (the SEBI liquid-equivalent
// categories). Misses oddly-named funds; over-counts a fund merely named "...liquid...".
fn liquid_assets(conn: &Connection) -> f64 {
    let cash = sum_query(
        conn,
        "SELECT COALESCE(SUM(CASE \
            WHEN type IN ('income','dividend','interest') THEN amount \
            WHEN type IN ('expense','emi') THEN -amount ELSE 0 END),0) FROM transactions",
    );
    let fd = sum_query(
        conn,
        "SELECT COALESCE(SUM(COALESCE(maturity_amount, principal)),0) FROM fd_holdings",
    );
    let liquid_mf = sum_query(
        conn,
        "SELECT COALESCE(SUM(units * COALESCE(current_nav, avg_nav)),0) FROM mf_holdings \
         WHERE lower(scheme_name) LIKE '%liquid%' OR lower(scheme_name) LIKE '%overnight%'",
    );
    cash.max(0.0) + fd + liquid_mf
}

// ─── Pillar builders ─────────────────────────────────────────────────────────

fn pillar_savings(conn: &Connection) -> Pillar {
    let income = income_90(conn);
    let expense = expense_90(conn);
    let (score, status, top_fix) = if income <= 0.0 {
        (
            None,
            "No income logged in the last 90 days".to_string(),
            "Log your salary/income so we can track your savings rate".to_string(),
        )
    } else {
        let rate = (income - expense) / income;
        let s = scale(rate, 0.0, 0.20); // 20%+ saved = full marks
        let status = format!("You save {:.0}% of income (target 20%+)", (rate * 100.0).max(0.0));
        let fix = if s >= 100.0 {
            String::new()
        } else {
            "Trim discretionary spending to push savings toward 20% of income".to_string()
        };
        (Some(s), status, fix)
    };
    Pillar {
        key: "savings".into(),
        label: "Savings Rate".into(),
        score,
        weight: W_SAVINGS,
        status,
        top_fix,
        potential_gain: 0.0,
    }
}

fn pillar_emergency(conn: &Connection) -> Pillar {
    let expense = expense_90(conn);
    let monthly = expense / 3.0;
    let liquid = liquid_assets(conn);
    let target = setting_f64(conn, "emergency_fund_target_months", 6.0);
    let (score, status, top_fix) = if monthly <= 0.0 {
        (
            None,
            "Not enough expense history to size an emergency fund".to_string(),
            "Log your monthly expenses to measure emergency-fund cover".to_string(),
        )
    } else {
        let months = liquid / monthly;
        let s = scale(months, 0.0, target); // target months' cover = full marks
        let status = format!(
            "{:.1} months of expenses in liquid savings (target {:.0})",
            months, target
        );
        let fix = if s >= 100.0 {
            String::new()
        } else {
            format!("Build cash/FD reserves toward {:.0} months of expenses", target)
        };
        (Some(s), status, fix)
    };
    Pillar {
        key: "emergency".into(),
        label: "Emergency Fund".into(),
        score,
        weight: W_EMERGENCY,
        status,
        top_fix,
        potential_gain: 0.0,
    }
}

fn pillar_debt(conn: &Connection) -> Pillar {
    let monthly_income = income_90(conn) / 3.0;
    let total_emi = sum_query(conn, "SELECT COALESCE(SUM(emi_amount),0) FROM loans");
    let cc_balance = sum_query(conn, "SELECT COALESCE(SUM(current_balance),0) FROM credit_cards");
    let cc_limit = sum_query(conn, "SELECT COALESCE(SUM(credit_limit),0) FROM credit_cards");

    let mut subs: Vec<f64> = Vec::new();
    let mut worst: Option<(&str, f64)> = None; // (fix, sub-score) for the weakest sub

    // EMI-to-income: <=20% ideal, >=50% critical. Needs income to be meaningful.
    if monthly_income > 0.0 {
        let ratio = total_emi / monthly_income;
        let s = scale(ratio, 0.50, 0.20);
        subs.push(s);
        if worst.map_or(true, |(_, w)| s < w) {
            worst = Some(("Reduce EMIs or prepay a loan to get EMIs under 20% of income", s));
        }
    }
    // Credit-card utilisation: <30% ideal, >=90% critical. Only if a card exists.
    if cc_limit > 0.0 {
        let util = cc_balance / cc_limit;
        let s = scale(util, 0.90, 0.30);
        subs.push(s);
        if worst.map_or(true, |(_, w)| s < w) {
            worst = Some(("Pay down cards to keep utilisation below 30% of the limit", s));
        }
    }

    if subs.is_empty() {
        // No loans-with-income and no cards → nothing to score; treat as debt-free good state.
        return Pillar {
            key: "debt".into(),
            label: "Debt Burden".into(),
            score: Some(100.0),
            weight: W_DEBT,
            status: "No EMIs or credit-card debt on record".into(),
            top_fix: String::new(),
            potential_gain: 0.0,
        };
    }

    let avg = subs.iter().sum::<f64>() / subs.len() as f64;
    let status = format!(
        "EMIs {} of income, card use {} of limit",
        if monthly_income > 0.0 {
            format!("{:.0}%", (total_emi / monthly_income * 100.0))
        } else {
            "n/a".to_string()
        },
        if cc_limit > 0.0 {
            format!("{:.0}%", (cc_balance / cc_limit * 100.0))
        } else {
            "n/a".to_string()
        },
    );
    let top_fix = if avg >= 100.0 {
        String::new()
    } else {
        worst.map(|(f, _)| f.to_string()).unwrap_or_default()
    };
    Pillar {
        key: "debt".into(),
        label: "Debt Burden".into(),
        score: Some(avg),
        weight: W_DEBT,
        status,
        top_fix,
        potential_gain: 0.0,
    }
}

fn pillar_diversification(conn: &Connection) -> Pillar {
    let allocation = calc_allocation(conn).unwrap_or_default();
    let classes: Vec<_> = allocation.iter().filter(|a| a.value > 0.0).collect();
    let (score, status, top_fix) = if classes.is_empty() {
        (
            None,
            "No holdings yet to diversify".to_string(),
            "Add your first investment to start building a portfolio".to_string(),
        )
    } else {
        let n = classes.len() as f64;
        let max_pct = classes.iter().map(|a| a.percent).fold(0.0_f64, f64::max);
        let breadth = scale(n, 0.0, 4.0); // 4+ asset classes = full breadth
        let concentration = scale(max_pct, 100.0, 40.0); // no class over 40% = full marks
        let s = breadth * 0.5 + concentration * 0.5;
        let status = format!("{} asset classes, largest is {:.0}% of portfolio", classes.len(), max_pct);
        let fix = if breadth < concentration {
            "Spread money across more asset classes (equity, MF, FD, gold…)".to_string()
        } else if s < 100.0 {
            "Rebalance so no single asset class exceeds ~40%".to_string()
        } else {
            String::new()
        };
        (Some(s), status, fix)
    };
    Pillar {
        key: "diversification".into(),
        label: "Diversification".into(),
        score,
        weight: W_DIVERSIFICATION,
        status,
        top_fix,
        potential_gain: 0.0,
    }
}

fn pillar_protection(conn: &Connection) -> Pillar {
    let has_life = conn
        .query_row(
            "SELECT COUNT(*) FROM insurance_holdings \
             WHERE insurance_type IN ('life','term','ulip')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    let has_health = conn
        .query_row(
            "SELECT COUNT(*) FROM insurance_holdings WHERE insurance_type = 'health'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    let score = (if has_life { 50.0 } else { 0.0 }) + (if has_health { 50.0 } else { 0.0 });
    let status = match (has_life, has_health) {
        (true, true) => "Life and health insurance in place".to_string(),
        (true, false) => "Life cover present, health cover missing".to_string(),
        (false, true) => "Health cover present, life cover missing".to_string(),
        (false, false) => "No life or health insurance on record".to_string(),
    };
    let top_fix = match (has_life, has_health) {
        (true, true) => String::new(),
        (_, false) => "Add a health insurance policy to protect against medical shocks".to_string(),
        (false, _) => "Add life/term cover if anyone depends on your income".to_string(),
    };
    Pillar {
        key: "protection".into(),
        label: "Protection".into(),
        score: Some(score),
        weight: W_PROTECTION,
        status,
        top_fix,
        potential_gain: 0.0,
    }
}

fn pillar_networth(conn: &Connection) -> Pillar {
    let nw = calc_net_worth(conn).map(|s| s.net_worth).unwrap_or(0.0);
    let positivity = if nw > 0.0 { 100.0 } else { 0.0 };

    // Growth vs the oldest snapshot in the trailing ~120 days.
    let old_nw: Option<f64> = conn
        .query_row(
            "SELECT net_worth FROM net_worth_snapshots \
             WHERE snapshot_date >= date('now','-120 days') \
             ORDER BY snapshot_date ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();

    let (score, growth_note) = match old_nw {
        Some(old) if old.abs() > f64::EPSILON => {
            let growth = (nw - old) / old.abs();
            let g = scale(growth, -0.10, 0.10); // -10% → 0, flat → 50, +10% → 100
            (positivity * 0.4 + g * 0.6, format!(", {:+.0}% over ~3 months", growth * 100.0))
        }
        _ => (positivity, String::new()),
    };

    let status = if nw > 0.0 {
        format!("Positive net worth{}", growth_note)
    } else {
        "Net worth is zero or negative".to_string()
    };
    let top_fix = if score >= 100.0 {
        String::new()
    } else if nw <= 0.0 {
        "Grow assets and pay down debt to move net worth positive".to_string()
    } else {
        "Keep net worth trending up — invest surplus consistently".to_string()
    };
    Pillar {
        key: "networth".into(),
        label: "Net-Worth Trend".into(),
        score: Some(score),
        weight: W_NETWORTH,
        status,
        top_fix,
        potential_gain: 0.0,
    }
}

// ─── Core computation ────────────────────────────────────────────────────────

pub(crate) fn compute_score(conn: &Connection) -> FinancialHealthScore {
    let mut pillars = vec![
        pillar_savings(conn),
        pillar_emergency(conn),
        pillar_debt(conn),
        pillar_diversification(conn),
        pillar_protection(conn),
        pillar_networth(conn),
    ];

    let active_weight: f64 = pillars.iter().filter(|p| p.score.is_some()).map(|p| p.weight).sum();

    let overall = if active_weight <= 0.0 {
        0.0
    } else {
        pillars
            .iter()
            .filter_map(|p| p.score.map(|s| s * p.weight))
            .sum::<f64>()
            / active_weight
    };

    // Honest per-pillar ceiling: how many overall points this pillar could still add,
    // using the re-normalised weight (matches how `overall` is actually computed).
    for p in pillars.iter_mut() {
        if let Some(s) = p.score {
            if active_weight > 0.0 {
                p.potential_gain = ((p.weight / active_weight) * (100.0 - s) * 10.0).round() / 10.0;
            }
        }
    }

    FinancialHealthScore {
        overall: (overall * 10.0).round() / 10.0,
        grade: grade_for(overall).to_string(),
        pillars,
    }
}

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_financial_health(state: State<DbState>) -> Result<FinancialHealthScore> {
    let conn = state.0.get()?;
    Ok(compute_score(&conn))
}

/// Records `score` against today's date (idempotent per day) and reports the most
/// recent *earlier* day's score, so the frontend can award improvement-only XP.
#[tauri::command]
pub fn record_health_snapshot(score: f64, state: State<DbState>) -> Result<HealthSnapshotResult> {
    let conn = state.0.get()?;
    let today = chrono::Local::now().date_naive().to_string();

    let already_recorded_today: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM health_score_history WHERE snapshot_date = ?1",
            [&today],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))?
        > 0;

    let previous_score: Option<f64> = conn
        .query_row(
            "SELECT score FROM health_score_history WHERE snapshot_date < ?1 \
             ORDER BY snapshot_date DESC LIMIT 1",
            [&today],
            |r| r.get(0),
        )
        .ok();

    conn.execute(
        "INSERT INTO health_score_history (snapshot_date, score) VALUES (?1, ?2) \
         ON CONFLICT(snapshot_date) DO UPDATE SET score = excluded.score",
        rusqlite::params![today, score],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HealthSnapshotResult {
        previous_score,
        today_score: score,
        already_recorded_today,
    })
}

// ─── Emergency fund (first-class view over the same liquid/expense inputs) ──────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyFund {
    /// Average monthly expense (trailing 90 days / 3).
    pub monthly_expense: f64,
    /// Liquid buffer: positive cash + FDs + liquid/overnight MFs.
    pub liquid_assets: f64,
    /// User-set goal in months of expenses (default 6).
    pub target_months: f64,
    /// `monthly_expense * target_months`.
    pub target_amount: f64,
    /// How many months of expenses the buffer currently covers.
    pub months_covered: f64,
    /// 0–100 toward the target.
    pub coverage_pct: f64,
    /// Amount still needed to hit the target (0 if met).
    pub shortfall: f64,
    /// "underfunded" | "on_track" | "funded" — drives the frontend colour/copy.
    pub status: String,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Emergency-fund status computed from the same liquid-assets / expense inputs the
/// health score uses. Read-only; target comes from `emergency_fund_target_months`.
#[tauri::command]
pub fn get_emergency_fund(state: State<DbState>) -> Result<EmergencyFund> {
    let conn = state.0.get()?;
    Ok(compute_emergency_fund(&conn))
}

fn compute_emergency_fund(conn: &Connection) -> EmergencyFund {
    let monthly = expense_90(conn) / 3.0;
    let liquid = liquid_assets(conn);
    let target_months = setting_f64(conn, "emergency_fund_target_months", 6.0);
    let target_amount = monthly * target_months;

    let months_covered = if monthly > 0.0 { liquid / monthly } else { 0.0 };
    let coverage_pct = if target_amount > 0.0 {
        (liquid / target_amount * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let shortfall = (target_amount - liquid).max(0.0);
    let status = if target_amount <= 0.0 {
        "underfunded"
    } else if coverage_pct >= 100.0 {
        "funded"
    } else if coverage_pct >= 50.0 {
        "on_track"
    } else {
        "underfunded"
    };

    EmergencyFund {
        monthly_expense: round2(monthly),
        liquid_assets: round2(liquid),
        target_months,
        target_amount: round2(target_amount),
        months_covered: round2(months_covered),
        coverage_pct: round2(coverage_pct),
        shortfall: round2(shortfall),
        status: status.to_string(),
    }
}

/// Sets the emergency-fund goal (months of expenses). Clamped to 1–24.
#[tauri::command]
pub fn set_emergency_fund_target(months: f64, state: State<DbState>) -> Result<()> {
    if !(months.is_finite() && months >= 1.0 && months <= 24.0) {
        return Err(AppError::Validation("target months must be between 1 and 24".into()));
    }
    let conn = state.0.get()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES ('emergency_fund_target_months', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [months.to_string()],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;
    use rusqlite::Connection;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        run_migrations(&c).unwrap();
        c
    }

    #[test]
    fn scale_clamps_and_interpolates() {
        assert_eq!(scale(0.20, 0.0, 0.20), 100.0);
        assert_eq!(scale(0.0, 0.0, 0.20), 0.0);
        assert_eq!(scale(0.10, 0.0, 0.20), 50.0);
        assert_eq!(scale(0.50, 0.0, 0.20), 100.0); // clamped high
        assert_eq!(scale(-0.5, 0.0, 0.20), 0.0); // clamped low
                                                 // inverted (lower is better)
        assert_eq!(scale(0.20, 0.50, 0.20), 100.0);
        assert_eq!(scale(0.50, 0.50, 0.20), 0.0);
    }

    #[test]
    fn emergency_fund_coverage_and_shortfall() {
        let c = conn();
        // 90 days of expenses = 90000 → monthly 30000; target 6 months = 180000.
        c.execute(
            "INSERT INTO transactions (type, amount, date) VALUES ('expense', 90000, date('now','-10 days'))",
            [],
        )
        .unwrap();
        // Liquid buffer via income cash: 90000 in the ledger.
        c.execute(
            "INSERT INTO transactions (type, amount, date) VALUES ('income', 180000, date('now','-10 days'))",
            [],
        )
        .unwrap();
        let ef = compute_emergency_fund(&c);
        assert_eq!(ef.monthly_expense, 30000.0);
        // cash = 180000 income - 90000 expense = 90000 liquid → 3 months.
        assert_eq!(ef.months_covered, 3.0);
        assert_eq!(ef.target_amount, 180000.0);
        assert_eq!(ef.shortfall, 90000.0);
        assert_eq!(ef.status, "on_track"); // 50% coverage
    }

    #[test]
    fn grade_bands() {
        assert_eq!(grade_for(90.0), "A+");
        assert_eq!(grade_for(70.0), "A");
        assert_eq!(grade_for(55.0), "B");
        assert_eq!(grade_for(40.0), "C");
        assert_eq!(grade_for(10.0), "D");
    }

    #[test]
    fn empty_db_scores_without_panicking() {
        let c = conn();
        let result = compute_score(&c);
        // No income/expense/holdings → savings, emergency, diversification are null;
        // debt (no debt) = 100, protection (none) = 0, networth (zero) = 0.
        assert!(result.overall >= 0.0 && result.overall <= 100.0);
        let debt = result.pillars.iter().find(|p| p.key == "debt").unwrap();
        assert_eq!(debt.score, Some(100.0));
        let savings = result.pillars.iter().find(|p| p.key == "savings").unwrap();
        assert_eq!(savings.score, None);
    }

    #[test]
    fn snapshot_reports_previous_and_is_idempotent_per_day() {
        let c = conn();
        c.execute(
            "INSERT INTO health_score_history (snapshot_date, score) VALUES ('2020-01-01', 40.0)",
            [],
        )
        .unwrap();
        let today = chrono::Local::now().date_naive().to_string();

        // First record today: previous is the old row, not-yet-recorded.
        let prev: Option<f64> = c
            .query_row(
                "SELECT score FROM health_score_history WHERE snapshot_date < ?1 \
                 ORDER BY snapshot_date DESC LIMIT 1",
                [&today],
                |r| r.get(0),
            )
            .ok();
        assert_eq!(prev, Some(40.0));

        c.execute(
            "INSERT INTO health_score_history (snapshot_date, score) VALUES (?1, 55.0) \
             ON CONFLICT(snapshot_date) DO UPDATE SET score = excluded.score",
            rusqlite::params![today],
        )
        .unwrap();
        let count: i64 = c
            .query_row("SELECT COUNT(*) FROM health_score_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2); // old + today, no duplicate
    }
}
