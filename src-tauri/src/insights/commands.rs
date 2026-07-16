use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::financial_health::commands::compute_score;
use crate::income_expenses::commands::{budget_status_impl, current_month_bounds};
use crate::portfolio::calculator::calc_net_worth;

/// A single actionable nudge — "one number + one action".
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Nudge {
    /// Stable across recomputes — the dedup (scheduler) and dismiss key.
    pub id: String,
    /// "critical" | "warning" | "positive" | "info". Drives colour + push threshold.
    pub severity: String,
    /// Source bucket: a pillar key, or "budget" / "maturity" / "networth".
    pub category: String,
    pub icon: String,
    pub title: String,
    /// Short status carrying the concrete number(s).
    pub body: String,
    pub action_label: String,
    /// vue-router path the action button navigates to.
    pub action_route: String,
    /// Higher = surfaced first. Also decides which get pushed when capped.
    pub priority: i32,
}

const DISMISS_SETTING_KEY: &str = "dismissed_insights";
// A dismissed nudge stays hidden this long, then returns if still unresolved —
// so ignoring a real problem doesn't bury it forever.
const DISMISS_DAYS: i64 = 7;

// Pillars at or below this score are worth nudging (matches the "C" grade floor).
const WEAK_PILLAR_THRESHOLD: f64 = 55.0;

// ─── Nudge builders ───────────────────────────────────────────────────────────

// Screen + button copy for a weak health pillar, by its stable key.
fn pillar_action(key: &str) -> (&'static str, &'static str, &'static str) {
    // (icon, action_label, route)
    match key {
        "savings" => ("💰", "Review spending", "/income-expenses"),
        "emergency" => ("🛟", "Build reserves", "/portfolio"),
        "debt" => ("⚔️", "Manage debt", "/liabilities"),
        "diversification" => ("🧩", "Diversify", "/portfolio"),
        "protection" => ("🛡️", "Add cover", "/portfolio"),
        "networth" => ("📈", "Grow net worth", "/portfolio"),
        _ => ("🎯", "Open", "/dashboard"),
    }
}

// A brand-new user with no data would otherwise get "critical" nudges from
// protection/net-worth pillars scoring 0 simply because nothing is entered yet —
// that's onboarding noise, not a real problem. Gate pillar nudges behind having
// any financial footprint at all (a transaction, or a non-zero net worth).
fn has_financial_footprint(conn: &Connection) -> bool {
    let tx: i64 = conn
        .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
        .unwrap_or(0);
    if tx > 0 {
        return true;
    }
    calc_net_worth(conn).map(|s| s.net_worth).unwrap_or(0.0).abs() > 0.0
}

fn pillar_nudges(conn: &Connection) -> Vec<Nudge> {
    if !has_financial_footprint(conn) {
        return Vec::new();
    }
    let score = compute_score(conn);
    let mut out = Vec::new();
    for p in &score.pillars {
        let s = match p.score {
            Some(s) if s <= WEAK_PILLAR_THRESHOLD && !p.top_fix.is_empty() => s,
            _ => continue,
        };
        let (icon, action_label, route) = pillar_action(&p.key);
        // Critical only when badly off. Priority stays *below* a blown budget (100):
        // base + a small gain-based tiebreak (capped) so the biggest-leverage pillar
        // ranks first among pillars without ever outranking a hard budget breach.
        let severity = if s < 30.0 { "critical" } else { "warning" };
        let base = if severity == "critical" { 80 } else { 60 };
        let tiebreak = (p.potential_gain.round() as i32).clamp(0, 9);
        out.push(Nudge {
            id: format!("pillar:{}", p.key),
            severity: severity.into(),
            category: p.key.clone(),
            icon: icon.into(),
            title: p.label.clone(),
            body: format!("{}. {}", p.status, p.top_fix),
            action_label: action_label.into(),
            action_route: route.into(),
            priority: base + tiebreak,
        });
    }
    out
}

