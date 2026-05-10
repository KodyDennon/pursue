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

#[derive(Clone, Serialize)]
pub struct ModelProgress {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub status: String,
}

pub struct ModelManager {
    client: Client,
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
            models_dir,
            active_locks: Arc::new(Mutex::new(HashSet::new())),
        }
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

        let result = self.ensure_model_inner(app, model_id, model_name, url, &target_path).await;
        
        // Release lock
        let mut locks = self.active_locks.lock().await;
        locks.remove(model_name);
        
        result
    }

    async fn ensure_model_inner(
        &self,
        app: &AppHandle,
        model_id: &str,
        model_name: &str,
        url: &str,
        target_path: &PathBuf,
    ) -> Result<PathBuf> {
        if target_path.exists() {
            let is_gguf = model_name.ends_with(".gguf");
            
            if let Ok(file_metadata) = fs::metadata(&target_path).await {
                let size = file_metadata.len();
                // Validate existing file
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
                                warn!("Model file {} is too small ({:.2} MB) for a GGUF model.", model_name, size as f64 / 1024.0 / 1024.0);
                                is_corrupted = true;
                            }
                        } else if &magic == b"<!DO" || &magic == b"<htm" {
                            warn!("Model file {} appears to be an HTML error page.", model_name);
                            is_corrupted = true;
                        }

                        if !is_corrupted {
                            return Ok(target_path.clone());
                        }
                    }
                }
            }
            warn!("Purging {} for re-download.", model_name);
            let _ = fs::remove_file(&target_path).await;
        }

        fs::create_dir_all(&self.models_dir).await?;
        info!("Initiating download for model: {} from {}", model_name, url);

        // Atomic & Resumable download: stream to .part file first
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

        let response = request.send().await?
            .error_for_status()?; // CRITICAL: Catch 404, 401, 403, 500 errors
        
        // Final validation: Ensure we aren't downloading an HTML login page or error
        if let Some(content_type) = response.headers().get("content-type") {
            let ct = content_type.to_str().unwrap_or_default();
            if ct.contains("text/html") {
                return Err(anyhow!("HuggingFace returned an HTML page instead of the model file. This repo may be restricted or requires a token. URL: {}", url));
            }
        }

        // Handle response status (200 OK or 206 Partial Content)
        let (mut file, total_bytes) = if response.status() == rquest::StatusCode::PARTIAL_CONTENT {
            let file = fs::OpenOptions::new()
                .append(true)
                .open(&part_path)
                .await?;
            let content_len = response.content_length().unwrap_or(0);
            (file, Some(content_len + downloaded))
        } else {
            downloaded = 0; // Reset if server doesn't support range
            let file = fs::File::create(&part_path).await?;
            (file, response.content_length())
        };

        if let Some(total) = total_bytes {
            if total < 1024 * 1024 {
                 return Err(anyhow!("Reported total size ({:.2} MB) is too small for model {}. Download aborted.", total as f64 / 1024.0 / 1024.0, model_name));
            }
        }

        let mut stream = response.bytes_stream();

        let _ = app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "starting".to_string(),
            },
        );

        while let Some(item) = stream.next().await {
            let chunk = item?;
            downloaded += chunk.len() as u64;
            file.write_all(&chunk).await?;

            let _ = app.emit(
                "model-progress",
                ModelProgress {
                    model_id: model_id.to_string(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    status: "downloading".to_string(),
                },
            );
        }

        file.flush().await?;
        drop(file);

        // Verify final size before finalizing
        let final_metadata = fs::metadata(&part_path).await?;
        if final_metadata.len() < 100 * 1024 * 1024 && model_name.ends_with(".gguf") {
             let _ = fs::remove_file(&part_path).await;
             return Err(anyhow!("Download completed but file is too small (integrity failure). URL likely returned truncated content."));
        }

        // Finalize atomic download
        fs::rename(&part_path, &target_path).await?;

        let _ = app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "completed".to_string(),
            },
        );

        Ok(target_path.clone())
    }
}
