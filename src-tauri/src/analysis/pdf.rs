use anyhow::Result;
use lopdf::Document;
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use std::path::Path;
use std::sync::OnceLock;

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

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();

impl PdfAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text_pages<P: AsRef<Path>>(&self, path: P) -> Result<Vec<String>> {
        let doc = Document::load(path)?;
        let page_count = doc.get_pages().len();
        let mut pages = Vec::with_capacity(page_count);

        for index in 0..page_count {
            let page_number = (index + 1) as u32;
            pages.push(doc.extract_text(&[page_number]).unwrap_or_default());
        }

        Ok(pages)
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

            // 1. Check for text layers and graphic overlays. These are analyst signals, not
            // proof of hidden content by themselves.
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
                                                layer_type: "redaction_overlay_candidate".to_string(),
                                                content: format!("Large filled rectangle @ Page {}", page_number),
                                                confidence: 0.65,
                                                metadata: serde_json::json!({
                                                    "bbox": [nx, ny, nw, nh],
                                                    "page": page_number,
                                                    "source": "pdf_content_stream",
                                                    "caveat": "Graphic rectangles may be legitimate layout elements; treat as redaction candidates only."
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
                                    layer_type: "pdf_text_layer".to_string(),
                                    content: stream_text.trim().to_string(),
                                    confidence: 0.5,
                                    metadata: serde_json::json!({
                                        "page": page_number,
                                        "source": "pdf_content_stream",
                                        "caveat": "Presence of a text layer is normal for born-digital or OCRed PDFs and is not hidden text without visual corroboration."
                                    }),
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
                    if stream.content.len() < 1024
                        || stream.content.len() > MAX_EXTRACTED_IMAGE_BYTES
                    {
                        continue;
                    }
                    let mut data = stream.content.clone();

                    if let Ok(filter) = dict.get(b"Filter").and_then(|f| f.as_name()) {
                        if filter == b"FlateDecode" {
                            data = match miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                                &data,
                                MAX_EXTRACTED_IMAGE_BYTES + 1,
                            ) {
                                Ok(decompressed)
                                    if decompressed.len() <= MAX_EXTRACTED_IMAGE_BYTES =>
                                {
                                    decompressed
                                }
                                _ => continue,
                            };
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
            let rendered = {
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
            };

            // The platform renderer can refuse individual pages (notably the WinRT/Edge PDF
            // renderer's opaque 0x80048040 on some scanned documents). Scanned pages — the
            // ones OCR actually needs rendered — are usually a single full-page embedded
            // JPEG, so pulling that image straight out of the PDF recovers the page.
            rendered
                .or_else(|native_error| {
                    render_pdf_page_pdfium(&path, page_index, options).map_err(|pdfium_error| {
                        anyhow::anyhow!(
                            "native renderer: {native_error}; PDFium fallback: {pdfium_error}"
                        )
                    })
                })
                .or_else(|render_error| {
                    extract_page_embedded_image(&path, page_index).map_err(|image_error| {
                        anyhow::anyhow!("{render_error}; embedded-image fallback: {image_error}")
                    })
                })
        })
        .await?
    }
}

fn render_pdf_page_pdfium(
    path: &Path,
    page_index: usize,
    options: PdfRenderOptions,
) -> Result<image::DynamicImage> {
    let pdfium = load_pdfium()?;
    // An empty password opens permission-encrypted government PDFs that have no user/open
    // password but disable copying. Retry None for ordinary unencrypted documents.
    let document = pdfium
        .load_pdf_from_file(path, Some(""))
        .or_else(|_| pdfium.load_pdf_from_file(path, None))?;
    let page_index =
        i32::try_from(page_index).map_err(|_| anyhow::anyhow!("PDF page index is too large"))?;
    let page = document.pages().get(page_index)?;
    let width_points = page.width().value as f64;
    let height_points = page.height().value as f64;
    let scale = bounded_render_scale(width_points, height_points, options);
    let target_width = (width_points * scale).round().clamp(1.0, i32::MAX as f64) as i32;
    let render_config = PdfRenderConfig::new()
        .set_target_width(target_width)
        .render_annotations(true)
        .render_form_data(true);

    let bitmap = page.render_with_config(&render_config)?;
    let image = bitmap.as_image()?;
    Ok(image)
}

