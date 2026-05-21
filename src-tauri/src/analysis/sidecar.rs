use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::{Arc, Mutex};
use crate::analysis::python_env;

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
    start_mutex: tokio::sync::Mutex<()>,
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
            start_mutex: tokio::sync::Mutex::new(()),
            child: Arc::new(Mutex::new(None)),
            ocr_semaphore: tokio::sync::Semaphore::new(1),
        }
    }

    pub async fn is_provisioned(&self, app: &tauri::AppHandle) -> bool {
        python_env::is_provisioned(app).await
    }

    pub async fn provision(&self, app: &tauri::AppHandle) -> Result<()> {
        python_env::provision_python(app).await.map(|_| ())
    }

    pub async fn start(&self, app: &tauri::AppHandle) -> Result<()> {
        let _start_guard = self.start_mutex.lock().await;

        {
            let child_guard = self.child.lock().unwrap();
            if child_guard.is_some() {
                return Ok(());
            }
        }

        // Check if sidecar is already running (e.g., from an orphaned process)
        if is_port_in_use(self.port) {
            tauri_plugin_log::log::warn!("Port {} is already in use. Killing orphaned process...", self.port);
            kill_port_owner(self.port).await;
            // Sleep briefly to let the OS release the socket
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        let port_str = self.port.to_string();

        let python_exe = python_env::provision_python(app).await?;
        
        let py_script = python_env::get_resource_path(app, "src-python/main.py").await?;

        let sidecar_cmd = app.shell()
            .command(python_exe.to_string_lossy().to_string())
            .args(vec![py_script.to_string_lossy().to_string()])
            .env("PORT", &port_str);

        let (mut rx, child) = sidecar_cmd.spawn().map_err(|e| anyhow!("failed to spawn sidecar: {}", e))?;
        
        {
            let mut child_guard = self.child.lock().unwrap();
            *child_guard = Some(child);
        }

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

impl Drop for VisionSidecar {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.take() {
                tauri_plugin_log::log::info!("VisionSidecar dropped, terminating sidecar process...");
                let _ = child.kill();
            }
        }
    }
}
