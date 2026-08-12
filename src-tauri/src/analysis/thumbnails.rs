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
                let file_name = input
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("invalid input filename"))?;
                let generated = output_dir.join(format!("{}.png", file_name));
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

            let result = tokio::task::spawn_blocking(move || -> Result<()> {
                use windows::core::HSTRING;
                use windows::Data::Pdf::PdfDocument;
                use windows::Storage::StorageFile;
                use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};

                crate::analysis::pdf::init_winrt_apartment();

                let input_str = input_path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid UTF-8 path: {:?}", input_path))?;
                let input_hstring = HSTRING::from(input_str);

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
                let img = image::load_from_memory(&buffer)
                    .map_err(|e| anyhow!("Failed to parse WinRT PNG bytes: {}", e))?;
                let thumbnail = img.thumbnail(512, 512);
                thumbnail
                    .save(&output_path)
                    .map_err(|e| anyhow!("Failed to save Windows PDF thumbnail: {}", e))?;

                Ok(())
            })
            .await;

            if let Ok(Ok(())) = result {
                return Ok(());
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            return generate_pdf_thumbnail_linux(_input, _output).await;
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        Err(anyhow!(
            "native PDF thumbnailing failed for {}",
            _input.display()
        ))
    }

    async fn generate_video_thumbnail(&self, input: &Path, output: &Path) -> Result<()> {
        // Use ffmpeg if available
        let mut command = Command::new("ffmpeg");
        command
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
            .arg(output);
        let status = crate::common::hide_console(&mut command).output().await;

        match status {
            Ok(out) if out.status.success() => Ok(()),
            _ => Err(anyhow!(
                "ffmpeg thumbnailing failed; ensure ffmpeg is installed"
            )),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
async fn generate_pdf_thumbnail_linux(input: &Path, output: &Path) -> Result<()> {
    let render_dir =
        std::env::temp_dir().join(format!("pursue-pdf-thumb-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&render_dir).await?;
    let result = render_pdf_thumbnail_via_poppler(input, output, &render_dir).await;
    let _ = tokio::fs::remove_dir_all(&render_dir).await;
    result
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
async fn render_pdf_thumbnail_via_poppler(
    input: &Path,
    output: &Path,
    render_dir: &Path,
) -> Result<()> {
    let out_prefix = render_dir.join("page");
    let status = Command::new("pdftoppm")
        .arg("-png")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("1")
        .arg("-scale-to")
        .arg("512")
        .arg(input)
        .arg(&out_prefix)
        .status()
        .await
        .map_err(|e| {
            anyhow!("failed to run pdftoppm (install poppler-utils, e.g. `apt install poppler-utils`): {e}")
        })?;

    if !status.success() {
        return Err(anyhow!(
            "pdftoppm exited with status {status} while thumbnailing {}",
            input.display()
        ));
    }

    let mut entries = tokio::fs::read_dir(render_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("png") {
            tokio::fs::rename(&path, output).await?;
            return Ok(());
        }
    }

    Err(anyhow!(
        "pdftoppm produced no output page for {}",
        input.display()
    ))
}
