pub mod records;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::fs;
use tauri::{AppHandle, Manager};

pub async fn init_db(app_handle: &AppHandle) -> anyhow::Result<SqlitePool> {
    let app_dir = app_handle.path().app_data_dir()?;
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)?;
    }
    let db_path = app_dir.join("pursue.db");

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
