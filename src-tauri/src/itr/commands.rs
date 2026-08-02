use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::db::DbState;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItrReturn {
    /// None when the row is new; set on rows returned from the DB.
    pub id: Option<i64>,
    /// "2024-25". Unique — re-saving the same year overwrites.
    pub assessment_year: String,
    pub form_type: String,
    pub regime: Option<String>,
    /// Masked only, e.g. "XXXXX1234X". Full PAN is never stored.
    pub pan_masked: Option<String>,
    pub filing_date: Option<String>,
    pub ack_number: Option<String>,

    pub salary_income: f64,
    pub house_property_income: f64,
    pub capital_gains_stcg: f64,
    pub capital_gains_ltcg: f64,
    pub other_sources_income: f64,
    pub business_income: f64,
    pub gross_total_income: f64,

    pub chapter_via_deductions: f64,
    pub total_income: f64,

    pub tax_on_total_income: f64,
    pub surcharge: f64,
    pub cess: f64,
    pub total_tax_liability: f64,

    pub tds_paid: f64,
    pub advance_tax_paid: f64,
    pub self_assessment_tax_paid: f64,
    pub tcs_paid: f64,
    pub total_tax_paid: f64,

    pub refund_due: f64,
    pub tax_payable: f64,

    /// "pdf" | "manual"
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItrSummary {
    pub returns_count: i64,
    pub lifetime_tax_paid: f64,
    pub lifetime_gross_income: f64,
    /// Lifetime tax liability over lifetime gross income, as a percentage (2 dp).
    pub average_effective_rate: f64,
    pub latest_assessment_year: Option<String>,
    pub latest_total_tax_liability: f64,
}

const SELECT_COLS: &str = "
    id, assessment_year, form_type, regime, pan_masked, filing_date, ack_number,
    salary_income, house_property_income, capital_gains_stcg, capital_gains_ltcg,
    other_sources_income, business_income, gross_total_income,
    chapter_via_deductions, total_income,
    tax_on_total_income, surcharge, cess, total_tax_liability,
    tds_paid, advance_tax_paid, self_assessment_tax_paid, tcs_paid, total_tax_paid,
    refund_due, tax_payable, source
";

fn map_row(r: &Row) -> rusqlite::Result<ItrReturn> {
    Ok(ItrReturn {
        id: r.get(0)?,
        assessment_year: r.get(1)?,
        form_type: r.get(2)?,
        regime: r.get(3)?,
        pan_masked: r.get(4)?,
        filing_date: r.get(5)?,
        ack_number: r.get(6)?,
        salary_income: r.get(7)?,
        house_property_income: r.get(8)?,
        capital_gains_stcg: r.get(9)?,
        capital_gains_ltcg: r.get(10)?,
        other_sources_income: r.get(11)?,
        business_income: r.get(12)?,
        gross_total_income: r.get(13)?,
        chapter_via_deductions: r.get(14)?,
        total_income: r.get(15)?,
        tax_on_total_income: r.get(16)?,
        surcharge: r.get(17)?,
        cess: r.get(18)?,
        total_tax_liability: r.get(19)?,
        tds_paid: r.get(20)?,
        advance_tax_paid: r.get(21)?,
        self_assessment_tax_paid: r.get(22)?,
        tcs_paid: r.get(23)?,
        total_tax_paid: r.get(24)?,
        refund_due: r.get(25)?,
        tax_payable: r.get(26)?,
        source: r.get(27)?,
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Upsert by `assessment_year`; returns the row id.
fn insert_or_update(conn: &Connection, ret: &ItrReturn) -> Result<i64> {
    if ret.assessment_year.trim().is_empty() {
        return Err(AppError::Validation("Assessment year is required".into()));
    }
    if ret.form_type.trim().is_empty() {
        return Err(AppError::Validation("Form type is required".into()));
    }

    conn.execute(
        "INSERT INTO itr_returns (
            assessment_year, form_type, regime, pan_masked, filing_date, ack_number,
            salary_income, house_property_income, capital_gains_stcg, capital_gains_ltcg,
            other_sources_income, business_income, gross_total_income,
            chapter_via_deductions, total_income,
            tax_on_total_income, surcharge, cess, total_tax_liability,
            tds_paid, advance_tax_paid, self_assessment_tax_paid, tcs_paid, total_tax_paid,
            refund_due, tax_payable, source, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6,
            ?7, ?8, ?9, ?10,
            ?11, ?12, ?13,
            ?14, ?15,
            ?16, ?17, ?18, ?19,
            ?20, ?21, ?22, ?23, ?24,
            ?25, ?26, ?27, datetime('now'), datetime('now')
         )
         ON CONFLICT(assessment_year) DO UPDATE SET
            form_type = excluded.form_type,
            regime = excluded.regime,
            pan_masked = excluded.pan_masked,
            filing_date = excluded.filing_date,
            ack_number = excluded.ack_number,
            salary_income = excluded.salary_income,
            house_property_income = excluded.house_property_income,
            capital_gains_stcg = excluded.capital_gains_stcg,
            capital_gains_ltcg = excluded.capital_gains_ltcg,
            other_sources_income = excluded.other_sources_income,
            business_income = excluded.business_income,
            gross_total_income = excluded.gross_total_income,
            chapter_via_deductions = excluded.chapter_via_deductions,
            total_income = excluded.total_income,
            tax_on_total_income = excluded.tax_on_total_income,
            surcharge = excluded.surcharge,
            cess = excluded.cess,
            total_tax_liability = excluded.total_tax_liability,
            tds_paid = excluded.tds_paid,
            advance_tax_paid = excluded.advance_tax_paid,
            self_assessment_tax_paid = excluded.self_assessment_tax_paid,
            tcs_paid = excluded.tcs_paid,
            total_tax_paid = excluded.total_tax_paid,
            refund_due = excluded.refund_due,
            tax_payable = excluded.tax_payable,
            source = excluded.source,
            updated_at = datetime('now')",
        params![
            ret.assessment_year.trim(), ret.form_type.trim(), ret.regime, ret.pan_masked,
            ret.filing_date, ret.ack_number,
            ret.salary_income, ret.house_property_income, ret.capital_gains_stcg,
            ret.capital_gains_ltcg, ret.other_sources_income, ret.business_income,
            ret.gross_total_income,
            ret.chapter_via_deductions, ret.total_income,
            ret.tax_on_total_income, ret.surcharge, ret.cess, ret.total_tax_liability,
            ret.tds_paid, ret.advance_tax_paid, ret.self_assessment_tax_paid,
            ret.tcs_paid, ret.total_tax_paid,
            ret.refund_due, ret.tax_payable, ret.source,
        ],
    )?;

    let id: i64 = conn.query_row(
        "SELECT id FROM itr_returns WHERE assessment_year = ?1",
        params![ret.assessment_year.trim()],
        |r| r.get(0),
    )?;
    Ok(id)
}

fn fetch_all(conn: &Connection) -> Result<Vec<ItrReturn>> {
    let sql = format!("SELECT {SELECT_COLS} FROM itr_returns ORDER BY assessment_year ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn compute_summary(conn: &Connection) -> Result<ItrSummary> {
    let rows = fetch_all(conn)?;
    if rows.is_empty() {
        return Ok(ItrSummary {
            returns_count: 0,
            lifetime_tax_paid: 0.0,
            lifetime_gross_income: 0.0,
            average_effective_rate: 0.0,
            latest_assessment_year: None,
            latest_total_tax_liability: 0.0,
        });
    }

    let lifetime_tax_paid: f64 = rows.iter().map(|r| r.total_tax_paid).sum();
    let lifetime_gross_income: f64 = rows.iter().map(|r| r.gross_total_income).sum();
    let lifetime_liability: f64 = rows.iter().map(|r| r.total_tax_liability).sum();
    let rate = if lifetime_gross_income > 0.0 {
        round2(lifetime_liability / lifetime_gross_income * 100.0)
    } else {
        0.0
    };
    // fetch_all sorts ascending, so the last row is the newest assessment year.
    let latest = rows.last().expect("non-empty checked above");

    Ok(ItrSummary {
        returns_count: rows.len() as i64,
        lifetime_tax_paid: round2(lifetime_tax_paid),
        lifetime_gross_income: round2(lifetime_gross_income),
        average_effective_rate: rate,
        latest_assessment_year: Some(latest.assessment_year.clone()),
        latest_total_tax_liability: latest.total_tax_liability,
    })
}

#[tauri::command]
pub fn save_itr_return(ret: ItrReturn, state: State<DbState>) -> Result<i64> {
    let conn = state.0.get()?;
    insert_or_update(&conn, &ret)
}

#[tauri::command]
pub fn list_itr_returns(state: State<DbState>) -> Result<Vec<ItrReturn>> {
    let conn = state.0.get()?;
    fetch_all(&conn)
}

#[tauri::command]
pub fn delete_itr_return(id: i64, state: State<DbState>) -> Result<()> {
    let conn = state.0.get()?;
    let affected = conn.execute("DELETE FROM itr_returns WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("ITR return {id}")));
    }
    Ok(())
}

#[tauri::command]
pub fn get_itr_summary(state: State<DbState>) -> Result<ItrSummary> {
    let conn = state.0.get()?;
    compute_summary(&conn)
}

/// Dumps the frontend's parse report to `<app data dir>/itr-parse-debug.log`,
/// overwriting any previous run, and returns the path so the UI can show it.
///
/// The report holds real income and tax figures in plain text, outside the
/// SQLCipher DB — it exists purely to tune the ITR-2 parser rules against a real
/// return, and is meant to be deleted once the patterns are right.
#[tauri::command]
pub fn write_itr_debug_log(content: String, app: AppHandle) -> Result<String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(format!("app data dir unavailable: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("itr-parse-debug.log");
    std::fs::write(&path, content)?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db_state;

    fn sample(ay: &str, salary: f64, tds: f64) -> ItrReturn {
        ItrReturn {
            id: None,
            assessment_year: ay.to_string(),
            form_type: "ITR-2".into(),
            regime: Some("old".into()),
            pan_masked: Some("XXXXX1234X".into()),
            filing_date: Some("2024-07-20".into()),
            ack_number: Some("123456789012345".into()),
            salary_income: salary,
            house_property_income: 0.0,
            capital_gains_stcg: 0.0,
            capital_gains_ltcg: 0.0,
            other_sources_income: 0.0,
            business_income: 0.0,
            gross_total_income: salary,
            chapter_via_deductions: 150_000.0,
            total_income: salary - 150_000.0,
            tax_on_total_income: 100_000.0,
            surcharge: 0.0,
            cess: 4_000.0,
            total_tax_liability: 104_000.0,
            tds_paid: tds,
            advance_tax_paid: 0.0,
            self_assessment_tax_paid: 0.0,
            tcs_paid: 0.0,
            total_tax_paid: tds,
            refund_due: 0.0,
            tax_payable: 0.0,
            source: "pdf".into(),
        }
    }

    #[test]
    fn save_is_upsert_by_assessment_year() {
        let (_dir, state) = test_db_state();
        let conn = state.0.get().unwrap();

        insert_or_update(&conn, &sample("2024-25", 1_000_000.0, 90_000.0)).unwrap();
        insert_or_update(&conn, &sample("2024-25", 1_200_000.0, 95_000.0)).unwrap();

        let rows = fetch_all(&conn).unwrap();
        assert_eq!(rows.len(), 1, "same assessment year must upsert, not duplicate");
        assert_eq!(rows[0].salary_income, 1_200_000.0);
        assert_eq!(rows[0].tds_paid, 95_000.0);
    }

    #[test]
    fn list_is_ordered_by_assessment_year_ascending() {
        let (_dir, state) = test_db_state();
        let conn = state.0.get().unwrap();

        insert_or_update(&conn, &sample("2024-25", 1_000_000.0, 90_000.0)).unwrap();
        insert_or_update(&conn, &sample("2022-23", 800_000.0, 60_000.0)).unwrap();
        insert_or_update(&conn, &sample("2023-24", 900_000.0, 75_000.0)).unwrap();

        let years: Vec<String> = fetch_all(&conn).unwrap()
            .into_iter().map(|r| r.assessment_year).collect();
        assert_eq!(years, vec!["2022-23", "2023-24", "2024-25"]);
    }

    #[test]
    fn summary_aggregates_lifetime_totals_and_effective_rate() {
        let (_dir, state) = test_db_state();
        let conn = state.0.get().unwrap();

        insert_or_update(&conn, &sample("2023-24", 1_000_000.0, 50_000.0)).unwrap();
        insert_or_update(&conn, &sample("2024-25", 2_000_000.0, 70_000.0)).unwrap();

        let s = compute_summary(&conn).unwrap();
        assert_eq!(s.returns_count, 2);
        assert_eq!(s.lifetime_tax_paid, 120_000.0);
        assert_eq!(s.lifetime_gross_income, 3_000_000.0);
        assert_eq!(s.latest_assessment_year.as_deref(), Some("2024-25"));
        // 104000 + 104000 = 208000 liability over 3,000,000 gross → 6.93%
        assert_eq!(s.average_effective_rate, 6.93);
    }

    #[test]
    fn summary_on_empty_db_is_zeroed_not_an_error() {
        let (_dir, state) = test_db_state();
        let conn = state.0.get().unwrap();

        let s = compute_summary(&conn).unwrap();
        assert_eq!(s.returns_count, 0);
        assert_eq!(s.lifetime_tax_paid, 0.0);
        assert_eq!(s.average_effective_rate, 0.0);
        assert!(s.latest_assessment_year.is_none());
    }

    #[test]
    fn delete_removes_only_the_named_row() {
        let (_dir, state) = test_db_state();
        let conn = state.0.get().unwrap();

        let id = insert_or_update(&conn, &sample("2023-24", 1_000_000.0, 50_000.0)).unwrap();
        insert_or_update(&conn, &sample("2024-25", 2_000_000.0, 70_000.0)).unwrap();

        conn.execute("DELETE FROM itr_returns WHERE id = ?1", rusqlite::params![id]).unwrap();

        let years: Vec<String> = fetch_all(&conn).unwrap()
            .into_iter().map(|r| r.assessment_year).collect();
        assert_eq!(years, vec!["2024-25"]);
    }

    #[test]
    fn save_rejects_a_blank_assessment_year() {
        let (_dir, state) = test_db_state();
        let conn = state.0.get().unwrap();

        let mut bad = sample("", 1_000_000.0, 10_000.0);
        bad.assessment_year = "   ".into();
        assert!(insert_or_update(&conn, &bad).is_err());
    }
}
