use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::{header, Client};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;
use uuid::Uuid;

use crate::models::DownloadResult;
use crate::vault::{VaultCrypto, VaultEncryptionStatus};

#[derive(Clone)]
pub struct LibraryManager {
    app_data_dir: PathBuf,
    library_path: PathBuf,
    snapshot_path: PathBuf,
    export_path: PathBuf,
    vault: VaultCrypto,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct IngestedArtifact {
    pub artifact_id: String,
    pub sha256: String,
    pub original_filename: Option<String>,
    pub media_type: Option<String>,
    pub byte_size: i64,
    pub source_url: Option<String>,
    pub relative_path: String,
    pub skipped_existing: bool,
}

impl LibraryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = app_handle.path().app_data_dir()?;
        let library_path = app_data_dir.join("library");
        let snapshot_path = app_data_dir.join("snapshots");
        let export_path = app_data_dir.join("exports");
        let vault = VaultCrypto::new(&app_data_dir);
        
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            ),
        );
        headers.insert(
            header::ACCEPT_LANGUAGE,
            header::HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            header::HeaderValue::from_static("gzip, deflate, br, zstd"),
        );
        headers.insert(
            "Priority",
            header::HeaderValue::from_static("u=0, i"),
        );
        headers.insert(
            "Sec-Ch-Ua",
            header::HeaderValue::from_static("\"Chromium\";v=\"148\", \"Google Chrome\";v=\"148\", \"Not/A)Brand\";v=\"99\""),
        );
        headers.insert(
            "Sec-Ch-Ua-Mobile",
            header::HeaderValue::from_static("?0"),
        );
        headers.insert(
            "Sec-Ch-Ua-Platform",
            header::HeaderValue::from_static("\"macOS\""),
        );
        headers.insert(
            "Upgrade-Insecure-Requests",
            header::HeaderValue::from_static("1"),
        );

        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36")
            .default_headers(headers)
            .cookie_store(true) // Crucial for Akamai sessions
            .build()?;

        Ok(Self {
            app_data_dir,
            library_path,
            snapshot_path,
            export_path,
            vault,
            client,
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.app_data_dir).await?;
        fs::create_dir_all(&self.library_path).await?;
        fs::create_dir_all(&self.snapshot_path).await?;
        fs::create_dir_all(&self.export_path).await?;
        fs::create_dir_all(self.app_data_dir.join("decrypted-cache")).await?;
        Ok(())
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn library_dir(&self) -> &Path {
        &self.library_path
    }

    pub fn snapshots_dir(&self) -> &Path {
        &self.snapshot_path
    }

    pub fn exports_dir(&self) -> &Path {
        &self.export_path
    }

    pub fn encryption_status(&self) -> VaultEncryptionStatus {
        self.vault.status()
    }

    pub fn get_full_path(&self, relative_path: &str) -> PathBuf {
        self.library_path.join(relative_path)
    }

    pub async fn get_readable_artifact_path(&self, relative_path: &str) -> Result<PathBuf> {
        Ok(self.get_full_path(relative_path))
    }

    pub async fn encrypt_generated_asset(&self, relative_path: &str) -> Result<String> {
        // Keeping this for potential future user-generated data, but currently artifacts are plaintext
        Ok(relative_path.to_string())
    }

    pub async fn artifact_plaintext_sha256(&self, relative_path: &str) -> Result<String> {
        let path = self.get_full_path(relative_path);
        let mut file = fs::File::open(&path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    pub fn get_relative_path(&self, absolute_path: &Path) -> Option<String> {
        absolute_path
            .strip_prefix(&self.library_path)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    }

    pub async fn ingest_from_url(
        &self,
        pool: &SqlitePool,
        record_id: &str,
        url: &str,
    ) -> Result<DownloadResult> {
        let artifact = self.download_to_library(url).await?;
        let actual_artifact_id = self.attach_artifact(pool, Some(record_id), &artifact, "official")
            .await?;

        Ok(DownloadResult {
            record_id: record_id.to_string(),
            artifact_id: actual_artifact_id,
            sha256: artifact.sha256,
            relative_path: artifact.relative_path,
            byte_size: artifact.byte_size,
            skipped_existing: artifact.skipped_existing,
        })
    }

    pub async fn ingest_from_bytes(
        &self,
        pool: &SqlitePool,
        record_id: &str,
        url: &str,
        bytes: &[u8],
    ) -> Result<DownloadResult> {
        let original_filename = filename_from_url(url);
        let temp_path = self
            .app_data_dir
            .join(format!("download-{}.tmp", Uuid::new_v4()));

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let byte_size = i64::try_from(bytes.len()).unwrap_or(0);

        fs::write(&temp_path, bytes).await?;

        let artifact = self
            .commit_temp_file(
                temp_path,
                hasher,
                byte_size,
                original_filename,
                None,
                Some(url.to_string()),
            )
            .await?;

        let actual_artifact_id = self.attach_artifact(pool, Some(record_id), &artifact, "official")
            .await?;

        Ok(DownloadResult {
            record_id: record_id.to_string(),
            artifact_id: actual_artifact_id,
            sha256: artifact.sha256,
            relative_path: artifact.relative_path,
            byte_size: artifact.byte_size,
            skipped_existing: artifact.skipped_existing,
        })
    }

    pub async fn ingest_manual_file(
        &self,
        pool: &SqlitePool,
        record_id: &str,
        path: &Path,
    ) -> Result<IngestedArtifact> {
        let artifact = self.copy_file_to_library(path).await?;
        let _ = self.attach_artifact(pool, Some(record_id), &artifact, "manual")
            .await?;
        Ok(artifact)
    }

    async fn attach_artifact(
        &self,
        pool: &SqlitePool,
        record_id: Option<&str>,
        artifact: &IngestedArtifact,
        source_type: &str,
    ) -> Result<String> {
        let mut tx = pool.begin().await?;

        let existing_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM artifacts WHERE sha256 = ?"
        )
        .bind(&artifact.sha256)
        .fetch_optional(&mut *tx)
        .await?;

        let artifact_id = if let Some(id) = existing_id {
            // Update existing artifact to point to the new record (or maintain old one)
            sqlx::query(
                "UPDATE artifacts SET record_id = COALESCE(?, record_id), source_url = COALESCE(?, source_url) WHERE id = ?"
            )
            .bind(record_id)
            .bind(&artifact.source_url)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
            id
        } else {
            // Insert new artifact
            sqlx::query(
                r#"
                INSERT INTO artifacts (
                    id, record_id, sha256, original_filename, media_type, byte_size,
                    source_url, relative_path, source_type, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&artifact.artifact_id)
            .bind(record_id)
            .bind(&artifact.sha256)
            .bind(&artifact.original_filename)
            .bind(&artifact.media_type)
            .bind(artifact.byte_size)
            .bind(&artifact.source_url)
            .bind(&artifact.relative_path)
            .bind(source_type)
            .bind(now())
            .execute(&mut *tx)
            .await?;
            artifact.artifact_id.clone()
        };

        if let Some(record_id) = record_id {
            sqlx::query(
                "UPDATE records SET local_path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&artifact.relative_path)
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(artifact_id)
    }

    async fn download_to_library(&self, url: &str) -> Result<IngestedArtifact> {
        let parsed_url = Url::parse(url).with_context(|| format!("failed to parse URL: {url}"))?;

        // Deterministic temp path for resuming
        let mut url_hasher = Sha256::new();
        url_hasher.update(url.as_bytes());
        let url_hash = hex::encode(url_hasher.finalize());
        let part_path = self
            .app_data_dir
            .join(format!("dl-{}.part", &url_hash[..16]));

        let mut downloaded_bytes = 0_u64;
        let mut hasher = Sha256::new();

        if part_path.exists() {
            if let Ok(metadata) = fs::metadata(&part_path).await {
                let size = metadata.len();
                if size > 0 {
                    // Re-read existing content to initialize hasher
                    let mut file = fs::File::open(&part_path).await?;
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        let n = file.read(&mut buffer).await?;
                        if n == 0 {
                            break;
                        }
                        hasher.update(&buffer[..n]);
                    }
                    downloaded_bytes = size;
                }
            }
        }

        let mut request = self.client.get(parsed_url);
        if downloaded_bytes > 0 {
            request = request.header(header::RANGE, format!("bytes={}-", downloaded_bytes));
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("failed to request {url}"))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "download failed with status {}: {}",
                response.status(),
                url
            ));
        }

        let (mut temp_file, byte_size) = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT
        {
            let file = fs::OpenOptions::new().append(true).open(&part_path).await?;
            (file, downloaded_bytes as i64)
        } else {
            // Server doesn't support range or file didn't exist
            let file = fs::File::create(&part_path).await?;
            hasher = Sha256::new(); // Reset hasher
            (file, 0_i64)
        };

        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let original_filename = filename_from_url(url);

        let mut total_downloaded = byte_size;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            total_downloaded += i64::try_from(chunk.len()).unwrap_or(0);
            hasher.update(&chunk);
            temp_file.write_all(&chunk).await?;
        }
        temp_file.flush().await?;
        drop(temp_file);

        let artifact = self
            .commit_temp_file(
                part_path.clone(),
                hasher,
                total_downloaded,
                original_filename,
                media_type,
                Some(url.to_string()),
            )
            .await?;

        // Clean up part file if it wasn't renamed (commit_temp_file might skip if existing)
        if part_path.exists() {
            let _ = fs::remove_file(&part_path).await;
        }

        Ok(artifact)
    }

    async fn copy_file_to_library(&self, path: &Path) -> Result<IngestedArtifact> {
        if !path.exists() {
            return Err(anyhow!("file does not exist: {}", path.display()));
        }

        let original_filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let temp_path = self
            .app_data_dir
            .join(format!("manual-{}.tmp", Uuid::new_v4()));
        let mut source = fs::File::open(path).await?;
        let mut dest = fs::File::create(&temp_path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut byte_size = 0_i64;

        loop {
            let read = source.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            dest.write_all(&buffer[..read]).await?;
            byte_size += i64::try_from(read).unwrap_or(0);
        }
        dest.flush().await?;

        self.commit_temp_file(temp_path, hasher, byte_size, original_filename, None, None)
            .await
    }

    async fn commit_temp_file(
        &self,
        temp_path: PathBuf,
        hasher: Sha256,
        byte_size: i64,
        original_filename: Option<String>,
        media_type: Option<String>,
        source_url: Option<String>,
    ) -> Result<IngestedArtifact> {
        let sha256 = hex::encode(hasher.finalize());
        let extension = original_filename
            .as_deref()
            .and_then(|name| Path::new(name).extension())
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        let final_path = self.path_for_hash(&sha256, extension.as_deref());
        let skipped_existing = final_path.exists();

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        if skipped_existing {
            fs::remove_file(&temp_path).await?;
        } else {
            // No encryption, just rename the temp file to final destination
            fs::rename(&temp_path, &final_path).await?;
        }

        let relative_path = self
            .get_relative_path(&final_path)
            .ok_or_else(|| anyhow!("failed to produce library-relative path"))?;

        Ok(IngestedArtifact {
            artifact_id: Uuid::new_v4().to_string(),
            sha256,
            original_filename,
            media_type,
            byte_size,
            source_url,
            relative_path,
            skipped_existing,
        })
    }

    fn path_for_hash(&self, hash: &str, extension: Option<&str>) -> PathBuf {
        let prefix = &hash[0..2];
        let filename = match extension {
            Some(ext) if !ext.is_empty() => format!("{hash}.{ext}"),
            _ => hash.to_string(),
        };
        self.library_path.join(prefix).join(filename)
    }
}

fn filename_from_url(raw_url: &str) -> Option<String> {
    Url::parse(raw_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back())
                .filter(|segment| !segment.is_empty())
                .map(percent_decode)
        })
        .or_else(|| {
            Path::new(raw_url)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
}

fn percent_decode(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .into_owned()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::filename_from_url;

    #[test]
    fn extracts_filename_from_url() {
        assert_eq!(
            filename_from_url("https://www.war.gov/files/example%20file.pdf"),
            Some("example file.pdf".to_string())
        );
    }
}
