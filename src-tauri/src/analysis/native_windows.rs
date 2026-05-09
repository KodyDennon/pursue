use anyhow::{anyhow, Result};
use std::path::Path;

pub async fn extract_text_windows<P: AsRef<Path>>(_path: P) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        // To be implemented using the `windows` crate
        Err(anyhow!("Windows Media OCR implementation pending"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(anyhow!("Windows Media OCR is only available on Windows"))
    }
}
