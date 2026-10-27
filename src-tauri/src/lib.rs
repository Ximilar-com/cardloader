mod commands;
mod db;
mod ebay;
mod error;
mod export;
mod i18n;
mod imaging;
mod local;
mod market;
mod mcp;
mod model;
mod pairing;
mod runner;
mod settings;
mod source;
mod sync;
mod update;
mod ximilar;

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// `cardloader mcp`: relays an AI agent's stdio to the running app's MCP server (see `mcp::bridge`).
pub fn mcp_bridge() -> i32 {
    mcp::bridge::run()
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cardloader_lib=info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = db::Db::open(&data_dir.join("cardloader.db"))?;
            db.recover_interrupted()?;
            let settings = Arc::new(settings::SettingsStore::load(
                config_dir.join("settings.json"),
            ));
            let handle = app.handle().clone();
            let runner = runner::Runner::new(
                Some(handle.clone()),
                Arc::new(handle),
                db.clone(),
                settings.clone(),
                data_dir,
            );
            let events = app.handle().clone();
            runner.local.on_change(move |status| {
                use tauri::Emitter;
                let _ = events.emit("model-updated", status);
            });
            let state = AppState {
                db,
                settings,
                runner,
            };
            let mcp = mcp::McpServer::new(state.clone(), config_dir);
            app.manage(state);
            app.manage(mcp.clone());
            app.manage(update::Updates::default());
            tauri::async_runtime::spawn(async move { mcp.apply().await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::set_token,
            commands::test_connection,
            commands::mcp_status,
            commands::regenerate_mcp_token,
            commands::list_sessions,
            commands::dashboard_activity,
            commands::get_session,
            commands::create_session,
            commands::update_session,
            commands::delete_session,
            commands::rename_session,
            commands::session_folder,
            commands::export_csv,
            commands::preview_csv,
            commands::import_csv,
            commands::run_session,
            commands::pause_session,
            commands::stop_session,
            commands::retry_items,
            commands::sync_session,
            commands::list_items,
            commands::get_item,
            commands::select_match,
            commands::search_cards,
            commands::update_item,
            commands::turn_items,
            commands::review_items,
            commands::turn_item,
            commands::locate_items,
            commands::delete_items,
            commands::list_models,
            commands::download_model,
            commands::cancel_model_download,
            commands::delete_model,
            commands::compare_local,
            commands::cancel_compare,
            commands::count_images,
            commands::app_info,
            commands::check_update,
            commands::install_update,
            commands::scanner_support,
            commands::list_scanners,
            commands::scanner_driver_settings,
            commands::list_remote_sessions,
            commands::rename_remote_session,
            commands::delete_remote_session,
            commands::get_remote_session,
            commands::list_remote_items,
            commands::set_ebay_cert,
            commands::ebay_connect,
            commands::ebay_connect_with,
            commands::ebay_disconnect,
            commands::ebay_setup,
            commands::ebay_opt_in,
            commands::ebay_create_location,
            commands::ebay_preview,
            commands::ebay_create_drafts,
            commands::ebay_publish,
            commands::ebay_refresh,
            commands::ebay_remove,
            commands::ebay_cancel,
            commands::ebay_job,
            commands::ebay_listing_url,
            commands::set_imgbb_key,
            commands::export_preview,
            commands::export_tcgplayer,
            commands::export_whatnot,
            commands::export_shopify,
            commands::cancel_export,
        ])
        .build(tauri::generate_context!())
        .expect("error while building cardloader")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                before_exit(app);
            }
        });
}

/// Stops the runs (their cards in flight go back to the queue on the next launch) and removes the MCP endpoint.
fn before_exit(app: &AppHandle) {
    app.state::<AppState>().runner.stop_all();
    app.state::<Arc<mcp::McpServer>>().forget_endpoint();
}
