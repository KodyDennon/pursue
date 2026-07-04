use anyhow::Result;
use lopdf::Document;
use std::path::Path;

const DEFAULT_PDF_RENDER_SCALE: f64 = 3.0;
const DEFAULT_MAX_RENDERED_PAGE_PIXELS: u64 = 24_000_000;
const MAX_EXTRACTED_IMAGE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct PdfRenderOptions {
    pub preferred_scale: f64,
    pub max_pixels: u64,
}

impl Default for PdfRenderOptions {
    fn default() -> Self {
        Self {
            preferred_scale: DEFAULT_PDF_RENDER_SCALE,
            max_pixels: DEFAULT_MAX_RENDERED_PAGE_PIXELS,
        }
    }
}

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

    pub fn page_count<P: AsRef<Path>>(&self, path: P) -> Result<usize> {
        let doc = Document::load(path)?;
        Ok(doc.get_pages().len())
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

                    if data.len() < 1024 || data.len() > MAX_EXTRACTED_IMAGE_BYTES {
                        continue;
                    } // Skip icons/small assets and oversized decompressed images.

                    let filename = format!("img_{}_{}.{}", object_id.0, object_id.1, extension);
                    let file_path = output_dir.join(&filename);
                    std::fs::write(&file_path, data)?;

                    extracted.push((filename, format!("image/{}", extension)));
                }
            }
        }

        Ok(extracted)
    }

    pub async fn render_page<P: AsRef<Path>>(
        &self,
        path: P,
        page_index: usize,
        options: PdfRenderOptions,
    ) -> Result<image::DynamicImage> {
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || {
            #[cfg(target_os = "macos")]
            {
                render_pdf_page_macos(&path, page_index, options)
            }
            #[cfg(target_os = "windows")]
            {
                render_pdf_page_windows(&path, page_index, options)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                render_pdf_page_linux(&path, page_index, options)
            }
        })
        .await?
    }
}

pub fn bounded_render_scale(
    width_points: f64,
    height_points: f64,
    options: PdfRenderOptions,
) -> f64 {
    let preferred = if options.preferred_scale.is_finite() && options.preferred_scale > 0.0 {
        options.preferred_scale
    } else {
        DEFAULT_PDF_RENDER_SCALE
    };
    let max_pixels = options.max_pixels.max(1) as f64;
    let base_pixels = width_points.max(1.0) * height_points.max(1.0);
    let preferred_pixels = base_pixels * preferred * preferred;
    if preferred_pixels <= max_pixels {
        return preferred;
    }

    (max_pixels / base_pixels).sqrt().clamp(0.1, preferred)
}

fn page_dimensions_points(path: &Path, page_index: usize) -> Result<(f64, f64)> {
    let doc = Document::load(path)?;
    let pages = doc.get_pages();
    let object_id = pages
        .values()
        .nth(page_index)
        .ok_or_else(|| anyhow::anyhow!("PDF page index {} out of range", page_index))?;
    let page_obj = doc.get_object(*object_id)?;
    let page_dict = page_obj.as_dict()?;
    let media_box = page_dict
        .get(b"MediaBox")
        .ok()
        .and_then(|o| o.as_array().ok())
        .ok_or_else(|| anyhow::anyhow!("PDF page {} has no MediaBox", page_index + 1))?;
    if media_box.len() < 4 {
        return Err(anyhow::anyhow!(
            "PDF page {} has an invalid MediaBox",
            page_index + 1
        ));
    }
    let x0 = media_box[0].as_f32().unwrap_or(0.0) as f64;
    let y0 = media_box[1].as_f32().unwrap_or(0.0) as f64;
    let x1 = media_box[2].as_f32().unwrap_or(612.0) as f64;
    let y1 = media_box[3].as_f32().unwrap_or(792.0) as f64;
    Ok(((x1 - x0).abs().max(1.0), (y1 - y0).abs().max(1.0)))
}

