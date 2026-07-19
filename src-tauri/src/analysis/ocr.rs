use anyhow::{anyhow, Result};
use image::GenericImageView;
use oar_ocr::oarocr::{OAROCRBuilder, OAROCR};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_OCR_IMAGE_PIXELS: u64 = 12_000_000;
const MAX_REDACTION_IMAGE_PIXELS: u64 = 8_000_000;

pub struct OcrEngine {
    state: Arc<Mutex<OcrState>>,
}

#[derive(Default)]
struct OcrState {
    initialized: Option<InitializedOcr>,
    failed_backends: HashSet<&'static str>,
}

#[derive(Clone)]
struct InitializedOcr {
    engine: Arc<OAROCR>,
    backend: &'static str,
}

struct OcrProviderAttempt {
    backend: &'static str,
    config: oar_ocr::core::config::onnx::OrtSessionConfig,
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
            state: Arc::new(Mutex::new(OcrState::default())),
        }
    }

    async fn ensure_active<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
    ) -> Result<InitializedOcr> {
        let mut state = self.state.lock().await;
        if let Some(initialized) = &state.initialized {
            return Ok(initialized.clone());
        }

        // Resolve local model paths
        let det_path = get_model_path(app, "pp-ocrv5_mobile_det.onnx")?;
        let rec_path = get_model_path(app, "pp-ocrv5_mobile_rec.onnx")?;
        let dict_path = get_model_path(app, "ppocrv5_dict.txt")?;

        let mut failures = Vec::new();
        for attempt in ocr_provider_attempts() {
            if state.failed_backends.contains(attempt.backend) {
                continue;
            }

            match build_ocr(&det_path, &rec_path, &dict_path, attempt.config) {
                Ok(engine) => {
                    let initialized = InitializedOcr {
                        engine: Arc::new(engine),
                        backend: attempt.backend,
                    };
                    log::info!("OCR ONNX sessions initialized with {}", attempt.backend);
                    crate::analysis::hardware::record_active_inference_backend(
                        "OCR",
                        attempt.backend,
                    );
                    state.initialized = Some(initialized.clone());
                    return Ok(initialized);
                }
                Err(error) => {
                    log::warn!(
                        "OCR initialization failed with {}; trying the next provider: {:#}",
                        attempt.backend,
                        error
                    );
                    failures.push(format!("{}: {:#}", attempt.backend, error));
                }
            }
        }

        Err(anyhow!(
            "OCR could not initialize any execution provider: {}",
            failures.join("; ")
        ))
    }

    async fn mark_backend_failed(&self, backend: &'static str, error: &anyhow::Error) {
        log::error!(
            "OCR inference failed on {}; disabling it for this run and failing over: {:#}",
            backend,
            error
        );
        let mut state = self.state.lock().await;
        if state
            .initialized
            .as_ref()
            .is_some_and(|active| active.backend == backend)
        {
            state.initialized = None;
        }
        state.failed_backends.insert(backend);
        crate::analysis::hardware::clear_active_inference_backend("OCR");
    }

    pub async fn extract_structured<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        image: &image::DynamicImage,
    ) -> Result<OcrOutput> {
        let (original_width, original_height) = image.dimensions();
        let resized = resize_to_pixel_cap(image, MAX_OCR_IMAGE_PIXELS);
        let image = resized.as_ref().unwrap_or(image);
        let (image_width, image_height) = image.dimensions();
        let rgb = image.to_rgb8();
        let mut inference_failures = Vec::new();
        let results = loop {
            let active = self.ensure_active(app).await?;
            match active.engine.predict(vec![rgb.clone()]) {
                Ok(results) => break results,
                Err(error) => {
                    let error = anyhow!(error);
                    inference_failures.push(format!("{}: {:#}", active.backend, error));
                    self.mark_backend_failed(active.backend, &error).await;
                    if inference_failures.len() >= ocr_provider_attempts().len() {
                        return Err(anyhow!(
                            "OCR inference failed on every provider: {}",
                            inference_failures.join("; ")
                        ));
                    }
                }
            }
        };

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

fn ocr_provider_attempts() -> Vec<OcrProviderAttempt> {
    use crate::analysis::hardware::{acceleration_preference, AccelerationPreference};
    use oar_ocr::core::config::onnx::{OrtExecutionProvider, OrtSessionConfig};

    let mut attempts = Vec::new();
    let preference = acceleration_preference(false);

    #[allow(unused_variables)]
    // PP-OCR's convolution/attention work is assigned to the accelerator, but
    // ONNX Runtime retains a handful of shape/control nodes on CPU. Provider
    // registration itself is fatal in the vendored oar-ocr-core patch, so this
    // cannot silently label an unavailable accelerator as active. A complete
    // CPU inference session remains the final separate attempt below.
    let accelerated_config = |provider: OrtExecutionProvider| {
        OrtSessionConfig::new()
            .with_intra_threads(1)
            .with_parallel_execution(false)
            .with_execution_providers(vec![provider])
    };

    let push_cuda = |_attempts: &mut Vec<OcrProviderAttempt>| {
        #[cfg(feature = "cuda")]
        {
            let attempts = _attempts;
            attempts.push(OcrProviderAttempt {
                backend: "NVIDIA CUDA",
                config: accelerated_config(OrtExecutionProvider::CUDA {
                    device_id: Some(crate::analysis::hardware::cuda_device_id()),
                    gpu_mem_limit: crate::analysis::hardware::cuda_memory_limit_bytes(),
                    arena_extend_strategy: Some("SameAsRequested".to_string()),
                    cudnn_conv_algo_search: Some("Heuristic".to_string()),
                    cudnn_conv_use_max_workspace: Some(false),
                }),
            });
        }
    };

    let push_coreml = |_attempts: &mut Vec<OcrProviderAttempt>| {
        #[cfg(feature = "metal")]
        {
            let attempts = _attempts;
            attempts.push(OcrProviderAttempt {
                backend: "Apple CoreML",
                config: accelerated_config(OrtExecutionProvider::CoreML {
                    ane_only: Some(false),
                    subgraphs: Some(true),
                }),
            });
        }
    };

    let push_directml = |_attempts: &mut Vec<OcrProviderAttempt>| {
        #[cfg(all(target_os = "windows", feature = "directml"))]
        {
            let attempts = _attempts;
            attempts.push(OcrProviderAttempt {
                backend: "Windows DirectML",
                config: accelerated_config(OrtExecutionProvider::DirectML { device_id: Some(0) })
                    .with_memory_pattern(false),
            });
        }
    };

    match preference {
        AccelerationPreference::Cpu => {}
        AccelerationPreference::Cuda => {
            push_cuda(&mut attempts);
            // A requested provider is the first choice, not permission to skip another
            // available GPU and jump straight to CPU.
            push_directml(&mut attempts);
        }
        AccelerationPreference::Metal => push_coreml(&mut attempts),
        AccelerationPreference::DirectMl => {
            push_directml(&mut attempts);
            push_cuda(&mut attempts);
        }
        AccelerationPreference::Auto => {
            push_cuda(&mut attempts);
            push_coreml(&mut attempts);
            push_directml(&mut attempts);
        }
    }

    attempts.push(OcrProviderAttempt {
        backend: "CPU fallback",
        config: OrtSessionConfig::new()
            .with_intra_threads(crate::analysis::hardware::cpu_inference_threads())
            .with_execution_providers(vec![OrtExecutionProvider::CPU]),
    });
    attempts
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

    #[test]
    fn test_ocr_provider_initialization_and_prediction_with_real_assets() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let models = manifest.join("assets/models");
        let image = image::open(manifest.join("icons/128x128.png"))
            .expect("the checked-in application icon must decode")
            .to_rgb8();
        let mut failures = Vec::new();
        let mut successful_backend = None;
        for attempt in ocr_provider_attempts() {
            match build_ocr(
                &models.join("pp-ocrv5_mobile_det.onnx"),
                &models.join("pp-ocrv5_mobile_rec.onnx"),
                &models.join("ppocrv5_dict.txt"),
                attempt.config,
            ) {
                Ok(engine) => match engine.predict(vec![image.clone()]) {
                    Ok(_) => {
                        successful_backend = Some(attempt.backend);
                        break;
                    }
                    Err(error) => failures.push(format!("{} inference: {error}", attempt.backend)),
                },
                Err(error) => failures.push(format!("{} initialization: {error}", attempt.backend)),
            }
        }
        let backend = successful_backend.unwrap_or_else(|| {
            panic!(
                "real OCR models failed on every configured provider: {}",
                failures.join("; ")
            )
        });
        if std::env::var("PURSUE_REQUIRE_CUDA_INFERENCE").as_deref() == Ok("1") {
            assert_eq!(
                backend,
                "NVIDIA CUDA",
                "OCR provider failures before {backend}: {}",
                failures.join("; ")
            );
        }

        // Pure redaction-scoring boundary tests remain independent of inference.
        let redaction_engine = OcrEngine::new();
        let white_img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            100,
            image::Rgb([255, 255, 255]),
        ));

        let white_redaction_ratio = redaction_engine
            .analyze_redactions_image(&white_img)
            .unwrap();
        assert_eq!(white_redaction_ratio, 0.0);

        // Test with a black 100x100 image (high redactions)
        let black_img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            100,
            image::Rgb([0, 0, 0]),
        ));
        let black_redaction_ratio = redaction_engine
            .analyze_redactions_image(&black_img)
            .unwrap();
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
