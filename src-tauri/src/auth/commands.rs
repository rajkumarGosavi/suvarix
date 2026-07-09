use tauri::{AppHandle, State};
use crate::backup::scheduler::SyncSchedulerState;
use crate::db::DbState;
use crate::error::{AppError, Result};
use crate::notifications::scheduler::SchedulerState;

#[tauri::command]
pub fn is_password_set(state: State<DbState>) -> Result<bool> {
    Ok(state.0.is_password_set())
}

#[tauri::command]
pub fn setup_master_password(
    password: String,
    state: State<DbState>,
    scheduler: State<SchedulerState>,
    sync_scheduler: State<SyncSchedulerState>,
    app: AppHandle,
) -> Result<()> {
    state.0.initialize(&password)?;
    scheduler.start(app.clone(), state.0.clone());
    sync_scheduler.start(app, state.0.clone());
    Ok(())
}

#[tauri::command]
pub fn verify_master_password(
    password: String,
    state: State<DbState>,
    scheduler: State<SchedulerState>,
    sync_scheduler: State<SyncSchedulerState>,
    app: AppHandle,
) -> Result<bool> {
    let ok = state.0.unlock(&password)?;
    if ok {
        // Runs before the sync scheduler starts, so a first-ever-run cleanup
        // (see `run_dedupe_once_on_unlock`) finishes before this device could
        // export any of the duplicates it's about to delete.
        if let Err(e) = crate::backup::commands::run_dedupe_once_on_unlock(&state.0) {
            tracing::warn!("one-time duplicate cleanup failed: {e}");
        }
        scheduler.start(app.clone(), state.0.clone());
        sync_scheduler.start(app, state.0.clone());
    }
    Ok(ok)
}

/// Locks the DB (drops pool + in-memory password) and stops the background
/// reminder + auto-sync schedulers. Must be called explicitly — closing the
/// window only hides it to the tray, it does not lock the DB.
#[tauri::command]
pub fn lock(
    state: State<DbState>,
    scheduler: State<SchedulerState>,
    sync_scheduler: State<SyncSchedulerState>,
) -> Result<()> {
    scheduler.stop();
    sync_scheduler.stop();
    state.0.lock();
    Ok(())
}

#[tauri::command]
pub fn change_master_password(
    current_password: String,
    new_password: String,
    state: State<DbState>,
) -> Result<()> {
    state.0.get()?;
    if !state.0.verify_password(&current_password)? {
        return Err(AppError::WrongPassword);
    }
    state.0.rekey(&new_password)
}
