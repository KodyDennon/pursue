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

    pub async fn extract(
        &self,
        app: &tauri::AppHandle,
        path: &Path,
        force_ocr: bool,
    ) -> Result<(String, String)> {
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();
        match extension.as_str() {
            "pdf" => self.extract_pdf(app, path, force_ocr).await,
            "txt" | "md" | "csv" | "json" => {
                Ok((fs::read_to_string(path).await?, "text-file".to_string()))
            }
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                self.extract_image(app, path).await
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }

    async fn extract_pdf(
        &self,
        app: &tauri::AppHandle,
        path: &Path,
        _force_ocr: bool,
    ) -> Result<(String, String)> {
        // Always-On Pixel OCR: We prioritize high-resolution Vision OCR over digital text layers
        // to ensure we capture graphic overlays, improper redactions, and visual evidence.

        info!("Initiating high-resolution Pixel OCR for foundation indexing...");

        #[cfg(target_os = "macos")]
        {
            if let Ok(text) = crate::analysis::native_macos::extract_text_macos(app, path).await {
                return Ok((text, "macos-vision-pdf".to_string()));
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(text) = crate::analysis::native_windows::extract_text_windows(app, path).await {
                return Ok((text, "windows-ocr-pdf".to_string()));
            }
        }

        // Fallback to digital text only if vision OCR fails completely
        let digital = self.pdf.extract_text(path).await?;
        if digital.trim().len() > 10 {
            return Ok((digital, "pdf-digital-fallback".to_string()));
        }

        let text = self.ocr.extract_text_fallback(app, path).await?;
        Ok((text, "rust-ocrs-fallback".to_string()))
    }

    async fn extract_image(&self, app: &tauri::AppHandle, path: &Path) -> Result<(String, String)> {
        #[cfg(target_os = "macos")]
        if let Ok(text) = crate::analysis::native_macos::extract_text_macos(app, path).await {
            return Ok((text, "macos-vision-image".to_string()));
        }

        #[cfg(target_os = "windows")]
        if let Ok(text) = crate::analysis::native_windows::extract_text_windows(app, path).await {
            return Ok((text, "windows-ocr-image".to_string()));
        }

        let text = self.ocr.extract_text_fallback(app, path).await?;
        Ok((text, "rust-ocrs".to_string()))
    }
}
