use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};

// ─── Structs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub xp_reward: i64,
    pub earned_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreakInfo {
    pub streak_type: String,
    pub current_count: i64,
    pub best_count: i64,
    pub last_activity_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamificationStats {
    pub total_xp: i64,
    pub level: String,
    pub level_progress_pct: f64,
    pub next_level_xp_needed: i64,
    pub badges: Vec<Badge>,
    pub streaks: Vec<StreakInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XpAwardResult {
    pub new_xp: i64,
    pub level_changed: bool,
    pub new_level: String,
    pub new_badges: Vec<Badge>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreakUpdateResult {
    pub current_count: i64,
    pub best_count: i64,
    pub is_new_best: bool,
    pub streak_bonus_xp: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeContext {
    #[serde(default)]
    pub check_first_investment: bool,
    #[serde(default)]
    pub check_first_goal: bool,
    #[serde(default)]
    pub check_first_milestone: bool,
    #[serde(default)]
    pub check_diversification: bool,
    #[serde(default)]
    pub check_debt_destroyer: bool,
    #[serde(default)]
    pub check_crore_club: bool,
    // Financial-health milestones — the frontend sets these true only when the
    // computed score / pillars actually cross the threshold (award-if-not-earned).
    #[serde(default)]
    pub check_health_a: bool,
    #[serde(default)]
    pub check_health_aplus: bool,
    #[serde(default)]
    pub check_emergency_ready: bool,
    #[serde(default)]
    pub check_debt_light: bool,
    // Outcome-bound wealth badges — the flag only *triggers* a check; the actual
    // threshold is verified backend-side against the DB (net worth / savings rate),
    // so the frontend never needs to know or be trusted with the number.
    #[serde(default)]
    pub check_first_lakh: bool,
    #[serde(default)]
    pub check_ten_lakh: bool,
    #[serde(default)]
    pub check_savings_star: bool,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn xp_to_level(xp: i64) -> (&'static str, f64, i64) {
    let thresholds: &[(&str, i64)] = &[
        ("Rookie", 100),
        ("Saver", 300),
        ("Investor", 700),
        ("Pro Investor", 1500),
        ("Market Expert", 3000),
        ("Finance Legend", i64::MAX),
    ];
    let floors = [0i64, 100, 300, 700, 1500, 3000];
    for (i, (name, next)) in thresholds.iter().enumerate() {
        if xp < *next {
            let floor = floors[i];
            let range = next - floor;
            let pct = if range > 0 {
                ((xp - floor) as f64 / range as f64) * 100.0
            } else {
                100.0
            };
            let needed = if *next == i64::MAX { 0 } else { next - xp };
            return (name, pct, needed);
        }
    }
    ("Finance Legend", 100.0, 0)
}

fn seed_xp_row(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO user_xp (id, total_xp, level) VALUES (1, 0, 'Rookie')",
        [],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub(crate) fn award_xp_internal(conn: &Connection, amount: i32) -> Result<(i64, bool, String)> {
    seed_xp_row(conn)?;
    let old_xp: i64 = conn
        .query_row("SELECT total_xp FROM user_xp WHERE id = 1", [], |r| r.get(0))
        .map_err(|e| AppError::Database(e.to_string()))?;
    let (old_level, _, _) = xp_to_level(old_xp);

    conn.execute(
        "UPDATE user_xp SET total_xp = total_xp + ?1, updated_at = datetime('now') WHERE id = 1",
        [amount],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    let new_xp: i64 = conn
        .query_row("SELECT total_xp FROM user_xp WHERE id = 1", [], |r| r.get(0))
        .map_err(|e| AppError::Database(e.to_string()))?;
    let (new_level, _, _) = xp_to_level(new_xp);
    let level_changed = old_level != new_level;

    conn.execute(
        "UPDATE user_xp SET level = ?1 WHERE id = 1",
        [new_level],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok((new_xp, level_changed, new_level.to_string()))
}

fn award_badge_if_new(conn: &Connection, badge_id: &str) -> Result<Option<Badge>> {
    let already: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM user_badges WHERE badge_id = ?1",
            [badge_id],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| AppError::Database(e.to_string()))? > 0;

    if already {
        return Ok(None);
    }

    conn.execute(
        "INSERT OR IGNORE INTO user_badges (badge_id) VALUES (?1)",
        [badge_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    award_xp_internal(conn, 20)?;

    let badge: Badge = conn
        .query_row(
            "SELECT b.id, b.name, b.description, b.icon, b.xp_reward, ub.earned_at \
             FROM badges b JOIN user_badges ub ON ub.badge_id = b.id WHERE b.id = ?1",
            [badge_id],
            |r| {
                Ok(Badge {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    icon: r.get(3)?,
                    xp_reward: r.get(4)?,
                    earned_at: r.get(5)?,
                })
            },
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Some(badge))
}

fn has_holding_in(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {} LIMIT 1", table),
        [],
        |r| r.get::<_, i64>(0),
    )
    .unwrap_or(0) > 0
}

// ─── Bootstrap ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn bootstrap_gamification(state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;

    let already_done: bool = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'gamification_bootstrapped'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_default()
        == "true";

    if already_done {
        return Ok(());
    }

    seed_xp_row(&conn)?;

    let holding_tables = [
        "equity_holdings", "mf_holdings", "fd_holdings", "ppf_epf_holdings",
        "real_estate_holdings", "gold_holdings", "crypto_holdings",
        "insurance_holdings", "bond_holdings",
    ];

    // XP + badges for existing holdings
    let filled: Vec<&str> = holding_tables.iter().copied().filter(|t| has_holding_in(&conn, t)).collect();
    if !filled.is_empty() {
        let _ = award_badge_if_new(&conn, "first_investment");
        award_xp_internal(&conn, (filled.len() as i32) * 10)?;
    }
    if filled.len() >= 5 {
        let _ = award_badge_if_new(&conn, "diversified_investor");
    }

    // XP + badges for achieved goals
    let achieved_goals: i64 = conn
        .query_row("SELECT COUNT(*) FROM goals WHERE achieved_at IS NOT NULL", [], |r| r.get(0))
        .unwrap_or(0);
    if achieved_goals > 0 {
        let _ = award_badge_if_new(&conn, "goal_getter");
        award_xp_internal(&conn, (achieved_goals.min(4) * 50) as i32)?;
    }

    // XP + badges for achieved milestones
    let achieved_milestones: i64 = conn
        .query_row("SELECT COUNT(*) FROM milestones WHERE achieved_at IS NOT NULL", [], |r| r.get(0))
        .unwrap_or(0);
    if achieved_milestones > 0 {
        let _ = award_badge_if_new(&conn, "milestone_hunter");
        award_xp_internal(&conn, (achieved_milestones.min(5) * 100) as i32)?;
    }

    // Crore club — any milestone >= 1Cr achieved
    let crore_achieved: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM milestones WHERE amount >= 10000000 AND achieved_at IS NOT NULL",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) > 0;
    if crore_achieved {
        let _ = award_badge_if_new(&conn, "crore_club");
    }

    // XP for existing transactions (capped at 100)
    let tx_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
        .unwrap_or(0);
    if tx_count > 0 {
        award_xp_internal(&conn, (tx_count.min(100) * 5) as i32)?;
    }

    // Centurion check after all bootstrap XP
    let xp_now: i64 = conn
        .query_row("SELECT total_xp FROM user_xp WHERE id = 1", [], |r| r.get(0))
        .unwrap_or(0);
    if xp_now >= 100 {
        let _ = award_badge_if_new(&conn, "centurion");
    }

    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('gamification_bootstrapped', 'true')",
        [],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(())
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_gamification_stats(state: State<DbState>) -> Result<GamificationStats> {
    let conn = state.0.get()?;
    seed_xp_row(&conn)?;

    let (total_xp, level, level_progress_pct, next_level_xp_needed) = {
        let xp: i64 = conn
            .query_row("SELECT total_xp FROM user_xp WHERE id = 1", [], |r| r.get(0))
            .map_err(|e| AppError::Database(e.to_string()))?;
        let (lvl, pct, needed) = xp_to_level(xp);
        (xp, lvl.to_string(), pct, needed)
    };

    let mut badges = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT b.id, b.name, b.description, b.icon, b.xp_reward, ub.earned_at \
                 FROM badges b LEFT JOIN user_badges ub ON ub.badge_id = b.id \
                 ORDER BY CASE WHEN ub.earned_at IS NULL THEN 1 ELSE 0 END, ub.earned_at DESC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Badge {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    icon: r.get(3)?,
                    xp_reward: r.get(4)?,
                    earned_at: r.get(5)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        for row in rows {
            badges.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
    }

    let mut streaks = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT streak_type, current_count, best_count, last_activity_date FROM streaks",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(StreakInfo {
                    streak_type: r.get(0)?,
                    current_count: r.get(1)?,
                    best_count: r.get(2)?,
                    last_activity_date: r.get(3)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;
        for row in rows {
            streaks.push(row.map_err(|e| AppError::Database(e.to_string()))?);
        }
    }

    Ok(GamificationStats {
        total_xp,
        level,
        level_progress_pct,
        next_level_xp_needed,
        badges,
        streaks,
    })
}

#[tauri::command]
pub fn award_xp(reason: String, amount: i32, state: State<DbState>) -> Result<XpAwardResult> {
    let conn = state.0.get()?;
    let _ = reason;
    let (new_xp, level_changed, new_level) = award_xp_internal(&conn, amount)?;

    let mut new_badges = Vec::new();
    if new_xp >= 100 {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "centurion") {
            new_badges.push(b);
        }
    }

    Ok(XpAwardResult {
        new_xp,
        level_changed,
        new_level,
        new_badges,
    })
}

#[tauri::command]
pub fn update_streak(streak_type: String, state: State<DbState>) -> Result<StreakUpdateResult> {
    let conn = state.0.get()?;

    let today = chrono::Local::now().date_naive().to_string();

    let row = conn
        .query_row(
            "SELECT current_count, best_count, last_activity_date FROM streaks WHERE streak_type = ?1",
            [&streak_type],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

    let (current, best, last_date) = row;

    if last_date.as_deref() == Some(&today) {
        return Ok(StreakUpdateResult {
            current_count: current,
            best_count: best,
            is_new_best: false,
            streak_bonus_xp: 0,
        });
    }

    let new_current = if let Some(ref last) = last_date {
        let last_naive = chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive() - chrono::Duration::days(999));
        let today_naive = chrono::Local::now().date_naive();
        let days_since = (today_naive - last_naive).num_days();
        if days_since <= 7 { current + 1 } else { 1 }
    } else {
        1
    };

    let new_best = new_current.max(best);
    let is_new_best = new_current > best;

    conn.execute(
        "UPDATE streaks SET current_count = ?1, best_count = ?2, last_activity_date = ?3, updated_at = datetime('now') WHERE streak_type = ?4",
        rusqlite::params![new_current, new_best, &today, &streak_type],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut streak_bonus_xp = 0i64;
    if new_current == 7 {
        award_xp_internal(&conn, 25)?;
        streak_bonus_xp = 25;
    }
    if new_current == 30 {
        award_xp_internal(&conn, 50)?;
        streak_bonus_xp = 50;
    }

    Ok(StreakUpdateResult {
        current_count: new_current,
        best_count: new_best,
        is_new_best,
        streak_bonus_xp,
    })
}

#[tauri::command]
pub fn check_and_award_badges(context: BadgeContext, state: State<DbState>) -> Result<Vec<Badge>> {
    let conn = state.0.get()?;
    let mut earned = Vec::new();

    if context.check_first_investment {
        let tables = [
            "equity_holdings", "mf_holdings", "fd_holdings", "ppf_epf_holdings",
            "real_estate_holdings", "gold_holdings", "crypto_holdings",
            "insurance_holdings", "bond_holdings",
        ];
        let has_any = tables.iter().any(|t| has_holding_in(&conn, t));
        if has_any {
            if let Ok(Some(b)) = award_badge_if_new(&conn, "first_investment") {
                earned.push(b);
            }
        }
    }

    if context.check_diversification {
        let tables = [
            "equity_holdings", "mf_holdings", "fd_holdings", "ppf_epf_holdings",
            "real_estate_holdings", "gold_holdings", "crypto_holdings",
            "insurance_holdings", "bond_holdings",
        ];
        let count = tables.iter().filter(|t| has_holding_in(&conn, t)).count();
        if count >= 5 {
            if let Ok(Some(b)) = award_badge_if_new(&conn, "diversified_investor") {
                earned.push(b);
            }
        }
    }

    if context.check_first_goal {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "goal_getter") {
            earned.push(b);
        }
    }

    if context.check_first_milestone {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "milestone_hunter") {
            earned.push(b);
        }
    }

    if context.check_debt_destroyer {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "debt_destroyer") {
            earned.push(b);
        }
    }

    if context.check_crore_club {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "crore_club") {
            earned.push(b);
        }
    }

    if context.check_health_a {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "health_a") {
            earned.push(b);
        }
    }

    if context.check_health_aplus {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "health_aplus") {
            earned.push(b);
        }
    }

    if context.check_emergency_ready {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "emergency_ready") {
            earned.push(b);
        }
    }

    if context.check_debt_light {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "debt_light") {
            earned.push(b);
        }
    }

    // Wealth outcomes — verified here from the DB, not from a frontend-supplied number.
    if context.check_first_lakh || context.check_ten_lakh {
        let nw = crate::portfolio::calculator::calc_net_worth(&conn)
            .map(|s| s.net_worth)
            .unwrap_or(0.0);
        if context.check_first_lakh && nw >= 100_000.0 {
            if let Ok(Some(b)) = award_badge_if_new(&conn, "first_lakh") {
                earned.push(b);
            }
        }
        if context.check_ten_lakh && nw >= 1_000_000.0 {
            if let Ok(Some(b)) = award_badge_if_new(&conn, "ten_lakh") {
                earned.push(b);
            }
        }
    }

    if context.check_savings_star && savings_rate_90d(&conn) >= 0.50 {
        if let Ok(Some(b)) = award_badge_if_new(&conn, "savings_star") {
            earned.push(b);
        }
    }

    Ok(earned)
}

