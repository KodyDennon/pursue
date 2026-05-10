use crate::library::LibraryManager;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use rquest::Client;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_log::log::{info, warn};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use sqlx::Row;

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

impl ModelManager {
    pub fn new(library: &LibraryManager) -> Self {
        let models_dir = library.app_data_dir().join("models");
        let client = Client::builder()
            .user_agent("PURSUE-Intelligence-OS/0.2.0")
            .redirect(rquest::redirect::Policy::limited(20))
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

        let result = self.ensure_model_inner(app, model_id, model_name, url, &target_path, hf_token).await;
        
        // Release lock
        let mut locks = self.active_locks.lock().await;
        locks.remove(model_name);
        
        result
    }

    /// Downloads all required files for a Safetensors model repository
    pub async fn provision_repository(
        &self,
        app: &AppHandle,
        model_id: &str,
        repo_id: &str,
    ) -> Result<PathBuf> {
        let repo_dir = self.models_dir.join(model_id);
        fs::create_dir_all(&repo_dir).await?;

        // 1. Fetch file list from HF API
        let hf_token = self.get_hf_token().await;
        let mut request = self.client.get(format!("https://huggingface.co/api/models/{}/tree/main", repo_id));
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
            let download_url = format!("https://huggingface.co/{}/resolve/main/{}", repo_id, file_path);
            let target_file_path = repo_dir.join(file_path);
            
            if let Some(parent) = target_file_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            info!("Syncing repo file [{}/{}]: {}", i + 1, files_to_download.len(), file_path);
            
            let _ = app.emit("model-progress", serde_json::json!({
                "model_id": model_id,
                "status": format!("Syncing {} ({} of {})", file_path, i+1, files_to_download.len()),
                "bytes_downloaded": 0,
                "total_bytes": files_to_download.len() as u64,
                "speed_mbps": null,
                "eta_seconds": null
            }));

            self.ensure_model_inner(app, model_id, file_path, &download_url, &target_file_path, hf_token.clone()).await?;
        }

        Ok(repo_dir)
    }

    async fn get_hf_token(&self) -> Option<String> {
        if let Some(pool) = &self.db {
            if let Ok(Some(row)) = sqlx::query("SELECT value_json FROM app_settings WHERE key = 'huggingface_token'")
                .fetch_optional(pool)
                .await 
            {
                let val: String = row.get("value_json");
                if let Ok(token) = serde_json::from_str::<String>(&val) {
                    if !token.trim().is_empty() {
                        return Some(token);
                    }
                }
            }
        }
        None
    }

    async fn ensure_model_inner(
        &self,
        app: &AppHandle,
        model_id: &str,
        model_name: &str,
        url: &str,
        target_path: &PathBuf,
        hf_token: Option<String>,
    ) -> Result<PathBuf> {
        if target_path.exists() {
            let is_gguf = model_name.ends_with(".gguf");
            
            if let Ok(file_metadata) = fs::metadata(&target_path).await {
                let size = file_metadata.len();
                if let Ok(mut file) = tokio::fs::File::open(&target_path).await {
                    use tokio::io::AsyncReadExt;
                    let mut magic = [0u8; 4];
                    if file.read_exact(&mut magic).await.is_ok() {
                        let mut is_corrupted = false;
                        if is_gguf {
                            if &magic != b"GGUF" {
                                warn!("Model file {} is corrupted (invalid GGUF magic).", model_name);
                                is_corrupted = true;
                            } else if size < 100 * 1024 * 1024 {
                                is_corrupted = true;
                            }
                        } else if &magic == b"<!DO" || &magic == b"<htm" {
                            is_corrupted = true;
                        }

                        if !is_corrupted {
                            return Ok(target_path.clone());
                        }
                    }
                }
            }
            let _ = fs::remove_file(&target_path).await;
        }

        fs::create_dir_all(&self.models_dir).await?;
        let part_path = target_path.with_extension("part");
        let mut downloaded = 0u64;
        
        if part_path.exists() {
            if let Ok(metadata) = fs::metadata(&part_path).await {
                downloaded = metadata.len();
            }
        }

        let mut request = self.client.get(url);
        if downloaded > 0 {
            request = request.header("Range", format!("bytes={}-", downloaded));
        }
        if let Some(token) = hf_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?.error_for_status()?;
        
        if let Some(content_type) = response.headers().get("content-type") {
            if content_type.to_str().unwrap_or_default().contains("text/html") {
                return Err(anyhow!("HuggingFace returned an HTML page instead of the model file. URL: {}", url));
            }
        }

        let (mut file, total_bytes) = if response.status() == rquest::StatusCode::PARTIAL_CONTENT {
            let file = fs::OpenOptions::new().append(true).open(&part_path).await?;
            let content_len = response.content_length().unwrap_or(0);
            (file, Some(content_len + downloaded))
        } else {
            downloaded = 0;
            let file = fs::File::create(&part_path).await?;
            (file, response.content_length())
        };

        let mut stream = response.bytes_stream();
        let session_start = std::time::Instant::now();
        let mut session_downloaded = 0u64;

        let _ = app.emit("model-progress", ModelProgress {
            model_id: model_id.to_string(),
            bytes_downloaded: downloaded,
            total_bytes,
            status: "starting".to_string(),
            speed_mbps: None,
            eta_seconds: None,
        });

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

            let _ = app.emit("model-progress", ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "downloading".to_string(),
                speed_mbps,
                eta_seconds,
            });
        }

        file.flush().await?;
        drop(file);
        fs::rename(&part_path, &target_path).await?;
        let _ = app.emit("model-progress", ModelProgress {
            model_id: model_id.to_string(),
            bytes_downloaded: downloaded,
            total_bytes,
            status: "completed".to_string(),
            speed_mbps: None,
            eta_seconds: None,
        });
        Ok(target_path.clone())

    }
}
