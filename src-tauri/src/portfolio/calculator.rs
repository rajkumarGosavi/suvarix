use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthSummary {
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub net_worth: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationItem {
    pub label: String,
    pub value: f64,
    pub percent: f64,
}

pub fn calc_net_worth(conn: &Connection) -> Result<NetWorthSummary> {
    let equity: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(quantity * COALESCE(current_price, avg_buy_price)), 0) FROM equity_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let mf: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(units * COALESCE(current_nav, avg_nav)), 0) FROM mf_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let fd: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(maturity_amount, principal)), 0) FROM fd_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let ppf_epf: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(balance), 0) FROM ppf_epf_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let real_estate: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(current_value, purchase_price)), 0) FROM real_estate_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let gold: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(weight_grams, units, 0) * COALESCE(current_price, avg_buy_price)), 0) FROM gold_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let crypto: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(quantity * COALESCE(current_price, avg_buy_price)), 0) FROM crypto_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let bonds: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(quantity * COALESCE(current_price, purchase_price)), 0) FROM bond_holdings",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    // Cash balance from transactions: income/dividends/interest inflow minus expenses/EMIs.
    // buy/sell/sip/redemption/deposit/withdrawal are excluded — those affect holdings already tracked above.
    let cash: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(
                CASE
                    WHEN type IN ('income','dividend','interest') THEN amount
                    WHEN type IN ('expense','emi') THEN -amount
                    ELSE 0
                END
             ), 0) FROM transactions",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let holdings_total = equity + mf + fd + ppf_epf + real_estate + gold + crypto + bonds;
    // Positive cash adds to assets; negative cash (overspent) adds to liabilities
    let total_assets      = holdings_total + cash.max(0.0);

    let loans: f64 = conn
        .query_row("SELECT COALESCE(SUM(outstanding), 0) FROM loans", [], |r| r.get(0))
        .unwrap_or(0.0);

    let credit_cards: f64 = conn
        .query_row("SELECT COALESCE(SUM(current_balance), 0) FROM credit_cards", [], |r| r.get(0))
        .unwrap_or(0.0);

    let total_liabilities = loans + credit_cards + (-cash).max(0.0);

    Ok(NetWorthSummary {
        total_assets,
        total_liabilities,
        net_worth: total_assets - total_liabilities,
    })
}

