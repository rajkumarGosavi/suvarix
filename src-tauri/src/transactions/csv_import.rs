use chrono::{NaiveDate, NaiveDateTime};
use csv::ReaderBuilder;
use std::collections::HashSet;
use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::common::ImportResult;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreview {
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionColumnMapping {
    pub date_col: usize,
    pub amount_col: usize,
    pub category_col: Option<usize>,
    pub description_col: Option<usize>,
    pub notes_col: Option<usize>,
}

#[tauri::command]
pub fn preview_transaction_csv(csv_content: String) -> Result<CsvPreview> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_content.as_bytes());

    let headers: Vec<String> = rdr.headers()
        .map_err(|e| AppError::Parse(e.to_string()))?
        .iter().map(|h| h.to_string()).collect();

    let sample_rows: Vec<Vec<String>> = rdr.records()
        .take(5)
        .filter_map(|r| r.ok())
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .collect();

    Ok(CsvPreview { headers, sample_rows })
}

/// Parses a date/datetime cell into "YYYY-MM-DD HH:MM:SS", preserving time when present.
fn parse_flexible_datetime(s: &str) -> Option<String> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    const DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y", "%m/%d/%Y"];
    for f in DATE_FORMATS {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Some(format!("{} 00:00:00", d.format("%Y-%m-%d")));
        }
    }
    None
}

#[tauri::command]
pub fn import_transactions_csv(
    csv_content: String,
    mapping: TransactionColumnMapping,
    state: State<DbState>,
) -> Result<ImportResult> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_content.as_bytes());
    rdr.headers().map_err(|e| AppError::Parse(e.to_string()))?;

    let mut conn = state.0.get()?;

    let mut seen: HashSet<(String, i64, String)> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT date, amount, description FROM transactions")?;
        let rows = stmt.query_map([], |r| {
            let date: String = r.get(0)?;
            let amount: f64 = r.get(1)?;
            let desc: Option<String> = r.get(2)?;
            Ok((date, amount, desc))
        })?;
        for row in rows.filter_map(|r| r.ok()) {
            let (date, amount, desc) = row;
            let paise = (amount.abs() * 100.0).round() as i64;
            let desc_norm = desc.unwrap_or_default().trim().to_lowercase();
            seen.insert((date, paise, desc_norm));
        }
    }

    let tx = conn.transaction()?;
    let mut imported = 0i64;
    let mut skipped = 0i64;

    for result in rdr.records() {
        let record = match result {
            Ok(r) => r,
            Err(_) => { skipped += 1; continue; }
        };

        let raw_date = record.get(mapping.date_col).unwrap_or("").trim();
        let date = match parse_flexible_datetime(raw_date) {
            Some(d) => d,
            None => { skipped += 1; continue; }
        };

        let raw_amount = record.get(mapping.amount_col).unwrap_or("").trim();
        let amount_signed: f64 = match raw_amount.replace(',', "").parse() {
            Ok(v) => v,
            Err(_) => { skipped += 1; continue; }
        };
        if amount_signed == 0.0 { skipped += 1; continue; }

        let txn_type = if amount_signed < 0.0 { "expense" } else { "income" };
        let amount = amount_signed.abs();

        let category = mapping.category_col
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let description = mapping.description_col
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let notes = mapping.notes_col
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let paise = (amount * 100.0).round() as i64;
        let desc_norm = description.clone().unwrap_or_default().trim().to_lowercase();
        let key = (date.clone(), paise, desc_norm);
        if seen.contains(&key) { skipped += 1; continue; }

        let res = tx.execute(
            "INSERT INTO transactions (date, type, amount, category, description, notes, source)
             VALUES (?1,?2,?3,?4,?5,?6,'csv_import')",
            rusqlite::params![date, txn_type, amount, category, description, notes],
        );
        match res {
            Ok(_) => { imported += 1; seen.insert(key); }
            Err(_) => skipped += 1,
        }
    }

    tx.commit()?;
    Ok(ImportResult { imported, skipped })
}
