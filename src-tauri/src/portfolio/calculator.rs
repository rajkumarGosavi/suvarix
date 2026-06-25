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

    let total_assets = equity + mf + fd + ppf_epf + real_estate + gold + crypto;

    let loans: f64 = conn
        .query_row("SELECT COALESCE(SUM(outstanding), 0) FROM loans", [], |r| r.get(0))
        .unwrap_or(0.0);

    let credit_cards: f64 = conn
        .query_row("SELECT COALESCE(SUM(current_balance), 0) FROM credit_cards", [], |r| r.get(0))
        .unwrap_or(0.0);

    let total_liabilities = loans + credit_cards;

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

    let total = equity + mf + fd + ppf_epf + real_estate + gold + crypto;
    let pct = |v: f64| if total > 0.0 { (v / total) * 100.0 } else { 0.0 };

    Ok(vec![
        AllocationItem { label: "Equity".into(), value: equity, percent: pct(equity) },
        AllocationItem { label: "Mutual Funds".into(), value: mf, percent: pct(mf) },
        AllocationItem { label: "FD/RD".into(), value: fd, percent: pct(fd) },
        AllocationItem { label: "PPF/EPF".into(), value: ppf_epf, percent: pct(ppf_epf) },
        AllocationItem { label: "Real Estate".into(), value: real_estate, percent: pct(real_estate) },
        AllocationItem { label: "Gold".into(), value: gold, percent: pct(gold) },
        AllocationItem { label: "Crypto".into(), value: crypto, percent: pct(crypto) },
    ])
}
