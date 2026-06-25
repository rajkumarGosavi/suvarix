use tauri::Manager;

pub mod auth;
pub mod db;
pub mod error;
pub mod income_expenses;
pub mod liabilities;
pub mod models;
pub mod notifications;
pub mod portfolio;
pub mod prices;
pub mod reports;
pub mod settings;
pub mod transactions;
pub mod data_sources;

use db::DbState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("finfolio.db");
            let db_state = DbState::new(db_path.to_str().unwrap())
                .expect("failed to initialize database");
            app.manage(db_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // auth
            auth::commands::is_password_set,
            auth::commands::setup_master_password,
            auth::commands::verify_master_password,
            auth::commands::change_master_password,
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
            // transactions
            transactions::commands::list_transactions,
            transactions::commands::add_transaction,
            transactions::commands::update_transaction,
            transactions::commands::delete_transaction,
            // liabilities
            liabilities::commands::list_loans,
            liabilities::commands::add_loan,
            liabilities::commands::update_loan,
            liabilities::commands::delete_loan,
            liabilities::commands::get_amortization_schedule,
            liabilities::commands::list_credit_cards,
            liabilities::commands::add_credit_card,
            liabilities::commands::update_credit_card,
            liabilities::commands::delete_credit_card,
            // income & expenses
            income_expenses::commands::get_category_summary,
            income_expenses::commands::get_budget_status,
            income_expenses::commands::set_budget,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
