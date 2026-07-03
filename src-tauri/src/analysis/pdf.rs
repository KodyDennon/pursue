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

    pub fn extract_forensics<P: AsRef<Path>>(&self, path: P) -> Result<Vec<ForensicDiscovery>> {
        let doc = Document::load(path)?;
        let mut discoveries = Vec::new();

        for (page_idx, (_page_num, object_id)) in doc.get_pages().iter().enumerate() {
            let page_number = (page_idx + 1) as u32;
            let page_obj = doc.get_object(*object_id)?;
            let page_dict = page_obj.as_dict()?;

            // Get MediaBox for coordinate scaling
            let media_box = page_dict
                .get(b"MediaBox")
                .ok()
                .and_then(|o| o.as_array().ok())
                .map(|a| {
                    (
                        a[2].as_f32().unwrap_or(1000.0),
                        a[3].as_f32().unwrap_or(1414.0),
                    )
                })
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
                                    "cm" | "Tm" if operation.operands.len() >= 6 => {
                                        // Update current matrix (simplified)
                                        current_matrix[4] =
                                            operation.operands[4].as_f32().unwrap_or(0.0);
                                        current_matrix[5] =
                                            operation.operands[5].as_f32().unwrap_or(0.0);
                                    }
                                    "cm" | "Tm" => {}
                                    "Tj" | "TJ" => {
                                        for operand in operation.operands {
                                            if let Ok(s) = operand.as_str() {
                                                stream_text.push_str(&String::from_utf8_lossy(s));
                                            }
                                        }
                                    }
                                    "re" if operation.operands.len() >= 4 => {
                                        // Rectangle - potential redaction
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
                                                content: format!(
                                                    "Graphic Overlay @ Page {}",
                                                    page_number
                                                ),
                                                confidence: 0.9,
                                                metadata: serde_json::json!({
                                                    "bbox": [nx, ny, nw, nh],
                                                    "page": page_number
                                                }),
                                            });
                                        }
                                    }
                                    "re" => {}
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
                            content: format!(
                                "{}: {}",
                                String::from_utf8_lossy(key),
                                String::from_utf8_lossy(s)
                            ),
                            confidence: 1.0,
                            metadata: serde_json::json!({ "key": String::from_utf8_lossy(key) }),
                        });
                    }
                }
            }
        }

        Ok(discoveries)
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
                            if let Ok(decompressed) =
                                miniz_oxide::inflate::decompress_to_vec_zlib(&data)
                            {
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

    pub async fn render_pdf_to_images<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Vec<image::DynamicImage>> {
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "macos")]
            {
                render_pdf_to_images_macos(&path)
            }
            #[cfg(target_os = "windows")]
            {
                render_pdf_to_images_windows(&path)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                render_pdf_to_images_linux(&path)
            }
        })
        .await?
    }
}

#[cfg(target_os = "macos")]
fn render_pdf_to_images_macos(path: &Path) -> Result<Vec<image::DynamicImage>> {
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSSize, NSString, NSURL};
    use objc2_pdf_kit::{PDFDisplayBox, PDFDocument};

    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid path"))?;
    let ns_path = NSString::from_str(path_str);
    let ns_url = NSURL::fileURLWithPath(&ns_path);

    let ns_doc = unsafe {
        let alloc = PDFDocument::alloc();
        PDFDocument::initWithURL(alloc, &ns_url)
    }
    .ok_or_else(|| anyhow::anyhow!("Failed to load PDF document"))?;

    let page_count = unsafe { ns_doc.pageCount() };
    let mut images = Vec::new();

    for i in 0..page_count {
        let page = unsafe { ns_doc.pageAtIndex(i) }
            .ok_or_else(|| anyhow::anyhow!("Failed to get page {}", i))?;

        let bounds = unsafe { page.boundsForBox(PDFDisplayBox::MediaBox) };
        let scale = 3.0; // 3.0x scale
        let width = bounds.size.width * scale;
        let height = bounds.size.height * scale;

        let ns_size = NSSize::new(width, height);

        let ns_image = unsafe { page.thumbnailOfSize_forBox(ns_size, PDFDisplayBox::MediaBox) };

        let tiff_data: Option<Retained<objc2_foundation::NSData>> =
            unsafe { objc2::msg_send![&ns_image, TIFFRepresentation] };

        let tiff_data = tiff_data
            .ok_or_else(|| anyhow::anyhow!("Failed to get TIFF representation for page {}", i))?;

        let bytes = tiff_data.to_vec();

        let img = image::load_from_memory(&bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse TIFF bytes for page {}: {}", i, e))?;

        images.push(img);
    }

    Ok(images)
}

