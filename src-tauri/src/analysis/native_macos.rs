use anyhow::{anyhow, Result};
use std::path::Path;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    // Forward declarations for PDFKit types not in objc2-vision/foundation
    #[link(name = "PDFKit", kind = "framework")]
    extern "C" {}

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
        let request = unsafe { VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc()) };
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);

        let handler = unsafe {
            VNImageRequestHandler::initWithURL_options(
                VNImageRequestHandler::alloc(),
                url,
                &NSDictionary::new(),
            )
        };

        let requests = unsafe {
            NSArray::from_slice(&[&*Retained::cast_unchecked::<VNRequest>(request.clone())])
        };
        handler
            .performRequests_error(&requests)
            .map_err(|e| anyhow!("Vision request failed: {:?}", e))?;

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

    fn extract_pdf_text(url: &Retained<NSURL>) -> Result<String> {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use std::ffi::CStr;

        // Use PDFKit to iterate pages and Vision to OCR each
        unsafe {
            let cls_name = CStr::from_bytes_with_nul(b"PDFDocument\0").unwrap();
            let cls = objc2::runtime::AnyClass::get(cls_name).ok_or_else(|| anyhow!("PDFKit not available"))?;
            let doc: *mut AnyObject = msg_send![cls, alloc];
            let doc: *mut AnyObject = msg_send![doc, initWithURL: &**url];
            if doc.is_null() { return Err(anyhow!("Failed to load PDF document")); }
            let doc: Retained<AnyObject> = Retained::from_raw(doc).unwrap();

            let count: isize = msg_send![&*doc, pageCount];
            let mut full_text = String::new();

            for i in 0..count {
                let page: *mut AnyObject = msg_send![&*doc, pageAtIndex: i];
                if page.is_null() { continue; }
                
                // Get page image at high resolution
                let box_rect: [f64; 4] = msg_send![page, boundsForBox: 0]; // 0 = kPDFDisplayBoxMediaBox
                
                let size = [box_rect[2] * 3.0, box_rect[3] * 3.0];
                let ns_image: *mut AnyObject = msg_send![page, thumbnailOfSize: size, forBox: 0];
                if ns_image.is_null() { continue; }
                let ns_image: Retained<AnyObject> = Retained::from_raw(ns_image).unwrap();

                // Convert NSImage to Vision request
                let cg_image: *mut AnyObject = msg_send![&*ns_image, CGImageForProposedRect: 0, context: 0, hints: 0];
                if cg_image.is_null() { continue; }

                let request = VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc());
                request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
                
                let handler = VNImageRequestHandler::initWithCGImage_options(
                    VNImageRequestHandler::alloc(),
                    std::mem::transmute(cg_image),
                    &NSDictionary::new()
                );

                let requests = NSArray::from_slice(&[&*Retained::cast_unchecked::<VNRequest>(request.clone())]);
                let _: bool = msg_send![&*handler, performRequests: &**requests, error: 0];

                if let Some(results) = request.results() {
                    for j in 0..results.count() {
                        let observation = results.objectAtIndex(j);
                        let candidates = observation.topCandidates(1);
                        if candidates.count() > 0 {
                            let text = candidates.objectAtIndex(0).string();
                            full_text.push_str(&text.to_string());
                            full_text.push('\n');
                        }
                    }
                }
                full_text.push_str("\n--- PAGE BREAK ---\n");
            }
            Ok(full_text)
        }
    }
}

pub async fn extract_text_macos<P: AsRef<Path>>(path: P) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let path = path.as_ref().to_path_buf();
        // Vision requests are blocking/heavy, run on a separate thread
        tokio::task::spawn_blocking(move || macos_impl::extract_text(&path)).await?
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(anyhow!("macOS Vision OCR is only available on macOS"))
    }
}
