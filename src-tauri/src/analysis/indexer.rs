use crate::analysis::ocr::OcrEngine;
use crate::analysis::pdf::{PdfAnalyzer, PdfRenderOptions};
use crate::models::Record;
use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;
use tauri::{AppHandle, Emitter};
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
    pub metadata: serde_json::Value,
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
                    metadata: serde_json::json!({
                        "mode": "bounded_text_file",
                        "truncated": truncated
                    }),
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

                let digital_pages = self.pdf.extract_text_pages(path).unwrap_or_default();
                let mut digital_text = digital_pages.join("\n");
                let mut warnings = Vec::new();
                if truncate_to_utf8_boundary(&mut digital_text, MAX_TEXT_EXTRACTION_BYTES) {
                    let warning = format!(
                        "PDF digital text truncated at {} MiB to keep analysis memory bounded.",
                        MAX_TEXT_EXTRACTION_BYTES / 1024 / 1024
                    );
                    emit_analysis_warning(app, id, &warning);
                    warnings.push(warning);
                }

                let mut full_text = String::new();
                let total_pages = self.pdf.page_count(path)?;
                let mut page_metadata = Vec::with_capacity(total_pages);
                let mut used_digital_pages = 0usize;
                let mut used_ocr_pages = 0usize;

                for idx in 0..total_pages {
                    let digital_page_text = digital_pages.get(idx).cloned().unwrap_or_default();
                    if is_reliable_pdf_text(&digital_page_text) {
                        used_digital_pages += 1;
                        full_text.push_str(digital_page_text.trim());
                        full_text.push('\n');
                        page_metadata.push(serde_json::json!({
                            "page": idx + 1,
                            "source": "pdf_digital",
                            "digital_chars": digital_page_text.trim().chars().count(),
                            "reliable": true
                        }));
                        continue;
                    }

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
                        Ok(page_img) => match self.ocr.extract_structured(app, &page_img).await {
                            Ok(ocr_output) => {
                                used_ocr_pages += 1;
                                full_text.push_str(&ocr_output.text);
                                full_text.push('\n');
                                page_metadata.push(serde_json::json!({
                                    "page": idx + 1,
                                    "source": "onnx_ocr",
                                    "digital_chars": digital_page_text.trim().chars().count(),
                                    "reliable": false,
                                    "average_confidence": ocr_output.average_confidence,
                                    "region_count": ocr_output.regions.len(),
                                    "image_width": ocr_output.image_width,
                                    "image_height": ocr_output.image_height,
                                    "resized": ocr_output.resized
                                }));
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
                                page_metadata.push(serde_json::json!({
                                    "page": idx + 1,
                                    "source": "failed",
                                    "digital_chars": digital_page_text.trim().chars().count(),
                                    "reliable": false,
                                    "error": error.to_string()
                                }));
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
                            page_metadata.push(serde_json::json!({
                                "page": idx + 1,
                                "source": "failed",
                                "digital_chars": digital_page_text.trim().chars().count(),
                                "reliable": false,
                                "error": error.to_string()
                            }));
                        }
                    }
                }

                let engine = match (used_digital_pages > 0, used_ocr_pages > 0) {
                    (true, true) => "pdf-hybrid",
                    (true, false) => "pdf-digital",
                    (false, true) => "onnx-ocr",
                    (false, false) => "pdf-empty",
                };
                let cleaned_text = crate::analysis::ocr_repair::repair_ocr_text(&full_text);
                Ok(TextExtractionResult {
                    text: cleaned_text,
                    engine: engine.to_string(),
                    warnings,
                    metadata: serde_json::json!({
                        "mode": "pdf_hybrid_page_extraction",
                        "pages": page_metadata,
                        "used_digital_pages": used_digital_pages,
                        "used_ocr_pages": used_ocr_pages,
                        "total_pages": total_pages
                    }),
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
                let ocr_output = self.ocr.extract_structured(app, &img).await?;
                let cleaned_ocr = crate::analysis::ocr_repair::repair_ocr_text(&ocr_output.text);
                let meta_text = media_record_text(record);
                let text = if cleaned_ocr.is_empty() {
                    meta_text
                } else if meta_text.is_empty() {
                    cleaned_ocr
                } else {
                    format!("{meta_text}\n\n--- OCR EXTRACTED TEXT ---\n{cleaned_ocr}")
                };

                Ok(TextExtractionResult {
                    text,
                    engine: "onnx-ocr".to_string(),
                    warnings: Vec::new(),
                    metadata: serde_json::json!({
                        "mode": "image_ocr",
                        "average_confidence": ocr_output.average_confidence,
                        "region_count": ocr_output.regions.len(),
                        "image_width": ocr_output.image_width,
                        "image_height": ocr_output.image_height,
                        "resized": ocr_output.resized,
                        "regions": ocr_output.regions
                    }),
                })
            }
            "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" => {
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "extracting-foundation",
                        "record_id": id,
                        "step": "Sampling video keyframes and performing local ONNX OCR on HUD / telemetry overlays..."
                    }),
                );

                extract_video_keyframes_ocr(app, &self.ocr, id, path, record).await
            }
            "mp3" | "wav" | "m4a" | "aac" | "ogg" | "flac" | "opus" => {
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "extracting-foundation",
                        "record_id": id,
                        "step": "Indexing audio record disclosure metadata..."
                    }),
                );

                Ok(TextExtractionResult {
                    text: media_record_text(record),
                    engine: "disclosure-metadata".to_string(),
                    warnings: Vec::new(),
                    metadata: serde_json::json!({
                        "mode": "metadata_only",
                        "caveat": "Audio record indexed on official disclosure metadata and DVIDS captions."
                    }),
                })
            }
            _ => {
                if let Ok((text, truncated)) = read_text_file_limited(path, MAX_TEXT_EXTRACTION_BYTES).await {
                    if !text.is_empty() && !text.contains('\0') {
                        let mut warnings = Vec::new();
                        if truncated {
                            let warning = format!(
                                "Text extraction truncated at {} MiB to keep analysis memory bounded.",
                                MAX_TEXT_EXTRACTION_BYTES / 1024 / 1024
                            );
                            emit_analysis_warning(app, id, &warning);
                            warnings.push(warning);
                        }
                        return Ok(TextExtractionResult {
                            text,
                            engine: "generic-text".to_string(),
                            warnings,
                            metadata: serde_json::json!({
                                "mode": "generic_text_fallback",
                                "truncated": truncated
                            }),
                        });
                    }
                }
                Ok(TextExtractionResult {
                    text: media_record_text(record),
                    engine: "disclosure-metadata".to_string(),
                    warnings: Vec::new(),
                    metadata: serde_json::json!({
                        "mode": "metadata_only",
                        "caveat": format!("Generic fallback for file extension `.{}`", extension)
                    }),
                })
            }
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

