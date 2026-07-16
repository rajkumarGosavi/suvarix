#[cfg(desktop)]
use tauri::menu::{Menu, MenuItem};
#[cfg(desktop)]
use tauri::tray::TrayIconBuilder;
use tauri::Manager;

pub mod constants;
pub mod auth;
pub mod categories;
pub mod db;
pub mod dev_tools;
pub mod error;
pub mod financial_health;
pub mod income_expenses;
pub mod insights;
pub mod liabilities;
pub mod logging;
pub mod models;
pub mod notifications;
pub mod portfolio;
pub mod prices;
pub mod reports;
pub mod settings;
pub mod transactions;
pub mod analytics;
pub mod backup;
pub mod data_sources;
pub mod goals;
pub mod reminders;
#[cfg(feature = "gamification")]
pub mod gamification;
#[cfg(test)]
pub mod test_utils;

use backup::scheduler::SyncSchedulerState;
use db::DbState;
use notifications::scheduler::SchedulerState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_receipt_ocr::init());

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec!["--hidden"]),
    ));

    // tauri-plugin-dialog has no directory picker on Android — auto-sync's
    // folder picker uses this plugin there instead (see backup::commands).
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_android_fs::init());

    builder
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("suvarix.db");
            let db_state = DbState::new(db_path.to_string_lossy().into_owned());
            app.manage(db_state);
            app.manage(SchedulerState::default());
            app.manage(SyncSchedulerState::default());

            #[cfg(desktop)]
            {
                // Tray icon — lets the app keep running (and keep notifying) after
                // the main window is closed instead of quitting the process.
                let show_item =
                    MenuItem::with_id(app, "show", "Open Suvarix", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
                TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => {
                            if let Some(scheduler) = app.try_state::<SchedulerState>() {
                                scheduler.stop();
                            }
                            if let Some(sync_scheduler) = app.try_state::<SyncSchedulerState>() {
                                sync_scheduler.stop();
                            }
                            if let Some(db) = app.try_state::<DbState>() {
                                db.0.lock();
                            }
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .build(app)?;

                // Closing the window hides it to the tray instead of quitting —
                // the background reminder scheduler keeps running while hidden.
                if let Some(window) = app.get_webview_window("main") {
                    let win = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = win.hide();
                        }
                    });
                }

                // Autostart launches with `--hidden` — don't show the window in that case.
                if std::env::args().any(|a| a == "--hidden") {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // auth
            auth::commands::is_password_set,
            auth::commands::setup_master_password,
            auth::commands::verify_master_password,
            auth::commands::change_master_password,
            auth::commands::lock,
            // portfolio – equity
            portfolio::commands::list_equity,
            portfolio::commands::add_equity,
            portfolio::commands::update_equity,
            portfolio::commands::delete_equity,
            // portfolio – mutual funds
            portfolio::commands::list_mf,
            portfolio::commands::add_mf,
            portfolio::commands::update_mf,
            portfolio::commands::delete_mf,
            // portfolio – FD
            portfolio::commands::list_fd,
            portfolio::commands::add_fd,
            portfolio::commands::update_fd,
            portfolio::commands::delete_fd,
            // portfolio – PPF/EPF
            portfolio::commands::list_ppf_epf,
            portfolio::commands::add_ppf_epf,
            portfolio::commands::update_ppf_epf,
            portfolio::commands::delete_ppf_epf,
            // portfolio – real estate
            portfolio::commands::list_real_estate,
            portfolio::commands::add_real_estate,
            portfolio::commands::update_real_estate,
            portfolio::commands::delete_real_estate,
            // portfolio – gold
            portfolio::commands::list_gold,
            portfolio::commands::add_gold,
            portfolio::commands::update_gold,
            portfolio::commands::delete_gold,
            // portfolio – crypto
            portfolio::commands::list_crypto,
            portfolio::commands::add_crypto,
            portfolio::commands::update_crypto,
            portfolio::commands::delete_crypto,
            // portfolio – insurance
            portfolio::commands::list_insurance,
            portfolio::commands::add_insurance,
            portfolio::commands::update_insurance,
            portfolio::commands::delete_insurance,
            // portfolio – summary
            portfolio::commands::get_net_worth,
            portfolio::commands::get_allocation_breakdown,
            // portfolio – SIP schedules
            portfolio::commands::list_sip_schedules,
            portfolio::commands::add_sip_schedule,
            portfolio::commands::update_sip_schedule,
            portfolio::commands::delete_sip_schedule,
            // portfolio – bonds
            portfolio::commands::list_bonds,
            portfolio::commands::add_bond,
            portfolio::commands::update_bond,
            portfolio::commands::delete_bond,
            // transactions
            transactions::commands::list_transactions,
            transactions::commands::count_transactions,
            transactions::commands::add_transaction,
            transactions::commands::update_transaction,
            transactions::commands::delete_transaction,
            transactions::csv_import::preview_transaction_csv,
            transactions::csv_import::import_transactions_csv,
            transactions::bank_import::import_bank_statement,
            // categories
            categories::commands::list_categories,
            categories::commands::add_category,
            categories::commands::update_category,
            categories::commands::delete_category,
            // liabilities
            liabilities::commands::list_loans,
            liabilities::commands::add_loan,
            liabilities::commands::update_loan,
            liabilities::commands::delete_loan,
            liabilities::commands::get_amortization_schedule,
            liabilities::payoff::get_debt_payoff_plan,
            liabilities::commands::list_credit_cards,
            liabilities::commands::add_credit_card,
            liabilities::commands::update_credit_card,
            liabilities::commands::delete_credit_card,
            // income & expenses
            income_expenses::commands::get_category_summary,
            income_expenses::commands::get_budget_status,
            income_expenses::commands::set_budget,
            income_expenses::commands::delete_budget,
            income_expenses::commands::get_monthly_trend,
            // prices
            prices::commands::refresh_equity_prices,
            prices::commands::refresh_mf_navs,
            prices::commands::get_market_indices,
            // reports
            reports::commands::get_capital_gains,
            reports::commands::get_net_worth_history,
            reports::commands::take_net_worth_snapshot,
            // settings
            settings::commands::get_setting,
            settings::commands::set_setting,
            settings::commands::backup_database,
            settings::commands::restore_database,
            // backup / sync
            backup::commands::export_sync_backup,
            backup::commands::import_sync_backup,
            backup::commands::set_sync_password,
            backup::commands::has_sync_password,
            backup::commands::sync_now,
            backup::commands::dedupe_duplicate_rows,
            backup::commands::get_sync_block_status,
            #[cfg(target_os = "android")]
            backup::commands::pick_sync_folder_android,
            settings::commands::wipe_all_data,
            settings::commands::get_app_data_dir,
            settings::commands::write_csv,
            // data sources – zerodha
            data_sources::commands::save_zerodha_config,
            data_sources::commands::get_zerodha_status,
            data_sources::commands::start_zerodha_login,
            data_sources::commands::sync_zerodha_holdings,
            data_sources::commands::disconnect_zerodha,
            data_sources::commands::import_cas_mf,
            // data sources – upstox
            data_sources::commands::save_upstox_config,
            data_sources::commands::get_upstox_status,
            data_sources::commands::start_upstox_login,
            data_sources::commands::sync_upstox_holdings,
            data_sources::commands::disconnect_upstox,
            // data sources – angel one
            data_sources::commands::save_angel_config,
            data_sources::commands::get_angel_status,
            data_sources::commands::login_angel,
            data_sources::commands::sync_angel_holdings,
            data_sources::commands::disconnect_angel,
            // data sources – csv import (all brokers)
            data_sources::commands::import_broker_equity_csv,
            data_sources::commands::parse_broker_equity_csv,
            data_sources::commands::import_mf_csv,
            data_sources::commands::import_generic_asset_csv,
            // goals
            goals::commands::list_goals,
            goals::commands::add_goal,
            goals::commands::update_goal,
            goals::commands::delete_goal,
            goals::commands::mark_goal_achieved,
            goals::commands::check_goal_achievements,
            // reminders
            reminders::commands::list_bills,
            reminders::commands::add_bill,
            reminders::commands::update_bill,
            reminders::commands::delete_bill,
            reminders::commands::get_upcoming_reminders,
            reminders::commands::mark_reminder_paid,
            reminders::commands::list_recurring,
            reminders::commands::add_recurring,
            reminders::commands::update_recurring,
            reminders::commands::delete_recurring,
            reminders::commands::toggle_recurring,
            reminders::commands::get_due_recurring,
            reminders::commands::apply_recurring,
            reminders::commands::check_milestones,
            reminders::commands::list_milestones,
            reminders::commands::add_milestone,
            reminders::commands::delete_milestone,
            reminders::commands::get_calendar_events,
            reminders::commands::get_maturity_alerts,
            // dev tools (available in all builds; guarded internally)
            dev_tools::is_dev_build,
            dev_tools::is_dummy_data_seeded,
            dev_tools::seed_dummy_data,
            dev_tools::clear_dummy_data,
            // analytics
            analytics::commands::track_event,
            analytics::commands::track_error,
            analytics::commands::track_perf,
            analytics::commands::get_event_stats,
            analytics::commands::get_error_log,
            analytics::commands::get_perf_stats,
            analytics::commands::export_analytics,
            analytics::commands::clear_analytics,
            // financial health (core — works with gamification off)
            financial_health::commands::get_financial_health,
            financial_health::commands::record_health_snapshot,
            financial_health::commands::get_emergency_fund,
            financial_health::commands::set_emergency_fund_target,
            // insights / behavioural nudges (core)
            insights::commands::get_insights,
            insights::commands::dismiss_insight,
            // gamification (only compiled with --features gamification)
            #[cfg(feature = "gamification")]
            gamification::commands::bootstrap_gamification,
            #[cfg(feature = "gamification")]
            gamification::commands::get_gamification_stats,
            #[cfg(feature = "gamification")]
            gamification::commands::award_xp,
            #[cfg(feature = "gamification")]
            gamification::commands::update_streak,
            #[cfg(feature = "gamification")]
            gamification::commands::check_and_award_badges,
            #[cfg(feature = "gamification")]
            gamification::commands::get_savings_streak,
            // gamification – savings challenges
            #[cfg(feature = "gamification")]
            gamification::challenges::list_challenge_templates,
            #[cfg(feature = "gamification")]
            gamification::challenges::join_challenge,
            #[cfg(feature = "gamification")]
            gamification::challenges::get_challenges,
            #[cfg(feature = "gamification")]
            gamification::challenges::evaluate_challenges,
            #[cfg(feature = "gamification")]
            gamification::challenges::abandon_challenge,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
