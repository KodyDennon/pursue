use std::path::Path;
use tauri_plugin_log::log::warn;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Verifies whether the downloaded model file is corrupted based on its format rules.
pub async fn is_model_corrupted(path: &Path, model_name: &str) -> bool {
    let is_gguf = model_name.ends_with(".gguf");
    let is_safetensors = model_name.ends_with(".safetensors");

    if let Ok(file_metadata) = fs::metadata(path).await {
        let size = file_metadata.len();
        if let Ok(mut file) = tokio::fs::File::open(path).await {
            let mut magic = [0u8; 8];
            if file.read_exact(&mut magic).await.is_ok() {
                let mut is_corrupted = false;
                if is_gguf {
                    if &magic[..4] != b"GGUF" {
                        warn!(
                            "Model file {} is corrupted (invalid GGUF magic).",
                            model_name
                        );
                        is_corrupted = true;
                    } else if size < 100 * 1024 * 1024 {
                        is_corrupted = true;
                    }
                } else if is_safetensors {
                    // Safetensors: first 8 bytes = LE u64 header length
                    let header_len = u64::from_le_bytes(magic);
                    if header_len == 0 || header_len > 100_000_000 {
                        warn!(
                            "Safetensors {} has invalid header length: {}",
                            model_name, header_len
                        );
                        is_corrupted = true;
                    } else {
                        // Read header JSON and sum tensor sizes to get expected file size
                        let mut header_buf = vec![0u8; header_len as usize];
                        if file.read_exact(&mut header_buf).await.is_ok() {
                            if let Ok(header_json) =
                                serde_json::from_slice::<serde_json::Value>(&header_buf)
                            {
                                // Calculate total tensor data size from offsets
                                let mut max_end: u64 = 0;
                                if let Some(obj) = header_json.as_object() {
                                    for (key, val) in obj {
                                        if key == "__metadata__" {
                                            continue;
                                        }
                                        if let Some(offsets) = val.get("data_offsets") {
                                            if let Some(arr) = offsets.as_array() {
                                                if let Some(end) =
                                                    arr.get(1).and_then(|v| v.as_u64())
                                                {
                                                    max_end = max_end.max(end);
                                                }
                                            }
                                        }
                                    }
                                }
                                let expected_size = 8 + header_len + max_end;
                                if size != expected_size {
                                    warn!(
                                        "Safetensors {} size mismatch: file={} expected={}",
                                        model_name, size, expected_size
                                    );
                                    is_corrupted = true;
                                }
                            } else {
                                warn!("Safetensors {} has invalid header JSON", model_name);
                                is_corrupted = true;
                            }
                        } else {
                            is_corrupted = true;
                        }
                    }
                } else if &magic[..4] == b"<!DO" || &magic[..4] == b"<htm" {
                    is_corrupted = true;
                }

                return is_corrupted;
            }
        }
    }
    true
}
