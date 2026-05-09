mod analysis;
mod cases;
mod commands;
mod db;
mod exports;
mod library;
mod models;
mod search;
mod sources;

use std::sync::Arc;

use commands::AppState;
use library::LibraryManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let library =
                Arc::new(LibraryManager::new(&handle).expect("failed to initialize library manager"));

            tauri::async_runtime::block_on(async move {
                let pool = db::init_db(&handle).await.expect("failed to initialize database");
                library.init().await.expect("failed to initialize evidence library");
                app.manage(AppState { db: pool, library });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sync_official_source,
            commands::list_records,
            commands::download_record,
            commands::download_missing_records,
            commands::get_bulk_download_status,
            commands::cancel_bulk_download,
            commands::import_manual_file,
            commands::analyze_record,
            commands::get_analysis_result,
            commands::search,
            commands::list_cases,
            commands::create_case,
            commands::update_case_notes,
            commands::add_record_to_case,
            commands::export_case
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
