use anyhow::{Context, Result};
use lopdf::Document;
use std::path::Path;
use tokio::process::Command;

pub struct PdfAnalyzer;

impl PdfAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn extract_text<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let path = path.as_ref();
        let lopdf_text = self.extract_with_lopdf(path).unwrap_or_default();
        if lopdf_text.trim().len() > 80 {
            return Ok(lopdf_text);
        }

        if command_available("pdftotext").await {
            let output = Command::new("pdftotext")
                .arg("-layout")
                .arg(path)
                .arg("-")
                .output()
                .await
                .context("failed to start pdftotext")?;
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                if text.trim().len() > lopdf_text.trim().len() {
                    return Ok(text);
                }
            }
        }

        Ok(lopdf_text)
    }

    fn extract_with_lopdf<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let doc = Document::load(path)?;
        let mut text = String::new();

        for (index, _) in doc.get_pages().iter().enumerate() {
            let page_number = (index + 1) as u32;
            if let Ok(page_text) = doc.extract_text(&[page_number]) {
                text.push_str(&page_text);
                text.push('\n');
            }
        }

        Ok(text)
    }
    pub async fn extract_images<P: AsRef<Path>>(
        &self,
        path: P,
        output_dir: &Path,
    ) -> Result<Vec<(String, String)>> {
        let path = path.as_ref();
        let doc = Document::load(path)?;
        let mut extracted = Vec::new();

        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        for (object_id, object) in doc.objects.iter() {
            if let Ok(dict) = object.as_dict() {
                if dict.get(b"Subtype").and_then(|s| s.as_name()).ok() == Some(b"Image") {
                    let extension = match dict.get(b"Filter").and_then(|f| f.as_name()).ok() {
                        Some(b"DCTDecode") => "jpg",
                        Some(b"JPXDecode") => "jp2",
                        _ => "png",
                    };

                    let stream = doc.get_object(*object_id)?.as_stream()?;
                    let data = stream.content.clone();

                    if data.len() < 1024 {
                        continue;
                    } // Skip icons/small assets

                    let filename = format!("img_{}_{}.{}", object_id.0, object_id.1, extension);
                    let file_path = output_dir.join(&filename);
                    std::fs::write(&file_path, data)?;

                    extracted.push((filename, format!("image/{}", extension)));
                }
            }
        }

        Ok(extracted)
    }
}

async fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("-v")
        .output()
        .await
        .map(|output| output.status.success() || !output.stderr.is_empty())
        .unwrap_or(false)
}
