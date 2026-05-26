mod analysis;
mod cases;
mod commands;
mod common;
mod db;
mod downloads;
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
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::Manager;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub library: Arc<LibraryManager>,
    pub analysis: Arc<AnalysisManager>,
    pub webview_semaphore: Arc<tokio::sync::Semaphore>,
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

            // Create a hidden WAR.gov-origin webview for same-origin source access.
            let _ = tauri::WebviewWindowBuilder::new(
                &handle,
                "war-gov-resolver",
                tauri::WebviewUrl::External("https://www.war.gov/UFO/".parse().unwrap()),
            )
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .visible(false)
            .build();

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
                    webview_semaphore: Arc::new(tokio::sync::Semaphore::new(4)), // Avoid deadlock with concurrency
                });
                anyhow::Ok(())
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sync_official_source_with_csv,
            repair_official_source_records,
            list_records,
            list_records_page,
            get_record,
            get_database_status,
            get_record_artifact_path,
            update_download_item_status,
            download_missing_records,
            queue_record_download,
            get_bulk_download_status,
            get_download_job_window,
            get_next_download_items,
            begin_download_item,
            append_download_chunk,
            finalize_download_item,
            download_war_gov_item_with_webview,
            fail_download_item,
            reset_download_item_part,
            cancel_bulk_download,
            resolve_dvids_metadata,
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
            get_disk_space_info,
            provision_model,
            check_model_status,
            get_system_stats,
            analyze_all_records,
            index_all_records,
            get_record_chunks,
            get_model_registry,
            verify_vault_integrity,
            get_vault_encryption_status,
            clear_evidence_cache,
            get_latest_download_job,
            get_app_settings,
            set_app_settings,
            cleanup_duplicates,
            cleanup_poisoned_artifacts,
            factory_reset,
            get_forensic_report,
            get_intelligence_logs,
            index_record,
            synthesize_intelligence,
            synthesize_all_records,
            reprocess_all_records,
            abort_analysis,
            check_neural_runtime_status,
            provision_neural_runtime,
            get_log_path,
            open_logs_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
