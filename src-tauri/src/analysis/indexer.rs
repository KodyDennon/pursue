use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::PdfAnalyzer;
use anyhow::{anyhow, Result};
use std::path::Path;
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
        _id: &str,
        path: &Path,
        _force_ocr: bool,
    ) -> Result<(String, String)> {
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "txt" | "md" | "csv" | "json" => {
                Ok((fs::read_to_string(path).await?, "text-file".to_string()))
            }
            "pdf" | "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                // UNIVERSAL NEURAL VISION: All visual documents are now routed through
                // the GOT-OCR-2.0 sidecar for maximum precision.
                let text = self.ocr.extract_text_fallback(app, path).await?;
                Ok((text, "neural-vision-got".to_string()))
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }
}
