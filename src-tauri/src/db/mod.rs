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
        sqlite3_auto_extension(Some(
            std::mem::transmute::<*const (), unsafe extern "C" fn()>(
                sqlite_vec::sqlite3_vec_init as *const (),
            ),
        ));
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

    sqlx::migrate!("./migrations").run(&pool).await.map_err(|error| {
        anyhow::anyhow!(
            "database schema is incompatible with this production baseline. Use Settings > Factory Reset to start with a fresh encrypted vault. Migration error: {error}"
        )
    })?;

    validate_required_schema(&pool).await?;

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

async fn validate_required_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    let required_tables = [
        "records",
        "artifacts",
        "record_assets",
        "download_jobs",
        "analysis_results",
        "analysis_chunks",
        "app_settings",
    ];

    for table in required_tables {
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?")
                .bind(table)
                .fetch_one(pool)
                .await?;
        if exists == 0 {
            return Err(anyhow::anyhow!(
                "database schema is missing required table `{table}`. Use Settings > Factory Reset to start with a fresh encrypted vault."
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    const BASELINE_SCHEMA: &str = include_str!("../../migrations/20260511000000_v1_baseline.sql");

    #[test]
    fn baseline_schema_contains_every_table_used_by_code() {
        for table in [
            "records",
            "artifacts",
            "record_assets",
            "download_jobs",
            "analysis_results",
            "analysis_chunks",
            "app_settings",
        ] {
            assert!(
                BASELINE_SCHEMA.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "baseline schema is missing table {table}"
            );
        }
    }

    #[test]
    fn database_init_does_not_rewrite_sqlx_migration_history() {
        let source = include_str!("mod.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            !production_source.contains("DELETE FROM _sqlx_migrations"),
            "database init must not delete SQLx migration history"
        );
    }
}