#[cfg(target_os = "windows")]
fn render_pdf_to_images_windows(path: &Path) -> Result<Vec<image::DynamicImage>> {
    use windows::core::HSTRING;
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};

    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid path"))?;
    let input_hstring = HSTRING::from(path_str);

    let file = StorageFile::GetFileFromPathAsync(&input_hstring)?.get()?;
    let pdf = PdfDocument::LoadFromFileAsync(&file)?.get()?;
    let page_count = pdf.PageCount()?;
    let mut images = Vec::new();

    for i in 0..page_count {
        let page = pdf.GetPage(i)?;
        let size = page.Size()?;

        let options = PdfPageRenderOptions::new()?;
        let dest_width = (size.Width * 3.0) as u32;
        options.SetDestinationWidth(dest_width)?;

        let stream = InMemoryRandomAccessStream::new()?;
        page.RenderWithOptionsToStreamAsync(&stream, &options)?
            .get()?;

        let size = stream.Size()? as u32;
        let input_stream = stream.GetInputStreamAt(0)?;
        let reader = DataReader::CreateDataReader(&input_stream)?;
        reader.LoadAsync(size)?.get()?;

        let mut buffer = vec![0u8; size as usize];
        reader.ReadBytes(&mut buffer)?;

        let img = image::load_from_memory(&buffer).map_err(|e| {
            anyhow::anyhow!("Failed to parse WinRT image bytes for page {}: {}", i, e)
        })?;
        images.push(img);
    }

    Ok(images)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn render_pdf_to_images_linux(path: &Path) -> Result<Vec<image::DynamicImage>> {
    let render_dir = std::env::temp_dir().join(format!("pursue-pdf-render-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&render_dir)?;
    let result = render_pdf_to_images_via_poppler(path, &render_dir);
    let _ = std::fs::remove_dir_all(&render_dir);
    result
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn render_pdf_to_images_via_poppler(
    path: &Path,
    render_dir: &Path,
) -> Result<Vec<image::DynamicImage>> {
    let out_prefix = render_dir.join("page");
    let status = std::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg("200")
        .arg(path)
        .arg(&out_prefix)
        .status()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to run pdftoppm (install poppler-utils, e.g. `apt install poppler-utils`): {e}"
            )
        })?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "pdftoppm exited with status {status} while rendering {}",
            path.display()
        ));
    }

    let mut page_files: Vec<std::path::PathBuf> = std::fs::read_dir(render_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();
    page_files.sort();

    if page_files.is_empty() {
        return Err(anyhow::anyhow!(
            "pdftoppm produced no pages for {}",
            path.display()
        ));
    }

    page_files
        .iter()
        .map(|page_path| {
            image::open(page_path)
                .map_err(|e| anyhow::anyhow!("failed to load rendered page {page_path:?}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_render_pdf_to_images() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../test.pdf");
        if !path.exists() {
            panic!("test.pdf not found at {:?}", path);
        }

        let analyzer = PdfAnalyzer::new();
        let images = analyzer.render_pdf_to_images(&path).await.unwrap();
        assert!(!images.is_empty(), "Should render at least one page");
        for (idx, img) in images.iter().enumerate() {
            assert!(img.width() > 0, "Page {} width should be > 0", idx);
            assert!(img.height() > 0, "Page {} height should be > 0", idx);
        }
    }
}
