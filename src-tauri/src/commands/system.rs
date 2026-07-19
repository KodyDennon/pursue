use crate::analysis::diagnostics;
use crate::analysis::model_manager::ModelManager;
use crate::commands::{database_status, to_error, AppState};
use crate::models::{BulkDownloadItem, BulkDownloadReport, BulkDownloadStatus, DatabaseStatus};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::info;

const HF_DEVICE_OAUTH_CLIENT_ID: &str = "26be6b09-91c5-47da-9861-d2d2bb7a7e36";
const HF_DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceDeviceAuthSession {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: String,
    interval: u64,
    expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct HuggingFaceAuthResult {
    username: String,
}

#[derive(Debug, Deserialize)]
struct HuggingFaceDeviceResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    interval: Option<u64>,
    expires_in: Option<u64>,
}

fn hugging_face_oauth_client_id() -> String {
    std::env::var("PURSUE_HF_OAUTH_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| HF_DEVICE_OAUTH_CLIENT_ID.to_string())
}

#[tauri::command]
pub async fn begin_hugging_face_device_auth() -> Result<HuggingFaceDeviceAuthSession, String> {
    let client_id = hugging_face_oauth_client_id();
    let response = reqwest::Client::new()
        .post("https://huggingface.co/oauth/device")
        .form(&[("client_id", client_id.as_str())])
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| format!("could not reach Hugging Face authentication: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Hugging Face could not start device authentication (HTTP {})",
            response.status()
        ));
    }
    let device = response
        .json::<HuggingFaceDeviceResponse>()
        .await
        .map_err(|error| {
            format!("Hugging Face returned an invalid authentication response: {error}")
        })?;
    let verification_uri_complete = device
        .verification_uri_complete
        .unwrap_or_else(|| device.verification_uri.clone());

    Ok(HuggingFaceDeviceAuthSession {
        device_code: device.device_code,
        user_code: device.user_code,
        verification_uri: device.verification_uri,
        verification_uri_complete,
        interval: device.interval.unwrap_or(5).clamp(1, 30),
        expires_in: device.expires_in.unwrap_or(900).clamp(60, 1_800),
    })
}

#[tauri::command]
pub async fn complete_hugging_face_device_auth(
    session: HuggingFaceDeviceAuthSession,
    state: State<'_, AppState>,
) -> Result<HuggingFaceAuthResult, String> {
    let client = reqwest::Client::new();
    let client_id = hugging_face_oauth_client_id();
    let deadline = Instant::now() + Duration::from_secs(session.expires_in.clamp(60, 1_800));
    let mut interval = Duration::from_secs(session.interval.clamp(1, 30));

    loop {
        if Instant::now() + interval > deadline {
            return Err(
                "Hugging Face sign-in expired. Start sign-in again to get a new code.".into(),
            );
        }
        tokio::time::sleep(interval).await;

        let response = client
            .post("https://huggingface.co/oauth/token")
            .form(&[
                ("grant_type", HF_DEVICE_GRANT_TYPE),
                ("device_code", session.device_code.as_str()),
                ("client_id", client_id.as_str()),
            ])
            .timeout(Duration::from_secs(20))
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(_) if Instant::now() < deadline => continue,
            Err(error) => return Err(format!("Hugging Face sign-in connection failed: {error}")),
        };
        let payload = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| format!("Hugging Face returned an invalid token response: {error}"))?;

        if let Some(access_token) = payload.get("access_token").and_then(|value| value.as_str()) {
            let whoami = client
                .get("https://huggingface.co/api/whoami-v2")
                .bearer_auth(access_token)
                .timeout(Duration::from_secs(20))
                .send()
                .await
                .map_err(|error| format!("Hugging Face account verification failed: {error}"))?;
            if !whoami.status().is_success() {
                return Err("Hugging Face issued a token that failed account verification.".into());
            }
            let identity = whoami
                .json::<serde_json::Value>()
                .await
                .map_err(|error| format!("Hugging Face returned an invalid profile: {error}"))?;
            let username = identity
                .get("name")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Hugging Face user")
                .to_string();

            let refresh_token = payload
                .get("refresh_token")
                .and_then(|value| value.as_str());
            crate::analysis::model_manager::store_hf_oauth_credentials(access_token, refresh_token)
                .map_err(to_error)?;
            sqlx::query("DELETE FROM app_settings WHERE key = 'huggingface_token'")
                .execute(&state.db)
                .await
                .map_err(to_error)?;
            save_json_setting(&state.db, "huggingface_username", &username).await?;
            return Ok(HuggingFaceAuthResult { username });
        }

        match payload.get("error").and_then(|value| value.as_str()) {
            Some("authorization_pending") | None if Instant::now() < deadline => continue,
            Some("slow_down") if Instant::now() < deadline => {
                interval = (interval + Duration::from_secs(5)).min(Duration::from_secs(30));
            }
            Some("access_denied") => return Err("Hugging Face sign-in was denied.".into()),
            Some("expired_token") => {
                return Err("Hugging Face sign-in expired. Start sign-in again.".into())
            }
            Some(error) => return Err(format!("Hugging Face sign-in failed: {error}")),
            None => return Err("Hugging Face sign-in returned no access token.".into()),
        }
    }
}

