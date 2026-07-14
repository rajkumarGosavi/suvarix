//! Debt payoff planner — simulates clearing all loans + credit-card balances under
//! a chosen strategy (avalanche = highest rate first, snowball = smallest balance
//! first), with an optional extra monthly payment and minimum-rollover (a cleared
//! debt's minimum is funnelled into the next priority debt).
//!
//! Pure computation over a `Vec<Debt>` so it's fully unit-testable; the Tauri
//! command just gathers debts from the DB and calls `build_plan`.

use serde::Serialize;
use tauri::State;

use crate::db::DbState;
use crate::error::Result;

// Hard cap so a budget that can't cover interest can't loop forever (50 years).
const MAX_MONTHS: i64 = 600;

#[derive(Debug, Clone)]
pub struct Debt {
    pub id: i64,
    pub kind: String, // "loan" | "credit_card"
    pub name: String,
    pub balance: f64,
    pub annual_rate_pct: f64,
    pub min_payment: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PayoffStep {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub balance: f64,
    pub annual_rate_pct: f64,
    /// 1-indexed month this debt hits zero; 0 if not cleared within the cap.
    pub payoff_month: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebtPlan {
    pub strategy: String,
    pub total_debt: f64,
    pub total_min_payment: f64,
    /// Minimums + extra — the fixed monthly budget the plan throws at the debt.
    pub monthly_budget: f64,
    pub months_to_debt_free: i64,
    pub total_interest: f64,
    /// Interest saved vs. paying only minimums (no extra, no rollover).
    pub interest_saved: f64,
    /// Months saved vs. that same minimum-only baseline.
    pub months_saved: i64,
    /// True if the budget can't clear the debt within the cap (e.g. below interest).
    pub never_clears: bool,
    pub steps: Vec<PayoffStep>,
}

fn priority_index(debts: &[Debt], active: &[bool], strategy: &str) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, d) in debts.iter().enumerate() {
        if !active[i] {
            continue;
        }
        best = Some(match best {
            None => i,
            Some(b) => {
                let take = match strategy {
                    // Snowball: smallest balance first (tie → higher rate).
                    "snowball" => {
                        d.balance < debts[b].balance
                            || (d.balance == debts[b].balance
                                && d.annual_rate_pct > debts[b].annual_rate_pct)
                    }
                    // Avalanche (default): highest rate first (tie → smaller balance).
                    _ => {
                        d.annual_rate_pct > debts[b].annual_rate_pct
                            || (d.annual_rate_pct == debts[b].annual_rate_pct
                                && d.balance < debts[b].balance)
                    }
                };
                if take {
                    i
                } else {
                    b
                }
            }
        });
    }
    best
}

// Runs the month-by-month simulation. Returns (months, total_interest, payoff_month
// per debt). `rollover` funnels the fixed budget's surplus into the priority debt;
// with it off and extra=0 this is the "minimums only" baseline.
fn simulate(debts: &[Debt], strategy: &str, extra: f64, rollover: bool) -> (i64, f64, Vec<i64>) {
    let n = debts.len();
    let mut balance: Vec<f64> = debts.iter().map(|d| d.balance).collect();
    let mut payoff_month = vec![0i64; n];
    let mut total_interest = 0.0;

    let total_min: f64 = debts.iter().map(|d| d.min_payment).sum();
    let budget = total_min + extra;

    let mut month = 0i64;
    while month < MAX_MONTHS {
        let active: Vec<bool> = balance.iter().map(|b| *b > 0.005).collect();
        if active.iter().all(|a| !*a) {
            break;
        }
        month += 1;

        // Accrue one month of interest.
        for i in 0..n {
            if balance[i] > 0.0 {
                let interest = balance[i] * debts[i].annual_rate_pct / 100.0 / 12.0;
                balance[i] += interest;
                total_interest += interest;
            }
        }

        if rollover {
            // Pay each active debt its minimum, then funnel the remaining budget
            // (extra + freed minimums of cleared debts) into the priority debt.
            let mut avail = budget;
            for i in 0..n {
                if balance[i] > 0.0 {
                    let pay = debts[i].min_payment.min(balance[i]).min(avail);
                    balance[i] -= pay;
                    avail -= pay;
                }
            }
            while avail > 0.005 {
                let act: Vec<bool> = balance.iter().map(|b| *b > 0.005).collect();
                match priority_index(debts, &act, strategy) {
                    Some(i) => {
                        let pay = avail.min(balance[i]);
                        balance[i] -= pay;
                        avail -= pay;
                    }
                    None => break,
                }
            }
        } else {
            // Baseline: every debt pays only its own minimum, independently.
            for i in 0..n {
                if balance[i] > 0.0 {
                    let pay = debts[i].min_payment.min(balance[i]);
                    balance[i] -= pay;
                }
            }
        }

        // Record anything that reached zero this month.
        for i in 0..n {
            if payoff_month[i] == 0 && balance[i] <= 0.005 {
                balance[i] = 0.0;
                payoff_month[i] = month;
            }
        }
    }

    (month, total_interest, payoff_month)
}

