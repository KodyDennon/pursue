use anyhow::{anyhow, Result};
use std::path::Path;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use objc2::rc::Retained;
    use objc2::msg_send;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequestTextRecognitionLevel, VNRequest,
    };
    use objc2_pdf_kit::{PDFDocument, PDFPage};
    use objc2_app_kit::NSImage;
    use objc2_core_graphics::CGImage;
    use objc2::runtime::AnyObject;

    pub fn extract_text(path: &Path) -> Result<String> {
        objc2::rc::autoreleasepool(|_| {
            let path_str = path.to_str().ok_or_else(|| anyhow!("invalid path"))?;
            let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

            if extension == "pdf" {
                extract_pdf_text(&url)
            } else {
                extract_image_text(&url)
            }
        })
    }

    fn extract_image_text(url: &Retained<NSURL>) -> Result<String> {
        let handler = unsafe {
            VNImageRequestHandler::initWithURL_options(
                VNImageRequestHandler::alloc(),
                url,
                &NSDictionary::new(),
            )
        };

        perform_vision_ocr(&handler)
    }

    fn extract_pdf_text(url: &Retained<NSURL>) -> Result<String> {
        let doc = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), url) }
            .ok_or_else(|| anyhow!("Failed to load PDF document"))?;

        let count = unsafe { doc.pageCount() };
        let mut full_text = String::new();

        for i in 0..count {
            // Process each page in a fresh autorelease pool to keep memory tight
            objc2::rc::autoreleasepool(|_| {
                let page = unsafe { doc.pageAtIndex(i) };
                if let Some(p) = page {
                    if let Ok(page_text) = extract_page_text(&p) {
                        full_text.push_str(&page_text);
                        full_text.push_str("\n--- PAGE BREAK ---\n");
                    }
                }
            });
        }
        Ok(full_text)
    }

    fn extract_page_text(page: &PDFPage) -> Result<String> {
        // 1. Render page to high-res image (4x scale for forensic clarity)
        let box_rect: [f64; 4] = unsafe { msg_send![&*page, boundsForBox: 0] }; // 0 = kPDFDisplayBoxMediaBox
        let size = [box_rect[2] * 4.0, box_rect[3] * 4.0];
        
        // PDFPage thumbnailOfSize:forBox: returns an autoreleased NSImage (+0)
        let ns_image: *mut NSImage = unsafe { msg_send![&*page, thumbnailOfSize: size, forBox: 0] };
        if ns_image.is_null() { return Err(anyhow!("Failed to render page")); }
        
        // Take ownership of the autoreleased object
        let ns_image = unsafe { Retained::retain_autoreleased(ns_image) }.ok_or_else(|| anyhow!("Failed to retain image"))?;

        // 2. Convert to CGImage
        let cg_image_ptr: *mut CGImage = unsafe { msg_send![&*ns_image, CGImageForProposedRect: 0, context: 0, hints: 0] };
        if cg_image_ptr.is_null() { return Err(anyhow!("Failed to get CGImage")); }
        let cg_image = unsafe { &*cg_image_ptr };

        // 3. Perform OCR
        let handler = unsafe {
            VNImageRequestHandler::initWithCGImage_options(
                VNImageRequestHandler::alloc(),
                cg_image,
                &NSDictionary::new()
            )
        };

        perform_vision_ocr(&handler)
    }

    fn perform_vision_ocr(handler: &VNImageRequestHandler) -> Result<String> {
        unsafe {
            // 2026 Strategy: Check for the structured 'VNRecognizeDocumentsRequest' first
            let cls_name = std::ffi::CStr::from_bytes_with_nul(b"VNRecognizeDocumentsRequest\0").unwrap();
            let structured_cls = objc2::runtime::AnyClass::get(cls_name);
            
            if let Some(cls) = structured_cls {
                let request: *mut VNRequest = msg_send![cls, alloc];
                let request: *mut VNRequest = msg_send![request, init];
                if !request.is_null() {
                    let request = Retained::from_raw(request).unwrap();
                    
                    let requests = NSArray::from_slice(&[&*request]);
                    let _: bool = msg_send![handler, performRequests: &*requests, error: 0];
                    
                    let results: *mut NSArray<AnyObject> = msg_send![&*request, results];
                    if !results.is_null() {
                        let results = &*results;
                        let mut full_text = String::new();
                        for i in 0..results.count() {
                            let obs = results.objectAtIndex(i);
                            // In 2026, DocumentObservation has a 'transcript' property
                            let transcript: *mut NSString = msg_send![&*obs, transcript];
                            if !transcript.is_null() {
                                full_text.push_str(&(*transcript).to_string());
                                full_text.push('\n');
                            }
                        }
                        if !full_text.is_empty() {
                            return Ok(full_text);
                        }
                    }
                }
            }

            // Fallback to stable RecognizeTextRequest
            let request = VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc());
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            request.setUsesLanguageCorrection(true);

            let requests = NSArray::from_slice(&[&*Retained::cast_unchecked::<VNRequest>(request.clone())]);
            let _: bool = msg_send![handler, performRequests: &*requests, error: 0];

            let results = request.results().ok_or_else(|| anyhow!("no results"))?;
            let mut full_text = String::new();

            for i in 0..results.count() {
                let observation = results.objectAtIndex(i);
                let candidates = observation.topCandidates(1);
                if candidates.count() > 0 {
                    let text = candidates.objectAtIndex(0).string();
                    full_text.push_str(&text.to_string());
                    full_text.push('\n');
                }
            }

            Ok(full_text)
        }
    }
}

#[cfg(target_os = "macos")]
pub async fn extract_text_macos<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref().to_path_buf();
    tokio::task::spawn_blocking(move || macos_impl::extract_text(&path)).await?
}
