use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct FinalizedPart {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_size: i64,
}

#[derive(Debug, Default)]
struct WriterState {
    // Kept open across `append()` calls instead of being reopened via OpenOptions on every
    // chunk (previously every ~64KB), which was the dominant cost of chunked downloads.
    file: Option<tokio::fs::File>,
    // Cached after the first disk stat so repeated `append()` calls don't re-stat the
    // filesystem; `None` means "not yet initialized from disk".
    offset: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DownloadPartWriter {
    path: PathBuf,
    state: Arc<Mutex<WriterState>>,
}

impl DownloadPartWriter {
    pub async fn new(root: PathBuf, item_id: &str) -> Result<Self> {
        tokio::fs::create_dir_all(&root).await?;
        Ok(Self {
            path: root.join(format!("{item_id}.part")),
            state: Arc::new(Mutex::new(WriterState::default())),
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    async fn offset_on_disk(&self) -> Result<u64> {
        match tokio::fs::metadata(&self.path).await {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn offset(&self) -> Result<u64> {
        let mut state = self.state.lock().await;
        if let Some(offset) = state.offset {
            return Ok(offset);
        }
        let offset = self.offset_on_disk().await?;
        state.offset = Some(offset);
        Ok(offset)
    }

    pub async fn append(&self, expected_offset: u64, bytes: &[u8]) -> Result<u64> {
        let mut state = self.state.lock().await;
        let current = match state.offset {
            Some(offset) => offset,
            None => self.offset_on_disk().await?,
        };
        if current != expected_offset {
            return Err(anyhow!(
                "offset mismatch for {}: expected {}, actual {}",
                self.path.display(),
                expected_offset,
                current
            ));
        }

        if state.file.is_none() {
            let file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .await?;
            state.file = Some(file);
        }
        // Safe: just ensured Some(_) above.
        let file = state.file.as_mut().unwrap();
        file.write_all(bytes).await?;
        file.flush().await?;

        let new_offset = current + bytes.len() as u64;
        state.offset = Some(new_offset);
        Ok(new_offset)
    }

    /// Discards any partial download progress: closes the held file handle, deletes the part
    /// file from disk if present, and resets the cached offset to 0. Callers must use this
    /// instead of deleting the file out from under the writer directly (e.g. via
    /// `tokio::fs::remove_file(writer.path())`) — since `append()` now caches the offset and
    /// holds the file open in memory, an external delete would leave the writer's in-memory
    /// state (and, on most platforms, its open handle) silently out of sync with disk.
    pub async fn reset(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        state.file = None;
        if self.path.exists() {
            tokio::fs::remove_file(&self.path).await?;
        }
        state.offset = Some(0);
        Ok(())
    }

    pub async fn finalize(&self) -> Result<FinalizedPart> {
        // Drop the write handle first so its buffered bytes are flushed/closed before we open
        // a fresh read handle over the same path.
        {
            let mut state = self.state.lock().await;
            state.file = None;
        }

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

    #[tokio::test]
    async fn append_reuses_a_persistent_file_handle_across_many_chunks() {
        // Regression test for the reopen-per-chunk cost this rewrite removes: many small
        // appends on the same writer instance must all land in the same file, in order,
        // without needing writer.offset() re-queried between them.
        let root = temp_root("persistent-handle");
        let writer = DownloadPartWriter::new(root.clone(), "item-4")
            .await
            .unwrap();

        let mut offset = 0_u64;
        let mut expected = Vec::new();
        for chunk_index in 0..50_u8 {
            let chunk = vec![chunk_index; 37];
            offset = writer.append(offset, &chunk).await.unwrap();
            expected.extend_from_slice(&chunk);
        }

        let finalized = writer.finalize().await.unwrap();
        assert_eq!(finalized.byte_size as usize, expected.len());
        assert_eq!(finalized.sha256, hex::encode(Sha256::digest(&expected)));
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn reset_clears_cached_offset_and_deletes_the_part_file() {
        let root = temp_root("reset");
        let writer = DownloadPartWriter::new(root.clone(), "item-5")
            .await
            .unwrap();
        writer.append(0, b"stale-partial-download").await.unwrap();
        assert!(writer.path().exists());

        writer.reset().await.unwrap();

        assert!(!writer.path().exists());
        assert_eq!(writer.offset().await.unwrap(), 0);

        // The writer must be fully usable again after reset, starting a fresh file at 0 —
        // this exercises the exact sequence records.rs's reset_part handling relies on.
        let offset = writer.append(0, b"fresh-download").await.unwrap();
        assert_eq!(offset, "fresh-download".len() as u64);
        let finalized = writer.finalize().await.unwrap();
        assert_eq!(
            finalized.sha256,
            hex::encode(Sha256::digest(b"fresh-download"))
        );
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn offset_reflects_disk_state_from_a_previous_writer_instance_even_after_reset_style_use()
    {
        // Mirrors existing_part_file_reports_resume_offset but confirms a brand-new writer
        // instance (as constructed fresh per Tauri command call) still correctly picks up
        // on-disk state written by an earlier instance's persistent handle.
        let root = temp_root("cross-instance-resume");
        let first = DownloadPartWriter::new(root.clone(), "item-6")
            .await
            .unwrap();
        first.append(0, b"abc").await.unwrap();
        first.append(3, b"def").await.unwrap();
        drop(first);

        let second = DownloadPartWriter::new(root.clone(), "item-6")
            .await
            .unwrap();
        assert_eq!(second.offset().await.unwrap(), 6);
        second.append(6, b"ghi").await.unwrap();

        let finalized = second.finalize().await.unwrap();
        assert_eq!(finalized.sha256, hex::encode(Sha256::digest(b"abcdefghi")));
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