/// Builds a full plan for `strategy` (with `extra` monthly) plus the minimum-only
/// baseline, and derives interest/months saved.
pub fn build_plan(debts: &[Debt], strategy: &str, extra: f64) -> DebtPlan {
    let total_debt: f64 = debts.iter().map(|d| d.balance).sum();
    let total_min: f64 = debts.iter().map(|d| d.min_payment).sum();
    let extra = extra.max(0.0);

    let (months, interest, payoff) = simulate(debts, strategy, extra, true);
    let (base_months, base_interest, _) = simulate(debts, strategy, 0.0, false);

    let never_clears = payoff.iter().any(|m| *m == 0) && !debts.is_empty();

    let steps = debts
        .iter()
        .enumerate()
        .map(|(i, d)| PayoffStep {
            id: d.id,
            kind: d.kind.clone(),
            name: d.name.clone(),
            balance: d.balance,
            annual_rate_pct: d.annual_rate_pct,
            payoff_month: payoff[i],
        })
        .collect::<Vec<_>>();

    DebtPlan {
        strategy: strategy.to_string(),
        total_debt,
        total_min_payment: total_min,
        monthly_budget: total_min + extra,
        months_to_debt_free: months,
        total_interest: (interest * 100.0).round() / 100.0,
        interest_saved: ((base_interest - interest).max(0.0) * 100.0).round() / 100.0,
        months_saved: (base_months - months).max(0),
        never_clears,
        steps,
    }
}

// ─── Command ──────────────────────────────────────────────────────────────────

fn gather_debts(conn: &rusqlite::Connection) -> Result<Vec<Debt>> {
    let mut debts = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT id, lender_name, loan_type, outstanding, interest_rate, emi_amount \
         FROM loans WHERE outstanding > 0",
    )?;
    for d in stmt
        .query_map([], |r| {
            Ok(Debt {
                id: r.get(0)?,
                kind: "loan".into(),
                name: {
                    let lender: String = r.get(1)?;
                    let lt: String = r.get(2)?;
                    format!("{lender} ({lt})")
                },
                balance: r.get(3)?,
                annual_rate_pct: r.get(4)?,
                min_payment: r.get(5)?,
            })
        })?
        .flatten()
    {
        debts.push(d);
    }

    // Credit cards have no stored rate — use a configurable APR (default 36%).
    let cc_apr: f64 = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'credit_card_apr'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|v: &f64| v.is_finite() && *v >= 0.0)
        .unwrap_or(36.0);

    let mut stmt = conn.prepare(
        "SELECT id, bank_name, card_name, current_balance, min_payment \
         FROM credit_cards WHERE current_balance > 0",
    )?;
    for d in stmt
        .query_map([], |r| {
            let bank: String = r.get(1)?;
            let card: String = r.get(2)?;
            let min_payment: Option<f64> = r.get(4)?;
            let balance: f64 = r.get(3)?;
            Ok(Debt {
                id: r.get(0)?,
                kind: "credit_card".into(),
                name: format!("{bank} {card}"),
                balance,
                annual_rate_pct: cc_apr,
                // Fall back to a 5%-of-balance minimum if none is recorded.
                min_payment: min_payment.filter(|m| *m > 0.0).unwrap_or(balance * 0.05),
            })
        })?
        .flatten()
    {
        debts.push(d);
    }

    Ok(debts)
}

