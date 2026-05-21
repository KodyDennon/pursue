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
        force_ocr: bool,
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
            "pdf" => {
                if !force_ocr {
                    if let Ok(digital_text) = self.pdf.extract_text(path).await {
                        if digital_text.trim().len() > 30 {
                            return Ok((digital_text, "pdf-digital".to_string()));
                        }
                    }
                }
                let text = self.ocr.extract_text_fallback(app, path).await?;
                Ok((text, "neural-vision-got".to_string()))
            }
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                let text = self.ocr.extract_text_fallback(app, path).await?;
                Ok((text, "neural-vision-got".to_string()))
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }
}
