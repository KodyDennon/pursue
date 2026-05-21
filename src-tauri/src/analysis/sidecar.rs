use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;

async fn download_file(client: &Client, url: &str, path: &Path, app: &tauri::AppHandle, msg: &str) -> Result<()> {
    let response = client.get(url).send().await.context("Failed to start download")?;
    let total_size = response.content_length().unwrap_or(0);
    
    let mut file = tokio::fs::File::create(path).await?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading chunk")?;
        tokio::io::copy(&mut &chunk[..], &mut file).await?;
        downloaded += chunk.len() as u64;
        
        if total_size > 0 {
            let progress = (downloaded as f64 / total_size as f64) * 100.0;
            let _ = app.emit("analysis-progress", serde_json::json!({
                "status": "loading-model",
                "progress": progress,
                "msg": format!("{} ({:.1}%)", msg, progress)
            }));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
async fn extract_zip(archive_path: PathBuf, dest_dir: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => dest_dir.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        anyhow::Ok(())
    }).await??;
    Ok(())
}

#[cfg(target_os = "macos")]
async fn extract_targz(archive_path: PathBuf, dest_dir: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let tar_gz = std::fs::File::open(&archive_path)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&dest_dir)?;
        anyhow::Ok(())
    }).await??;
    Ok(())
}

async fn get_resource_path(app: &tauri::AppHandle, relative_path: &str) -> Result<PathBuf> {
    // In production, use Tauri's resource resolver
    if !cfg!(debug_assertions) {
        return app.path().resolve(relative_path, tauri::path::BaseDirectory::Resource)
            .context(format!("Failed to resolve resource: {}", relative_path));
    }

    // In development, try to find the project root
    let mut path = std::env::current_dir()?;
    
    // If run from src-tauri, go up
    if path.ends_with("src-tauri") {
        path = path.parent().unwrap().to_path_buf();
    }
    
    let target = path.join(relative_path);
    if target.exists() {
        return Ok(target);
    }

    // Fallback to Tauri's resolver if dev heuristic fails
    app.path().resolve(relative_path, tauri::path::BaseDirectory::Resource)
        .context(format!("Failed to resolve resource: {}", relative_path))
}