/// Computes a debt payoff plan across all loans + credit cards.
/// `strategy` = "avalanche" | "snowball"; `extra_monthly` adds to the budget.
#[tauri::command]
pub fn get_debt_payoff_plan(
    strategy: String,
    extra_monthly: Option<f64>,
    state: State<DbState>,
) -> Result<DebtPlan> {
    let conn = state.0.get()?;
    let debts = gather_debts(&conn)?;
    Ok(build_plan(&debts, &strategy, extra_monthly.unwrap_or(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debt(id: i64, bal: f64, rate: f64, min: f64) -> Debt {
        Debt { id, kind: "loan".into(), name: format!("D{id}"), balance: bal, annual_rate_pct: rate, min_payment: min }
    }

    #[test]
    fn zero_interest_single_debt_clears_predictably() {
        let plan = build_plan(&[debt(1, 1000.0, 0.0, 100.0)], "avalanche", 0.0);
        assert_eq!(plan.months_to_debt_free, 10);
        assert_eq!(plan.total_interest, 0.0);
        assert_eq!(plan.steps[0].payoff_month, 10);
        assert!(!plan.never_clears);
    }

    #[test]
    fn extra_payment_saves_interest_and_time() {
        // Two interest-bearing debts; extra + rollover must beat minimum-only.
        let debts = vec![debt(1, 50_000.0, 18.0, 2_000.0), debt(2, 30_000.0, 24.0, 1_500.0)];
        let plan = build_plan(&debts, "avalanche", 5_000.0);
        assert!(plan.interest_saved > 0.0, "extra + rollover should save interest");
        assert!(plan.months_saved > 0, "and clear the debt sooner");
        assert!(!plan.never_clears);
    }

    #[test]
    fn avalanche_clears_highest_rate_first() {
        let debts = vec![
            debt(1, 40_000.0, 12.0, 1_000.0), // lower rate
            debt(2, 40_000.0, 30.0, 1_000.0), // higher rate → paid first
        ];
        let plan = build_plan(&debts, "avalanche", 4_000.0);
        let hi = plan.steps.iter().find(|s| s.id == 2).unwrap();
        let lo = plan.steps.iter().find(|s| s.id == 1).unwrap();
        assert!(hi.payoff_month <= lo.payoff_month, "high-rate debt clears first");
    }

    #[test]
    fn snowball_clears_smallest_balance_first() {
        let debts = vec![
            debt(1, 60_000.0, 30.0, 1_000.0), // bigger balance, higher rate
            debt(2, 10_000.0, 12.0, 1_000.0), // smallest balance → paid first
        ];
        let plan = build_plan(&debts, "snowball", 4_000.0);
        let small = plan.steps.iter().find(|s| s.id == 2).unwrap();
        let big = plan.steps.iter().find(|s| s.id == 1).unwrap();
        assert!(small.payoff_month <= big.payoff_month, "smallest balance clears first");
    }

    #[test]
    fn budget_below_interest_flags_never_clears() {
        // 24% on 1,000,000 = 20,000/mo interest; min 1,000 can never dent it.
        let plan = build_plan(&[debt(1, 1_000_000.0, 24.0, 1_000.0)], "avalanche", 0.0);
        assert!(plan.never_clears);
        assert_eq!(plan.steps[0].payoff_month, 0);
    }
}