fn load_pdfium() -> Result<&'static Pdfium> {
    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }

    let library_name = if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    };
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PURSUE_PDFIUM_PATH") {
        let path = std::path::PathBuf::from(path);
        candidates.push(if path.is_dir() {
            path.join(library_name)
        } else {
            path
        });
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join(library_name));
            candidates.push(exe_dir.join("resources/assets/pdfium").join(library_name));
            // macOS bundle: Contents/MacOS/app -> Contents/Resources/assets/pdfium.
            candidates.push(
                exe_dir
                    .join("../Resources/assets/pdfium")
                    .join(library_name),
            );
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets/pdfium").join(library_name));
        candidates.push(cwd.join("src-tauri/assets/pdfium").join(library_name));
    }

    let mut errors = Vec::new();
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => {
                let _ = PDFIUM.set(Pdfium::new(bindings));
                return PDFIUM
                    .get()
                    .ok_or_else(|| anyhow::anyhow!("PDFium initialization raced and was lost"));
            }
            Err(error) => errors.push(format!("{}: {error}", candidate.display())),
        }
    }

    Err(anyhow::anyhow!(
        "bundled PDFium library was not found or could not be loaded{}",
        if errors.is_empty() {
            String::new()
        } else {
            format!(" ({})", errors.join("; "))
        }
    ))
}

