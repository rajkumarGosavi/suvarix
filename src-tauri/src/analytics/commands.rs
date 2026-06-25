use crate::db::DbState;
use crate::error::{AppError, Result};
use rusqlite::params;
use tauri::State;

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventStat {
    pub event_name: String,
    pub count: i64,
    pub last_seen: String,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    pub id: i64,
    pub error_type: String,
    pub message: String,
    pub created_at: String,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PerfStat {
    pub metric_name: String,
    pub avg_ms: f64,
    pub count: i64,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsExport {
    pub exported_at: String,
    pub app_version: String,
    pub events: Vec<EventStat>,
    pub errors: Vec<ErrorEntry>,
    pub perf: Vec<PerfStat>,
}

#[tauri::command]
pub fn track_event(name: String, properties: Option<String>, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO app_events (event_name, properties) VALUES (?1, ?2)",
        params![name, properties],
    )?;
    conn.execute(
        "DELETE FROM app_events WHERE created_at < datetime('now', '-90 days')",
        [],
    )?;
    Ok(())
}

#[tauri::command]
pub fn track_error(
    error_type: String,
    message: String,
    stack: Option<String>,
    context: Option<String>,
    state: State<DbState>,
) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO app_errors (error_type, message, stack, context) VALUES (?1, ?2, ?3, ?4)",
        params![error_type, message, stack, context],
    )?;
    conn.execute(
        "DELETE FROM app_errors WHERE id NOT IN (SELECT id FROM app_errors ORDER BY id DESC LIMIT 200)",
        [],
    )?;
    Ok(())
}

#[tauri::command]
pub fn track_perf(metric_name: String, value_ms: f64, state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute(
        "INSERT INTO perf_metrics (metric_name, value_ms) VALUES (?1, ?2)",
        params![metric_name, value_ms],
    )?;
    conn.execute(
        "DELETE FROM perf_metrics WHERE created_at < datetime('now', '-90 days')",
        [],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_event_stats(state: State<DbState>) -> Result<Vec<EventStat>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT event_name, COUNT(*) AS count, MAX(created_at) AS last_seen \
         FROM app_events \
         GROUP BY event_name \
         ORDER BY count DESC \
         LIMIT 20",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(EventStat {
            event_name: r.get(0)?,
            count: r.get(1)?,
            last_seen: r.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

#[tauri::command]
pub fn get_error_log(state: State<DbState>) -> Result<Vec<ErrorEntry>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT id, error_type, message, created_at \
         FROM app_errors \
         ORDER BY created_at DESC \
         LIMIT 50",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ErrorEntry {
            id: r.get(0)?,
            error_type: r.get(1)?,
            message: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

#[tauri::command]
pub fn get_perf_stats(state: State<DbState>) -> Result<Vec<PerfStat>> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    let mut stmt = conn.prepare(
        "SELECT metric_name, AVG(value_ms), COUNT(*) \
         FROM perf_metrics \
         GROUP BY metric_name \
         ORDER BY metric_name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(PerfStat {
            metric_name: r.get(0)?,
            avg_ms: r.get(1)?,
            count: r.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

#[tauri::command]
pub fn export_analytics(state: State<DbState>) -> Result<AnalyticsExport> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;

    let mut s1 = conn.prepare(
        "SELECT event_name, COUNT(*) AS count, MAX(created_at) AS last_seen \
         FROM app_events GROUP BY event_name ORDER BY count DESC",
    )?;
    let events = s1
        .query_map([], |r| {
            Ok(EventStat { event_name: r.get(0)?, count: r.get(1)?, last_seen: r.get(2)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut s2 = conn.prepare(
        "SELECT id, error_type, message, created_at FROM app_errors ORDER BY created_at DESC",
    )?;
    let errors = s2
        .query_map([], |r| {
            Ok(ErrorEntry { id: r.get(0)?, error_type: r.get(1)?, message: r.get(2)?, created_at: r.get(3)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut s3 = conn.prepare(
        "SELECT metric_name, AVG(value_ms), COUNT(*) \
         FROM perf_metrics GROUP BY metric_name ORDER BY metric_name",
    )?;
    let perf = s3
        .query_map([], |r| {
            Ok(PerfStat { metric_name: r.get(0)?, avg_ms: r.get(1)?, count: r.get(2)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(AnalyticsExport {
        exported_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        events,
        errors,
        perf,
    })
}

#[tauri::command]
pub fn clear_analytics(state: State<DbState>) -> Result<()> {
    let conn = state.0.lock().map_err(|_| AppError::Database("lock error".into()))?;
    conn.execute("DELETE FROM app_events", [])?;
    conn.execute("DELETE FROM app_errors", [])?;
    conn.execute("DELETE FROM perf_metrics", [])?;
    Ok(())
}
