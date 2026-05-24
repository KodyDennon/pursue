use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::process::Command;

pub struct ThumbnailManager;

impl ThumbnailManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate_thumbnail(&self, input_path: &Path, output_path: &Path) -> Result<()> {
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
            thumbnail
                .save(&output)
                .map_err(|e| anyhow!("failed to save thumbnail: {}", e))?;
            Ok(())
        })
        .await?
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
                let generated = output_dir.join(format!(
                    "{}.png",
                    input.file_name().unwrap().to_str().unwrap()
                ));
                if generated.exists() {
                    tokio::fs::rename(generated, output).await?;
                    return Ok(());
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let input_path = _input.to_path_buf();
            let output_path = _output.to_path_buf();
            
            return tokio::task::spawn_blocking(move || -> Result<()> {
                use windows::core::HSTRING;
                use windows::Data::Pdf::PdfDocument;
                use windows::Storage::StorageFile;
                use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
                
                let input_hstring = HSTRING::from(input_path.to_str().unwrap());
                
                // Load PDF via WinRT
                let file = StorageFile::GetFileFromPathAsync(&input_hstring)?.get()?;
                let pdf = PdfDocument::LoadFromFileAsync(&file)?.get()?;
                
                if pdf.PageCount()? == 0 {
                    return Err(anyhow!("PDF has no pages"));
                }
                
                // Render Page 0 to stream
                let page = pdf.GetPage(0)?;
                let stream = InMemoryRandomAccessStream::new()?;
                page.RenderToStreamAsync(&stream)?.get()?;
                
                // Read stream bytes
                let size = stream.Size()? as u32;
                let input_stream = stream.GetInputStreamAt(0)?;
                let reader = DataReader::CreateDataReader(&input_stream)?;
                reader.LoadAsync(size)?.get()?;
                
                let mut buffer = vec![0u8; size as usize];
                reader.ReadBytes(&mut buffer)?;
                
                // Resize to 512x512 to exactly match macOS qlmanage constraints
                let img = image::load_from_memory(&buffer).map_err(|e| anyhow!("Failed to parse WinRT PNG bytes: {}", e))?;
                let thumbnail = img.thumbnail(512, 512);
                thumbnail.save(&output_path).map_err(|e| anyhow!("Failed to save Windows PDF thumbnail: {}", e))?;
                
                Ok(())
            })
            .await?;
        }

        // Fallback or non-mac/win: use image crate if it's already an image-based PDF or failed
        Err(anyhow!(
            "high-fidelity PDF thumbnailing requires native platform support (macOS or Windows)"
        ))
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
            _ => Err(anyhow!(
                "ffmpeg thumbnailing failed; ensure ffmpeg is installed"
            )),
        }
    }
}
