use anyhow::{anyhow, Result};
use image::GenericImageView;
use oar_ocr::oarocr::{OAROCRBuilder, OAROCR};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_OCR_IMAGE_PIXELS: u64 = 12_000_000;
const MAX_REDACTION_IMAGE_PIXELS: u64 = 4_000_000;

pub struct OcrEngine {
    ocr: Arc<Mutex<Option<Arc<OAROCR>>>>,
}

impl OcrEngine {
    pub fn new() -> Self {
        Self {
            ocr: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn ensure_initialized<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
    ) -> Result<Arc<OAROCR>> {
        let mut guard = self.ocr.lock().await;
        if let Some(ocr) = &*guard {
            return Ok(ocr.clone());
        }

        // Get hardware acceleration config
        let mut providers = Vec::new();

        #[cfg(feature = "cuda")]
        {
            use oar_ocr::core::config::onnx::OrtExecutionProvider;
            providers.push(OrtExecutionProvider::CUDA {
                device_id: Some(0),
                gpu_mem_limit: None,
                arena_extend_strategy: None,
                cudnn_conv_algo_search: None,
                do_copy_in_default_stream: None,
                cudnn_conv_use_max_workspace: None,
            });
        }

        #[cfg(feature = "metal")]
        {
            use oar_ocr::core::config::onnx::OrtExecutionProvider;
            providers.push(OrtExecutionProvider::CoreML {
                ane_only: Some(false),
                subgraphs: Some(false),
            });
        }

        // CPU is always added as a final fallback
        use oar_ocr::core::config::onnx::OrtExecutionProvider;
        providers.push(OrtExecutionProvider::CPU);

        use oar_ocr::core::config::onnx::OrtSessionConfig;
        let session_config = OrtSessionConfig::new().with_execution_providers(providers);

        // Resolve local model paths
        let det_path = get_model_path(app, "pp-ocrv5_mobile_det.onnx")?;
        let rec_path = get_model_path(app, "pp-ocrv5_mobile_rec.onnx")?;
        let dict_path = get_model_path(app, "ppocrv5_dict.txt")?;

        let ocr_instance = OAROCRBuilder::new(
            det_path.to_string_lossy().to_string(),
            rec_path.to_string_lossy().to_string(),
            dict_path.to_string_lossy().to_string(),
        )
        .ort_session(session_config)
        .build()?;

        let ocr_arc = Arc::new(ocr_instance);
        *guard = Some(ocr_arc.clone());
        Ok(ocr_arc)
    }

    pub async fn extract_text<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        image: &image::DynamicImage,
    ) -> Result<String> {
        let ocr = self.ensure_initialized(app).await?;
        let resized = resize_to_pixel_cap(image, MAX_OCR_IMAGE_PIXELS);
        let image = resized.as_ref().unwrap_or(image);
        let results = ocr.predict(vec![image.to_rgb8()])?;

        let mut full_text = String::new();
        if let Some(result) = results.first() {
            for region in &result.text_regions {
                if let Some((text, _confidence)) = region.text_with_confidence() {
                    full_text.push_str(text);
                    full_text.push(' ');
                }
            }
        }
        Ok(full_text.trim().to_string())
    }

    pub fn analyze_redactions(&self, image_path: &Path) -> Result<f32> {
        let extension = image_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if extension == "pdf" {
            return Ok(0.0);
        }

        let img = image::open(image_path)?;
        self.analyze_redactions_image(&img)
    }