#[tauri::command]
pub async fn set_hugging_face_manual_token(
    token: String,
    state: State<'_, AppState>,
) -> Result<HuggingFaceAuthResult, String> {
    let token = token.trim();
    if token.len() < 20 {
        return Err("Enter a valid Hugging Face read token.".into());
    }
    let response = reqwest::Client::new()
        .get("https://huggingface.co/api/whoami-v2")
        .bearer_auth(token)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| format!("Hugging Face token verification failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Hugging Face rejected this token (HTTP {}). Use a read token with access to the accepted Gemma license.",
            response.status()
        ));
    }
    let identity = response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("Hugging Face returned an invalid profile: {error}"))?;
    let username = identity
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Hugging Face user")
        .to_string();

    crate::analysis::model_manager::store_hf_manual_token(token).map_err(to_error)?;
    sqlx::query("DELETE FROM app_settings WHERE key = 'huggingface_token'")
        .execute(&state.db)
        .await
        .map_err(to_error)?;
    save_json_setting(&state.db, "huggingface_username", &username).await?;
    Ok(HuggingFaceAuthResult { username })
}

async fn save_json_setting<T: Serialize + ?Sized>(
    pool: &sqlx::SqlitePool,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let value_json = serde_json::to_string(value).map_err(to_error)?;
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(key)
    .bind(value_json)
    .execute(pool)
    .await
    .map_err(to_error)?;
    Ok(())
}

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
            let path = manager.models_dir().join(filename);
            path.exists()
                && !crate::analysis::verifier::is_model_corrupted(&path, filename).await
                && model
                    .expected_bytes
                    .map(|expected| {
                        std::fs::metadata(&path)
                            .map(|metadata| metadata.len() == expected)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
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

    // Preserve and report the original full-precision E4B cache independently. It is
    // never deleted or treated as the Q4 download; high-memory accelerators can use it.
    let bf16_dir = manager.models_dir().join("gemma-4-e4b");
    let bf16_ready = bf16_dir.join("config.json").exists()
        && bf16_dir.join("tokenizer.json").exists()
        && std::fs::read_dir(&bf16_dir)
            .map(|entries| {
                entries.filter_map(Result::ok).any(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some("safetensors")
                })
            })
            .unwrap_or(false);
    status.insert("gemma-4-e4b-bf16".to_string(), bf16_ready);

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

    let model_name = definition
        .filename
        .clone()
        .unwrap_or_else(|| definition.id.clone());
    let source_url = definition
        .download_url()
        .unwrap_or_else(|| definition.repo_id.clone());

    manager
        .ensure_model(
            &app_handle,
            &id,
            name.as_deref().unwrap_or(&model_name),
            url.as_deref().unwrap_or(&source_url),
            definition.expected_bytes,
            definition.expected_sha256.as_deref(),
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

    let records = sqlx::query("SELECT r.id, r.local_path, a.sha256 AS artifact_sha256 FROM records r LEFT JOIN artifacts a ON a.relative_path = r.local_path WHERE r.local_path IS NOT NULL")
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
    if key == "huggingface_token" {
        return Ok(serde_json::Value::Null);
    }
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
    if key == "huggingface_token" {
        return Err(
            "Hugging Face credentials must be stored with set_hugging_face_manual_token".into(),
        );
    }
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

    // Runs after every sync, so wrap the whole pass in one transaction rather than leaving
    // each DELETE individually committed — a failure partway through previously left some
    // duplicate groups half-cleaned with no way to tell which.
    let mut tx = pool.begin().await.map_err(to_error)?;
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
        .fetch_all(&mut *tx)
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
                .execute(&mut *tx)
                .await
                .map_err(to_error)?;
            removed += 1;
        }
    }
    tx.commit().await.map_err(to_error)?;

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

    // Poisoned artifacts should be rare, so one transaction for the whole batch (rather than
    // per-row) is both simpler and sufficient — same rationale as cleanup_duplicates.
    let mut tx = pool.begin().await.map_err(to_error)?;
    let mut removed = 0;
    let mut files_to_delete = Vec::new();
    for row in poisoned {
        let path: String = row.get("relative_path");
        files_to_delete.push(library.get_full_path(&path));

        // Reset record
        sqlx::query("UPDATE records SET local_path = NULL, updated_at = CURRENT_TIMESTAMP WHERE local_path = ?")
            .bind(&path)
            .execute(&mut *tx)
            .await
            .map_err(to_error)?;

        // Delete artifact record
        sqlx::query("DELETE FROM artifacts WHERE relative_path = ?")
            .bind(&path)
            .execute(&mut *tx)
            .await
            .map_err(to_error)?;

        removed += 1;
    }
    tx.commit().await.map_err(to_error)?;

    // Delete the actual files only after the DB transaction commits, so a mid-batch DB
    // failure can't leave files removed with no matching DB change (rolled back) or vice versa.
    for full_path in files_to_delete {
        if full_path.exists() {
            let _ = tokio::fs::remove_file(&full_path).await;
        }
    }

    Ok(removed)
}

