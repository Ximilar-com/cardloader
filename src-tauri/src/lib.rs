mod commands;
mod db;
mod error;
mod export;
mod imaging;
mod local;
mod model;
mod pairing;
mod runner;
mod settings;
mod source;
mod sync;
mod ximilar;

use std::sync::Arc;

use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
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
            app.manage(AppState {
                db,
                settings,
                runner,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::set_token,
            commands::test_connection,
            commands::list_sessions,
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
            commands::list_models,
            commands::download_model,
            commands::cancel_model_download,
            commands::delete_model,
            commands::compare_local,
            commands::cancel_compare,
            commands::count_images,
            commands::scanner_support,
            commands::list_scanners,
            commands::list_remote_sessions,
            commands::rename_remote_session,
            commands::delete_remote_session,
            commands::get_remote_session,
            commands::list_remote_items,
        ])
        .build(tauri::generate_context!())
        .expect("error while building cardloader")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                app.state::<AppState>().runner.stop_all();
            }
        });
}
