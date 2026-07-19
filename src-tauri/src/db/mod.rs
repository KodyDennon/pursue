pub mod analysis_repo;
pub mod records;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::fs;
use tauri::AppHandle;

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

    let app_dir = crate::storage::resolve_storage_root(app_handle)?;
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)?;
    }
    let db_path = app_dir.join("pursue.db");

    let pool = connect_db(&db_path).await?;

    let backup = create_versioned_backup(&pool, &db_path).await?;
    initialize_schema(&pool).await.map_err(|error| {
        let recovery = backup
            .as_ref()
            .map(|path| format!(" A verified pre-migration backup is at {}.", path.display()))
            .unwrap_or_default();
        anyhow::anyhow!(
            "database migration was stopped without replacing the existing vault: {error}.{recovery}"
        )
    })?;
    mark_schema_version_backed_up(&db_path)?;

    finish_db_startup(pool).await
}

async fn connect_db(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
        .foreign_keys(true)
        // WAL allows only one writer at a time, and the sync transaction can hold the write
        // lock for a while; give concurrent download writes room to wait it out.
        .busy_timeout(std::time::Duration::from_secs(60));

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
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(pool)
        .await?;
    if integrity != "ok" {
        return Err(anyhow::anyhow!(
            "SQLite integrity verification failed after migration: {integrity}"
        ));
    }
    Ok(())
}

async fn finish_db_startup(pool: SqlitePool) -> anyhow::Result<SqlitePool> {
    // Preserve interrupted download jobs for browser-side resume instead of marking them failed.
    let _ = sqlx::query("UPDATE download_jobs SET status = 'running', summary_json = '{\"resume_available\": true, \"reason\": \"Application interrupted\"}' WHERE status IN ('running', 'queued')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("UPDATE download_job_items SET status = 'queued', error = 'Application interrupted; ready to resume', error_class = 'interrupted' WHERE status IN ('downloading', 'queued')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("UPDATE records SET analysis_status = 'pending', analysis_error = 'Previous analysis was interrupted; ready to retry' WHERE analysis_status IN ('indexing', 'extracting-foundation', 'synthesizing')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("UPDATE records SET analysis_status = 'pending', analysis_error = 'Neural OCR sidecar failed previously; ready to retry after runtime health check' WHERE analysis_status = 'failed' AND analysis_error LIKE '%127.0.0.1:8374/ocr%'")
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

async fn create_versioned_backup(
    pool: &SqlitePool,
    db_path: &std::path::Path,
) -> anyhow::Result<Option<std::path::PathBuf>> {
    if !db_path.is_file() || fs::metadata(db_path)?.len() == 0 {
        return Ok(None);
    }
    let marker = db_path.with_extension("schema-backup-version");
    if fs::read_to_string(&marker)
        .ok()
        .is_some_and(|value| value.trim() == env!("CARGO_PKG_VERSION"))
    {
        return Ok(None);
    }

    let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(pool)
        .await?;
    if integrity != "ok" {
        return Err(anyhow::anyhow!(
            "existing database failed integrity verification before migration: {integrity}"
        ));
    }
    let checkpoint: (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(pool)
        .await?;
    if checkpoint.0 != 0 {
        return Err(anyhow::anyhow!(
            "database was busy and could not be checkpointed before migration"
        ));
    }

    let backup_dir = db_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("database path has no parent"))?
        .join("database-backups");
    fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(format!(
        "pursue-before-v{}-{}.db",
        env!("CARGO_PKG_VERSION"),
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    fs::copy(db_path, &backup_path)?;
    let backup_file = fs::File::open(&backup_path)?;
    backup_file.sync_all()?;
    verify_backup_database(&backup_path).await?;

    let mut backups = fs::read_dir(&backup_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "db"))
        .collect::<Vec<_>>();
    backups.sort_by_key(|entry| entry.file_name());
    let remove_count = backups.len().saturating_sub(3);
    for stale in backups.into_iter().take(remove_count) {
        if let Err(error) = fs::remove_file(stale.path()) {
            log::warn!("Could not prune old database backup: {error}");
        }
    }
    Ok(Some(backup_path))
}

async fn verify_backup_database(path: &std::path::Path) -> anyhow::Result<()> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true);
    let backup = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&backup)
        .await?;
    backup.close().await;
    if integrity != "ok" {
        return Err(anyhow::anyhow!(
            "pre-migration database backup failed integrity verification: {integrity}"
        ));
    }
    Ok(())
}

fn mark_schema_version_backed_up(db_path: &std::path::Path) -> anyhow::Result<()> {
    crate::storage::write_file_atomically(
        &db_path.with_extension("schema-backup-version"),
        env!("CARGO_PKG_VERSION").as_bytes(),
    )
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
pub async fn test_pool() -> anyhow::Result<SqlitePool> {
    use std::sync::Once;
    static REGISTER_VEC_EXTENSION: Once = Once::new();
    REGISTER_VEC_EXTENSION.call_once(|| unsafe {
        sqlite3_auto_extension(Some(
            std::mem::transmute::<*const (), unsafe extern "C" fn()>(
                sqlite_vec::sqlite3_vec_init as *const (),
            ),
        ));
    });

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
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
    fn database_init_never_automatically_replaces_an_incompatible_vault() {
        let source = include_str!("mod.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(!production_source.contains("quarantine_incompatible_database"));
        assert!(!production_source.contains("record_schema_reset_notice"));
        assert!(production_source.contains("create_versioned_backup"));
    }
}
