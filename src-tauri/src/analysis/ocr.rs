use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tokio::process::Command;
use uuid::Uuid;

pub struct OcrEngine;

impl OcrEngine {
    pub fn new() -> Self {
        Self
    }

    pub async fn extract_text_from_image<P: AsRef<Path>>(&self, image_path: P) -> Result<String> {
        if !command_available("tesseract").await {
            return Err(anyhow!(
                "image OCR requires the local tesseract binary. Install it with `brew install tesseract` on macOS or the Windows installer, then rerun analysis."
            ));
        }

        let output = Command::new("tesseract")
            .arg(image_path.as_ref())
            .arg("stdout")
            .arg("-l")
            .arg("eng")
            .output()
            .await
            .context("failed to start tesseract")?;

        if !output.status.success() {
            return Err(anyhow!(
                "tesseract failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub async fn extract_text_from_scanned_pdf<P: AsRef<Path>>(&self, pdf_path: P) -> Result<String> {
        if !command_available("ocrmypdf").await {
            return Err(anyhow!(
                "scanned PDF OCR requires the local ocrmypdf binary. Install it with `brew install ocrmypdf` on macOS or the Windows package, then rerun analysis."
            ));
        }

        let sidecar = std::env::temp_dir().join(format!("pursue-{}.txt", Uuid::new_v4()));
        let output_pdf = std::env::temp_dir().join(format!("pursue-{}.pdf", Uuid::new_v4()));
        let output = Command::new("ocrmypdf")
            .arg("--skip-text")
            .arg("--sidecar")
            .arg(&sidecar)
            .arg(pdf_path.as_ref())
            .arg(&output_pdf)
            .output()
            .await
            .context("failed to start ocrmypdf")?;

        let _ = tokio::fs::remove_file(&output_pdf).await;

        if !output.status.success() {
            let _ = tokio::fs::remove_file(&sidecar).await;
            return Err(anyhow!(
                "ocrmypdf failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let text = tokio::fs::read_to_string(&sidecar).await.unwrap_or_default();
        let _ = tokio::fs::remove_file(&sidecar).await;
        Ok(text)
    }
}

async fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}
