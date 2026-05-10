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

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    
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
