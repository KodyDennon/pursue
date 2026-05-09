use anyhow::{anyhow, Result};
use std::path::Path;

#[cfg(target_os = "windows")]
pub async fn extract_text_windows<P: AsRef<Path>>(_path: P) -> Result<String> {
    // To be implemented using the `windows` crate
    Err(anyhow!("Windows Media OCR implementation pending"))
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub async fn extract_text_windows<P: AsRef<Path>>(_path: P) -> Result<String> {
    Err(anyhow!("Windows Media OCR is only available on Windows"))
}
