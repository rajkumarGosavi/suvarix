use rusqlite::Connection;
use std::collections::HashSet;
use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::models::common::ImportResult;

/// One parsed row from a bank statement, already normalized by the frontend
/// (`bankStatementParser.ts`): date is `YYYY-MM-DD[ HH:MM:SS]`, amount is
/// positive, sign already resolved into `txn_type`.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BankTxnInput {
    pub date: String,
    pub amount: f64,
    pub txn_type: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub ref_no: Option<String>,
    pub tag: Option<String>,
}

/// Whitelist of accepted `source` values so a caller can't stamp arbitrary text.
fn normalize_source(source: &str) -> Result<&'static str> {
    match source {
        "bank_hdfc" => Ok("bank_hdfc"),
        "bank_icici" => Ok("bank_icici"),
        _ => Err(AppError::Parse(format!("unknown bank source: {source}"))),
    }
}

/// Insert pre-parsed bank-statement rows into the transactions ledger.
///
/// Dedup is two-tier and stronger than the plain CSV importer:
///   1. If a row carries a bank reference number (`ref_no`) that already exists
///      in `transactions.external_ref`, skip it (exact match, survives edits).
///   2. Otherwise fall back to the `(date, paise, description_lower)` heuristic
///      used by `csv_import.rs`.
#[tauri::command]
pub fn import_bank_statement(
    rows: Vec<BankTxnInput>,
    source: String,
    state: State<DbState>,
) -> Result<ImportResult> {
    let mut conn = state.0.get()?;
    import_bank_statement_impl(&mut conn, &rows, &source)
}

pub fn import_bank_statement_impl(
    conn: &mut Connection,
    rows: &[BankTxnInput],
    source: &str,
) -> Result<ImportResult> {
    let source = normalize_source(source)?;

    // Preload existing external_refs and (date, paise, desc) keys.
    let mut seen_ref: HashSet<String> = HashSet::new();
    let mut seen_key: HashSet<(String, i64, String)> = HashSet::new();
    {
        let mut stmt = conn.prepare(
            "SELECT date, amount, description, external_ref FROM transactions",
        )?;
        let rows_it = stmt.query_map([], |r| {
            let date: String = r.get(0)?;
            let amount: f64 = r.get(1)?;
            let desc: Option<String> = r.get(2)?;
            let ext: Option<String> = r.get(3)?;
            Ok((date, amount, desc, ext))
        })?;
        for row in rows_it.filter_map(|r| r.ok()) {
            let (date, amount, desc, ext) = row;
            let paise = (amount.abs() * 100.0).round() as i64;
            let desc_norm = desc.unwrap_or_default().trim().to_lowercase();
            seen_key.insert((date, paise, desc_norm));
            if let Some(e) = ext {
                let e = e.trim();
                if !e.is_empty() {
                    seen_ref.insert(e.to_string());
                }
            }
        }
    }

    let tx = conn.transaction()?;
    let mut imported = 0i64;
    let mut skipped = 0i64;

    for row in rows {
        let txn_type = match row.txn_type.as_str() {
            "income" | "expense" => row.txn_type.as_str(),
            _ => {
                skipped += 1;
                continue;
            }
        };
        if !(row.amount.is_finite() && row.amount > 0.0) {
            skipped += 1;
            continue;
        }
        let date = row.date.trim();
        if date.is_empty() {
            skipped += 1;
            continue;
        }

        let ref_no = row
            .ref_no
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let description = row
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let category = row
            .category
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let tag = row
            .tag
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        // Tier 1: reference-number dedup.
        if let Some(ref r) = ref_no {
            if seen_ref.contains(r) {
                skipped += 1;
                continue;
            }
        }
        // Tier 2: (date, paise, desc) heuristic.
        let paise = (row.amount * 100.0).round() as i64;
        let desc_norm = description.clone().unwrap_or_default().trim().to_lowercase();
        let key = (date.to_string(), paise, desc_norm);
        if seen_key.contains(&key) {
            skipped += 1;
            continue;
        }

        let res = tx.execute(
            "INSERT INTO transactions (date, type, amount, category, description, source, external_ref, tag)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            rusqlite::params![date, txn_type, row.amount, category, description, source, ref_no, tag],
        );
        match res {
            Ok(_) => {
                imported += 1;
                seen_key.insert(key);
                if let Some(r) = ref_no {
                    seen_ref.insert(r);
                }
            }
            Err(_) => skipped += 1,
        }
    }

    tx.commit()?;
    Ok(ImportResult { imported, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_pool;

    fn row(date: &str, amount: f64, ty: &str, desc: &str, ref_no: Option<&str>) -> BankTxnInput {
        BankTxnInput {
            date: date.to_string(),
            amount,
            txn_type: ty.to_string(),
            category: Some("Uncategorized".to_string()),
            description: Some(desc.to_string()),
            ref_no: ref_no.map(str::to_string),
            tag: Some("HDFC".to_string()),
        }
    }

    #[test]
    fn imports_rows_with_tag_and_source() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        let rows = vec![
            row("2026-01-01 00:00:00", 500.0, "expense", "UPI SWIGGY", Some("REF1")),
            row("2026-01-02 00:00:00", 90000.0, "income", "SALARY CREDIT", Some("REF2")),
        ];
        let res = import_bank_statement_impl(&mut conn, &rows, "bank_hdfc").unwrap();
        assert_eq!(res.imported, 2);
        assert_eq!(res.skipped, 0);

        let (tag, src): (String, String) = conn
            .query_row(
                "SELECT tag, source FROM transactions WHERE external_ref='REF1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(tag, "HDFC");
        assert_eq!(src, "bank_hdfc");
    }

    #[test]
    fn dedups_on_external_ref() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        let first = vec![row("2026-01-01 00:00:00", 500.0, "expense", "UPI SWIGGY", Some("REF1"))];
        import_bank_statement_impl(&mut conn, &first, "bank_hdfc").unwrap();

        // Same ref, different description/amount → still a duplicate.
        let again = vec![row("2026-01-05 00:00:00", 999.0, "expense", "EDITED NOTE", Some("REF1"))];
        let res = import_bank_statement_impl(&mut conn, &again, "bank_hdfc").unwrap();
        assert_eq!(res.imported, 0);
        assert_eq!(res.skipped, 1);
    }

    #[test]
    fn dedups_on_date_amount_desc_when_no_ref() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        let first = vec![row("2026-01-01 00:00:00", 500.0, "expense", "ATM CASH WDL", None)];
        import_bank_statement_impl(&mut conn, &first, "bank_hdfc").unwrap();

        let dup = vec![row("2026-01-01 00:00:00", 500.0, "expense", "ATM CASH WDL", None)];
        let res = import_bank_statement_impl(&mut conn, &dup, "bank_hdfc").unwrap();
        assert_eq!(res.imported, 0);
        assert_eq!(res.skipped, 1);
    }

    #[test]
    fn rejects_unknown_source_and_bad_rows() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        assert!(import_bank_statement_impl(&mut conn, &[], "bank_sbi").is_err());

        let bad = vec![
            row("2026-01-01", 0.0, "expense", "zero amount", None),
            row("2026-01-01", 100.0, "transfer", "bad type", None),
            row("", 100.0, "income", "empty date", None),
        ];
        let res = import_bank_statement_impl(&mut conn, &bad, "bank_icici").unwrap();
        assert_eq!(res.imported, 0);
        assert_eq!(res.skipped, 3);
    }
}
