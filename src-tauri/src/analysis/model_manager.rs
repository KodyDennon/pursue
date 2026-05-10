use crate::library::LibraryManager;
use anyhow::Result;
use futures_util::StreamExt;
use rquest::Client;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tauri_plugin_log::log::warn;
use tokio::fs;
use tokio::io::AsyncWriteExt;

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
        
        if target_path.exists() {
            let is_gguf = model_name.ends_with(".gguf");
            
            if let Ok(mut file) = tokio::fs::File::open(&target_path).await {
                use tokio::io::AsyncReadExt;
                let mut magic = [0u8; 4];
                if file.read_exact(&mut magic).await.is_ok() {
                    let mut is_corrupted = false;
                    
                    if is_gguf {
                        if &magic != b"GGUF" {
                            warn!("Model file {} is corrupted (invalid GGUF magic).", model_name);
                            is_corrupted = true;
                        } else if file.metadata().await?.len() < 100 * 1024 * 1024 {
                            warn!("Model file {} is too small for a GGUF model.", model_name);
                            is_corrupted = true;
                        }
                    } else if &magic == b"<!DO" || &magic == b"<htm" {
                        warn!("Model file {} appears to be an HTML error page.", model_name);
                        is_corrupted = true;
                    }

                    if !is_corrupted {
                        return Ok(target_path);
                    }
                }
            }
            warn!("Purging {} for re-download.", model_name);
            let _ = fs::remove_file(&target_path).await;
        }

        fs::create_dir_all(&self.models_dir).await?;

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

        let response = request.send().await?;
        
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

        let mut stream = response.bytes_stream();

        app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "starting".to_string(),
            },
        )?;

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

        // Finalize atomic download
        fs::rename(&part_path, &target_path).await?;

        app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: downloaded,
                total_bytes,
                status: "completed".to_string(),
            },
        )?;

        Ok(target_path)
    }
}
