use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::PdfAnalyzer;
use anyhow::{anyhow, Result};
use std::path::Path;
use tauri::Emitter;
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
        id: &str,
        path: &Path,
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
                match self.ocr.extract_text_fallback(app, path).await {
                    Ok(text) => Ok((text, "neural-vision".to_string())),
                    Err(e) => {
                        tauri_plugin_log::log::warn!(
                            "[Analysis] Neural Vision extraction failed for record {}: {}. Falling back to digital...",
                            id,
                            e
                        );
                        // Emit warning progress event to modal thought stream
                        let _ = app.emit(
                            "analysis-progress",
                            serde_json::json!({
                                "status": "extracting-foundation",
                                "record_id": id,
                                "step": "Warning: GOT-OCR failed, falling back to digital text extraction"
                            }),
                        );
                        // Fall back to digital text
                        if let Ok(digital_text) = self.pdf.extract_text(path).await {
                            if digital_text.trim().len() > 30 {
                                return Ok((digital_text, "pdf-digital".to_string()));
                            }
                        }
                        // If digital text extraction also fails or is too short
                        Err(anyhow!(
                            "Neural Vision failed ({}) and digital extraction returned insufficient text",
                            e
                        ))
                    }
                }
            }
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                let text = self.ocr.extract_text_fallback(app, path).await?;
                Ok((text, "neural-vision".to_string()))
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }
}
