use crate::analysis::diagnostics;
use crate::analysis::model_manager::ModelManager;
use crate::commands::{database_status, to_error, AppState};
use crate::models::{BulkDownloadItem, BulkDownloadReport, BulkDownloadStatus, DatabaseStatus};
use sqlx::Row;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::info;

#[tauri::command]
pub async fn get_database_status(state: State<'_, AppState>) -> Result<DatabaseStatus, String> {
    database_status(&state.db, &state.library)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn get_hardware_diagnostics() -> Result<diagnostics::HardwareSpecs, String> {
    Ok(diagnostics::get_hardware_specs())
}

#[tauri::command]
pub async fn get_system_stats() -> Result<diagnostics::SystemStats, String> {
    Ok(diagnostics::get_system_stats())
}

#[tauri::command]
pub async fn check_model_status(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, bool>, String> {
    let manager = ModelManager::new(&state.library);
    let mut status = std::collections::HashMap::new();
    let registry = crate::analysis::registry::get_model_registry();

    for model in registry {
        let is_ready = if let Some(filename) = &model.filename {
            manager.models_dir().join(filename).exists()
        } else {
            let repo_dir = manager.models_dir().join(&model.id);
            let has_config = repo_dir.join("config.json").exists();
            let has_weights = std::fs::read_dir(&repo_dir)
                .map(|mut d| {
                    d.any(|e| {
                        e.map(|entry| {
                            entry.path().extension().and_then(|s| s.to_str()) == Some("safetensors")
                        })
                        .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            has_config && has_weights
        };
        status.insert(model.id, is_ready);
    }

    Ok(status)
}

#[tauri::command]
pub async fn provision_model(
    id: String,
    url: Option<String>,
    name: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let manager = ModelManager::new(&state.library).with_db(state.db.clone());
    let registry = crate::analysis::registry::get_model_registry();
    let definition = registry
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| format!("unknown model id: {id}"))?;

    let (model_name, source_url) = match definition.filename.as_deref() {
        Some("bge-small-en-v1.5.onnx") => (
            "bge-small-en-v1.5.onnx".to_string(),
            format!(
                "https://huggingface.co/{}/resolve/main/onnx/model.onnx",
                definition.repo_id
            ),
        ),
        Some(filename) => (
            filename.to_string(),
            format!(
                "https://huggingface.co/{}/resolve/main/{}",
                definition.repo_id, filename
            ),
        ),
        None => (definition.id.clone(), definition.repo_id.clone()),
    };

    manager
        .ensure_model(
            &app_handle,
            &id,
            name.as_deref().unwrap_or(&model_name),
            url.as_deref().unwrap_or(&source_url),
        )
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(to_error)
}

#[tauri::command]
pub async fn verify_vault_integrity(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let pool = state.db.clone();
    let library = state.library.clone();

    let records = sqlx::query("SELECT r.id, r.local_path, a.sha256 AS artifact_sha256 FROM records r LEFT JOIN artifacts a ON a.record_id = r.id WHERE r.local_path IS NOT NULL")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

    let total = records.len();
    let mut verified = 0;
    let mut corrupted = 0;
    let mut missing = 0;

    use tauri::Emitter;

    for (i, row) in records.into_iter().enumerate() {
        use sqlx::Row;
        let id: String = row.get("id");
        let local_path: String = row.get("local_path");
        let expected_hash: Option<String> = row.get("artifact_sha256");

        let _ = app_handle.emit(
            "integrity-progress",
            serde_json::json!({
                "current": i,
                "total": total,
                "record_id": id
            }),
        );

        let full_path = library.get_full_path(&local_path);
        if !full_path.exists() {
            missing += 1;
            continue;
        }

        if let Some(expected) = expected_hash {
            if let Ok(hash) = library.artifact_plaintext_sha256(&local_path).await {
                if hash != expected {
                    corrupted += 1;
                } else {
                    verified += 1;
                }
            } else {
                corrupted += 1;
            }
        } else {
            verified += 1;
        }

        tokio::task::yield_now().await;
    }

    let _ = app_handle.emit(
        "integrity-progress",
        serde_json::json!({
            "current": total,
            "total": total,
            "status": "completed"
        }),
    );

    Ok(serde_json::json!({
        "total": total,
        "verified": verified,
        "corrupted": corrupted,
        "missing": missing
    }))
}

#[tauri::command]
pub async fn get_vault_encryption_status(
    state: State<'_, AppState>,
) -> Result<crate::vault::VaultEncryptionStatus, String> {
    Ok(state.library.encryption_status())
}

#[tauri::command]
pub async fn clear_evidence_cache(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let pool = state.db.clone();
    let library = state.library.clone();

    let rows = sqlx::query("SELECT relative_path FROM artifacts")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

    let mut files_removed = 0_i64;
    let mut bytes_removed = 0_i64;
    for row in rows {
        let relative_path: String = row.get("relative_path");
        let full_path = library.get_full_path(&relative_path);
        if let Ok(metadata) = tokio::fs::metadata(&full_path).await {
            bytes_removed += i64::try_from(metadata.len()).unwrap_or(0);
        }
        if tokio::fs::remove_file(&full_path).await.is_ok() {
            files_removed += 1;
        }
    }

    let cache_path = library.app_data_dir().join("decrypted-cache");
    if cache_path.exists() {
        let _ = tokio::fs::remove_dir_all(&cache_path).await;
    }
    let _ = tokio::fs::create_dir_all(&cache_path).await;

    sqlx::query("DELETE FROM artifacts")
        .execute(&pool)
        .await
        .map_err(to_error)?;
    sqlx::query("DELETE FROM record_assets")
        .execute(&pool)
        .await
        .map_err(to_error)?;
    sqlx::query("UPDATE records SET local_path = NULL, thumbnail_path = NULL, updated_at = CURRENT_TIMESTAMP")
        .execute(&pool)
        .await
        .map_err(to_error)?;

    Ok(serde_json::json!({
        "files_removed": files_removed,
        "bytes_removed": bytes_removed
    }))
}

#[tauri::command]
pub async fn get_latest_download_job(
    state: State<'_, AppState>,
) -> Result<Option<BulkDownloadReport>, String> {
    let job = sqlx::query_as::<_, BulkDownloadStatus>(
        "SELECT * FROM download_jobs WHERE status IN ('queued', 'running') ORDER BY updated_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(to_error)?;

    match job {
        Some(job) => {
            let items = sqlx::query_as::<_, BulkDownloadItem>(
                "SELECT * FROM download_job_items WHERE job_id = ? ORDER BY updated_at DESC LIMIT 50",
            )
            .bind(&job.id)
            .fetch_all(&state.db)
            .await
            .map_err(to_error)?;

            Ok(Some(BulkDownloadReport { job, items }))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn get_app_settings(
    key: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let row = sqlx::query("SELECT value_json FROM app_settings WHERE key = ?")
        .bind(&key)
        .fetch_optional(&state.db)
        .await
        .map_err(to_error)?;

    match row {
        Some(row) => {
            let val: String = row.get("value_json");
            serde_json::from_str(&val).map_err(to_error)
        }
        None => Ok(serde_json::Value::Null),
    }
}

#[tauri::command]
pub async fn set_app_settings(
    key: String,
    value: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let val_str = serde_json::to_string(&value).map_err(to_error)?;
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&key)
    .bind(&val_str)
    .execute(&state.db)
    .await
    .map_err(to_error)?;
    Ok(())
}

#[tauri::command]
pub async fn cleanup_duplicates(state: State<'_, AppState>) -> Result<usize, String> {
    let pool = state.db.clone();

    let duplicates = sqlx::query(
        r#"
        SELECT title, document_url, COUNT(*) as c
        FROM records
        WHERE document_url IS NOT NULL AND source_type = 'official'
        GROUP BY title, document_url
        HAVING c > 1
        "#,
    )
    .fetch_all(&pool)
    .await
    .map_err(to_error)?;

    let mut removed = 0;
    for dup in duplicates {
        use sqlx::Row;
        let title: String = dup.get("title");
        let url: String = dup.get("document_url");

        let mut group = sqlx::query(
            "SELECT id, analysis_status, stable_key FROM records WHERE title = ? AND document_url = ?",
        )
        .bind(&title)
        .bind(&url)
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

        group.sort_by(|a, b| {
            let a_status: Option<String> = a.get("analysis_status");
            let b_status: Option<String> = b.get("analysis_status");
            let a_key: String = a.get("stable_key");
            let b_key: String = b.get("stable_key");

            let a_score = if a_status.as_deref() == Some("completed") {
                10
            } else {
                0
            } + if a_key.contains("|title:") { 1 } else { 0 };
            let b_score = if b_status.as_deref() == Some("completed") {
                10
            } else {
                0
            } + if b_key.contains("|title:") { 1 } else { 0 };

            b_score.cmp(&a_score)
        });

        for record in group.iter().skip(1) {
            let id: String = record.get("id");
            sqlx::query("DELETE FROM records WHERE id = ?")
                .bind(&id)
                .execute(&pool)
                .await
                .map_err(to_error)?;
            removed += 1;
        }
    }

    Ok(removed)
}

#[tauri::command]
pub async fn cleanup_poisoned_artifacts(state: State<'_, AppState>) -> Result<usize, String> {
    let pool = state.db.clone();
    let library = state.library.clone();

    // Identify artifacts < 1KB (likely 403 error pages)
    let poisoned = sqlx::query(
        "SELECT relative_path FROM artifacts WHERE byte_size < 1024 AND source_type = 'official'",
    )
    .fetch_all(&pool)
    .await
    .map_err(to_error)?;

    let mut removed = 0;
    for row in poisoned {
        let path: String = row.get("relative_path");
        let full_path = library.get_full_path(&path);

        // Reset record
        sqlx::query("UPDATE records SET local_path = NULL, updated_at = CURRENT_TIMESTAMP WHERE local_path = ?")
            .bind(&path)
            .execute(&pool)
            .await
            .map_err(to_error)?;

        // Delete artifact record
        sqlx::query("DELETE FROM artifacts WHERE relative_path = ?")
            .bind(&path)
            .execute(&pool)
            .await
            .map_err(to_error)?;

        // Delete local file
        if full_path.exists() {
            let _ = tokio::fs::remove_file(&full_path).await;
        }

        removed += 1;
    }

    Ok(removed)
}

#[tauri::command]
#[allow(unreachable_code)]
pub async fn factory_reset(state: State<'_, AppState>, handle: AppHandle) -> Result<(), String> {
    info!("INITIATING FULL SYSTEM PURGE (Factory Reset)");
    state.db.close().await;
    let app_dir = handle.path().app_data_dir().map_err(to_error)?;
    if app_dir.exists() {
        std::fs::remove_dir_all(&app_dir).map_err(to_error)?;
    }
    info!("System purge complete. Triggering restart...");
    handle.restart();
    Ok(())
}

#[tauri::command]
pub async fn get_evidence_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let pool = state.db.clone();
    let row = sqlx::query(
        r#"
        SELECT 
            (SELECT COUNT(*) FROM records) as total_count,
            (SELECT COUNT(*) FROM records WHERE local_path IS NOT NULL) as local_count,
            (SELECT COALESCE(SUM(byte_size), 0) FROM artifacts) as total_size
        "#,
    )
    .fetch_one(&pool)
    .await
    .map_err(to_error)?;

    use sqlx::Row;
    let total_count: i64 = row.get("total_count");
    let local_count: i64 = row.get("local_count");
    let total_size: i64 = row.get("total_size");

    let pending_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM records WHERE analysis_status IS NULL OR analysis_status = 'pending'",
    )
    .fetch_one(&pool)
    .await
    .map_err(to_error)?;

    let indexed_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM records WHERE analysis_status = 'indexed'")
            .fetch_one(&pool)
            .await
            .map_err(to_error)?;

    let completed_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM records WHERE analysis_status = 'completed'")
            .fetch_one(&pool)
            .await
            .map_err(to_error)?;

    Ok(serde_json::json!({
        "total_count": total_count,
        "local_count": local_count,
        "total_size": total_size,
        "pending_count": pending_count,
        "indexed_count": indexed_count,
        "completed_count": completed_count,
        "unanalyzed_count": pending_count
    }))
}
