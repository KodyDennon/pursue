mod db;
mod models;
mod scraper;

use sqlx::SqlitePool;
use tauri::{Manager, State};

struct AppState {
    db: SqlitePool,
}

#[tauri::command]
async fn get_records(state: State<'_, AppState>) -> Result<Vec<models::Record>, String> {
    sqlx::query_as::<_, models::Record>("SELECT * FROM records ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sync_records(state: State<'_, AppState>) -> Result<usize, String> {
    scraper::sync_official_records(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let pool = db::init_db(&handle).await.expect("Failed to init database");
                
                // Initial sync in background
                let pool_clone = pool.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = scraper::sync_official_records(&pool_clone).await;
                });

                app.manage(AppState { db: pool });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_records, sync_records])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
