mod analysis;
mod cases;
mod commands;
mod common;
mod db;
mod exports;
mod library;
mod models;
mod search;
mod sources;
mod vault;

use std::sync::Arc;

use crate::commands::*;
use analysis::AnalysisManager;
use library::LibraryManager;
use tauri::Manager;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub library: Arc<LibraryManager>,
    pub analysis: Arc<AnalysisManager>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("pursue".into()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let pool = db::init_db(&handle).await?;

                let library = Arc::new(LibraryManager::new(&handle)?);
                let analysis = Arc::new(AnalysisManager::new(pool.clone(), library.clone()));

                library.init().await?;

                // Initialize search engine with correct models path
                crate::search::init_search_engine(library.app_data_dir().join("models"));

                handle.manage(AppState {
                    db: pool,
                    library,
                    analysis,
                });
                anyhow::Ok(())
            })?;
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
            get_hardware_diagnostics,
            provision_model,
            check_model_status,
            get_system_stats,
            analyze_all_records,
            index_all_records,
            get_record_chunks,
            get_model_registry,
            get_evidence_stats,
            verify_vault_integrity,
            get_vault_encryption_status,
            clear_evidence_cache,
            get_latest_download_job,
            get_app_settings,
            set_app_settings,
            cleanup_duplicates,
            factory_reset,
            get_forensic_report,
            get_intelligence_logs,
            index_record,
            synthesize_intelligence,
            reprocess_all_records
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