/// Last-resort page "render": pull the largest embedded raster image out of the page's
/// XObject resources. Covers scanned documents (one full-page JPEG per page) when the
/// platform PDF renderer refuses the page. Only DCT (JPEG) streams are attempted — other
/// encodings would need full colorspace handling that the real renderer exists for.
fn extract_page_embedded_image(path: &Path, page_index: usize) -> Result<image::DynamicImage> {
    let doc = Document::load(path)?;
    let pages = doc.get_pages();
    let (_, &page_id) = pages
        .iter()
        .nth(page_index)
        .ok_or_else(|| anyhow::anyhow!("page index {} out of range", page_index))?;

    let resolve = |object: &lopdf::Object| -> Result<lopdf::Object> {
        Ok(match object.as_reference() {
            Ok(reference) => doc.get_object(reference)?.clone(),
            Err(_) => object.clone(),
        })
    };

    let page_dict = doc.get_object(page_id)?.as_dict()?.clone();
    let resources = resolve(
        page_dict
            .get(b"Resources")
            .map_err(|_| anyhow::anyhow!("page has no Resources"))?,
    )?;
    let xobjects = resolve(
        resources
            .as_dict()?
            .get(b"XObject")
            .map_err(|_| anyhow::anyhow!("page has no XObject resources"))?,
    )?;

    let mut best_jpeg: Option<Vec<u8>> = None;
    for (_name, value) in xobjects.as_dict()?.iter() {
        let Ok(object) = resolve(value) else { continue };
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(|s| s.as_name()).ok() != Some(b"Image") {
            continue;
        }
        let is_dct = match stream.dict.get(b"Filter") {
            Ok(lopdf::Object::Name(name)) => name == b"DCTDecode",
            Ok(lopdf::Object::Array(filters)) => filters
                .last()
                .and_then(|f| f.as_name().ok())
                .map(|name| name == b"DCTDecode")
                .unwrap_or(false),
            _ => false,
        };
        if !is_dct {
            continue;
        }
        if best_jpeg
            .as_ref()
            .map(|current| stream.content.len() > current.len())
            .unwrap_or(true)
        {
            best_jpeg = Some(stream.content.clone());
        }
    }

    let jpeg = best_jpeg
        .ok_or_else(|| anyhow::anyhow!("page {} has no embedded JPEG image", page_index + 1))?;
    image::load_from_memory(&jpeg)
        .map_err(|e| anyhow::anyhow!("embedded page image failed to decode: {e}"))
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

// Only the poppler-based Linux render path needs to read page dimensions up front;
// macOS and Windows use native renderers that handle sizing themselves.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
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

    // This runs on tokio's blocking-thread pool; WinRT activation requires the thread to
    // have an initialized apartment. Without this, whether a call works depends on which
    // pool thread it lands on, which showed up as per-page 0x8004xxxx render failures.
    init_winrt_apartment();

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

    let width_points = f64::from(size.Width);
    let height_points = f64::from(size.Height);
    let scale = bounded_render_scale(width_points, height_points, options);

    // The WinRT PDF renderer intermittently fails large/complex pages at high resolution
    // (observed 0x80048040 in production on scanned 200+ page documents). Rendering the
    // same page at a lower destination width reliably succeeds, and OCR quality at 1.5x
    // page width is still acceptable — retry downward before giving up on the page.
    // Rasterizing prep is documented to make RenderToStreamAsync more reliable; without it
    // the renderer's D3D device intermittently drops under back-to-back page rendering
    // (observed as 0x80048040 on multi-hundred-page scanned documents). Failure here is
    // non-fatal — rendering may still succeed.
    if let Ok(op) = page.PreparePageAsync() {
        let _ = op.get();
    }

    let mut last_error: Option<anyhow::Error> = None;
    for (attempt, scale_attempt) in [scale, (scale / 2.0).max(1.0), 1.0].into_iter().enumerate() {
        if attempt > 0 {
            // Give a removed/resetting D3D device a moment to come back before retrying.
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let dest_width = (width_points * scale_attempt).round().max(1.0) as u32;
        let render_result = (|| -> windows::core::Result<Vec<u8>> {
            let render_options = PdfPageRenderOptions::new()?;
            render_options.SetDestinationWidth(dest_width)?;
            let stream = InMemoryRandomAccessStream::new()?;
            page.RenderWithOptionsToStreamAsync(&stream, &render_options)?
                .get()?;
            let stream_size = stream.Size()? as u32;
            let input_stream = stream.GetInputStreamAt(0)?;
            let reader = DataReader::CreateDataReader(&input_stream)?;
            reader.LoadAsync(stream_size)?.get()?;
            let mut buffer = vec![0u8; stream_size as usize];
            reader.ReadBytes(&mut buffer)?;
            Ok(buffer)
        })();

        match render_result {
            Ok(buffer) => {
                let _ = page.Close();
                return image::load_from_memory(&buffer).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to parse WinRT image bytes for page {}: {}",
                        page_index,
                        e
                    )
                });
            }
            Err(error) => {
                log::warn!(
                    "WinRT PDF render failed for page {} at width {} ({}); retrying lower",
                    page_index + 1,
                    dest_width,
                    error
                );
                last_error = Some(error.into());
            }
        }
    }

    let _ = page.Close();
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("WinRT PDF render failed")))
}

/// Best-effort per-thread WinRT (MTA) apartment initialization for blocking-pool threads.
#[cfg(target_os = "windows")]
pub(crate) fn init_winrt_apartment() {
    use std::cell::Cell;
    thread_local! {
        static WINRT_INITIALIZED: Cell<bool> = const { Cell::new(false) };
    }
    WINRT_INITIALIZED.with(|initialized| {
        if !initialized.get() {
            // RPC_E_CHANGED_MODE etc. just mean the thread already has an apartment.
            let _ = unsafe {
                windows::Win32::System::WinRT::RoInitialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                )
            };
            initialized.set(true);
        }
    });
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
    fn pdfium_renders_configured_regression_pages() {
        let Some(path) = std::env::var_os("PURSUE_PDF_REGRESSION_PATH") else {
            return;
        };
        let pages = std::env::var("PURSUE_PDF_REGRESSION_PAGES")
            .unwrap_or_else(|_| "1".to_string())
            .split(',')
            .map(|page| page.trim().parse::<usize>().expect("valid 1-based page"))
            .collect::<Vec<_>>();

        for page in pages {
            let image = render_pdf_page_pdfium(
                Path::new(&path),
                page.checked_sub(1).expect("pages are 1-based"),
                PdfRenderOptions::default(),
            )
            .unwrap_or_else(|error| panic!("PDFium failed to render page {page}: {error:#}"));
            assert!(image.width() > 0 && image.height() > 0);
        }
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
