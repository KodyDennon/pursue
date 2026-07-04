use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::{PdfAnalyzer, PdfRenderOptions};
use crate::models::Record;
use anyhow::{anyhow, Result};
use std::fmt::Write as _;
use std::path::Path;
use tauri::Emitter;
use tokio::io::AsyncReadExt;

// Quality-first ceiling: ~16 MiB of UTF-8 text is far beyond normal OCR/digital-text output
// for a single disclosure record, but still prevents pathological inputs from growing without
// bound and taking the desktop process down.
const MAX_TEXT_EXTRACTION_BYTES: usize = 16 * 1024 * 1024;

pub struct TextExtractor {
    pub ocr: OcrEngine,
    pub pdf: PdfAnalyzer,
}

pub struct TextExtractionResult {
    pub text: String,
    pub engine: String,
    pub warnings: Vec<String>,
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
    ) -> Result<TextExtractionResult> {
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "txt" | "md" | "csv" | "json" => {
                let (text, truncated) =
                    read_text_file_limited(path, MAX_TEXT_EXTRACTION_BYTES).await?;
                let mut warnings = Vec::new();
                if truncated {
                    let warning = format!(
                        "Text extraction truncated at {} MiB to keep analysis memory bounded.",
                        MAX_TEXT_EXTRACTION_BYTES / 1024 / 1024
                    );
                    emit_analysis_warning(app, id, &warning);
                    warnings.push(warning);
                }
                Ok(TextExtractionResult {
                    text,
                    engine: "text-file".to_string(),
                    warnings,
                })
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

                let mut digital_text = self.pdf.extract_text(path).await.unwrap_or_default();
                let mut warnings = Vec::new();
                if truncate_to_utf8_boundary(&mut digital_text, MAX_TEXT_EXTRACTION_BYTES) {
                    let warning = format!(
                        "PDF digital text truncated at {} MiB to keep analysis memory bounded.",
                        MAX_TEXT_EXTRACTION_BYTES / 1024 / 1024
                    );
                    emit_analysis_warning(app, id, &warning);
                    warnings.push(warning);
                }
                if digital_text.trim().len() > 30 {
                    return Ok(TextExtractionResult {
                        text: digital_text,
                        engine: "pdf-digital".to_string(),
                        warnings,
                    });
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

                let mut full_text = String::new();
                let total_pages = self.pdf.page_count(path)?;

                for idx in 0..total_pages {
                    let _ = app.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "status": "extracting-foundation",
                            "record_id": id,
                            "step": format!("OCR Processing Page {} of {}", idx + 1, total_pages)
                        }),
                    );

                    match self
                        .pdf
                        .render_page(path, idx, PdfRenderOptions::default())
                        .await
                    {
                        Ok(page_img) => match self.ocr.extract_text(app, &page_img).await {
                            Ok(page_text) => {
                                full_text.push_str(&page_text);
                                full_text.push('\n');
                                if truncate_to_utf8_boundary(
                                    &mut full_text,
                                    MAX_TEXT_EXTRACTION_BYTES,
                                ) {
                                    let warning = format!(
                                        "OCR text truncated at {} MiB after page {} of {}.",
                                        MAX_TEXT_EXTRACTION_BYTES / 1024 / 1024,
                                        idx + 1,
                                        total_pages
                                    );
                                    emit_analysis_warning(app, id, &warning);
                                    warnings.push(warning);
                                    break;
                                }
                            }
                            Err(error) => {
                                let warning = format!(
                                    "OCR failed for page {} of {}: {}",
                                    idx + 1,
                                    total_pages,
                                    error
                                );
                                emit_analysis_warning(app, id, &warning);
                                warnings.push(warning);
                            }
                        },
                        Err(error) => {
                            let warning = format!(
                                "PDF render failed for page {} of {}: {}",
                                idx + 1,
                                total_pages,
                                error
                            );
                            emit_analysis_warning(app, id, &warning);
                            warnings.push(warning);
                        }
                    }
                }

                Ok(TextExtractionResult {
                    text: full_text.trim().to_string(),
                    engine: "onnx-ocr".to_string(),
                    warnings,
                })
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
                Ok(TextExtractionResult {
                    text,
                    engine: "onnx-ocr".to_string(),
                    warnings: Vec::new(),
                })
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

                Ok(TextExtractionResult {
                    text: media_record_text(record),
                    engine: "disclosure-metadata".to_string(),
                    warnings: Vec::new(),
                })
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }
}

async fn read_text_file_limited(path: &Path, max_bytes: usize) -> Result<(String, bool)> {
    let file = tokio::fs::File::open(path).await?;
    let mut limited = file.take((max_bytes + 1) as u64);
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
    limited.read_to_end(&mut bytes).await?;
    let truncated = bytes.len() > max_bytes;
    if truncated {
        bytes.truncate(max_bytes);
    }
    Ok((String::from_utf8_lossy(&bytes).to_string(), truncated))
}

fn truncate_to_utf8_boundary(text: &mut String, max_bytes: usize) -> bool {
    if text.len() <= max_bytes {
        return false;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    true
}

fn emit_analysis_warning(app: &tauri::AppHandle, record_id: &str, warning: &str) {
    let _ = app.emit(
        "analysis-progress",
        serde_json::json!({
            "status": "analysis-warning",
            "record_id": record_id,
            "warning": warning
        }),
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_to_utf8_boundary_preserves_valid_string() {
        let mut text = "abcédef".to_string();
        assert!(truncate_to_utf8_boundary(&mut text, 4));
        assert_eq!(text, "abc");
    }

    #[tokio::test]
    async fn read_text_file_limited_reports_truncation() {
        let path =
            std::env::temp_dir().join(format!("pursue-text-limit-{}.txt", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, "abcdefghij").await.unwrap();

        let (text, truncated) = read_text_file_limited(&path, 5).await.unwrap();
        assert_eq!(text, "abcde");
        assert!(truncated);

        let _ = tokio::fs::remove_file(path).await;
    }
}