#[tauri::command]
#[allow(unreachable_code)]
pub async fn factory_reset(state: State<'_, AppState>, handle: AppHandle) -> Result<(), String> {
    info!("INITIATING FULL SYSTEM PURGE (Factory Reset)");

    // 1. Close database pool to release locks
    state.db.close().await;

    // 2. Allow a moment for file handles to close
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let app_dir = state.library.app_data_dir().to_path_buf();
    if app_dir.exists() {
        // Windows Hardening: Standard std::fs::remove_dir_all fails if files are locked (e.g. log file).
        // We attempt a recursive delete and log errors for locked files without failing the whole reset.
        if let Err(e) = remove_dir_all_robust(&app_dir) {
            log::warn!(
                "Partial failure during factory reset: {}. Some files may remain.",
                e
            );
        }
    }

    // A custom storage root may have been active; also purge the default
    // location (where migrated-from data may remain) and drop the pointer so
    // the fresh start happens at the platform default.
    if let Ok(default_root) = crate::storage::default_storage_root(&handle) {
        if default_root != app_dir && default_root.exists() {
            if let Err(e) = remove_dir_all_robust(&default_root) {
                log::warn!("Partial failure purging default storage root: {}", e);
            }
        }
    }
    if let Err(e) = crate::storage::clear_pointer(&handle) {
        log::warn!(
            "Failed to clear storage pointer during factory reset: {}",
            e
        );
    }

    info!("System purge complete. Triggering restart...");
    handle.restart();
    Ok(())
}

fn remove_dir_all_robust(path: &std::path::Path) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let _ = remove_dir_all_robust(&path);
            } else {
                // If it fails (e.g. log file lock), we just skip it on Windows
                let _ = std::fs::remove_file(&path);
            }
        }
        let _ = std::fs::remove_dir(path);
    }
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DiskSpaceInfo {
    pub available_bytes: u64,
    pub total_bytes: u64,
}

#[tauri::command]
pub async fn get_disk_space_info(state: State<'_, AppState>) -> Result<DiskSpaceInfo, String> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let app_dir = state.library.app_data_dir();

    let mut best_match = None;
    let mut best_match_len = 0;

    for disk in &disks {
        if app_dir.starts_with(disk.mount_point()) {
            let path_len = disk.mount_point().as_os_str().len();
            if path_len > best_match_len {
                best_match = Some(disk);
                best_match_len = path_len;
            }
        }
    }

    if let Some(disk) = best_match {
        Ok(DiskSpaceInfo {
            available_bytes: disk.available_space(),
            total_bytes: disk.total_space(),
        })
    } else {
        Err("Could not determine disk space for application directory".to_string())
    }
}

/// Exact signed-update lane for this binary. Windows CUDA and DirectML builds
/// intentionally use different targets so an update can never replace one
/// provider bundle with the other merely because both share the same OS/arch.
#[tauri::command]
pub fn get_update_target() -> &'static str {
    #[cfg(all(target_os = "windows", feature = "cuda"))]
    {
        return "windows-cuda-x86_64";
    }
    #[cfg(all(target_os = "windows", not(feature = "cuda"), feature = "directml"))]
    {
        return "windows-directml-x86_64";
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64", feature = "metal"))]
    {
        return "macos-metal-aarch64";
    }
    #[allow(unreachable_code)]
    "unsupported-development-build"
}

/// Flush durable database state immediately before the signed updater hands
/// control to the platform installer. The pool remains usable if installation
/// is cancelled or fails before the process exits.
#[tauri::command]
pub async fn prepare_for_update(state: State<'_, AppState>) -> Result<(), String> {
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&state.db)
        .await
        .map_err(to_error)?;
    if integrity != "ok" {
        return Err(format!(
            "database integrity check blocked the update: {integrity}"
        ));
    }
    let checkpoint: (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&state.db)
        .await
        .map_err(to_error)?;
    if checkpoint.0 != 0 {
        return Err(format!(
            "database checkpoint remained busy (status {})",
            checkpoint.0
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_log_path(app_handle: AppHandle) -> Result<String, String> {
    let log_dir = app_handle.path().app_log_dir().map_err(|e| e.to_string())?;
    let log_file = log_dir.join("pursue.log");
    Ok(log_file.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn open_logs_directory(app_handle: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let log_dir = app_handle.path().app_log_dir().map_err(|e| e.to_string())?;
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    }
    app_handle
        .opener()
        .open_path(log_dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(())
}
