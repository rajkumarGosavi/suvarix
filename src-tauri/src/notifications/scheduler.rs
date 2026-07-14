// Background reminder scheduler — runs while the DB is unlocked (including
// when the main window is hidden to the system tray), periodically checking
// bills/loans/credit-card due dates and FD/bond maturities, firing native OS
// notifications for anything not already notified. Started on unlock, stopped
// on lock/quit so it never touches the pool while `AuthRequired`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::db::DbPool;
use crate::error::Result;
use crate::insights::commands::compute_nudges;
use crate::reminders::commands::{maturity_alerts, upcoming_reminders};

const CHECK_INTERVAL_SECS: u64 = 30 * 60;
const DEDUP_SETTING_KEY: &str = "notified_reminder_ids";
const DEDUP_CAP: usize = 500;
const BILL_LOOKAHEAD_DAYS: i32 = 1;
const MATURITY_LOOKAHEAD_DAYS: i64 = 7;
// At most this many nudge notifications per tick — the in-app feed carries the
// rest, so a struggling user isn't buried in OS notifications.
const NUDGE_PUSH_CAP: usize = 2;

/// Holds the running background task so it can be cancelled on lock/quit.
/// Uses Tauri's own runtime handle (not raw `tokio::spawn`) — `#[tauri::command]`
/// handlers run on Tauri's command thread pool, which isn't itself a Tokio
/// worker thread, so `tokio::spawn` there panics with "no reactor running".
#[derive(Default)]
pub struct SchedulerState(Mutex<Option<tauri::async_runtime::JoinHandle<()>>>);

impl SchedulerState {
    /// Starts the periodic check loop, replacing any previously running one —
    /// safe to call again on re-unlock without an intervening lock.
    pub fn start(&self, app: AppHandle, db: Arc<DbPool>) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(old) = guard.take() {
                old.abort();
            }
            *guard = Some(spawn_reminder_loop(app, db));
        }
    }

    /// Cancels the running loop, if any. Called on lock() and on app quit.
    pub fn stop(&self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

fn spawn_reminder_loop(app: AppHandle, db: Arc<DbPool>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        loop {
            interval.tick().await; // first tick fires immediately
            if let Err(e) = run_check(&app, &db) {
                // A locked DB (AuthRequired) shows up here too — expected when
                // the user locks mid-interval; just skip this tick and keep looping.
                tracing::warn!("reminder scheduler check skipped: {e}");
            }
        }
    })
}

fn run_check(app: &AppHandle, db: &DbPool) -> Result<()> {
    let reminders = upcoming_reminders(db, BILL_LOOKAHEAD_DAYS)?;
    let maturities = maturity_alerts(db, MATURITY_LOOKAHEAD_DAYS)?;
    let mut notified = load_notified(db)?;
    let mut newly = Vec::new();
    tracing::debug!(
        reminders = reminders.len(),
        maturities = maturities.len(),
        already_notified = notified.len(),
        "reminder scheduler tick"
    );

    for r in &reminders {
        if r.days_until_due > BILL_LOOKAHEAD_DAYS as i64 {
            continue;
        }
        let key = format!("reminder:{}:{}:{}", r.source, r.source_id, r.due_date);
        if notified.contains(&key) {
            continue;
        }
        notify(
            app,
            &format!("Bill due: {}", r.name),
            &format!("₹{:.2} due {}", r.amount, r.due_date),
        );
        newly.push(key);
    }

    for m in &maturities {
        if m.days_until_maturity > MATURITY_LOOKAHEAD_DAYS {
            continue;
        }
        let key = format!("maturity:{}:{}:{}", m.source, m.source_id, m.maturity_date);
        if notified.contains(&key) {
            continue;
        }
        notify(
            app,
            &format!("Maturity: {}", m.name),
            &format!("Matures {}", m.maturity_date),
        );
        newly.push(key);
    }

    // Behavioural nudges: push only the urgent ones (critical/warning), deduped per
    // ISO week so an unresolved problem re-nudges next week rather than every tick.
    {
        let conn = db.get()?;
        let week = {
            use chrono::Datelike;
            let iso = chrono::Local::now().iso_week();
            format!("{}W{:02}", iso.year(), iso.week())
        };
        let mut pushed = 0usize;
        for n in compute_nudges(&conn) {
            if pushed >= NUDGE_PUSH_CAP {
                break;
            }
            if n.severity != "critical" && n.severity != "warning" {
                continue;
            }
            let key = format!("nudge:{}:{}", n.id, week);
            if notified.contains(&key) {
                continue;
            }
            notify(app, &n.title, &n.body);
            newly.push(key);
            pushed += 1;
        }
    }

    if !newly.is_empty() {
        tracing::debug!(count = newly.len(), "reminder scheduler firing new notifications");
        notified.extend(newly);
        if notified.len() > DEDUP_CAP {
            let excess = notified.len() - DEDUP_CAP;
            notified.drain(0..excess);
        }
        save_notified(db, &notified)?;
    }
    Ok(())
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn load_notified(db: &DbPool) -> Result<Vec<String>> {
    let conn = db.get()?;
    let raw: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [DEDUP_SETTING_KEY],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_notified(db: &DbPool, notified: &[String]) -> Result<()> {
    let conn = db.get()?;
    let json = serde_json::to_string(notified)?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![DEDUP_SETTING_KEY, json],
    )?;
    Ok(())
}