fn is_reliable_pdf_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.chars().count() < 30 {
        return false;
    }

    let total = trimmed.chars().count().max(1) as f32;
    let alpha_numeric = trimmed.chars().filter(|c| c.is_alphanumeric()).count() as f32;
    let replacement = trimmed.matches('\u{fffd}').count() as f32;
    let control = trimmed
        .chars()
        .filter(|c| c.is_control() && *c != '\n' && *c != '\t' && *c != '\r')
        .count() as f32;
    let words = trimmed.split_whitespace().count();
    let unique_chars = trimmed
        .chars()
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    alpha_numeric / total >= 0.35
        && replacement / total <= 0.01
        && control / total <= 0.01
        && words >= 5
        && unique_chars >= 10
}

async fn extract_video_keyframes_ocr(
    app: &AppHandle,
    ocr: &OcrEngine,
    _id: &str,
    video_path: &Path,
    record: &Record,
) -> Result<TextExtractionResult> {
    let temp_dir = std::env::temp_dir().join(format!("pursue-video-ocr-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await?;

    let duration = probe_video_duration(video_path).await.unwrap_or(30.0);
    let offsets = [
        (duration * 0.05).clamp(0.5, 2.0),
        duration * 0.25,
        duration * 0.50,
        duration * 0.75,
        (duration * 0.95).min(duration - 0.5),
    ];
    let timestamps: Vec<String> = offsets
        .iter()
        .map(|s| {
            let total = (*s as u64).max(0);
            let hrs = total / 3600;
            let mins = (total % 3600) / 60;
            let secs = total % 60;
            format!("{:02}:{:02}:{:02}", hrs, mins, secs)
        })
        .collect();

    let mut extracted_frame_texts = Vec::new();
    let mut frame_metadata = Vec::new();

    for (idx, ts) in timestamps.iter().enumerate() {
        let frame_path = temp_dir.join(format!("frame_{idx}.png"));
        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.arg("-ss")
            .arg(ts)
            .arg("-i")
            .arg(video_path)
            .arg("-vframes")
            .arg("1")
            .arg("-f")
            .arg("image2")
            .arg("-y")
            .arg(&frame_path);

        if let Ok(output) = crate::common::hide_console(&mut cmd).output().await {
            if output.status.success() && frame_path.exists() {
                if let Ok(img) = image::open(&frame_path) {
                    if let Ok(ocr_output) = ocr.extract_structured(app, &img).await {
                        if !ocr_output.text.trim().is_empty() {
                            extracted_frame_texts
                                .push(format!("[Video Frame at {ts}]:\n{}", ocr_output.text.trim()));
                            frame_metadata.push(serde_json::json!({
                                "timestamp": ts,
                                "confidence": ocr_output.average_confidence,
                                "regions_found": ocr_output.regions.len()
                            }));
                        }
                    }
                }
            }
        }
    }

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    let mut combined_text = media_record_text(record);
    if !extracted_frame_texts.is_empty() {
        combined_text.push_str("\n\n--- EXTRACTED VIDEO KEYFRAME TEXT (HUD / TELEMETRY / OVERLAY) ---\n");
        combined_text.push_str(&extracted_frame_texts.join("\n\n"));

        Ok(TextExtractionResult {
            text: combined_text,
            engine: "video-keyframe-ocr".to_string(),
            warnings: Vec::new(),
            metadata: serde_json::json!({
                "mode": "video_keyframe_ocr",
                "keyframes_analyzed": frame_metadata.len(),
                "keyframe_details": frame_metadata
            }),
        })
    } else {
        Ok(TextExtractionResult {
            text: combined_text,
            engine: "disclosure-metadata".to_string(),
            warnings: Vec::new(),
            metadata: serde_json::json!({
                "mode": "metadata_only",
                "caveat": "Video analyzed; no burned-in text/HUD overlays detected on sampled keyframes."
            }),
        })
    }
}

async fn probe_video_duration(video_path: &Path) -> Option<f64> {
    let mut cmd = tokio::process::Command::new("ffprobe");
    cmd.arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(video_path);

    if let Ok(output) = crate::common::hide_console(&mut cmd).output().await {
        if output.status.success() {
            let str_out = String::from_utf8_lossy(&output.stdout);
            if let Ok(sec) = str_out.trim().parse::<f64>() {
                if sec > 0.0 {
                    return Some(sec);
                }
            }
        }
    }
    None
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

    #[test]
    fn pdf_text_reliability_rejects_short_or_garbled_layers() {
        assert!(!is_reliable_pdf_text("abc"));
        assert!(!is_reliable_pdf_text(
            "\u{fffd}\u{fffd}\u{fffd}       x x x x"
        ));
        assert!(is_reliable_pdf_text(
            "This is a normal born digital PDF text layer with several searchable words."
        ));
    }
}
