use crate::library::LibraryManager;
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri_plugin_log::log::info;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

const HF_KEYRING_SERVICE: &str = "com.pursue-data-analyzer.huggingface";
const HF_ACCESS_TOKEN_ACCOUNT: &str = "oauth-access-token";
const HF_REFRESH_TOKEN_ACCOUNT: &str = "oauth-refresh-token";

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn read_hf_keyring_secret(account: &str) -> Option<String> {
    keyring::Entry::new(HF_KEYRING_SERVICE, account)
        .ok()?
        .get_password()
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn read_hf_keyring_secret(_account: &str) -> Option<String> {
    None
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn store_hf_oauth_credentials(
    access_token: &str,
    refresh_token: Option<&str>,
) -> Result<()> {
    keyring::Entry::new(HF_KEYRING_SERVICE, HF_ACCESS_TOKEN_ACCOUNT)
        .context("could not open the operating-system credential store")?
        .set_password(access_token)
        .context("could not secure the Hugging Face access token")?;
    if let Some(refresh_token) = refresh_token {
        keyring::Entry::new(HF_KEYRING_SERVICE, HF_REFRESH_TOKEN_ACCOUNT)
            .context("could not open the operating-system credential store")?
            .set_password(refresh_token)
            .context("could not secure the Hugging Face refresh token")?;
    } else if let Ok(entry) = keyring::Entry::new(HF_KEYRING_SERVICE, HF_REFRESH_TOKEN_ACCOUNT) {
        let _ = entry.delete_credential();
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn store_hf_manual_token(access_token: &str) -> Result<()> {
    keyring::Entry::new(HF_KEYRING_SERVICE, HF_ACCESS_TOKEN_ACCOUNT)
        .context("could not open the operating-system credential store")?
        .set_password(access_token)
        .context("could not secure the Hugging Face access token")?;
    // A manual token has no matching OAuth refresh token. Remove any old one
    // so an expired manual credential cannot silently switch accounts.
    if let Ok(entry) = keyring::Entry::new(HF_KEYRING_SERVICE, HF_REFRESH_TOKEN_ACCOUNT) {
        let _ = entry.delete_credential();
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn store_hf_oauth_credentials(
    _access_token: &str,
    _refresh_token: Option<&str>,
) -> Result<()> {
    Err(anyhow!(
        "Hugging Face OAuth credential storage is only available on Windows and macOS"
    ))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn store_hf_manual_token(_access_token: &str) -> Result<()> {
    Err(anyhow!(
        "Hugging Face credential storage is only available on Windows and macOS"
    ))
}

#[derive(Clone, Serialize)]
pub struct ModelProgress {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub status: String,
    pub speed_mbps: Option<f64>,
    pub eta_seconds: Option<u64>,
}

pub struct ModelManager {
    client: Client,
    db: Option<sqlx::SqlitePool>,
    models_dir: PathBuf,
    active_locks: Arc<Mutex<HashSet<String>>>,
}

struct ModelProgressThrottle {
    last_emit_at: Instant,
    last_emit_bytes: u64,
    last_emit_percent: Option<u64>,
}

#[derive(Clone, Copy)]
struct ModelIntegrity<'a> {
    expected_bytes: Option<u64>,
    expected_sha256: Option<&'a str>,
}

#[derive(Clone, Copy)]
struct ModelDownload<'a> {
    model_id: &'a str,
    model_name: &'a str,
    integrity: ModelIntegrity<'a>,
}

impl ModelProgressThrottle {
    fn new(downloaded: u64, total_bytes: Option<u64>) -> Self {
        Self {
            last_emit_at: Instant::now(),
            last_emit_bytes: downloaded,
            last_emit_percent: percent(downloaded, total_bytes),
        }
    }

    fn should_emit(&mut self, downloaded: u64, total_bytes: Option<u64>) -> bool {
        let elapsed = self.last_emit_at.elapsed();
        let byte_delta = downloaded.saturating_sub(self.last_emit_bytes);
        let next_percent = percent(downloaded, total_bytes);
        let percent_changed = matches!(
            (self.last_emit_percent, next_percent),
            (Some(previous), Some(next)) if next > previous
        );

        if elapsed >= Duration::from_millis(500) || byte_delta >= 4 * 1024 * 1024 || percent_changed
        {
            self.last_emit_at = Instant::now();
            self.last_emit_bytes = downloaded;
            self.last_emit_percent = next_percent;
            return true;
        }
        false
    }
}

fn percent(downloaded: u64, total_bytes: Option<u64>) -> Option<u64> {
    let total = total_bytes?;
    if total == 0 {
        return None;
    }
    Some(((downloaded.saturating_mul(100)) / total).min(100))
}

impl ModelManager {
    pub fn new(library: &LibraryManager) -> Self {
        let models_dir = library.app_data_dir().join("models");
        let client = Client::builder()
            .user_agent(concat!("PURSUE-Data-Analyzer/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::limited(20))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            db: None,
            models_dir,
            active_locks: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn with_db(mut self, db: sqlx::SqlitePool) -> Self {
        self.db = Some(db);
        self
    }

    pub fn models_dir(&self) -> &PathBuf {
        &self.models_dir
    }

    pub async fn ensure_model(
        &self,
        app: &AppHandle,
        model_id: &str,
        model_name: &str,
        url: &str,
        expected_bytes: Option<u64>,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf> {
        // If the URL is a repo ID (no resolve/gguf/onnx), provision as repo
        if !url.contains("/resolve/") && !url.ends_with(".gguf") && !url.ends_with(".onnx") {
            return self.provision_repository(app, model_id, url).await;
        }

        let target_path = self.models_dir.join(model_name);

        // Wait for lock on this specific model
        loop {
            let mut locks = self.active_locks.lock().await;
            if locks.contains(model_name) {
                drop(locks);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
            locks.insert(model_name.to_string());
            break;
        }

        // Fetch HF token if available
        let hf_token = self.get_hf_token().await;

        let result = self
            .ensure_model_inner(
                app,
                ModelDownload {
                    model_id,
                    model_name,
                    integrity: ModelIntegrity {
                        expected_bytes,
                        expected_sha256,
                    },
                },
                url,
                &target_path,
                hf_token,
            )
            .await;

        // Release lock
        let mut locks = self.active_locks.lock().await;
        locks.remove(model_name);

        result
    }

    /// Downloads repository using HTTP from Hugging Face without requiring external CLI tools
    pub async fn provision_repository(
        &self,
        app: &AppHandle,
        model_id: &str,
        repo_id: &str,
    ) -> Result<PathBuf> {
        let repo_dir = self.models_dir.join(model_id);
        fs::create_dir_all(&repo_dir).await?;

        info!(
            "Downloading repository {} via HTTP to {}",
            repo_id,
            repo_dir.display()
        );

        let _ = app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: 0,
                total_bytes: None,
                status: format!("Initializing download for {}...", repo_id),
                speed_mbps: None,
                eta_seconds: None,
            },
        );

        self.provision_repository_http(app, model_id, repo_id).await
    }

    /// Downloads all required files for a repository via HTTP
    async fn provision_repository_http(
        &self,
        app: &AppHandle,
        model_id: &str,
        repo_id: &str,
    ) -> Result<PathBuf> {
        let repo_dir = self.models_dir.join(model_id);
        fs::create_dir_all(&repo_dir).await?;

        // 1. Fetch file list from HF API
        let hf_token = self.get_hf_token().await;
        let mut request = self.client.get(format!(
            "https://huggingface.co/api/models/{}/tree/main?recursive=1",
            repo_id
        ));
        if let Some(token) = &hf_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?.error_for_status()?;
        let files: Vec<serde_json::Value> = response.json().await?;

        // 2. Identify required files
        let required_patterns = [".json", ".safetensors", ".txt", ".model"];
        let mut files_to_download = Vec::new();
        for file in files {
            if let Some(path) = file["path"].as_str() {
                if required_patterns.iter().any(|p| path.ends_with(p)) {
                    files_to_download.push(path.to_string());
                }
            }
        }

        if files_to_download.is_empty() {
            return Err(anyhow!("No model files found in repository {}", repo_id));
        }

        // 3. Download each file
        for (i, file_path) in files_to_download.iter().enumerate() {
            let download_url = format!(
                "https://huggingface.co/{}/resolve/main/{}",
                repo_id, file_path
            );
            let target_file_path = repo_dir.join(file_path);

            if let Some(parent) = target_file_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            info!(
                "Syncing repo file [{}/{}]: {}",
                i + 1,
                files_to_download.len(),
                file_path
            );

            let _ = app.emit(
                "model-progress",
                ModelProgress {
                    model_id: model_id.to_string(),
                    bytes_downloaded: (i * 100) as u64,
                    total_bytes: Some(files_to_download.len() as u64 * 100),
                    status: format!(
                        "Syncing {} ({} of {})",
                        file_path,
                        i + 1,
                        files_to_download.len()
                    ),
                    speed_mbps: None,
                    eta_seconds: None,
                },
            );

            self.ensure_model_inner(
                app,
                ModelDownload {
                    model_id,
                    model_name: file_path,
                    integrity: ModelIntegrity {
                        expected_bytes: None,
                        expected_sha256: None,
                    },
                },
                &download_url,
                &target_file_path,
                hf_token.clone(),
            )
            .await?;
        }

        Ok(repo_dir)
    }

    async fn get_hf_token(&self) -> Option<String> {
        if let Some(token) = read_hf_keyring_secret(HF_ACCESS_TOKEN_ACCOUNT) {
            return Some(token);
        }
        if let Some(token) = self.db_string_setting("huggingface_token").await {
            // One-time migration from pre-keychain releases. Delete plaintext
            // only after the OS credential store confirms a durable write.
            if store_hf_manual_token(&token).is_ok() {
                if let Some(pool) = &self.db {
                    let _ = sqlx::query("DELETE FROM app_settings WHERE key = 'huggingface_token'")
                        .execute(pool)
                        .await;
                }
            }
            return Some(token);
        }
        ["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .filter(|token| !token.trim().is_empty())
    }

    async fn db_string_setting(&self, key: &str) -> Option<String> {
        let pool = self.db.as_ref()?;
        let row = sqlx::query("SELECT value_json FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()??;
        let value_json: String = row.get("value_json");
        serde_json::from_str::<String>(&value_json)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    async fn refresh_hf_oauth_token(&self) -> Option<String> {
        self.db.as_ref()?;
        let refresh_token = read_hf_keyring_secret(HF_REFRESH_TOKEN_ACCOUNT)?;
        let client_id = std::env::var("PURSUE_HF_OAUTH_CLIENT_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "26be6b09-91c5-47da-9861-d2d2bb7a7e36".to_string());
        let response = self
            .client
            .post("https://huggingface.co/oauth/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", client_id.as_str()),
            ])
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let payload = response.json::<serde_json::Value>().await.ok()?;
        let access_token = payload.get("access_token")?.as_str()?.to_string();
        if access_token.trim().is_empty() {
            return None;
        }

        let replacement = payload
            .get("refresh_token")
            .and_then(|value| value.as_str());
        store_hf_oauth_credentials(&access_token, replacement.or(Some(&refresh_token))).ok()?;
        Some(access_token)
    }

    async fn ensure_model_inner(
        &self,
        app: &AppHandle,
        download: ModelDownload<'_>,
        url: &str,
        target_path: &PathBuf,
        hf_token: Option<String>,
    ) -> Result<PathBuf> {
        let ModelDownload {
            model_id,
            model_name,
            integrity,
        } = download;
        let ModelIntegrity {
            expected_bytes,
            expected_sha256,
        } = integrity;
        if target_path.exists() {
            if self
                .verify_download(target_path, model_name, expected_bytes, expected_sha256)
                .await
                .is_ok()
            {
                return Ok(target_path.clone());
            }
            fs::remove_file(&target_path)
                .await
                .with_context(|| format!("failed to remove invalid managed model {model_name}"))?;
        }

        fs::create_dir_all(&self.models_dir).await?;
        let part_path = target_path.with_extension("part");
        let mut downloaded = 0u64;

        if part_path.exists() {
            if let Ok(metadata) = fs::metadata(&part_path).await {
                downloaded = metadata.len();
            }
        }

        if expected_bytes.is_some_and(|expected| downloaded > expected) {
            fs::remove_file(&part_path).await.with_context(|| {
                format!("failed to discard oversized partial download for {model_name}")
            })?;
            downloaded = 0;
        }

        let required_remaining = expected_bytes
            .map(|total| total.saturating_sub(downloaded))
            .unwrap_or(0);
        if required_remaining > 0 {
            let available = available_space_for(&self.models_dir).ok_or_else(|| {
                anyhow!("could not determine free disk space for the model directory")
            })?;
            // Leave 1 GiB for SQLite, logs, and the final atomic rename.
            let reserve = 1024_u64 * 1024 * 1024;
            if available < required_remaining.saturating_add(reserve) {
                return Err(anyhow!(
                    "insufficient disk space for {model_name}: need at least {:.2} GiB free, have {:.2} GiB",
                    required_remaining.saturating_add(reserve) as f64 / 1024_f64.powi(3),
                    available as f64 / 1024_f64.powi(3)
                ));
            }
        }

        let mut response = self
            .download_request(url, downloaded, hf_token.as_deref())
            .send()
            .await
            .with_context(|| {
                format!("failed to reach Hugging Face while downloading {model_name}")
            })?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(refreshed_token) = self.refresh_hf_oauth_token().await {
                response = self
                    .download_request(url, downloaded, Some(&refreshed_token))
                    .send()
                    .await
                    .with_context(|| {
                        format!(
                            "failed to retry {model_name} after refreshing Hugging Face sign-in"
                        )
                    })?;
            }
        }
        if matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Err(anyhow!(
                "Hugging Face denied access to {model_name}. Accept the Google Gemma license, sign in with Hugging Face from PURSUE setup (or provide HF_TOKEN), and retry"
            ));
        }
        if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            if part_path.exists()
                && self
                    .verify_download(&part_path, model_name, expected_bytes, expected_sha256)
                    .await
                    .is_ok()
            {
                fs::rename(&part_path, target_path).await?;
                return Ok(target_path.clone());
            }
            if part_path.exists() {
                fs::remove_file(&part_path).await.with_context(|| {
                    format!("failed to discard an invalid resume file for {model_name}")
                })?;
            }
            return Err(anyhow!(
				"Hugging Face rejected the saved resume offset for {model_name}; PURSUE discarded the invalid partial file, so retrying will start cleanly"
            ));
        }
        let response = response.error_for_status().with_context(|| {
            format!("Hugging Face returned an error while downloading {model_name}")
        })?;

        if let Some(content_type) = response.headers().get("content-type") {
            if content_type
                .to_str()
                .unwrap_or_default()
                .contains("text/html")
            {
                return Err(anyhow!(
                    "HuggingFace returned an HTML page instead of the model file. URL: {}",
                    url
                ));
            }
        }

        let (mut file, total_bytes) = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let content_range = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| anyhow!("resume response for {model_name} omitted Content-Range"))?;
            let expected_prefix = format!("bytes {downloaded}-");
            if !content_range.starts_with(&expected_prefix) {
                return Err(anyhow!(
                    "resume response for {model_name} started at the wrong offset: {content_range}"
                ));
            }
            let file = fs::OpenOptions::new().append(true).open(&part_path).await?;
            let content_len = response.content_length().unwrap_or(0);
            (file, expected_bytes.or(Some(content_len + downloaded)))
        } else {
            // Server ignored Range header (common after redirects) — start fresh
            downloaded = 0;
            let file = fs::File::create(&part_path).await?;
            (file, expected_bytes.or(response.content_length()))
        };

        if let (Some(expected), Some(reported)) = (expected_bytes, total_bytes) {
            if expected != reported {
                return Err(anyhow!(
                    "Hugging Face size changed for {model_name}: pinned {expected} bytes, server reported {reported} bytes"
                ));
            }
        }

        let mut stream = response.bytes_stream();
        let session_start = std::time::Instant::now();
        let mut session_downloaded = 0u64;
        let mut progress_throttle = ModelProgressThrottle::new(downloaded, total_bytes);

        let _ = app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "starting".to_string(),
                speed_mbps: None,
                eta_seconds: None,
            },
        );

        while let Some(item) = stream.next().await {
            let chunk = item?;
            let chunk_len = chunk.len() as u64;
            downloaded += chunk_len;
            session_downloaded += chunk_len;
            file.write_all(&chunk).await?;

            let elapsed = session_start.elapsed().as_secs_f64();
            let mut speed_mbps = None;
            let mut eta_seconds = None;

            if elapsed > 1.0 {
                let speed_bps = session_downloaded as f64 / elapsed;
                speed_mbps = Some(speed_bps / 1024.0 / 1024.0);

                if let Some(total) = total_bytes {
                    if total > downloaded && speed_bps > 0.0 {
                        eta_seconds = Some(((total - downloaded) as f64 / speed_bps) as u64);
                    }
                }
            }

            if progress_throttle.should_emit(downloaded, total_bytes) {
                let _ = app.emit(
                    "model-progress",
                    ModelProgress {
                        model_id: model_id.to_string(),
                        bytes_downloaded: downloaded,
                        total_bytes,
                        status: "downloading".to_string(),
                        speed_mbps,
                        eta_seconds,
                    },
                );
            }
        }

        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        self.verify_download(&part_path, model_name, expected_bytes, expected_sha256)
            .await?;
        fs::rename(&part_path, &target_path).await?;
        let _ = app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "completed".to_string(),
                speed_mbps: None,
                eta_seconds: None,
            },
        );
        Ok(target_path.clone())
    }

    fn download_request(
        &self,
        url: &str,
        downloaded: u64,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut request = self.client.get(url);
        if downloaded > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={downloaded}-"));
        }
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request
    }

    async fn verify_download(
        &self,
        path: &std::path::Path,
        model_name: &str,
        expected_bytes: Option<u64>,
        expected_sha256: Option<&str>,
    ) -> Result<()> {
        if super::verifier::is_model_corrupted(path, model_name).await {
            return Err(anyhow!("{model_name} failed format validation"));
        }
        let metadata = fs::metadata(path).await?;
        if let Some(expected) = expected_bytes {
            if metadata.len() != expected {
                return Err(anyhow!(
                    "{model_name} is incomplete: expected {expected} bytes, found {}",
                    metadata.len()
                ));
            }
        }
        if let Some(expected) = expected_sha256 {
            let actual = sha256_file(path).await?;
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(anyhow!(
                    "{model_name} failed SHA-256 verification: expected {expected}, found {actual}"
                ));
            }
        }
        Ok(())
    }
}

async fn sha256_file(path: &std::path::Path) -> Result<String> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 4 * 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn available_space_for(path: &std::path::Path) -> Option<u64> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(sysinfo::Disk::available_space)
}