// Trailing-90-day savings rate: (income − expense/emi) / income. 0.0 when no income
// has been logged (so the badge can't be earned on empty data).
fn savings_rate_90d(conn: &Connection) -> f64 {
    let income: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount),0) FROM transactions \
             WHERE type = 'income' AND date(date) >= date('now','-90 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);
    if income <= 0.0 {
        return 0.0;
    }
    let expense: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount),0) FROM transactions \
             WHERE type IN ('expense','emi') AND date(date) >= date('now','-90 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);
    (income - expense) / income
}

// ─── Behaviour streak: consecutive positive-savings months ─────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavingsStreak {
    /// Consecutive *completed* months (ending last month) with net savings > 0.
    pub current_streak: i64,
    /// All-time best run (persisted in app_settings).
    pub best_streak: i64,
    /// Net savings for the in-progress current month.
    pub this_month_saved: f64,
    pub this_month_positive: bool,
    /// XP granted this call (improvement-only; 0 unless a new best was set).
    pub xp_awarded: i64,
}

const SAVINGS_STREAK_BEST_KEY: &str = "savings_streak_best";

// "YYYY-MM" one month earlier.
fn prev_ym(ym: &str) -> String {
    let y: i32 = ym[..4].parse().unwrap_or(2000);
    let m: u32 = ym[5..7].parse().unwrap_or(1);
    if m <= 1 {
        format!("{:04}-12", y - 1)
    } else {
        format!("{:04}-{:02}", y, m - 1)
    }
}