fn budget_nudges(conn: &Connection) -> Vec<Nudge> {
    let (start, end) = current_month_bounds();
    let statuses = match budget_status_impl(conn, &start, &end) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for b in &statuses {
        if b.monthly_limit <= 0.0 {
            continue;
        }
        let pct = b.percent_used;
        let (severity, priority) = if pct >= 100.0 {
            ("critical", 100)
        } else if pct >= 80.0 {
            ("warning", 75)
        } else {
            continue;
        };
        let body = if pct >= 100.0 {
            format!(
                "{} budget blown — spent {:.0}% (₹{:.0} over limit).",
                b.category,
                pct,
                b.spent - b.monthly_limit
            )
        } else {
            format!("{} budget {:.0}% used with the month not over.", b.category, pct)
        };
        out.push(Nudge {
            id: format!("budget:{}", b.category),
            severity: severity.into(),
            category: "budget".into(),
            icon: "📊".into(),
            title: "Budget alert".into(),
            body,
            action_label: "Review budget".into(),
            action_route: "/income-expenses".into(),
            priority,
        });
    }
    out
}

// Inline near-term maturity scan (FD + bond) — conn-only so the whole feed stays
// unit-testable without a pool. Mirrors reminders::maturity_alerts' filters.
fn maturity_nudges(conn: &Connection) -> Vec<Nudge> {
    const LOOKAHEAD_DAYS: i64 = 7;
    let today = Local::now().date_naive();
    let mut out = Vec::new();

    let mut push_row = |source: &str, name: String, date: String| {
        let days = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map(|d| (d - today).num_days())
            .unwrap_or(0);
        if !(0..=LOOKAHEAD_DAYS).contains(&days) {
            return;
        }
        let when = if days == 0 {
            "today".to_string()
        } else {
            format!("in {days} day{}", if days == 1 { "" } else { "s" })
        };
        out.push(Nudge {
            id: format!("maturity:{source}:{name}:{date}"),
            severity: "info".into(),
            category: "maturity".into(),
            icon: "⏰".into(),
            title: "Maturity coming up".into(),
            body: format!("{name} matures {when} ({date}). Plan the reinvestment."),
            action_label: "Plan reinvestment".into(),
            action_route: "/portfolio".into(),
            priority: 50,
        });
    };

    if let Ok(mut stmt) = conn.prepare(
        "SELECT bank_name, maturity_date FROM fd_holdings \
         WHERE julianday(maturity_date) - julianday('now') BETWEEN 0 AND 7",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        }) {
            for (name, date) in rows.flatten() {
                push_row("fd", name, date);
            }
        }
    }
    if let Ok(mut stmt) = conn.prepare(
        "SELECT issuer_name, maturity_date FROM bond_holdings \
         WHERE maturity_date IS NOT NULL \
           AND julianday(maturity_date) - julianday('now') BETWEEN 0 AND 7",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        }) {
            for (name, date) in rows.flatten() {
                push_row("bond", name, date);
            }
        }
    }
    out
}

// Positive reinforcement: net worth at an all-time high vs recorded snapshots.
fn networth_high_nudge(conn: &Connection) -> Option<Nudge> {
    let nw = calc_net_worth(conn).map(|s| s.net_worth).unwrap_or(0.0);
    if nw <= 0.0 {
        return None;
    }
    // Prior peak from history (any earlier snapshot). Needs at least one to compare.
    let prev_max: Option<f64> = conn
        .query_row("SELECT MAX(net_worth) FROM net_worth_snapshots", [], |r| r.get(0))
        .ok()
        .flatten();
    match prev_max {
        Some(prev) if nw > prev && prev > 0.0 => Some(Nudge {
            id: "networth:new_high".into(),
            severity: "positive".into(),
            category: "networth".into(),
            icon: "🎉".into(),
            title: "New net-worth high!".into(),
            body: "Your net worth just beat its previous record. Keep the momentum.".into(),
            action_label: "View trend".into(),
            action_route: "/reports".into(),
            priority: 30,
        }),
        _ => None,
    }
}

