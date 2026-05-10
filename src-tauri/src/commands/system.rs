use crate::analysis::diagnostics;
use crate::analysis::model_manager::ModelManager;
use crate::commands::{AppState, to_error, database_status};
use crate::models::{DatabaseStatus, BulkDownloadReport, BulkDownloadStatus, BulkDownloadItem};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::info;
use sqlx::{Row};

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
pub async fn check_model_status(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, bool>, String> {
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
                .map(|mut d| d.any(|e| {
                    e.map(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("safetensors"))
                        .unwrap_or(false)
                }))
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
    url: String,
    name: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let manager = ModelManager::new(&state.library)
        .with_db(state.db.clone());
    manager
        .ensure_model(&app_handle, &id, &name, &url)
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(to_error)
}

#[tauri::command]
pub async fn verify_vault_integrity(state: State<'_, AppState>, app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let pool = state.db.clone();
    let library = state.library.clone();
    
    let records = sqlx::query("SELECT id, local_path, artifact_sha256 FROM records WHERE local_path IS NOT NULL")
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
        
        let _ = app_handle.emit("integrity-progress", serde_json::json!({
            "current": i,
            "total": total,
            "record_id": id
        }));

        let full_path = library.get_full_path(&local_path);
        if !full_path.exists() {
            missing += 1;
            continue;
        }

        if let Some(expected) = expected_hash {
            if let Ok(bytes) = tokio::fs::read(&full_path).await {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let hash = hex::encode(hasher.finalize());
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

    let _ = app_handle.emit("integrity-progress", serde_json::json!({
        "current": total,
        "total": total,
        "status": "completed"
    }));

    Ok(serde_json::json!({
        "total": total,
        "verified": verified,
        "corrupted": corrupted,
        "missing": missing
    }))
}

#[tauri::command]
pub async fn get_latest_download_job(state: State<'_, AppState>) -> Result<Option<BulkDownloadReport>, String> {
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
pub async fn get_app_settings(key: String, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
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
pub async fn set_app_settings(key: String, value: serde_json::Value, state: State<'_, AppState>) -> Result<(), String> {
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
        SELECT document_url, COUNT(*) as c
        FROM records
        WHERE document_url IS NOT NULL AND source_type = 'official'
        GROUP BY document_url
        HAVING c > 1
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(to_error)?;

    let mut removed = 0;
    for dup in duplicates {
        use sqlx::Row;
        let url: String = dup.get("document_url");
        
        let mut group = sqlx::query("SELECT id, analysis_status, stable_key FROM records WHERE document_url = ?")
            .bind(&url)
            .fetch_all(&pool)
            .await
            .map_err(to_error)?;
            
        group.sort_by(|a, b| {
            let a_status: Option<String> = a.get("analysis_status");
            let b_status: Option<String> = b.get("analysis_status");
            let a_key: String = a.get("stable_key");
            let b_key: String = b.get("stable_key");
            
            let a_score = if a_status.as_deref() == Some("completed") { 10 } else { 0 } + if a_key.contains("|title:") { 1 } else { 0 };
            let b_score = if b_status.as_deref() == Some("completed") { 10 } else { 0 } + if b_key.contains("|title:") { 1 } else { 0 };
            
            b_score.cmp(&a_score)
        });
        
        for record in group.iter().skip(1) {
            let id: String = record.get("id");
            sqlx::query("DELETE FROM records WHERE id = ?").bind(&id).execute(&pool).await.map_err(to_error)?;
            removed += 1;
        }
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
        "#
    )
    .fetch_one(&pool)
    .await
    .map_err(to_error)?;

    use sqlx::Row;
    let total_count: i64 = row.get("total_count");
    let local_count: i64 = row.get("local_count");
    let total_size: i64 = row.get("total_size");

    let pending_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM records WHERE analysis_status IS NULL OR analysis_status = 'pending'"
    )
    .fetch_one(&pool)
    .await
    .map_err(to_error)?;

    let indexed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM records WHERE analysis_status = 'indexed'"
    )
    .fetch_one(&pool)
    .await
    .map_err(to_error)?;

    let completed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM records WHERE analysis_status = 'completed'"
    )
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
        "unanalyzed_count": pending_count + indexed_count
    }))
}
