use anyhow::Result;
use reqwest::Client;
use scraper::Html;
use std::path::Path;
use tokio::fs;

pub async fn scrape_and_save(url: &str, output_path: &Path) -> Result<()> {
    let client = Client::new();
    let res = client.get(url).send().await?.text().await?;
    let text = {
        let document = Html::parse_document(&res);

        let mut text = String::new();
        text.push_str(&format!("Source URL: {}\n\n", url));

        let root = document.root_element();
        for node in root.text() {
            let chunk = node.trim();
            if !chunk.is_empty() {
                text.push_str(chunk);
                text.push('\n');
            }
        }
        text
    };

    fs::write(output_path, text).await?;
    Ok(())
}
