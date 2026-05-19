use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Arc;
use tokio::sync::Mutex;

use tauri::Emitter;

#[cfg(debug_assertions)]
async fn setup_python_env(app: &tauri::AppHandle, py_dir: &std::path::Path) -> Result<String> {
    use tokio::process::Command;
    let venv_dir = py_dir.join("venv");
    let req_file = py_dir.join("requirements.txt");
    
    let python_cmd = if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    };

    if !python_cmd.exists() {
        let _ = app.emit("analysis-progress", serde_json::json!({
            "status": "loading-model",
            "msg": "Creating Python virtual environment..."
        }));

        let out = Command::new("python3")
            .arg("-m")
            .arg("venv")
            .arg(&venv_dir)
            .output()
            .await?;
            
        if !out.status.success() {
            return Err(anyhow!("Failed to create venv: {}", String::from_utf8_lossy(&out.stderr)));
        }

        let _ = app.emit("analysis-progress", serde_json::json!({
            "status": "loading-model",
            "msg": "Installing GOT-OCR-2.0 dependencies (this may take a few minutes)..."
        }));

        let pip_cmd = if cfg!(windows) {
            venv_dir.join("Scripts").join("pip.exe")
        } else {
            venv_dir.join("bin").join("pip")
        };

        let out = Command::new(pip_cmd)
            .arg("install")
            .arg("-r")
            .arg(&req_file)
            .output()
            .await?;

        if !out.status.success() {
            return Err(anyhow!("Failed to install dependencies: {}", String::from_utf8_lossy(&out.stderr)));
        }
    }

    Ok(python_cmd.to_string_lossy().to_string())
}

#[derive(Serialize)]
struct OCRRequest {
    image_path: String,
}

#[derive(Deserialize)]
struct OCRResponse {
    text: String,
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
                .timeout(Duration::from_secs(300)) // Long timeout for large OCR tasks
                .build()
                .unwrap(),
            port: 8374,
            child: Arc::new(Mutex::new(None)),
            ocr_semaphore: tokio::sync::Semaphore::new(1),
        }
    }

    pub async fn start(&self, app: &tauri::AppHandle) -> Result<()> {
        let mut child_guard = self.child.lock().await;
        if child_guard.is_some() {
            return Ok(());
        }

        let port_str = self.port.to_string();

        // In development, we use 'python3' as a standard command.
        // In production, we assume a bundled sidecar binary named 'got-ocr'.
        #[cfg(debug_assertions)]
        let sidecar_cmd = {
            let current_dir = std::env::current_dir()?;
            let py_dir = current_dir.parent().unwrap().join("src-python");
            let py_script = py_dir.join("main.py");
            
            let python_exe = setup_python_env(app, &py_dir).await?;
            app.shell().command(python_exe).args(vec![py_script.to_str().unwrap().to_string()])
        };

        #[cfg(not(debug_assertions))]
        let sidecar_cmd = app.shell().sidecar("got-ocr").map_err(|e| anyhow!("failed to find bundled sidecar: {}", e))?;

        let sidecar_cmd = sidecar_cmd.env("PORT", &port_str);

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
