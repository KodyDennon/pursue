use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone)]
pub struct FinalizedPart {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_size: i64,
}

#[derive(Debug, Clone)]
pub struct DownloadPartWriter {
    path: PathBuf,
}

impl DownloadPartWriter {
    pub async fn new(root: PathBuf, item_id: &str) -> Result<Self> {
        tokio::fs::create_dir_all(&root).await?;
        Ok(Self {
            path: root.join(format!("{item_id}.part")),
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub async fn offset(&self) -> Result<u64> {
        match tokio::fs::metadata(&self.path).await {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn append(&self, expected_offset: u64, bytes: &[u8]) -> Result<u64> {
        let current = self.offset().await?;
        if current != expected_offset {
            return Err(anyhow!(
                "offset mismatch for {}: expected {}, actual {}",
                self.path.display(),
                expected_offset,
                current
            ));
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(bytes).await?;
        file.flush().await?;
        Ok(current + bytes.len() as u64)
    }

    pub async fn finalize(&self) -> Result<FinalizedPart> {
        let mut file = tokio::fs::File::open(&self.path).await?;
        let mut hasher = Sha256::new();
        let mut byte_size = 0_i64;
        let mut buffer = [0_u8; 64 * 1024];

        loop {
            let read = file.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            byte_size += i64::try_from(read).unwrap_or(0);
        }

        Ok(FinalizedPart {
            path: self.path.clone(),
            sha256: hex::encode(hasher.finalize()),
            byte_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::DownloadPartWriter;
    use sha2::{Digest, Sha256};
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pursue-download-test-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[tokio::test]
    async fn append_rejects_out_of_order_offsets() {
        let root = temp_root("offset");
        let writer = DownloadPartWriter::new(root.clone(), "item-1")
            .await
            .unwrap();

        writer.append(0, b"abc").await.unwrap();
        let error = writer.append(99, b"def").await.unwrap_err();

        assert!(error.to_string().contains("offset mismatch"));
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn existing_part_file_reports_resume_offset() {
        let root = temp_root("resume");
        let writer = DownloadPartWriter::new(root.clone(), "item-2")
            .await
            .unwrap();
        writer.append(0, b"abcdef").await.unwrap();

        let resumed = DownloadPartWriter::new(root.clone(), "item-2")
            .await
            .unwrap();

        assert_eq!(resumed.offset().await.unwrap(), 6);
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn finalize_hashes_streamed_file_without_loading_it_all() {
        let root = temp_root("finalize");
        let writer = DownloadPartWriter::new(root.clone(), "item-3")
            .await
            .unwrap();
        writer.append(0, b"abc").await.unwrap();
        writer.append(3, b"def").await.unwrap();

        let finalized = writer.finalize().await.unwrap();
        let expected = hex::encode(Sha256::digest(b"abcdef"));

        assert_eq!(finalized.byte_size, 6);
        assert_eq!(finalized.sha256, expected);
        assert!(finalized.path.ends_with("item-3.part"));
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
