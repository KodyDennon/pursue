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
    use objc2_pdf_kit::PDFDocument;
    use objc2_app_kit::NSImage;
    use objc2_core_graphics::CGImage;

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
        
        unsafe {
            let _: bool = msg_send![&handler, performRequests: &*requests, error: 0];
        }

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
        let doc = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), url) }
            .ok_or_else(|| anyhow!("Failed to load PDF document"))?;

        let count = unsafe { doc.pageCount() };
        let mut full_text = String::new();

        for i in 0..count {
            let page = unsafe { doc.pageAtIndex(i) };
            let page = match page {
                Some(p) => p,
                None => continue,
            };
            
            // Render page to NSImage (high resolution)
            let box_rect: [f64; 4] = unsafe { msg_send![&*page, boundsForBox: 0] }; // 0 = kPDFDisplayBoxMediaBox
            let size = [box_rect[2] * 3.0, box_rect[3] * 3.0];
            
            let ns_image: *mut NSImage = unsafe { msg_send![&*page, thumbnailOfSize: size, forBox: 0] };
            if ns_image.is_null() { continue; }
            let ns_image = unsafe { Retained::from_raw(ns_image) }.unwrap();

            // Convert NSImage to CGImage
            let cg_image_ptr: *mut CGImage = unsafe { msg_send![&*ns_image, CGImageForProposedRect: 0, context: 0, hints: 0] };
            if cg_image_ptr.is_null() { continue; }
            let cg_image = unsafe { &*cg_image_ptr };

            let request = unsafe { VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc()) };
            
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            
            let handler = unsafe {
                VNImageRequestHandler::initWithCGImage_options(
                    VNImageRequestHandler::alloc(),
                    cg_image,
                    &NSDictionary::new()
                )
            };

            let requests = unsafe {
                NSArray::from_slice(&[&*Retained::cast_unchecked::<VNRequest>(request.clone())])
            };
            
            unsafe {
                let _: bool = msg_send![&handler, performRequests: &*requests, error: 0];
            }

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
