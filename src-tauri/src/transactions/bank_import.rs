use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::HashSet;
use tauri::State;

use crate::db::DbState;
use crate::error::{AppError, Result};

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
    /// Id of an existing manual transaction this bank row confirms. When set,
    /// the manual row is stamped with the bank reference instead of inserting.
    #[serde(default)]
    pub merge_into: Option<i64>,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BankImportResult {
    pub imported: i64,
    pub skipped: i64,
    pub merged: i64,
}

/// A manual transaction that may be the same real-world payment as a bank row.
#[derive(serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatchCandidate {
    pub id: i64,
    pub date: String,
    pub amount: f64,
    pub description: Option<String>,
    pub category: Option<String>,
    pub day_diff: i64,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BankRowMatch {
    /// Index of the row in the request.
    pub index: usize,
    /// Row will be skipped by exact dedup anyway (ref or date+amount+desc seen).
    pub already_imported: bool,
    /// Closest date first.
    pub candidates: Vec<MatchCandidate>,
}

pub const DEFAULT_MATCH_WINDOW_DAYS: i64 = 3;
pub const MAX_MATCH_WINDOW_DAYS: i64 = 7;

/// Whitelist of accepted `source` values so a caller can't stamp arbitrary text.
fn normalize_source(source: &str) -> Result<&'static str> {
    match source {
        "bank_hdfc" => Ok("bank_hdfc"),
        "bank_icici" => Ok("bank_icici"),
        _ => Err(AppError::Parse(format!("unknown bank source: {source}"))),
    }
}

fn trimmed(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn parse_day(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date.trim().get(..10)?, "%Y-%m-%d").ok()
}

fn to_paise(amount: f64) -> i64 {
    (amount.abs() * 100.0).round() as i64
}

/// Stable stand-in reference for a bank row without a ref number, stored on a
/// merged manual row so re-importing the same statement recognises it.
fn synthetic_ref(source: &str, date: &str, txn_type: &str, paise: i64, desc_norm: &str) -> String {
    let day = date.trim().get(..10).unwrap_or(date.trim());
    format!("auto:{source}:{day}:{txn_type}:{paise}:{desc_norm}")
}

struct Existing {
    seen_ref: HashSet<String>,
    seen_key: HashSet<(String, i64, String)>,
}

fn load_existing(conn: &Connection) -> Result<Existing> {
    let mut seen_ref = HashSet::new();
    let mut seen_key = HashSet::new();
    let mut stmt = conn.prepare("SELECT date, amount, description, external_ref FROM transactions")?;
    let rows_it = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    for (date, amount, desc, ext) in rows_it.filter_map(|r| r.ok()) {
        seen_key.insert((date, to_paise(amount), desc.unwrap_or_default().trim().to_lowercase()));
        if let Some(e) = ext {
            let e = e.trim();
            if !e.is_empty() {
                seen_ref.insert(e.to_string());
            }
        }
    }
    Ok(Existing { seen_ref, seen_key })
}

/// Keys used for exact dedup of one bank row: (effective ref, (date, paise, desc)).
fn row_keys(row: &BankTxnInput, source: &str) -> (String, (String, i64, String)) {
    let date = row.date.trim().to_string();
    let paise = to_paise(row.amount);
    let desc_norm = trimmed(&row.description).unwrap_or_default().to_lowercase();
    let eff_ref = trimmed(&row.ref_no)
        .unwrap_or_else(|| synthetic_ref(source, &date, &row.txn_type, paise, &desc_norm));
    (eff_ref, (date, paise, desc_norm))
}

/// For each bank row, find manual transactions (no bank reference yet) with the
/// same type and exact amount within ±`window_days` (default 3, max 7). The
/// frontend shows these as "possible duplicates" so the user can Merge or Skip.
#[tauri::command]
pub fn find_bank_import_matches(
    rows: Vec<BankTxnInput>,
    source: String,
    window_days: Option<i64>,
    state: State<DbState>,
) -> Result<Vec<BankRowMatch>> {
    let conn = state.0.get()?;
    find_bank_import_matches_impl(&conn, &rows, &source, window_days)
}

