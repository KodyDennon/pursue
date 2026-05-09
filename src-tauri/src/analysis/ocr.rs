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

    pub async fn extract_text_from_scanned_pdf<P: AsRef<Path>>(
        &self,
        pdf_path: P,
    ) -> Result<String> {
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

        let text = tokio::fs::read_to_string(&sidecar)
            .await
            .unwrap_or_default();
        let _ = tokio::fs::remove_file(&sidecar).await;
        Ok(text)
    }
    pub fn analyze_redactions(&self, image_path: &Path) -> Result<f32> {
        let img = image::open(image_path)?;
        let luma = img.to_luma8();
        let (width, height) = luma.dimensions();
        
        let mut redaction_pixels = 0u64;
        let mut row_black_counts = vec![0u32; height as usize];
        
        // Pass 1: Count horizontal black pixels
        for y in 0..height {
            let mut current_streak = 0;
            for x in 0..width {
                if luma.get_pixel(x, y).0[0] < 15 {
                    current_streak += 1;
                } else {
                    if current_streak > (width / 8) { // If streak is larger than 1/8th of width, it's a solid block
                        row_black_counts[y as usize] += current_streak;
                    }
                    current_streak = 0;
                }
            }
            if current_streak > (width / 8) {
                row_black_counts[y as usize] += current_streak;
            }
        }
        
        // Pass 2: Filter isolated lines (must be blocky)
        for y in 1..(height - 1) {
            let y_u = y as usize;
            if row_black_counts[y_u] > 0 {
                // If the row above and below have no black lines, it's just a thin line/border, not a redaction block
                if row_black_counts[y_u - 1] == 0 && row_black_counts[y_u + 1] == 0 {
                    continue;
                }
                redaction_pixels += row_black_counts[y_u] as u64;
            }
        }

        let total_pixels = (width as u64) * (height as u64);
        let ratio = (redaction_pixels as f32) / (total_pixels as f32);
        Ok(ratio)
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
