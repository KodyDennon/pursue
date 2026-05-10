use anyhow::{Context, Result};
use lopdf::Document;
use std::path::Path;
use tokio::process::Command;

pub struct ForensicDiscovery {
    pub layer_type: String,
    pub content: String,
    pub confidence: f32,
    pub metadata: serde_json::Value,
}

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

    pub fn extract_forensics<P: AsRef<Path>>(&self, path: P) -> Result<Vec<ForensicDiscovery>> {
        let doc = Document::load(path)?;
        let mut discoveries = Vec::new();

        // 1. Check for Hidden Text Layers (Text objects not in the primary visible stream)
        // Forensic heuristic: look for text operations in all streams/XObjects
        for (object_id, object) in doc.objects.iter() {
            if let Ok(stream) = object.as_stream() {
                if let Ok(content) = stream.decode_content() {
                    let mut stream_text = String::new();
                    for operation in content.operations {
                        if operation.operator == "Tj" || operation.operator == "TJ" {
                            for operand in operation.operands {
                                if let Ok(s) = operand.as_str() {
                                    stream_text.push_str(&String::from_utf8_lossy(s));
                                }
                            }
                        }
                    }
                    
                    if !stream_text.trim().is_empty() && stream_text.len() > 5 {
                        discoveries.push(ForensicDiscovery {
                            layer_type: "hidden_text".to_string(),
                            content: stream_text.trim().to_string(),
                            confidence: 0.8,
                            metadata: serde_json::json!({ "object_id": format!("{}_{}", object_id.0, object_id.1) }),
                        });
                    }
                }
            }
        }

        // 2. Metadata Deep Dive
        if let Ok(info_ref) = doc.trailer.get(b"Info").and_then(|obj| obj.as_reference()) {
            if let Ok(info) = doc.get_dict(info_ref) {
                for (key, value) in info.iter() {
                    if let Ok(s) = value.as_str() {
                        discoveries.push(ForensicDiscovery {
                            layer_type: "metadata_leak".to_string(),
                            content: format!("{}: {}", String::from_utf8_lossy(key), String::from_utf8_lossy(s)),
                            confidence: 1.0,
                            metadata: serde_json::json!({ "key": String::from_utf8_lossy(key) }),
                        });
                    }
                }
            }
        }

        Ok(discoveries)
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
                    let mut data = stream.content.clone();

                    if let Ok(filter) = dict.get(b"Filter").and_then(|f| f.as_name()) {
                        if filter == b"FlateDecode" {
                            if let Ok(decompressed) = miniz_oxide::inflate::decompress_to_vec_zlib(&data) {
                                data = decompressed;
                            }
                        }
                    }

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