    pub fn analyze_redactions_image(&self, img: &image::DynamicImage) -> Result<f32> {
        let resized = resize_to_pixel_cap(img, MAX_REDACTION_IMAGE_PIXELS);
        let img = resized.as_ref().unwrap_or(img);
        let luma = img.to_luma8();
        let (width, height) = luma.dimensions();
        if width < 3 || height < 3 {
            return Ok(0.0);
        }

        let mut redaction_pixels = 0u64;
        let mut row_black_counts = vec![0u32; height as usize];

        // Pass 1: Count horizontal black pixels
        for y in 0..height {
            let mut current_streak = 0;
            for x in 0..width {
                if luma.get_pixel(x, y).0[0] < 15 {
                    current_streak += 1;
                } else {
                    if current_streak > (width / 8) {
                        row_black_counts[y as usize] += current_streak;
                    }
                    current_streak = 0;
                }
            }
            if current_streak > (width / 8) {
                row_black_counts[y as usize] += current_streak;
            }
        }

        // Pass 2: Filter isolated lines (must be blocky)
        for y in 1..(height - 1) {
            let y_u = y as usize;
            if row_black_counts[y_u] > 0 {
                if row_black_counts[y_u - 1] == 0 && row_black_counts[y_u + 1] == 0 {
                    continue;
                }
                redaction_pixels += row_black_counts[y_u] as u64;
            }
        }

        let total_pixels = (width as u64) * (height as u64);
        let ratio = (redaction_pixels as f32) / (total_pixels as f32);
        Ok(ratio)
    }
}

fn resize_to_pixel_cap(img: &image::DynamicImage, max_pixels: u64) -> Option<image::DynamicImage> {
    let (width, height) = img.dimensions();
    let pixels = (width as u64).saturating_mul(height as u64);
    if pixels <= max_pixels || pixels == 0 {
        return None;
    }

    let scale = ((max_pixels as f64) / (pixels as f64)).sqrt();
    let target_width = ((width as f64) * scale).round().max(1.0) as u32;
    let target_height = ((height as f64) * scale).round().max(1.0) as u32;
    Some(img.resize(
        target_width,
        target_height,
        image::imageops::FilterType::Triangle,
    ))
}

fn get_model_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>, filename: &str) -> Result<PathBuf> {
    use tauri::Manager;
    let rel_path = format!("src-tauri/assets/models/{}", filename);
    if let Ok(path) = app.path().resolve(
        format!("assets/models/{}", filename),
        tauri::path::BaseDirectory::Resource,
    ) {
        if path.exists() {
            return Ok(path);
        }
    }
    let mut path = std::env::current_dir()?;
    if path.ends_with("src-tauri") {
        path = path.parent().unwrap().to_path_buf();
    }
    let target = path.join(&rel_path);
    if target.exists() {
        return Ok(target);
    }
    Err(anyhow!("Model file {} not found", filename))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[tokio::test]
    async fn test_ocr_initialization_and_prediction() {
        let app = tauri::test::mock_app();
        let handle = app.handle();

        let engine = OcrEngine::new();
        let ocr_res = engine.ensure_initialized(handle).await;
        assert!(
            ocr_res.is_ok(),
            "Failed to initialize OCR engine: {:?}",
            ocr_res.err()
        );

        // Test with a white 100x100 image (no redactions)
        let white_img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            100,
            image::Rgb([255, 255, 255]),
        ));
        let text = engine.extract_text(handle, &white_img).await.unwrap();
        assert_eq!(text, "");

        let white_redaction_ratio = engine.analyze_redactions_image(&white_img).unwrap();
        assert_eq!(white_redaction_ratio, 0.0);

        // Test with a black 100x100 image (high redactions)
        let black_img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            100,
            image::Rgb([0, 0, 0]),
        ));
        let black_redaction_ratio = engine.analyze_redactions_image(&black_img).unwrap();
        assert!(black_redaction_ratio > 0.9);
    }

    #[test]
    fn resize_to_pixel_cap_only_downscales_oversized_images() {
        let small = image::DynamicImage::ImageRgb8(image::RgbImage::new(10, 10));
        assert!(resize_to_pixel_cap(&small, 100).is_none());

        let large = image::DynamicImage::ImageRgb8(image::RgbImage::new(100, 100));
        let resized = resize_to_pixel_cap(&large, 2_500).expect("large image is downscaled");
        let (width, height) = resized.dimensions();
        assert!((width as u64) * (height as u64) <= 2_500);
    }
}
