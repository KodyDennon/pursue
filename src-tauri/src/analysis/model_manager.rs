use crate::library::LibraryManager;
use anyhow::Result;
use futures_util::StreamExt;
use rquest::Client;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
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
        Self {
            client: Client::new(),
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
            return Ok(target_path);
        }

        fs::create_dir_all(&self.models_dir).await?;

        let response = self.client.get(url).send().await?;
        let total_bytes = response.content_length();
        let mut stream = response.bytes_stream();
        let mut file = fs::File::create(&target_path).await?;
        let mut downloaded = 0u64;

        app.emit(
            "model-progress",
            ModelProgress {
                model_id: model_id.to_string(),
                bytes_downloaded: 0,
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
