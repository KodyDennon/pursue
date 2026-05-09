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

use crate::commands::*;
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
            let library = Arc::new(
                LibraryManager::new(&handle).expect("failed to initialize library manager"),
            );

            tauri::async_runtime::block_on(async move {
                let pool = db::init_db(&handle)
                    .await
                    .expect("failed to initialize database");
                library
                    .init()
                    .await
                    .expect("failed to initialize evidence library");

                // Initialize search engine with correct models path
                crate::search::init_search_engine(library.app_data_dir().join("models"));

                app.manage(AppState { db: pool, library });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sync_official_source,
            sync_official_source_with_csv,
            list_records,
            get_record,
            get_database_status,
            download_record,
            get_record_artifact_path,
            download_record_with_bytes,
            download_missing_records,
            get_bulk_download_status,
            cancel_bulk_download,
            import_manual_file,
            ingest_web_page,
            analyze_record,
            get_analysis_result,
            search,
            list_cases,
            create_case,
            update_case_notes,
            add_record_to_case,
            export_case,
            get_hardware_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
