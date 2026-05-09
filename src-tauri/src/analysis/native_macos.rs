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

    pub fn extract_text(path: &Path) -> Result<String> {
        objc2::rc::autoreleasepool(|_| {
            let path_str = path.to_str().ok_or_else(|| anyhow!("invalid path"))?;
            let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));

            // 1. Create the request
            let request = unsafe { VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc()) };
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            request.setUsesLanguageCorrection(true);

            // 2. Create the handler
            let handler = unsafe {
                VNImageRequestHandler::initWithURL_options(
                    VNImageRequestHandler::alloc(),
                    &url,
                    &NSDictionary::new(),
                )
            };

            // 3. Perform the request
            let requests = unsafe {
                NSArray::from_slice(&[&*Retained::cast_unchecked::<VNRequest>(request.clone())])
            };
            handler
                .performRequests_error(&requests)
                .map_err(|e| anyhow!("Vision request failed: {:?}", e))?;

            // 4. Extract results
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
        })
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
