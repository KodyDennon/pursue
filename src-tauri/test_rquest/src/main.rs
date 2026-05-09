use rquest::{Client, tls::Impersonate};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .impersonate(Impersonate::Chrome124)
        .build()?;
    
    let res = client.get("https://www.war.gov/UFO/").send().await?;
    let text = res.text().await?;
    std::fs::write("output.html", text)?;
    Ok(())
}
