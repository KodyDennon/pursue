use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::PdfAnalyzer;
use crate::models::Record;
use anyhow::{anyhow, Result};
use std::fmt::Write as _;
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
        record: &Record,
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
            "mp4" | "mov" | "avi" | "mkv" | "webm" | "mp3" | "wav" | "m4a" | "aac" | "ogg"
            | "flac" => {
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "extracting-foundation",
                        "record_id": id,
                        "step": "No local speech-to-text model is installed; indexing disclosure metadata instead..."
                    }),
                );

                Ok((media_record_text(record), "disclosure-metadata".to_string()))
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }
}

/// No local speech-to-text model is bundled, so audio/video records are indexed on their
/// real disclosure metadata (title, agency, dates, location, DVIDS captions) instead of a
/// transcript.
fn media_record_text(record: &Record) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "{}", record.title);
    if let Some(video_title) = &record.video_title {
        if video_title != &record.title {
            let _ = writeln!(text, "{video_title}");
        }
    }
    if let Some(agency) = &record.agency {
        let _ = writeln!(text, "Agency: {agency}");
    }
    if let Some(release_date) = &record.release_date {
        let _ = writeln!(text, "Release date: {release_date}");
    }
    if let Some(incident_date) = &record.incident_date {
        let _ = writeln!(text, "Incident date: {incident_date}");
    }
    if let Some(incident_location) = &record.incident_location {
        let _ = writeln!(text, "Incident location: {incident_location}");
    }
    if let Some(summary) = &record.summary {
        let _ = writeln!(text, "{summary}");
    }
    if let Some(alt_text) = &record.image_alt_text {
        let _ = writeln!(text, "{alt_text}");
    }
    text.trim().to_string()
}