async fn provision_python(app: &tauri::AppHandle) -> Result<PathBuf> {
    let app_data = app.path().app_data_dir()?;
    let py_env_dir = app_data.join("python-runtime");
    let marker_file = py_env_dir.join("provisioned.ok");
    
    let (python_exe, _pip_exe) = if cfg!(windows) {
        (py_env_dir.join("python.exe"), py_env_dir.join("python.exe"))
    } else {
        // Indygreg standalone structure: extracts into a 'python' folder
        let base = py_env_dir.join("python");
        (base.join("bin").join("python3"), base.join("bin").join("pip3"))
    };

    if marker_file.exists() && python_exe.exists() {
        return Ok(python_exe);
    }

    let _ = app.emit("analysis-progress", serde_json::json!({
        "status": "loading-model",
        "msg": "Provisioning Neural Vision Runtime (First time only)..."
    }));

    if let Err(e) = (|| async {
        // If marker is missing but dir exists, we might be in a broken state. 
        // For safety, if it's the first run (marker missing), we ensure fresh deps.
        if !py_env_dir.exists() {
            std::fs::create_dir_all(&py_env_dir)?;
        }
        
        let client = Client::new();

        #[cfg(target_os = "windows")]
        {
            if !python_exe.exists() {
                let url = "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip";
                let zip_path = py_env_dir.join("python.zip");
                download_file(&client, url, &zip_path, app, "Downloading Python distribution").await?;
                extract_zip(zip_path.clone(), py_env_dir.clone()).await?;
                let _ = tokio::fs::remove_file(zip_path).await;

                // Bootstrap Pip
                let get_pip_url = "https://bootstrap.pypa.io/get-pip.py";
                let get_pip_path = py_env_dir.join("get-pip.py");
                download_file(&client, get_pip_url, &get_pip_path, app, "Bootstrapping pip").await?;
                
                let out = tokio::process::Command::new(&python_exe)
                    .arg(&get_pip_path)
                    .output()
                    .await?;
                if !out.status.success() {
                    return Err(anyhow!("Failed to install pip: {}", String::from_utf8_lossy(&out.stderr)));
                }
                let _ = tokio::fs::remove_file(get_pip_path).await;

                // Enable site-packages in ._pth file
                if let Ok(entries) = std::fs::read_dir(&py_env_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("_pth") {
                            let content = std::fs::read_to_string(&path)?;
                            let new_content = content.replace("#import site", "import site");
                            std::fs::write(&path, new_content)?;
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if !python_exe.exists() {
                let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
                let url = format!("https://github.com/indygreg/python-build-standalone/releases/download/20240107/cpython-3.11.7+20240107-{}-apple-darwin-install_only.tar.gz", arch);
                let tar_path = py_env_dir.join("python.tar.gz");
                download_file(&client, &url, &tar_path, app, "Downloading Python distribution").await?;
                extract_targz(tar_path.clone(), py_env_dir.clone()).await?;
                let _ = tokio::fs::remove_file(tar_path).await;
            }
        }

        // Install Requirements
        let req_file = get_resource_path(app, "src-python/requirements.txt").await?;
        
        let _ = app.emit("analysis-progress", serde_json::json!({
            "status": "loading-model",
            "msg": "Installing GOT-OCR-2.0 dependencies (this may take a few minutes)..."
        }));

        // Always use python -m pip to avoid shebang path issues with spaces
        let out = tokio::process::Command::new(&python_exe)
            .args(&["-m", "pip", "install", "--no-cache-dir", "-r"])
            .arg(&req_file)
            .output()
            .await?;

        if !out.status.success() {
            return Err(anyhow!("Failed to install dependencies: {}", String::from_utf8_lossy(&out.stderr)));
        }

        // Create marker file on success
        std::fs::write(&marker_file, "provisioned")?;
        
        anyhow::Ok(())
    })().await {
        let _ = app.emit("analysis-progress", serde_json::json!({
            "status": "failed",
            "error": format!("Python Provisioning Failed: {}", e)
        }));
        return Err(e);
    }

    Ok(python_exe)
}

#[derive(Serialize)]
struct OCRRequest {
    image_path: String,
}

#[derive(Deserialize)]
struct OCRResponse {
    text: String,
}

fn is_port_in_use(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_err()
}

#[cfg(target_os = "windows")]
async fn kill_port_owner(port: u16) {
    if let Ok(output) = tokio::process::Command::new("cmd")
        .args(&["/C", &format!("for /f \"tokens=5\" %a in ('netstat -aon ^| findstr :{}') do taskkill /F /PID %a", port)])
        .output()
        .await {
        tauri_plugin_log::log::info!("Killed orphaned process on port {}: {:?}", port, String::from_utf8_lossy(&output.stdout));
    }
}

#[cfg(not(target_os = "windows"))]
async fn kill_port_owner(port: u16) {
    if let Ok(output) = tokio::process::Command::new("sh")
        .args(&["-c", &format!("lsof -t -i:{} | xargs kill -9", port)])
        .output()
        .await {
        tauri_plugin_log::log::info!("Killed orphaned process on port {}: {:?}", port, String::from_utf8_lossy(&output.stdout));
    }
}

pub struct VisionSidecar {
    client: Client,
    port: u16,
    child: Arc<Mutex<Option<CommandChild>>>,
    ocr_semaphore: tokio::sync::Semaphore,
}

impl VisionSidecar {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(1800)) // Long timeout (30 mins) for extremely large documents
                .build()
                .unwrap(),
            port: 8374,
            child: Arc::new(Mutex::new(None)),
            ocr_semaphore: tokio::sync::Semaphore::new(1),
        }
    }

    pub async fn is_provisioned(&self, app: &tauri::AppHandle) -> bool {
        let app_data = match app.path().app_data_dir() {
            Ok(d) => d,
            Err(_) => return false,
        };
        let py_env_dir = app_data.join("python-runtime");
        let marker_file = py_env_dir.join("provisioned.ok");
        let python_exe = if cfg!(windows) {
            py_env_dir.join("python.exe")
        } else {
            py_env_dir.join("python").join("bin").join("python3")
        };
        marker_file.exists() && python_exe.exists()
    }

    pub async fn provision(&self, app: &tauri::AppHandle) -> Result<()> {
        provision_python(app).await.map(|_| ())
    }

    pub async fn start(&self, app: &tauri::AppHandle) -> Result<()> {
        let mut child_guard = self.child.lock().await;
        if child_guard.is_some() {
            return Ok(());
        }

        // Check if sidecar is already running (e.g., from an orphaned process)
        if is_port_in_use(self.port) {
            tauri_plugin_log::log::warn!("Port {} is already in use. Killing orphaned process...", self.port);
            kill_port_owner(self.port).await;
            // Sleep briefly to let the OS release the socket
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        let port_str = self.port.to_string();

        let python_exe = provision_python(app).await?;
        
        let py_script = get_resource_path(app, "src-python/main.py").await?;

        let sidecar_cmd = app.shell()
            .command(python_exe.to_string_lossy().to_string())
            .args(vec![py_script.to_string_lossy().to_string()])
            .env("PORT", &port_str);

        let (mut rx, child) = sidecar_cmd.spawn().map_err(|e| anyhow!("failed to spawn sidecar: {}", e))?;
        
        *child_guard = Some(child);

        // Stream Python logs to UI
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            use tauri_plugin_shell::process::CommandEvent;
            let mut buffer = String::new();

            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(data) | CommandEvent::Stderr(data) => {
                        let chunk = String::from_utf8_lossy(&data);
                        buffer.push_str(&chunk);

                        while let Some(idx) = buffer.find(|c| c == '\n' || c == '\r') {
                            let line = buffer[..idx].trim().to_string();
                            buffer.drain(..=idx); // Remove the line and the newline char

                            if line.is_empty() {
                                continue;
                            }

                            if line.contains("%|") || (line.contains("Downloading") && line.contains('%')) {
                                if let Some(percent_str) = line.split('%').next().and_then(|s| s.split_whitespace().last()) {
                                    if let Ok(pct) = percent_str.parse::<f64>() {
                                        let _ = app_clone.emit("analysis-progress", serde_json::json!({
                                            "status": "loading-model",
                                            "progress": pct,
                                            "msg": line
                                        }));
                                        continue;
                                    }
                                }
                            }

                            let _ = app_clone.emit("analysis-progress", serde_json::json!({
                                "status": "batch-planning",
                                "msg": format!("Neural Engine: {}", line)
                            }));
                        }
                    }
                    _ => {}
                }
            }
        });

        // Wait for health check
        self.wait_for_ready(app).await
    }

    async fn wait_for_ready(&self, app: &tauri::AppHandle) -> Result<()> {
        let url = format!("http://127.0.0.1:{}/health", self.port);
        // Allow up to 10 minutes (300 * 2s) for the initial 2GB model download
        for i in 0..300 {
            if let Ok(resp) = self.client.get(&url).send().await {
                if resp.status().is_success() {
                    let body: serde_json::Value = resp.json().await?;
                    if body["status"] == "ready" {
                        let _ = app.emit("analysis-progress", serde_json::json!({
                            "status": "batch-planning",
                            "msg": "Neural Engine successfully initialized."
                        }));
                        return Ok(());
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            if i % 10 == 0 && i > 0 {
                tauri_plugin_log::log::info!("Still waiting for Neural Vision Sidecar (attempt {}/300)...", i);
            }
        }
        Err(anyhow!("Neural Vision Sidecar failed to start in time (Timeout after 10 minutes)"))
    }

    pub async fn extract_text(&self, image_path: &std::path::Path) -> Result<String> {
        let _permit = self.ocr_semaphore.acquire().await?;
        let url = format!("http://127.0.0.1:{}/ocr", self.port);
        let req = OCRRequest {
            image_path: image_path.to_str().ok_or_else(|| anyhow!("invalid path"))?.to_string(),
        };

        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            let err: serde_json::Value = resp.json().await.unwrap_or_default();
            return Err(anyhow!("Neural OCR failed: {}", err["detail"]));
        }

        let data: OCRResponse = resp.json().await?;
        Ok(data.text)
    }
}
