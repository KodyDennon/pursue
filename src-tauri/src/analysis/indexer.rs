use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::PdfAnalyzer;
use anyhow::{anyhow, Result};
use std::path::Path;
use tauri_plugin_log::log::info;
use tokio::fs;

pub struct TextExtractor {
    pub ocr: OcrEngine,
    pub pdf: PdfAnalyzer,
}

impl TextExtractor {
    pub fn new(ocr: OcrEngine, pdf: PdfAnalyzer) -> Self {
        Self { ocr, pdf }
    }

    pub async fn extract(&self, path: &Path) -> Result<(String, String)> {
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();
        match extension.as_str() {
            "pdf" => self.extract_pdf(path).await,
            "txt" | "md" | "csv" | "json" => {
                Ok((fs::read_to_string(path).await?, "text-file".to_string()))
            }
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                self.extract_image(path).await
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }

    async fn extract_pdf(&self, path: &Path) -> Result<(String, String)> {
        let digital = self.pdf.extract_text(path).await?;
        if digital.trim().len() > 100 {
            return Ok((digital, "pdf-text".to_string()));
        }

        #[cfg(target_os = "macos")]
        {
            info!("Digital text sparse. Triggering macOS Vision OCR...");
            if let Ok(text) = crate::analysis::native_macos::extract_text_macos(path).await {
                return Ok((text, "macos-vision-pdf".to_string()));
            }
        }

        #[cfg(target_os = "windows")]
        {
            info!("Digital text sparse. Triggering Windows Media OCR...");
            if let Ok(text) = crate::analysis::native_windows::extract_text_windows(path).await {
                return Ok((text, "windows-ocr-pdf".to_string()));
            }
        }

        let text = self.ocr.extract_text_fallback(path).await?;
        Ok((text, "rust-ocrs".to_string()))
    }

    async fn extract_image(&self, path: &Path) -> Result<(String, String)> {
        #[cfg(target_os = "macos")]
        if let Ok(text) = crate::analysis::native_macos::extract_text_macos(path).await {
            return Ok((text, "macos-vision-image".to_string()));
        }

        #[cfg(target_os = "windows")]
        if let Ok(text) = crate::analysis::native_windows::extract_text_windows(path).await {
            return Ok((text, "windows-ocr-image".to_string()));
        }

        let text = self.ocr.extract_text_fallback(path).await?;
        Ok((text, "rust-ocrs".to_string()))
    }
}
