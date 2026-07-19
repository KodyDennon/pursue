use crate::commands::{to_error, AppState};
use crate::storage;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

const MIGRATION_ERROR_FILE: &str = "storage-migration-error.json";

#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageLocationInfo {
    pub default_root: String,
    pub configured_root: Option<String>,
    pub effective_root: String,
    pub is_custom: bool,
    /// Error from the most recent migration attempt, if it failed. Cleared
    /// once reported.
    pub last_migration_error: Option<String>,
}

#[tauri::command]
pub async fn get_storage_location(
    state: State<'_, AppState>,
    handle: AppHandle,
) -> Result<StorageLocationInfo, String> {
    let default_root = storage::default_storage_root(&handle).map_err(to_error)?;
    let configured = storage::configured_storage_root(&handle);
    let effective = state.library.app_data_dir().to_path_buf();
    Ok(StorageLocationInfo {
        default_root: default_root.to_string_lossy().into_owned(),
        configured_root: configured.map(|p| p.to_string_lossy().into_owned()),
        effective_root: effective.to_string_lossy().into_owned(),
        is_custom: effective != default_root,
        last_migration_error: take_migration_error(&handle),
    })
}

/// Switches the storage root to `new_root`, optionally copying all existing
/// data there first, then restarts the app so every subsystem re-resolves its
/// paths. Existing files are left at the old location; the UI tells the user
/// they can delete them after verifying the move.
#[tauri::command]
#[allow(unreachable_code)]
pub async fn set_storage_location(
    new_root: String,
    migrate: bool,
    state: State<'_, AppState>,
    handle: AppHandle,
) -> Result<(), String> {
    let new_root = PathBuf::from(new_root.trim());
    let current_root = state.library.app_data_dir().to_path_buf();
    storage::validate_new_root(&new_root, &current_root).map_err(to_error)?;

    if migrate {
        // Quiesce SQLite before copying so the db/-wal/-shm files are
        // consistent. From here on the app must restart regardless of the
        // outcome, because the pool cannot be reopened in place.
        let checkpoint: (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
            .fetch_one(&state.db)
            .await
            .map_err(|error| {
                format!("could not checkpoint the database before storage migration: {error}")
            })?;
        if checkpoint.0 != 0 {
            return Err(format!(
                "database remained busy before storage migration (checkpoint status {})",
                checkpoint.0
            ));
        }
        state.db.close().await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let progress_handle = handle.clone();
        let from = current_root.clone();
        let to = new_root.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut last_emit = std::time::Instant::now();
            storage::migrate_storage(&from, &to, |copied, total| {
                if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
                    last_emit = std::time::Instant::now();
                    let _ = progress_handle.emit(
                        "storage-migration-progress",
                        serde_json::json!({
                            "status": "copying",
                            "bytes_copied": copied,
                            "bytes_total": total,
                        }),
                    );
                }
            })
        })
        .await
        .map_err(to_error)
        .and_then(|inner| inner.map_err(to_error));

        match result {
            Ok(stats) => {
                if let Err(error) = verify_migrated_database(&new_root).await {
                    restart_after_migration_error(&handle, &error).await;
                    return Ok(());
                }
                let _ = handle.emit(
                    "storage-migration-progress",
                    serde_json::json!({
                        "status": "finalizing",
                        "bytes_copied": stats.bytes_copied,
                        "bytes_total": stats.bytes_total,
                    }),
                );
            }
            Err(error) => {
                restart_after_migration_error(&handle, &error).await;
                return Ok(());
            }
        }
    }

    if let Err(error) = storage::write_pointer(&handle, &new_root) {
        store_migration_error(&handle, &error.to_string());
    }

    let _ = handle.emit(
        "storage-migration-progress",
        serde_json::json!({ "status": "restarting" }),
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    handle.restart();
    Ok(())
}

async fn restart_after_migration_error(handle: &AppHandle, error: &str) {
    store_migration_error(handle, error);
    let _ = handle.emit(
        "storage-migration-progress",
        serde_json::json!({ "status": "error", "message": error }),
    );
    // The pointer was not changed, so relaunching reopens the fully intact old
    // location. A restart is mandatory because the old SQLite pool was closed
    // before the copy began.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    handle.restart();
}

async fn verify_migrated_database(root: &std::path::Path) -> Result<(), String> {
    use sqlx::sqlite::SqliteConnectOptions;

    let path = root.join("pursue.db");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(false)
        .read_only(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("copied database could not be opened: {error}"))?;
    let result: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("copied database integrity check failed: {error}"))?;
    pool.close().await;
    if result != "ok" {
        return Err(format!(
            "copied database failed SQLite integrity verification: {result}"
        ));
    }
    Ok(())
}

fn migration_error_path(handle: &AppHandle) -> Option<PathBuf> {
    handle
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(MIGRATION_ERROR_FILE))
}

fn store_migration_error(handle: &AppHandle, message: &str) {
    if let Some(path) = migration_error_path(handle) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = storage::write_file_atomically(&path, message.as_bytes());
    }
}

fn take_migration_error(handle: &AppHandle) -> Option<String> {
    let path = migration_error_path(handle)?;
    let message = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    Some(message)
}