// Count consecutive months (starting from the month before `this_ym`, going back)
// that have a recorded, strictly-positive net savings. A gap (month with no data)
// or a non-positive month ends the run.
fn positive_month_streak(
    by_month: &std::collections::HashMap<String, f64>,
    this_ym: &str,
) -> i64 {
    let mut streak = 0i64;
    let mut ym = prev_ym(this_ym);
    while let Some(&net) = by_month.get(&ym) {
        if net > 0.0 {
            streak += 1;
            ym = prev_ym(&ym);
        } else {
            break;
        }
    }
    streak
}

/// Computes the consecutive-positive-savings-month streak from the ledger, updates
/// the persisted best, and awards improvement-only XP when a new best is set.
#[tauri::command]
pub fn get_savings_streak(state: State<DbState>) -> Result<SavingsStreak> {
    let conn = state.0.get()?;

    // Net savings per calendar month: income − (expense + emi).
    let mut by_month: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT strftime('%Y-%m', date) AS ym, \
                    SUM(CASE WHEN type='income' THEN amount ELSE 0 END) - \
                    SUM(CASE WHEN type IN ('expense','emi') THEN amount ELSE 0 END) AS net \
                 FROM transactions GROUP BY ym",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
            .map_err(|e| AppError::Database(e.to_string()))?;
        for (ym, net) in rows.flatten() {
            by_month.insert(ym, net);
        }
    }

    let now = chrono::Local::now();
    let this_ym = now.format("%Y-%m").to_string();
    let this_month_saved = by_month.get(&this_ym).copied().unwrap_or(0.0);

    // Walk backwards from last completed month while each has recorded, positive savings.
    let streak = positive_month_streak(&by_month, &this_ym);

    // Persisted best + improvement-only XP.
    let stored_best: i64 = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [SAVINGS_STREAK_BEST_KEY],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let mut xp_awarded = 0i64;
    let best = if streak > stored_best {
        // Reward each newly-reached month, capped so it can't be farmed.
        let gained = ((streak - stored_best).min(3) * 10) as i32;
        award_xp_internal(&conn, gained)?;
        xp_awarded = gained as i64;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![SAVINGS_STREAK_BEST_KEY, streak.to_string()],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        streak
    } else {
        stored_best
    };

    Ok(SavingsStreak {
        current_streak: streak,
        best_streak: best,
        this_month_saved,
        this_month_positive: this_month_saved > 0.0,
        xp_awarded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn prev_ym_rolls_over_year() {
        assert_eq!(prev_ym("2026-01"), "2025-12");
        assert_eq!(prev_ym("2026-07"), "2026-06");
    }

    #[test]
    fn streak_counts_back_until_gap_or_negative() {
        let mut m = HashMap::new();
        // this month = 2026-07 (in progress, ignored by the walk).
        m.insert("2026-06".to_string(), 500.0); // +
        m.insert("2026-05".to_string(), 1200.0); // +
        m.insert("2026-04".to_string(), -100.0); // breaks here
        m.insert("2026-03".to_string(), 800.0); // not reached
        assert_eq!(positive_month_streak(&m, "2026-07"), 2);
    }

    #[test]
    fn streak_breaks_on_missing_month() {
        let mut m = HashMap::new();
        m.insert("2026-06".to_string(), 500.0);
        // 2026-05 absent → gap ends the run at 1.
        m.insert("2026-04".to_string(), 800.0);
        assert_eq!(positive_month_streak(&m, "2026-07"), 1);
    }
}
