use anyhow::Result;
use lopdf::Document;
use std::path::Path;

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
        // If we have a decent amount of digital text, return it.
        // Otherwise, we return it anyway and let the caller decide if OCR is needed.
        Ok(lopdf_text)
    }

    pub fn extract_forensics<P: AsRef<Path>>(&self, path: P) -> Result<Vec<ForensicDiscovery>> {
        let doc = Document::load(path)?;
        let mut discoveries = Vec::new();

        for (page_idx, (_page_num, object_id)) in doc.get_pages().iter().enumerate() {
            let page_number = (page_idx + 1) as u32;
            let page_obj = doc.get_object(*object_id)?;
            let page_dict = page_obj.as_dict()?;
            
            // Get MediaBox for coordinate scaling
            let media_box = page_dict.get(b"MediaBox")
                .ok()
                .and_then(|o| o.as_array().ok())
                .map(|a| (a[2].as_f32().unwrap_or(1000.0), a[3].as_f32().unwrap_or(1414.0)))
                .unwrap_or((1000.0, 1414.0));

            // 1. Check for Hidden Text Layers & Graphic Overlays
            if let Ok(content_obj_id) = page_dict.get(b"Contents") {
                let contents = if let Ok(arr) = content_obj_id.as_array() {
                    arr.clone()
                } else {
                    vec![content_obj_id.clone()]
                };

                for content_id in contents {
                    let stream_obj_owned;
                    let stream_obj = if let Ok(reference) = content_id.as_reference() {
                        doc.get_object(reference)?
                    } else {
                        stream_obj_owned = content_id.clone();
                        &stream_obj_owned
                    };

                    if let Ok(stream) = stream_obj.as_stream() {
                        if let Ok(content) = stream.decode_content() {
                            let mut current_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                            let mut stream_text = String::new();
                            
                            for operation in content.operations {
                                match operation.operator.as_str() {
                                    "cm" | "Tm" => {
                                        // Update current matrix (simplified)
                                        if operation.operands.len() >= 6 {
                                            current_matrix[4] = operation.operands[4].as_f32().unwrap_or(0.0);
                                            current_matrix[5] = operation.operands[5].as_f32().unwrap_or(0.0);
                                        }
                                    },
                                    "Tj" | "TJ" => {
                                        for operand in operation.operands {
                                            if let Ok(s) = operand.as_str() {
                                                stream_text.push_str(&String::from_utf8_lossy(s));
                                            }
                                        }
                                    },
                                    "re" => {
                                        // Rectangle - potential redaction
                                        if operation.operands.len() >= 4 {
                                            let x = operation.operands[0].as_f32().unwrap_or(0.0);
                                            let y = operation.operands[1].as_f32().unwrap_or(0.0);
                                            let w = operation.operands[2].as_f32().unwrap_or(0.0);
                                            let h = operation.operands[3].as_f32().unwrap_or(0.0);
                                            
                                            // Only log large "redaction-like" boxes
                                            if w > 10.0 && h > 5.0 {
                                                // Normalize to 1000x1414 coordinate system
                                                let nx = (x / media_box.0) * 1000.0;
                                                let ny = 1414.0 - ((y + h) / media_box.1) * 1414.0;
                                                let nw = (w / media_box.0) * 1000.0;
                                                let nh = (h / media_box.1) * 1414.0;

                                                discoveries.push(ForensicDiscovery {
                                                    layer_type: "improper_redaction".to_string(),
                                                    content: format!("Graphic Overlay @ Page {}", page_number),
                                                    confidence: 0.9,
                                                    metadata: serde_json::json!({
                                                        "bbox": [nx, ny, nw, nh],
                                                        "page": page_number
                                                    }),
                                                });
                                            }
                                        }
                                    },
                                    _ => {}
                                }
                            }
                            
                            if !stream_text.trim().is_empty() && stream_text.len() > 5 {
                                discoveries.push(ForensicDiscovery {
                                    layer_type: "hidden_text".to_string(),
                                    content: stream_text.trim().to_string(),
                                    confidence: 0.7,
                                    metadata: serde_json::json!({ "page": page_number }),
                                });
                            }
                        }
                    }
                }
            }
        }

        // 2. Metadata Deep Dive (Trailer Info)
        if let Ok(info_ref) = doc.trailer.get(b"Info").and_then(|obj| obj.as_reference()) {
            if let Ok(info) = doc.get_dictionary(info_ref) {
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
