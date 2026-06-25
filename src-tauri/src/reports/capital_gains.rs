use rusqlite::Connection;
use serde::Serialize;
use crate::error::Result;
use chrono::NaiveDate;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GainTxn {
    pub symbol: String,
    pub asset_class: String,
    pub buy_date: String,
    pub sell_date: String,
    pub quantity: f64,
    pub buy_price: f64,
    pub sell_price: f64,
    pub gain: f64,
    pub gain_type: String,   // "STCG" | "LTCG"
    pub holding_days: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapitalGainsReport {
    pub stcg: f64,
    pub ltcg: f64,
    pub transactions: Vec<GainTxn>,
}

struct BuyLot {
    date: NaiveDate,
    qty_rem: f64,
    price: f64,
    symbol: String,
}

struct TxRow {
    date: String,
    asset_class: String,
    holding_id: Option<i64>,
    quantity: f64,
    price: f64,
    description: String,
}

fn parse_fy(fy: &str) -> Option<(NaiveDate, NaiveDate)> {
    // "2024-25" → 2024-04-01 .. 2025-03-31
    let start_year: i32 = fy.split('-').next()?.parse().ok()?;
    Some((
        NaiveDate::from_ymd_opt(start_year, 4, 1)?,
        NaiveDate::from_ymd_opt(start_year + 1, 3, 31)?,
    ))
}

fn lot_key(asset_class: &str, holding_id: Option<i64>, description: &str) -> String {
    match holding_id {
        Some(id) => format!("{}:{}", asset_class, id),
        None     => format!("{}::{}", asset_class, description),
    }
}

pub fn calculate(conn: &Connection, fy: &str, method: &str) -> Result<CapitalGainsReport> {
    let (fy_start, fy_end) = match parse_fy(fy) {
        Some(d) => d,
        None    => return Ok(CapitalGainsReport { stcg: 0.0, ltcg: 0.0, transactions: vec![] }),
    };

    let parse_date = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or(fy_start);

    // ── All BUY transactions (all time — needed for correct FIFO basis) ────────
    let buys: Vec<TxRow> = {
        let mut stmt = conn.prepare(
            "SELECT date, asset_class, holding_id, quantity, price, description \
             FROM transactions \
             WHERE type='buy' AND asset_class IN ('equity','mf') \
               AND quantity > 0 AND price > 0 \
             ORDER BY date ASC",
        )?;
        let rows: Vec<_> = stmt
            .query_map([], |r| Ok(TxRow {
                date:        r.get(0)?,
                asset_class: r.get(1)?,
                holding_id:  r.get(2)?,
                quantity:    r.get(3)?,
                price:       r.get(4)?,
                description: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            }))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    // ── Build per-holding queues ───────────────────────────────────────────────
    let mut queues: HashMap<String, VecDeque<BuyLot>> = HashMap::new();
    for b in &buys {
        queues
            .entry(lot_key(&b.asset_class, b.holding_id, &b.description))
            .or_default()
            .push_back(BuyLot {
                date:    parse_date(&b.date),
                qty_rem: b.quantity,
                price:   b.price,
                symbol:  b.description.clone(),
            });
    }

    // LIFO: reverse every queue so newest lot is at the front
    if method.to_uppercase() == "LIFO" {
        for q in queues.values_mut() {
            let v: Vec<_> = q.drain(..).collect();
            q.extend(v.into_iter().rev());
        }
    }

    // ── SELL transactions in the financial year ────────────────────────────────
    let fy_start_str = fy_start.to_string();
    let fy_end_str   = fy_end.to_string();

    let sells: Vec<TxRow> = {
        let mut stmt = conn.prepare(
            "SELECT date, asset_class, holding_id, quantity, price, description \
             FROM transactions \
             WHERE type='sell' AND asset_class IN ('equity','mf') \
               AND quantity > 0 AND price > 0 \
               AND date >= ?1 AND date <= ?2 \
             ORDER BY date ASC",
        )?;
        let rows: Vec<_> = stmt
            .query_map(rusqlite::params![fy_start_str, fy_end_str], |r| Ok(TxRow {
                date:        r.get(0)?,
                asset_class: r.get(1)?,
                holding_id:  r.get(2)?,
                quantity:    r.get(3)?,
                price:       r.get(4)?,
                description: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            }))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    // ── Match sells against buy lots ───────────────────────────────────────────
    let mut gain_txns: Vec<GainTxn> = vec![];
    let mut stcg_total = 0.0_f64;
    let mut ltcg_total = 0.0_f64;

    for sell in &sells {
        let k = lot_key(&sell.asset_class, sell.holding_id, &sell.description);
        let q = match queues.get_mut(&k) { Some(q) => q, None => continue };

        let sell_date = parse_date(&sell.date);
        let mut qty_rem = sell.quantity;

        while qty_rem > 1e-9 {
            let lot = match q.front_mut() { Some(l) => l, None => break };

            let matched       = qty_rem.min(lot.qty_rem);
            let holding_days  = (sell_date - lot.date).num_days();
            let gain          = (sell.price - lot.price) * matched;
            let is_stcg       = holding_days < 365;

            let symbol = if sell.description.is_empty() {
                lot.symbol.clone()
            } else {
                sell.description.clone()
            };

            gain_txns.push(GainTxn {
                symbol,
                asset_class: sell.asset_class.clone(),
                buy_date:    lot.date.to_string(),
                sell_date:   sell.date.clone(),
                quantity:    matched,
                buy_price:   lot.price,
                sell_price:  sell.price,
                gain,
                gain_type:   if is_stcg { "STCG".into() } else { "LTCG".into() },
                holding_days,
            });

            if is_stcg { stcg_total += gain; } else { ltcg_total += gain; }

            lot.qty_rem -= matched;
            qty_rem     -= matched;

            if lot.qty_rem < 1e-9 { q.pop_front(); }
        }
    }

    Ok(CapitalGainsReport { stcg: stcg_total, ltcg: ltcg_total, transactions: gain_txns })
}
