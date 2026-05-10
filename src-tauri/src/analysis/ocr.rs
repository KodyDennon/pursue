use anyhow::{anyhow, Result};
use std::path::Path;
use ocrs::OcrEngine as NativeOcr;
use std::sync::OnceLock;

static OCR_INSTANCE: OnceLock<NativeOcr> = OnceLock::new();

pub struct OcrEngine;

impl OcrEngine {
    pub fn new() -> Self {
        Self
    }

    /// Pure Rust cross-platform OCR fallback using 'ocrs' crate
    pub async fn extract_text_fallback<P: AsRef<Path>>(&self, image_path: P) -> Result<String> {
        let engine = self.get_or_init_engine()?;
        
        // Load image using 'image' crate
        let img = image::open(image_path)?;
        let img = img.to_rgb8();
        let (width, height) = img.dimensions();
        
        // Convert to ocrs input format
        let layout_input = ocrs::ImageSource::from_bytes(&img.as_raw(), (width, height))
            .map_err(|e| anyhow!("failed to prepare OCR input: {:?}", e))?;
            
        // Prepare OcrInput
        let input = engine.prepare_input(layout_input)
            .map_err(|e| anyhow!("failed to prepare OCR input: {:?}", e))?;

        let res = engine.get_text(&input)
            .map_err(|e| anyhow!("OCR extraction failed: {:?}", e))?;
            
        Ok(res)
    }

    fn get_or_init_engine(&self) -> Result<&NativeOcr> {
        if let Some(engine) = OCR_INSTANCE.get() {
            return Ok(engine);
        }

        // Note: ocrs requires models (detection and recognition)
        // We'll bail for now as this is a fallback.
        Err(anyhow!("Bundled OCR engine models missing. Reverting to OS-native OCR."))
    }

    pub fn analyze_redactions(&self, image_path: &Path) -> Result<f32> {
        let img = image::open(image_path)?;
        let luma = img.to_luma8();
        let (width, height) = luma.dimensions();
        
        let mut redaction_pixels = 0u64;
        let mut row_black_counts = vec![0u32; height as usize];
        
        // Pass 1: Count horizontal black pixels
        for y in 0..height {
            let mut current_streak = 0;
            for x in 0..width {
                if luma.get_pixel(x, y).0[0] < 15 {
                    current_streak += 1;
                } else {
                    if current_streak > (width / 8) { // If streak is larger than 1/8th of width, it's a solid block
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
                // If the row above and below have no black lines, it's just a thin line/border, not a redaction block
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