#[cfg(target_os = "macos")]
fn render_pdf_page_macos(
    path: &Path,
    page_index: usize,
    options: PdfRenderOptions,
) -> Result<image::DynamicImage> {
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::AnyThread;
    use objc2_foundation::{NSSize, NSString, NSURL};
    use objc2_pdf_kit::{PDFDisplayBox, PDFDocument};

    autoreleasepool(|_| {
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
        if page_index >= page_count {
            return Err(anyhow::anyhow!(
                "PDF page index {} out of range ({} pages)",
                page_index,
                page_count
            ));
        }

        let page = unsafe { ns_doc.pageAtIndex(page_index) }
            .ok_or_else(|| anyhow::anyhow!("Failed to get page {}", page_index))?;

        let bounds = unsafe { page.boundsForBox(PDFDisplayBox::MediaBox) };
        let scale = bounded_render_scale(bounds.size.width, bounds.size.height, options);
        let width = bounds.size.width * scale;
        let height = bounds.size.height * scale;

        let ns_size = NSSize::new(width, height);

        let ns_image = unsafe { page.thumbnailOfSize_forBox(ns_size, PDFDisplayBox::MediaBox) };

        let tiff_data: Option<Retained<objc2_foundation::NSData>> =
            unsafe { objc2::msg_send![&ns_image, TIFFRepresentation] };

        let tiff_data = tiff_data.ok_or_else(|| {
            anyhow::anyhow!("Failed to get TIFF representation for page {}", page_index)
        })?;

        let bytes = tiff_data.to_vec();

        image::load_from_memory(&bytes).map_err(|e| {
            anyhow::anyhow!("Failed to parse TIFF bytes for page {}: {}", page_index, e)
        })
    })
}

#[cfg(target_os = "windows")]
fn render_pdf_page_windows(
    path: &Path,
    page_index: usize,
    options: PdfRenderOptions,
) -> Result<image::DynamicImage> {
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
    let page_count = pdf.PageCount()? as usize;
    if page_index >= page_count {
        return Err(anyhow::anyhow!(
            "PDF page index {} out of range ({} pages)",
            page_index,
            page_count
        ));
    }
    let page = pdf.GetPage(page_index as u32)?;
    let size = page.Size()?;

    let render_options = PdfPageRenderOptions::new()?;
    let width_points = f64::from(size.Width);
    let height_points = f64::from(size.Height);
    let scale = bounded_render_scale(width_points, height_points, options);
    let dest_width = (width_points * scale).round().max(1.0) as u32;
    render_options.SetDestinationWidth(dest_width)?;

    let stream = InMemoryRandomAccessStream::new()?;
    page.RenderWithOptionsToStreamAsync(&stream, &render_options)?
        .get()?;

    let size = stream.Size()? as u32;
    let input_stream = stream.GetInputStreamAt(0)?;
    let reader = DataReader::CreateDataReader(&input_stream)?;
    reader.LoadAsync(size)?.get()?;

    let mut buffer = vec![0u8; size as usize];
    reader.ReadBytes(&mut buffer)?;

    image::load_from_memory(&buffer).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse WinRT image bytes for page {}: {}",
            page_index,
            e
        )
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn render_pdf_page_linux(
    path: &Path,
    page_index: usize,
    options: PdfRenderOptions,
) -> Result<image::DynamicImage> {
    let render_dir =
        std::env::temp_dir().join(format!("pursue-pdf-render-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&render_dir)?;
    let result = render_pdf_page_via_poppler(path, page_index, options, &render_dir);
    let _ = std::fs::remove_dir_all(&render_dir);
    result
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn render_pdf_page_via_poppler(
    path: &Path,
    page_index: usize,
    options: PdfRenderOptions,
    render_dir: &Path,
) -> Result<image::DynamicImage> {
    let (width, height) = page_dimensions_points(path, page_index)?;
    let scale = bounded_render_scale(width, height, options);
    let dpi = (72.0 * scale).round().clamp(24.0, 300.0).to_string();
    let page_number = (page_index + 1).to_string();
    let out_prefix = render_dir.join("page");
    let status = std::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-f")
        .arg(&page_number)
        .arg("-l")
        .arg(&page_number)
        .arg("-r")
        .arg(dpi)
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
            "pdftoppm produced no page {} for {}",
            page_index + 1,
            path.display()
        ));
    }

    image::open(&page_files[0])
        .map_err(|e| anyhow::anyhow!("failed to load rendered page {:?}: {e}", page_files[0]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_render_pdf_page() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../test.pdf");
        if !path.exists() {
            panic!("test.pdf not found at {:?}", path);
        }

        let analyzer = PdfAnalyzer::new();
        assert!(analyzer.page_count(&path).unwrap() > 0);
        let img = analyzer
            .render_page(&path, 0, PdfRenderOptions::default())
            .await
            .unwrap();
        assert!(img.width() > 0, "Page width should be > 0");
        assert!(img.height() > 0, "Page height should be > 0");
    }

    #[test]
    fn bounded_scale_preserves_preferred_scale_for_normal_pages() {
        let scale = bounded_render_scale(612.0, 792.0, PdfRenderOptions::default());
        assert_eq!(scale, DEFAULT_PDF_RENDER_SCALE);
    }

    #[test]
    fn bounded_scale_reduces_oversized_pages() {
        let scale = bounded_render_scale(
            20_000.0,
            20_000.0,
            PdfRenderOptions {
                preferred_scale: 3.0,
                max_pixels: 24_000_000,
            },
        );
        assert!(scale < 3.0);
    }
}
