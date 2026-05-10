use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::process::Command;

pub struct ThumbnailManager;

impl ThumbnailManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate_thumbnail(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        let extension = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "pdf" => self.generate_pdf_thumbnail(input_path, output_path).await,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" => {
                self.generate_image_thumbnail(input_path, output_path).await
            }
            "mp4" | "mov" | "m4v" | "avi" | "mkv" => {
                self.generate_video_thumbnail(input_path, output_path).await
            }
            _ => Err(anyhow!("unsupported thumbnail file type `{}`", extension)),
        }
    }

    async fn generate_image_thumbnail(&self, input: &Path, output: &Path) -> Result<()> {
        let input = input.to_path_buf();
        let output = output.to_path_buf();
        
        tokio::task::spawn_blocking(move || {
            let img = image::open(&input).map_err(|e| anyhow!("failed to open image: {}", e))?;
            let thumbnail = img.thumbnail(512, 512);
            thumbnail.save(&output).map_err(|e| anyhow!("failed to save thumbnail: {}", e))?;
            Ok(())
        }).await?
    }

    async fn generate_pdf_thumbnail(&self, _input: &Path, _output: &Path) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let input = _input;
            let output = _output;
            // Use qlmanage for high-quality native thumbnails on Mac
            let output_dir = output.parent().ok_or_else(|| anyhow!("no parent dir"))?;
            let status = Command::new("qlmanage")
                .arg("-t")
                .arg("-s")
                .arg("512")
                .arg("-o")
                .arg(output_dir)
                .arg(input)
                .output()
                .await?;

            if status.status.success() {
                // qlmanage names the file input_path.png
                let generated = output_dir.join(format!("{}.png", input.file_name().unwrap().to_str().unwrap()));
                if generated.exists() {
                    tokio::fs::rename(generated, output).await?;
                    return Ok(());
                }
            }
        }
        
        // Fallback or non-mac: use image crate if it's already an image-based PDF or failed
        Err(anyhow!("high-fidelity PDF thumbnailing requires native platform support"))
    }

    async fn generate_video_thumbnail(&self, input: &Path, output: &Path) -> Result<()> {
        // Use ffmpeg if available
        let status = Command::new("ffmpeg")
            .arg("-i")
            .arg(input)
            .arg("-ss")
            .arg("00:00:01")
            .arg("-vframes")
            .arg("1")
            .arg("-s")
            .arg("512x288") // 16:9 thumb
            .arg("-f")
            .arg("image2")
            .arg(output)
            .output()
            .await;

        match status {
            Ok(out) if out.status.success() => Ok(()),
            _ => Err(anyhow!("ffmpeg thumbnailing failed; ensure ffmpeg is installed")),
        }
    }
}
