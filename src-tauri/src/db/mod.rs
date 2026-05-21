pub mod analysis_repo;
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

    let pool = connect_db(&db_path).await?;

    match initialize_schema(&pool).await {
        Ok(()) => {}
        Err(error) if is_incompatible_schema_error(&error) => {
            pool.close().await;
            quarantine_incompatible_database(&db_path)?;
            let fresh_pool = connect_db(&db_path).await?;
            initialize_schema(&fresh_pool).await?;
            record_schema_reset_notice(&fresh_pool, &error.to_string()).await?;
            return finish_db_startup(fresh_pool).await;
        }
        Err(error) => return Err(error),
    }

    finish_db_startup(pool).await
}

async fn connect_db(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(30));

    Ok(sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(10) // Allow more concurrent reads/writes
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(60))
        .connect_with(options)
        .await?)
}

async fn initialize_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await?;

    sqlx::migrate!("./migrations").run(pool).await.map_err(|error| {
        anyhow::anyhow!(
            "database schema is incompatible with this production baseline. Use Settings > Factory Reset to start with a fresh encrypted vault. Migration error: {error}"
        )
    })?;

    validate_required_schema(pool).await?;
    Ok(())
}

async fn finish_db_startup(pool: SqlitePool) -> anyhow::Result<SqlitePool> {
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

fn is_incompatible_schema_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("previously applied but has been modified")
        || message.contains("migration") && message.contains("not found")
        || message.contains("database schema is missing required table")
}

fn quarantine_incompatible_database(db_path: &std::path::Path) -> anyhow::Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    for suffix in ["", "-wal", "-shm"] {
        let path = std::path::PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if path.exists() {
            let quarantined = std::path::PathBuf::from(format!(
                "{}.incompatible-{}{}",
                db_path.display(),
                timestamp,
                suffix
            ));

            // Windows Hardening: Retry renaming a few times in case of lingering locks
            let mut last_err = None;
            for attempt in 0..5 {
                match fs::rename(&path, &quarantined) {
                    Ok(_) => {
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        std::thread::sleep(std::time::Duration::from_millis(100 * (attempt + 1)));
                    }
                }
            }

            if let Some(e) = last_err {
                log::error!(
                    "Failed to quarantine incompatible database file {:?}: {}",
                    path,
                    e
                );
                // On Windows, if we can't rename, we might have to copy and delete (best effort)
                if let Err(e2) = std::fs::copy(&path, &quarantined) {
                    log::error!("Copy fallback also failed: {}", e2);
                    return Err(e.into());
                }
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

async fn record_schema_reset_notice(pool: &SqlitePool, reason: &str) -> anyhow::Result<()> {
    let value = serde_json::json!({
        "reset_at": chrono::Utc::now().to_rfc3339(),
        "reason": reason,
        "message": "An incompatible pre-production database was moved aside and a fresh encrypted production vault was created."
    });
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('schema_reset_notice', ?, CURRENT_TIMESTAMP)",
    )
    .bind(value.to_string())
    .execute(pool)
    .await?;
    Ok(())
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
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?",
        )
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

    #[test]
    fn detects_modified_baseline_as_incompatible_schema() {
        let error = anyhow::anyhow!(
            "database schema is incompatible with this production baseline. Migration error: migration 20260511000000 was previously applied but has been modified"
        );
        assert!(super::is_incompatible_schema_error(&error));
    }
}
