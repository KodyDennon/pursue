use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use crate::library::LibraryManager;
use rquest::Client;
use futures_util::StreamExt;

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

    pub async fn ensure_model(&self, model_name: &str, url: &str) -> Result<PathBuf> {
        let target_path = self.models_dir.join(model_name);
        if target_path.exists() {
            return Ok(target_path);
        }

        fs::create_dir_all(&self.models_dir).await?;
        
        let mut response = self.client.get(url).send().await?;
        let mut stream = response.bytes_stream();
        let mut file = fs::File::create(&target_path).await?;

        while let Some(item) = stream.next().await {
            let chunk = item?;
            tokio::io::copy(&mut &chunk[..], &mut file).await?;
        }

        Ok(target_path)
    }

    pub fn get_model_path(&self, model_name: &str) -> PathBuf {
        self.models_dir.join(model_name)
    }
}