pub fn find_bank_import_matches_impl(
    conn: &Connection,
    rows: &[BankTxnInput],
    source: &str,
    window_days: Option<i64>,
) -> Result<Vec<BankRowMatch>> {
    let source = normalize_source(source)?;
    let window = window_days.unwrap_or(DEFAULT_MATCH_WINDOW_DAYS).clamp(0, MAX_MATCH_WINDOW_DAYS);
    let existing = load_existing(conn)?;

    // Unverified manual rows: the only ones a bank row may merge into.
    struct Manual { id: i64, day: NaiveDate, date: String, paise: i64, ty: String, amount: f64, desc: Option<String>, cat: Option<String> }
    let mut stmt = conn.prepare(
        "SELECT id, date, amount, type, description, category FROM transactions
         WHERE (source IS NULL OR source='manual')
           AND (external_ref IS NULL OR external_ref='')",
    )?;
    let manual: Vec<Manual> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .filter_map(|(id, date, amount, ty, desc, cat)| {
            Some(Manual { id, day: parse_day(&date)?, date, paise: to_paise(amount), ty, amount, desc, cat })
        })
        .collect();

    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let (eff_ref, key) = row_keys(row, source);
        let already_imported = existing.seen_ref.contains(&eff_ref) || existing.seen_key.contains(&key);
        let mut candidates = Vec::new();
        if let (false, Some(day)) = (already_imported, parse_day(&row.date)) {
            for m in manual.iter().filter(|m| m.ty == row.txn_type && m.paise == key.1) {
                let diff = (m.day - day).num_days().abs();
                if diff <= window {
                    candidates.push(MatchCandidate {
                        id: m.id,
                        date: m.date.clone(),
                        amount: m.amount,
                        description: m.desc.clone(),
                        category: m.cat.clone(),
                        day_diff: diff,
                    });
                }
            }
            candidates.sort_by_key(|c| (c.day_diff, c.id));
        }
        out.push(BankRowMatch { index, already_imported, candidates });
    }
    Ok(out)
}

/// Insert pre-parsed bank-statement rows into the transactions ledger.
///
/// Dedup is two-tier and stronger than the plain CSV importer:
///   1. If a row carries a bank reference number (`ref_no`) that already exists
///      in `transactions.external_ref`, skip it (exact match, survives edits).
///      Rows without a ref use a synthetic ref, which only exists on merged rows.
///   2. Otherwise fall back to the `(date, paise, description_lower)` heuristic
///      used by `csv_import.rs`.
///
/// Rows with `merge_into` stamp that manual transaction with the bank ref
/// (source stays 'manual' → shown as "Verified") instead of inserting. If the
/// target is gone or already verified, the row falls through to a normal insert.
#[tauri::command]
pub fn import_bank_statement(
    rows: Vec<BankTxnInput>,
    source: String,
    state: State<DbState>,
) -> Result<BankImportResult> {
    let mut conn = state.0.get()?;
    import_bank_statement_impl(&mut conn, &rows, &source)
}

