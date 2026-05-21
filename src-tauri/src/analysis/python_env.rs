use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use tauri::Manager;
use tokio::fs;

/// Downloads a file with progress reporting via Tauri events.
pub async fn download_file(
    client: &Client,
    url: &str,
    path: &Path,
    app: &tauri::AppHandle,
    msg: &str,
) -> Result<()> {
    log::info!("Downloading {} from {}", msg, url);
    let response = client
        .get(url)
        .send()
        .await
        .context(format!("Failed to start download from {}", url))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut file = fs::File::create(path)
        .await
        .context(format!("Failed to create file at {:?}", path))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading chunk")?;
        tokio::io::copy(&mut &chunk[..], &mut file).await?;
        downloaded += chunk.len() as u64;

        if total_size > 0 {
            let progress = (downloaded as f64 / total_size as f64) * 100.0;
            let _ = app.emit(
                "analysis-progress",
                serde_json::json!({
                    "status": "loading-ocr-engine",
                    "progress": progress,
                    "msg": format!("{} ({:.1}%)", msg, progress)
                }),
            );
        }
    }
    log::info!("Download complete: {}", msg);
    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn extract_zip(archive_path: PathBuf, dest_dir: PathBuf) -> Result<()> {
    log::info!("Extracting ZIP {:?} to {:?}", archive_path, dest_dir);
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
    })
    .await?
    .context("ZIP extraction failed")?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub async fn extract_targz(archive_path: PathBuf, dest_dir: PathBuf) -> Result<()> {
    log::info!("Extracting TAR.GZ {:?} to {:?}", archive_path, dest_dir);
    tokio::task::spawn_blocking(move || {
        let tar_gz = std::fs::File::open(&archive_path)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&dest_dir)?;
        anyhow::Ok(())
    })
    .await?
    .context("TAR.GZ extraction failed")?;
    Ok(())
}

/// Resolves a path for a project resource, handling both dev and prod environments.
pub async fn get_resource_path(app: &tauri::AppHandle, relative_path: &str) -> Result<PathBuf> {
    // In production, use Tauri's resource resolver
    if !cfg!(debug_assertions) {
        return app
            .path()
            .resolve(relative_path, tauri::path::BaseDirectory::Resource)
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
    app.path()
        .resolve(relative_path, tauri::path::BaseDirectory::Resource)
        .context(format!("Failed to resolve resource: {}", relative_path))
}

/// Checks if the Python runtime and all dependencies are already provisioned.
pub async fn is_provisioned(app: &tauri::AppHandle) -> bool {
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

/// Provision the isolated Python environment and its dependencies.
pub async fn provision_python(app: &tauri::AppHandle) -> Result<PathBuf> {
    let app_data = app.path().app_data_dir()?;
    let py_env_dir = app_data.join("python-runtime");
    let marker_file = py_env_dir.join("provisioned.ok");

    let (python_exe, _pip_exe) = if cfg!(windows) {
        (py_env_dir.join("python.exe"), py_env_dir.join("python.exe"))
    } else {
        let base = py_env_dir.join("python");
        (
            base.join("bin").join("python3"),
            base.join("bin").join("pip3"),
        )
    };

    if marker_file.exists() && python_exe.exists() {
        log::info!("Python runtime already provisioned at {:?}", python_exe);
        return Ok(python_exe);
    }

    log::info!("Starting Python runtime provisioning...");
    let _ = app.emit(
        "analysis-progress",
        serde_json::json!({
            "status": "loading-ocr-engine",
            "msg": "Provisioning Neural Vision Runtime (First time only)..."
        }),
    );

    if let Err(e) = (|| async {
        if !py_env_dir.exists() {
            fs::create_dir_all(&py_env_dir).await?;
        }

        let client = Client::new();
        let _ = &client; // silence unused warning on non-win/mac platforms

        #[cfg(target_os = "windows")]
        {
            if !python_exe.exists() {
                let url = "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip";
                let zip_path = py_env_dir.join("python.zip");
                download_file(&client, url, &zip_path, app, "Downloading Python distribution").await?;
                extract_zip(zip_path.clone(), py_env_dir.clone()).await?;
                let _ = fs::remove_file(zip_path).await;

                // Bootstrap Pip
                let get_pip_url = "https://bootstrap.pypa.io/get-pip.py";
                let get_pip_path = py_env_dir.join("get-pip.py");
                download_file(&client, get_pip_url, &get_pip_path, app, "Bootstrapping pip").await?;

                let out = tokio::process::Command::new(&python_exe)
                    .arg(&get_pip_path)
                    .output()
                    .await?;
                if !out.status.success() {
                    return Err(anyhow!(
                        "Failed to install pip: {}",
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
                let _ = fs::remove_file(get_pip_path).await;

                // Enable site-packages in ._pth file
                if let Ok(mut entries) = std::fs::read_dir(&py_env_dir) {
                    while let Some(Ok(entry)) = entries.next() {
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
                let arch = if cfg!(target_arch = "aarch64") {
                    "aarch64"
                } else {
                    "x86_64"
                };
                let url = format!("https://github.com/indygreg/python-build-standalone/releases/download/20240107/cpython-3.11.7+20240107-{}-apple-darwin-install_only.tar.gz", arch);
                let tar_path = py_env_dir.join("python.tar.gz");
                download_file(&client, &url, &tar_path, app, "Downloading Python distribution").await?;
                extract_targz(tar_path.clone(), py_env_dir.clone()).await?;
                let _ = fs::remove_file(tar_path).await;
            }
        }

        // Install Requirements
        let req_file = get_resource_path(app, "src-python/requirements.txt").await?;

        let _ = app.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "loading-ocr-engine",
                "msg": "Installing Neural Vision dependencies (this may take a few minutes)..."
            }),
        );

        // Always use python -m pip to avoid shebang path issues with spaces
        let out = tokio::process::Command::new(&python_exe)
            .args(&["-m", "pip", "install", "--no-cache-dir", "-r"])
            .arg(&req_file)
            .output()
            .await?;

        if !out.status.success() {
            return Err(anyhow!(
                "Failed to install dependencies: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }

        // Create marker file on success
        std::fs::write(&marker_file, "provisioned")?;

        anyhow::Ok(())
    })()
    .await
    {
        log::error!("Python provisioning failed: {}", e);
        let _ = app.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "failed",
                "error": format!("Python Provisioning Failed: {}", e)
            }),
        );
        return Err(e);
    }

    log::info!("Python provisioning complete.");
    Ok(python_exe)
}
