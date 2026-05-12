use anyhow::{anyhow, Result};
use std::path::Path;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSImage;
    use objc2_core_graphics::CGImage;
    use objc2_foundation::{NSArray, NSDictionary, NSObject, NSString, NSURL};
    use objc2_pdf_kit::{PDFDocument, PDFPage};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest};

    pub fn extract_text(path: &Path) -> Result<String> {
        if !path.exists() {
            return Err(anyhow!("File does not exist: {}", path.display()));
        }

        objc2::rc::autoreleasepool(|_| {
            let path_str = path.to_str().ok_or_else(|| anyhow!("invalid path"))?;
            let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if extension == "pdf" {
                extract_pdf_text(&url)
            } else {
                extract_image_text(&url)
            }
        })
    }

    fn extract_image_text(url: &Retained<NSURL>) -> Result<String> {
        unsafe {
            let cls = objc2::runtime::AnyClass::get(c"VNImageRequestHandler").unwrap();
            let handler: *mut VNImageRequestHandler = msg_send![cls, alloc];
            let options = NSDictionary::<NSObject, NSObject>::new();
            let handler: *mut VNImageRequestHandler =
                msg_send![handler, initWithURL: &**url, options: &*options];
            let handler = Retained::from_raw(handler).unwrap();
            perform_vision_ocr(&handler)
        }
    }

    fn extract_pdf_text(url: &Retained<NSURL>) -> Result<String> {
        unsafe {
            let cls = objc2::runtime::AnyClass::get(c"PDFDocument").unwrap();
            let doc: *mut PDFDocument = msg_send![cls, alloc];
            let doc: *mut PDFDocument = msg_send![doc, initWithURL: &**url];
            if doc.is_null() {
                return Err(anyhow!("Failed to load PDF document"));
            }
            let doc = Retained::from_raw(doc).unwrap();

            let count: isize = msg_send![&*doc, pageCount];
            let mut full_text = String::new();

            for i in 0..count {
                objc2::rc::autoreleasepool(|_| {
                    let page: *mut PDFPage = msg_send![&*doc, pageAtIndex: i];
                    if let Some(p) = page.as_ref() {
                        if let Ok(page_text) = extract_page_text(p) {
                            full_text.push_str(&page_text);
                            full_text.push_str("\n--- PAGE BREAK ---\n");
                        }
                    }
                });
            }
            Ok(full_text)
        }
    }

    fn extract_page_text(page: &PDFPage) -> Result<String> {
        unsafe {
            let box_rect: [f64; 4] = msg_send![page, boundsForBox: 0]; // 0 = kPDFDisplayBoxMediaBox
            let size = [box_rect[2] * 4.0, box_rect[3] * 4.0];

            let ns_image: *mut NSImage = msg_send![page, thumbnailOfSize: size, forBox: 0];
            if ns_image.is_null() {
                return Err(anyhow!("Failed to render page"));
            }
            let ns_image = Retained::retain_autoreleased(ns_image)
                .ok_or_else(|| anyhow!("Failed to retain image"))?;

            let cg_image: *mut CGImage =
                msg_send![&*ns_image, CGImageForProposedRect: 0, context: 0, hints: 0];
            if cg_image.is_null() {
                return Err(anyhow!("Failed to get CGImage"));
            }

            let cls = objc2::runtime::AnyClass::get(c"VNImageRequestHandler").unwrap();
            let handler: *mut VNImageRequestHandler = msg_send![cls, alloc];
            let options = NSDictionary::<NSObject, NSObject>::new();
            let handler: *mut VNImageRequestHandler =
                msg_send![handler, initWithCGImage: cg_image, options: &*options];
            let handler = Retained::from_raw(handler).unwrap();

            perform_vision_ocr(&handler)
        }
    }

    fn perform_vision_ocr(handler: &VNImageRequestHandler) -> Result<String> {
        unsafe {
            let cls = objc2::runtime::AnyClass::get(c"VNRecognizeTextRequest").unwrap();
            let request: *mut VNRecognizeTextRequest = msg_send![cls, alloc];
            let request: *mut VNRecognizeTextRequest = msg_send![request, init];
            let request = Retained::from_raw(request).unwrap();

            let _: () = msg_send![&*request, setRecognitionLevel: 1]; // 1 = Accurate
            let _: () = msg_send![&*request, setUsesLanguageCorrection: true];

            let requests =
                NSArray::from_slice(&[&*Retained::cast_unchecked::<VNRequest>(request.clone())]);

            let mut error: *mut objc2_foundation::NSError = std::ptr::null_mut();
            let success: bool = msg_send![handler, performRequests: &*requests, error: &mut error];

            if !success {
                return Err(anyhow!("Vision request failed"));
            }

            let results: *mut NSArray<AnyObject> = msg_send![&*request, results];
            if results.is_null() {
                return Err(anyhow!("no results"));
            }
            let results = &*results;

            let mut full_text = String::new();

            for i in 0..results.count() {
                let obs = results.objectAtIndex(i);
                let candidates: *mut NSArray<AnyObject> = msg_send![&*obs, topCandidates: 1];
                if !candidates.is_null() && (*candidates).count() > 0 {
                    let top = (*candidates).objectAtIndex(0);
                    let text: *mut NSString = msg_send![&*top, string];
                    full_text.push_str(&(*text).to_string());
                    full_text.push('\n');
                }
            }

            Ok(full_text)
        }
    }
}

pub async fn extract_text_macos<P: AsRef<Path>>(path: P) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || macos_impl::extract_text(&path)).await?
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(anyhow!("macOS Vision OCR is only available on macOS"))
    }
}