pub fn import_bank_statement_impl(
    conn: &mut Connection,
    rows: &[BankTxnInput],
    source: &str,
) -> Result<BankImportResult> {
    let source = normalize_source(source)?;
    let Existing { mut seen_ref, mut seen_key } = load_existing(conn)?;

    let tx = conn.transaction()?;
    let mut imported = 0i64;
    let mut skipped = 0i64;
    let mut merged = 0i64;

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

        let ref_no = trimmed(&row.ref_no);
        let description = trimmed(&row.description);
        let category = trimmed(&row.category);
        let tag = trimmed(&row.tag);
        let (eff_ref, key) = row_keys(row, source);

        // Tier 1: reference-number dedup (real or synthetic).
        if seen_ref.contains(&eff_ref) {
            skipped += 1;
            continue;
        }
        // Tier 2: (date, paise, desc) heuristic.
        if seen_key.contains(&key) {
            skipped += 1;
            continue;
        }

        // Merge into a manual row: same type + amount, still unverified.
        if let Some(target) = row.merge_into {
            let n = tx.execute(
                "UPDATE transactions SET external_ref=?1, updated_at=datetime('now')
                 WHERE id=?2 AND type=?3 AND ROUND(ABS(amount)*100)=?4
                   AND (source IS NULL OR source='manual')
                   AND (external_ref IS NULL OR external_ref='')",
                rusqlite::params![eff_ref, target, txn_type, key.1],
            )?;
            if n == 1 {
                merged += 1;
                seen_ref.insert(eff_ref);
                continue;
            }
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
    Ok(BankImportResult { imported, skipped, merged })
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
            merge_into: None,
        }
    }

    fn add_manual(conn: &Connection, date: &str, amount: f64, ty: &str, desc: &str) -> i64 {
        conn.execute(
            "INSERT INTO transactions (date, type, amount, description) VALUES (?1,?2,?3,?4)",
            rusqlite::params![date, ty, amount, desc],
        )
        .unwrap();
        conn.last_insert_rowid()
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

    #[test]
    fn finds_manual_match_within_window_closest_first() {
        let (_dir, pool) = test_db_pool();
        let conn = pool.get().unwrap();
        let far = add_manual(&conn, "2026-01-03", 100.0, "expense", "tea far");
        let near = add_manual(&conn, "2026-01-11 09:30", 100.0, "expense", "Tea");
        add_manual(&conn, "2026-01-20", 100.0, "expense", "outside window");
        add_manual(&conn, "2026-01-10", 100.0, "income", "wrong type");
        add_manual(&conn, "2026-01-10", 101.0, "expense", "wrong amount");

        let rows = vec![row("2026-01-10 00:00:00", 100.0, "expense", "UPI/CHAI POINT", Some("R9"))];
        let m = find_bank_import_matches_impl(&conn, &rows, "bank_hdfc", Some(7)).unwrap();
        let ids: Vec<i64> = m[0].candidates.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![near, far]);
        assert!(!m[0].already_imported);

        // Default window (3 days) drops the 7-day-old one.
        let m = find_bank_import_matches_impl(&conn, &rows, "bank_hdfc", None).unwrap();
        assert_eq!(m[0].candidates.len(), 1);
    }

    #[test]
    fn merge_marks_manual_row_verified_and_reimport_skips() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        let id = add_manual(&conn, "2026-01-11", 100.0, "expense", "Tea");

        let mut r = row("2026-01-10 00:00:00", 100.0, "expense", "UPI/CHAI POINT", Some("R9"));
        r.merge_into = Some(id);
        let res = import_bank_statement_impl(&mut conn, &[r], "bank_hdfc").unwrap();
        assert_eq!((res.imported, res.merged, res.skipped), (0, 1, 0));

        let (src, ext, desc): (String, String, String) = conn
            .query_row("SELECT source, external_ref, description FROM transactions WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((src.as_str(), ext.as_str(), desc.as_str()), ("manual", "R9", "Tea"));

        // Verified rows are no longer candidates; re-import is skipped by ref.
        let again = vec![row("2026-01-10 00:00:00", 100.0, "expense", "UPI/CHAI POINT", Some("R9"))];
        let m = find_bank_import_matches_impl(&conn, &again, "bank_hdfc", None).unwrap();
        assert!(m[0].already_imported && m[0].candidates.is_empty());
        let res = import_bank_statement_impl(&mut conn, &again, "bank_hdfc").unwrap();
        assert_eq!((res.imported, res.skipped), (0, 1));
    }

    #[test]
    fn merge_without_ref_uses_synthetic_ref_for_reimport() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        let id = add_manual(&conn, "2026-01-10", 500.0, "expense", "cash");

        let mut r = row("2026-01-10 00:00:00", 500.0, "expense", "ATM WDL", None);
        r.merge_into = Some(id);
        assert_eq!(import_bank_statement_impl(&mut conn, &[r], "bank_hdfc").unwrap().merged, 1);

        let again = vec![row("2026-01-10 00:00:00", 500.0, "expense", "ATM WDL", None)];
        let res = import_bank_statement_impl(&mut conn, &again, "bank_hdfc").unwrap();
        assert_eq!((res.imported, res.skipped), (0, 1));
    }

    #[test]
    fn merge_into_claimed_row_falls_back_to_insert() {
        let (_dir, pool) = test_db_pool();
        let mut conn = pool.get().unwrap();
        let id = add_manual(&conn, "2026-01-10", 100.0, "expense", "Tea");

        let mut a = row("2026-01-10 00:00:00", 100.0, "expense", "CHAI 1", Some("A1"));
        a.merge_into = Some(id);
        let mut b = row("2026-01-10 00:00:00", 100.0, "expense", "CHAI 2", Some("A2"));
        b.merge_into = Some(id);
        let res = import_bank_statement_impl(&mut conn, &[a, b], "bank_hdfc").unwrap();
        assert_eq!((res.merged, res.imported), (1, 1));
    }
}
