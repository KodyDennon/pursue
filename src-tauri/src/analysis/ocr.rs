use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tokio::process::Command;
use uuid::Uuid;
use image::GenericImageView;

pub struct OcrEngine;

#[derive(Debug, Default)]
pub struct OcrMetadata {
    pub redaction_score: f32,
    pub is_blank: bool,
}

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
    pub fn analyze_redactions(&self, image_path: &Path) -> Result<f32> {
        let img = image::open(image_path)?;
        let (width, height) = img.dimensions();
        let total_pixels = width as u64 * height as u64;
        let mut black_pixels = 0u64;

        let luma = img.to_luma8();
        for pixel in luma.pixels() {
            if pixel.0 < 10 { // Very dark/Black
                black_pixels += 1;
            }
        }

        let ratio = (black_pixels as f32) / (total_pixels as f32);
        Ok(ratio)
    }

    pub async fn is_blank_page(&self, image_path: &Path) -> Result<bool> {
        let img = image::open(image_path)?;
        let luma = img.to_luma8();
        let pixels: Vec<f32> = luma.pixels().map(|p| p.0 as f32).collect();
        
        if pixels.is_empty() { return Ok(true); }

        let mut mean = 0.0;
        for &p in &pixels {
            mean += p;
        }
        mean /= pixels.len() as f32;

        let mut variance = 0.0;
        for &p in &pixels {
            variance += (p - mean).powi(2);
        }
        variance /= pixels.len() as f32;

        // Very low variance usually means a blank white or solid color page
        Ok(variance < 100.0)
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
