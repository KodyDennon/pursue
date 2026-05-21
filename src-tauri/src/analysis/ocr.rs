use crate::analysis::sidecar::VisionSidecar;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

pub struct OcrEngine {
    vision: Arc<VisionSidecar>,
}

impl OcrEngine {
    pub fn new(vision: Arc<VisionSidecar>) -> Self {
        Self { vision }
    }

    /// Neural vision OCR using GOT-OCR-2.0 sidecar
    pub async fn extract_text_fallback<P: AsRef<Path>>(
        &self,
        app: &tauri::AppHandle,
        path: P,
    ) -> Result<String> {
        let path = path.as_ref();
        if let Err(e) = self.vision.start(app).await {
            return Err(anyhow::anyhow!(
                "Failed to initialize Neural Vision Engine: {}",
                e
            ));
        }
        self.vision.extract_text(path).await
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
                    if current_streak > (width / 8) {
                        // If streak is larger than 1/8th of width, it's a solid block
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