/// Builds the full prioritised nudge list from current DB state (highest priority
/// first). Pure read; conn-only for testability. Does NOT filter dismissals — that
/// is applied in `get_insights` (the scheduler wants the unfiltered urgent set).
pub fn compute_nudges(conn: &Connection) -> Vec<Nudge> {
    let mut nudges = Vec::new();
    nudges.extend(budget_nudges(conn));
    nudges.extend(pillar_nudges(conn));
    nudges.extend(maturity_nudges(conn));
    nudges.extend(networth_high_nudge(conn));
    nudges.sort_by_key(|n| std::cmp::Reverse(n.priority));
    nudges
}

// ─── Dismissal state (app_settings JSON: [{id, until}]) ────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct Dismissal {
    id: String,
    until: String, // YYYY-MM-DD (exclusive-ish; hidden while today < until)
}

fn load_dismissals(conn: &Connection) -> Vec<Dismissal> {
    let raw: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [DISMISS_SETTING_KEY],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_dismissals(conn: &Connection, items: &[Dismissal]) -> Result<()> {
    let json = serde_json::to_string(items)?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![DISMISS_SETTING_KEY, json],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

// ─── Commands ──────────────────────────────────────────────────────────────────

/// The in-app feed: current nudges minus any still within their dismissal window.
#[tauri::command]
pub fn get_insights(state: State<DbState>) -> Result<Vec<Nudge>> {
    let conn = state.0.get()?;
    let today = Local::now().date_naive().to_string();
    let hidden: std::collections::HashSet<String> = load_dismissals(&conn)
        .into_iter()
        .filter(|d| d.until.as_str() > today.as_str())
        .map(|d| d.id)
        .collect();
    Ok(compute_nudges(&conn)
        .into_iter()
        .filter(|n| !hidden.contains(&n.id))
        .collect())
}

/// Hides a nudge for `DISMISS_DAYS`. If it's still valid after that, it returns.
#[tauri::command]
pub fn dismiss_insight(id: String, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    let today = Local::now().date_naive();
    let until = (today + chrono::Duration::days(DISMISS_DAYS)).to_string();

    let mut items = load_dismissals(&conn);
    // Drop already-expired rows and any prior entry for this id, then re-add.
    let today_s = today.to_string();
    items.retain(|d| d.until.as_str() > today_s.as_str() && d.id != id);
    items.push(Dismissal { id, until });
    save_dismissals(&conn, &items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        run_migrations(&c).unwrap();
        c
    }

    #[test]
    fn empty_db_yields_no_critical_nudges() {
        let c = conn();
        let nudges = compute_nudges(&c);
        // Nothing logged → no budgets, no maturities, no net-worth high.
        assert!(nudges.iter().all(|n| n.severity != "critical"));
    }

    #[test]
    fn blown_budget_produces_critical_nudge_sorted_first() {
        let c = conn();
        let (start, _) = current_month_bounds();
        c.execute(
            "INSERT INTO budgets (category, monthly_limit) VALUES ('Food', 1000)",
            [],
        )
        .unwrap();
        // Spend over the limit inside the current month.
        c.execute(
            "INSERT INTO transactions (type, amount, category, date) \
             VALUES ('expense', 1500, 'Food', ?1)",
            [&start],
        )
        .unwrap();

        let nudges = compute_nudges(&c);
        let first = nudges.first().expect("expected at least one nudge");
        assert_eq!(first.severity, "critical");
        assert_eq!(first.category, "budget");
        assert!(first.body.contains("Food"));
    }

    #[test]
    fn dismissal_hides_then_expires() {
        let c = conn();
        // Seed a dismissal that is already expired → must not hide.
        c.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                DISMISS_SETTING_KEY,
                r#"[{"id":"budget:Food","until":"2000-01-01"}]"#
            ],
        )
        .unwrap();
        let hidden: Vec<_> = load_dismissals(&c)
            .into_iter()
            .filter(|d| d.until.as_str() > Local::now().date_naive().to_string().as_str())
            .collect();
        assert!(hidden.is_empty(), "expired dismissal should not hide anything");
    }
}