pub fn calc_allocation(conn: &Connection) -> Result<Vec<AllocationItem>> {
    let equity: f64 = conn.query_row(
        "SELECT COALESCE(SUM(quantity * COALESCE(current_price, avg_buy_price)), 0) FROM equity_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let mf: f64 = conn.query_row(
        "SELECT COALESCE(SUM(units * COALESCE(current_nav, avg_nav)), 0) FROM mf_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let fd: f64 = conn.query_row(
        "SELECT COALESCE(SUM(COALESCE(maturity_amount, principal)), 0) FROM fd_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let ppf_epf: f64 = conn.query_row(
        "SELECT COALESCE(SUM(balance), 0) FROM ppf_epf_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let real_estate: f64 = conn.query_row(
        "SELECT COALESCE(SUM(COALESCE(current_value, purchase_price)), 0) FROM real_estate_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let gold: f64 = conn.query_row(
        "SELECT COALESCE(SUM(COALESCE(weight_grams, units, 0) * COALESCE(current_price, avg_buy_price)), 0) FROM gold_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let crypto: f64 = conn.query_row(
        "SELECT COALESCE(SUM(quantity * COALESCE(current_price, avg_buy_price)), 0) FROM crypto_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let bonds: f64 = conn.query_row(
        "SELECT COALESCE(SUM(quantity * COALESCE(current_price, purchase_price)), 0) FROM bond_holdings", [], |r| r.get(0)).unwrap_or(0.0);
    let cash: f64 = conn.query_row(
        "SELECT COALESCE(SUM(
             CASE
                 WHEN type IN ('income','dividend','interest') THEN amount
                 WHEN type IN ('expense','emi') THEN -amount
                 ELSE 0
             END
          ), 0) FROM transactions",
        [], |r| r.get(0)).unwrap_or(0.0);
    // Only count positive cash balance in allocation (negative means overspent — shown as liability)
    let cash_alloc = cash.max(0.0);

    let total = equity + mf + fd + ppf_epf + real_estate + gold + crypto + bonds + cash_alloc;
    let pct = |v: f64| if total > 0.0 { (v / total) * 100.0 } else { 0.0 };

    let mut items = vec![
        AllocationItem { label: "Equity".into(), value: equity, percent: pct(equity) },
        AllocationItem { label: "Mutual Funds".into(), value: mf, percent: pct(mf) },
        AllocationItem { label: "FD/RD".into(), value: fd, percent: pct(fd) },
        AllocationItem { label: "PPF/EPF".into(), value: ppf_epf, percent: pct(ppf_epf) },
        AllocationItem { label: "Real Estate".into(), value: real_estate, percent: pct(real_estate) },
        AllocationItem { label: "Gold".into(), value: gold, percent: pct(gold) },
        AllocationItem { label: "Crypto".into(), value: crypto, percent: pct(crypto) },
        AllocationItem { label: "Bonds".into(), value: bonds, percent: pct(bonds) },
    ];
    if cash_alloc > 0.0 {
        items.push(AllocationItem { label: "Cash".into(), value: cash_alloc, percent: pct(cash_alloc) });
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    #[test]
    fn calc_net_worth_on_empty_db_is_all_zero() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();
        let summary = calc_net_worth(&conn).unwrap();
        assert_eq!(summary.total_assets, 0.0);
        assert_eq!(summary.total_liabilities, 0.0);
        assert_eq!(summary.net_worth, 0.0);
    }

    #[test]
    fn calc_net_worth_sums_holdings_liabilities_and_cash() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        conn.execute(
            "INSERT INTO accounts (name, type) VALUES ('Zerodha', 'broker')",
            [],
        ).unwrap();
        let account_id = conn.last_insert_rowid();

        // Equity: 10 * 150 = 1500 (uses current_price over avg_buy_price)
        conn.execute(
            "INSERT INTO equity_holdings (account_id, isin, symbol, exchange, name, quantity, avg_buy_price, current_price)
             VALUES (?1, 'INE000A01000', 'FOO', 'NSE', 'Foo Ltd', 10, 100, 150)",
            rusqlite::params![account_id],
        ).unwrap();

        // MF: 20 units * 50 nav = 1000
        conn.execute(
            "INSERT INTO mf_holdings (account_id, scheme_code, scheme_name, amc_name, folio_number, units, avg_nav, current_nav)
             VALUES (?1, 'SC1', 'Scheme One', 'AMC', 'F1', 20, 40, 50)",
            rusqlite::params![account_id],
        ).unwrap();

        // Loan: outstanding 500
        conn.execute(
            "INSERT INTO loans (loan_type, lender_name, principal, outstanding, interest_rate, emi_amount, tenure_months, disbursement_date)
             VALUES ('personal', 'Bank', 1000, 500, 10, 100, 12, '2024-01-01')",
            [],
        ).unwrap();

        // Cash: income 300 - expense 100 = net +200
        conn.execute(
            "INSERT INTO transactions (date, type, amount) VALUES ('2024-01-01', 'income', 300)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions (date, type, amount) VALUES ('2024-01-02', 'expense', 100)",
            [],
        ).unwrap();

        let summary = calc_net_worth(&conn).unwrap();
        assert_eq!(summary.total_assets, 1500.0 + 1000.0 + 200.0);
        assert_eq!(summary.total_liabilities, 500.0);
        assert_eq!(summary.net_worth, summary.total_assets - summary.total_liabilities);
    }

    #[test]
    fn calc_net_worth_negative_cash_becomes_liability_not_negative_asset() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        // Overspent: expense 500 with no income => net cash -500
        conn.execute(
            "INSERT INTO transactions (date, type, amount) VALUES ('2024-01-01', 'expense', 500)",
            [],
        ).unwrap();

        let summary = calc_net_worth(&conn).unwrap();
        assert_eq!(summary.total_assets, 0.0);
        assert_eq!(summary.total_liabilities, 500.0);
        assert_eq!(summary.net_worth, -500.0);
    }

    #[test]
    fn calc_allocation_on_empty_db_returns_zero_percent_no_nan() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();
        let items = calc_allocation(&conn).unwrap();
        assert!(!items.is_empty());
        for item in &items {
            assert_eq!(item.value, 0.0);
            assert_eq!(item.percent, 0.0);
            assert!(!item.percent.is_nan());
        }
    }

    #[test]
    fn calc_allocation_percentages_sum_to_100_across_asset_classes() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();

        conn.execute(
            "INSERT INTO accounts (name, type) VALUES ('Zerodha', 'broker')",
            [],
        ).unwrap();
        let account_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO equity_holdings (account_id, isin, symbol, exchange, name, quantity, avg_buy_price, current_price)
             VALUES (?1, 'INE000A01000', 'FOO', 'NSE', 'Foo Ltd', 10, 100, 100)",
            rusqlite::params![account_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO mf_holdings (account_id, scheme_code, scheme_name, amc_name, folio_number, units, avg_nav, current_nav)
             VALUES (?1, 'SC1', 'Scheme One', 'AMC', 'F1', 30, 100, 100)",
            rusqlite::params![account_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions (date, type, amount) VALUES ('2024-01-01', 'income', 600)",
            [],
        ).unwrap();

        let items = calc_allocation(&conn).unwrap();
        let total_pct: f64 = items.iter().map(|i| i.percent).sum();
        assert!((total_pct - 100.0).abs() < 1e-9, "percentages should sum to ~100, got {total_pct}");

        let cash_item = items.iter().find(|i| i.label == "Cash").expect("cash present when positive");
        assert_eq!(cash_item.value, 600.0);
    }
}
