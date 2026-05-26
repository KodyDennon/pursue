use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::PdfAnalyzer;
use anyhow::{anyhow, Result};
use std::path::Path;
use tauri::Emitter;

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
                let text = tokio::fs::read_to_string(path).await?;
                Ok((text, "text-file".to_string()))
            }
            "pdf" => {
                // Step 1: Attempt digital text extraction
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "extracting-foundation",
                        "record_id": id,
                        "step": "Checking PDF digital text layer..."
                    }),
                );

                let digital_text = self.pdf.extract_text(path).await.unwrap_or_default();
                if digital_text.trim().len() > 30 {
                    return Ok((digital_text, "pdf-digital".to_string()));
                }

                // Step 2: Fallback to rendering and local ONNX OCR
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "extracting-foundation",
                        "record_id": id,
                        "step": "Digital text layer empty. Running local ONNX OCR..."
                    }),
                );

                let pages = self.pdf.render_pdf_to_images(path).await?;
                let mut full_text = String::new();
                let total_pages = pages.len();

                for (idx, page_img) in pages.into_iter().enumerate() {
                    let _ = app.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "status": "extracting-foundation",
                            "record_id": id,
                            "step": format!("OCR Processing Page {} of {}", idx + 1, total_pages)
                        }),
                    );

                    let page_text = self.ocr.extract_text(app, &page_img).await?;
                    full_text.push_str(&page_text);
                    full_text.push('\n');
                }

                Ok((full_text.trim().to_string(), "onnx-ocr".to_string()))
            }
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "extracting-foundation",
                        "record_id": id,
                        "step": "Running local ONNX OCR on image..."
                    }),
                );

                let img = image::open(path)?;
                let text = self.ocr.extract_text(app, &img).await?;
                Ok((text, "onnx-ocr".to_string()))
            }
            "mp4" | "mov" | "avi" | "mkv" | "webm" => Ok((
                "[Media file: foundation text extraction skipped]".to_string(),
                "media-placeholder".to_string(),
            )),
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }
}
