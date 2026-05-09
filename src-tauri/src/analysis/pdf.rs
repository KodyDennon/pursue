use anyhow::{Context, Result};
use lopdf::Document;
use std::path::Path;
use tokio::process::Command;

pub struct PdfAnalyzer;

impl PdfAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn extract_text<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let path = path.as_ref();
        let lopdf_text = self.extract_with_lopdf(path).unwrap_or_default();
        if lopdf_text.trim().len() > 80 {
            return Ok(lopdf_text);
        }

        if command_available("pdftotext").await {
            let output = Command::new("pdftotext")
                .arg("-layout")
                .arg(path)
                .arg("-")
                .output()
                .await
                .context("failed to start pdftotext")?;
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                if text.trim().len() > lopdf_text.trim().len() {
                    return Ok(text);
                }
            }
        }

        Ok(lopdf_text)
    }

    fn extract_with_lopdf<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let doc = Document::load(path)?;
        let mut text = String::new();

        for (index, _) in doc.get_pages().iter().enumerate() {
            let page_number = (index + 1) as u32;
            if let Ok(page_text) = doc.extract_text(&[page_number]) {
                text.push_str(&page_text);
                text.push('\n');
            }
        }

        Ok(text)
    }
}

async fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("-v")
        .output()
        .await
        .map(|output| output.status.success() || !output.stderr.is_empty())
        .unwrap_or(false)
}
