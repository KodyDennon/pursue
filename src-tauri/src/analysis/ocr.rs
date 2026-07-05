use anyhow::{anyhow, Result};
use image::GenericImageView;
use oar_ocr::oarocr::{OAROCRBuilder, OAROCR};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_OCR_IMAGE_PIXELS: u64 = 12_000_000;
const MAX_REDACTION_IMAGE_PIXELS: u64 = 8_000_000;

pub struct OcrEngine {
    ocr: Arc<Mutex<Option<Arc<OAROCR>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrTextRegion {
    pub text: String,
    pub confidence: Option<f32>,
    pub bounding_box: serde_json::Value,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrOutput {
    pub text: String,
    pub average_confidence: Option<f32>,
    pub regions: Vec<OcrTextRegion>,
    pub image_width: u32,
    pub image_height: u32,
    pub resized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionAnalysis {
    pub score: f32,
    pub regions: Vec<RedactionRegion>,
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

        // Resolve local model paths
        let det_path = get_model_path(app, "pp-ocrv5_mobile_det.onnx")?;
        let rec_path = get_model_path(app, "pp-ocrv5_mobile_rec.onnx")?;
        let dict_path = get_model_path(app, "ppocrv5_dict.txt")?;

        let session_config = hardware_ort_session_config();
        let ocr_instance = match build_ocr(&det_path, &rec_path, &dict_path, session_config) {
            Ok(ocr) => ocr,
            Err(accelerated_error) if acceleration_enabled() => {
                log::warn!(
                    "Accelerated OCR initialization failed; retrying with CPU-only ONNX Runtime: {:#}",
                    accelerated_error
                );
                build_ocr(&det_path, &rec_path, &dict_path, cpu_ort_session_config()).map_err(
                    |cpu_error| {
                        anyhow!(
                            "OCR initialization failed with hardware acceleration ({:#}); CPU-only fallback also failed ({:#})",
                            accelerated_error,
                            cpu_error
                        )
                    },
                )?
            }
            Err(error) => return Err(error),
        };

        let ocr_arc = Arc::new(ocr_instance);
        *guard = Some(ocr_arc.clone());
        Ok(ocr_arc)
    }

    pub async fn extract_structured<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        image: &image::DynamicImage,
    ) -> Result<OcrOutput> {
        let ocr = self.ensure_initialized(app).await?;
        let (original_width, original_height) = image.dimensions();
        let resized = resize_to_pixel_cap(image, MAX_OCR_IMAGE_PIXELS);
        let image = resized.as_ref().unwrap_or(image);
        let (image_width, image_height) = image.dimensions();
        let results = ocr.predict(vec![image.to_rgb8()])?;

        let mut full_text = String::new();
        let mut regions = Vec::new();
        if let Some(result) = results.first() {
            for region in &result.text_regions {
                if let Some(text) = region.text.as_ref() {
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    full_text.push_str(text);
                    full_text.push(' ');
                    regions.push(OcrTextRegion {
                        text: text.to_string(),
                        confidence: region.confidence,
                        bounding_box: serde_json::to_value(&region.bounding_box)
                            .unwrap_or(serde_json::Value::Null),
                        label: region.label.as_ref().map(|v| v.to_string()),
                    });
                }
            }
        }

        let average_confidence = if regions.is_empty() {
            None
        } else {
            let mut total = 0.0f32;
            let mut count = 0usize;
            for region in &regions {
                if let Some(confidence) = region.confidence {
                    total += confidence;
                    count += 1;
                }
            }
            (count > 0).then_some(total / count as f32)
        };

        Ok(OcrOutput {
            text: full_text.trim().to_string(),
            average_confidence,
            regions,
            image_width,
            image_height,
            resized: image_width != original_width || image_height != original_height,
        })
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
        Ok(self.analyze_redactions_image_structured(img)?.score)
    }

    pub fn analyze_redactions_image_structured(
        &self,
        img: &image::DynamicImage,
    ) -> Result<RedactionAnalysis> {
        let resized = resize_to_pixel_cap(img, MAX_REDACTION_IMAGE_PIXELS);
        let img = resized.as_ref().unwrap_or(img);
        let luma = img.to_luma8();
        let (width, height) = luma.dimensions();
        if width < 3 || height < 3 {
            return Ok(RedactionAnalysis {
                score: 0.0,
                regions: Vec::new(),
            });
        }

        let mut redaction_pixels = 0u64;
        let mut row_black_counts = vec![0u32; height as usize];
        let mut row_extents = vec![None::<(u32, u32)>; height as usize];

        // Pass 1: Count horizontal black pixels
        for y in 0..height {
            let mut current_streak = 0;
            let mut streak_start = 0;
            for x in 0..width {
                if luma.get_pixel(x, y).0[0] < 15 {
                    if current_streak == 0 {
                        streak_start = x;
                    }
                    current_streak += 1;
                } else {
                    if current_streak > (width / 8) {
                        row_black_counts[y as usize] += current_streak;
                        merge_row_extent(&mut row_extents[y as usize], streak_start, x);
                    }
                    current_streak = 0;
                }
            }
            if current_streak > (width / 8) {
                row_black_counts[y as usize] += current_streak;
                merge_row_extent(&mut row_extents[y as usize], streak_start, width);
            }
        }

        // Pass 2: Filter isolated lines (must be blocky)
        let mut regions = Vec::new();
        let mut active_region: Option<RedactionRegion> = None;
        for y in 1..(height - 1) {
            let y_u = y as usize;
            if row_black_counts[y_u] > 0 {
                if row_black_counts[y_u - 1] == 0 && row_black_counts[y_u + 1] == 0 {
                    continue;
                }
                redaction_pixels += row_black_counts[y_u] as u64;
                if let Some((start_x, end_x)) = row_extents[y_u] {
                    let width = end_x.saturating_sub(start_x).max(1);
                    match &mut active_region {
                        Some(region)
                            if y <= region.y + region.height + 1
                                && ranges_overlap(
                                    start_x,
                                    end_x,
                                    region.x,
                                    region.x + region.width,
                                ) =>
                        {
                            let min_x = region.x.min(start_x);
                            let max_x = (region.x + region.width).max(end_x);
                            region.x = min_x;
                            region.width = max_x.saturating_sub(min_x).max(1);
                            region.height = y.saturating_sub(region.y) + 1;
                        }
                        Some(region) => {
                            if region.height >= 3 {
                                regions.push(region.clone());
                            }
                            active_region = Some(RedactionRegion {
                                x: start_x,
                                y,
                                width,
                                height: 1,
                            });
                        }
                        None => {
                            active_region = Some(RedactionRegion {
                                x: start_x,
                                y,
                                width,
                                height: 1,
                            });
                        }
                    }
                }
            } else if let Some(region) = active_region.take() {
                if region.height >= 3 {
                    regions.push(region);
                }
            }
        }
        if let Some(region) = active_region {
            if region.height >= 3 {
                regions.push(region);
            }
        }

        let total_pixels = (width as u64) * (height as u64);
        let ratio = (redaction_pixels as f32) / (total_pixels as f32);
        Ok(RedactionAnalysis {
            score: ratio,
            regions,
        })
    }
}

fn hardware_ort_session_config() -> oar_ocr::core::config::onnx::OrtSessionConfig {
    use oar_ocr::core::config::onnx::{OrtExecutionProvider, OrtSessionConfig};

    let mut providers = Vec::new();

    #[cfg(feature = "cuda")]
    {
        providers.push(OrtExecutionProvider::CUDA {
            device_id: Some(crate::analysis::hardware::cuda_device_id()),
            gpu_mem_limit: crate::analysis::hardware::cuda_memory_limit_bytes(),
            arena_extend_strategy: None,
            cudnn_conv_algo_search: Some("heuristic".to_string()),
            cudnn_conv_use_max_workspace: None,
        });
    }

    #[cfg(feature = "metal")]
    {
        providers.push(OrtExecutionProvider::CoreML {
            ane_only: Some(false),
            subgraphs: Some(true),
        });
    }

    #[cfg(feature = "directml")]
    {
        providers.push(OrtExecutionProvider::DirectML { device_id: Some(0) });
    }

    providers.push(OrtExecutionProvider::CPU);
    OrtSessionConfig::new()
        .with_intra_threads(crate::analysis::hardware::cpu_inference_threads())
        .with_execution_providers(providers)
}

fn cpu_ort_session_config() -> oar_ocr::core::config::onnx::OrtSessionConfig {
    use oar_ocr::core::config::onnx::{OrtExecutionProvider, OrtSessionConfig};

    OrtSessionConfig::new()
        .with_intra_threads(crate::analysis::hardware::cpu_inference_threads())
        .with_execution_providers(vec![OrtExecutionProvider::CPU])
}

fn acceleration_enabled() -> bool {
    cfg!(any(
        feature = "cuda",
        feature = "metal",
        feature = "directml"
    ))
}

fn build_ocr(
    det_path: &Path,
    rec_path: &Path,
    dict_path: &Path,
    session_config: oar_ocr::core::config::onnx::OrtSessionConfig,
) -> Result<OAROCR> {
    Ok(OAROCRBuilder::new(
        det_path.to_string_lossy().to_string(),
        rec_path.to_string_lossy().to_string(),
        dict_path.to_string_lossy().to_string(),
    )
    .ort_session(session_config)
    .build()?)
}

fn merge_row_extent(extent: &mut Option<(u32, u32)>, start: u32, end: u32) {
    *extent = Some(match extent {
        Some((old_start, old_end)) => ((*old_start).min(start), (*old_end).max(end)),
        None => (start, end),
    });
}

fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start <= b_end && b_start <= a_end
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
        let output = engine.extract_structured(handle, &white_img).await.unwrap();
        assert_eq!(output.text, "");

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

    #[test]
    fn redaction_analysis_reports_regions_for_blocky_black_runs() {
        let mut img = image::RgbImage::from_pixel(120, 80, image::Rgb([255, 255, 255]));
        for y in 20..30 {
            for x in 10..100 {
                img.put_pixel(x, y, image::Rgb([0, 0, 0]));
            }
        }
        let engine = OcrEngine::new();
        let result = engine
            .analyze_redactions_image_structured(&image::DynamicImage::ImageRgb8(img))
            .unwrap();
        assert!(result.score > 0.05);
        assert_eq!(result.regions.len(), 1);
        assert!(result.regions[0].width >= 80);
    }
}
