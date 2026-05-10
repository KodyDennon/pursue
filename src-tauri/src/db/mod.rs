pub mod records;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::fs;
use tauri::{AppHandle, Manager};

// Direct FFI to avoid libsqlite3-sys version conflicts with sqlx
extern "C" {
    fn sqlite3_auto_extension(xEntryPoint: Option<unsafe extern "C" fn()>) -> std::os::raw::c_int;
}

pub async fn init_db(app_handle: &AppHandle) -> anyhow::Result<SqlitePool> {
    // Register sqlite-vec extension globally before any connections are opened
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    let app_dir = app_handle.path().app_data_dir()?;
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)?;
    }
    let db_path = app_dir.join("pursue.db");

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(30));

    let pool = SqlitePool::connect_with(options).await?;

    // --- MIGRATION RECONCILIATION LAYER ---
    // When squashing migrations into a baseline (v1.0), SQLx panics if previously applied 
    // migrations are missing from the folder. We reconcile this by removing archived 
    // references if the baseline migration is present.
    let m_path = "./migrations";
    let has_baseline = fs::read_dir(m_path).map(|d| {
        d.filter_map(|e| e.ok()).any(|e| e.file_name().to_str().unwrap_or("").contains("v1_baseline"))
    }).unwrap_or(false);

    if has_baseline {
        let _ = sqlx::query("CREATE TABLE IF NOT EXISTS _sqlx_migrations (version BIGINT PRIMARY KEY, success BOOLEAN NOT NULL)")
            .execute(&pool).await;
        // Remove versions that are NOT the baseline and are before it
        let _ = sqlx::query("DELETE FROM _sqlx_migrations WHERE version < 20260511000000")
            .execute(&pool).await;
    }
    // --------------------------------------

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    
    // Cleanup stalled jobs from previous sessions
    let _ = sqlx::query("UPDATE download_jobs SET status = 'failed', summary_json = '{\"error\": \"Application interrupted\"}' WHERE status IN ('running', 'queued')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("UPDATE download_job_items SET status = 'failed', error = 'Application interrupted' WHERE status IN ('downloading', 'queued')")
        .execute(&pool)
        .await;

    // Automatic Maintenance: WAL Checkpointing
    // Prevents the -wal file from growing indefinitely by truncating it periodically
    let pool_clone = pool.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600)); // Every 10 mins
        loop {
            interval.tick().await;
            let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&pool_clone)
                .await;
        }
    });

    Ok(pool)
}
