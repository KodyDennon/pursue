use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

static ACTIVE_INFERENCE_BACKENDS: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccelerationBackend {
    pub id: String,
    pub label: String,
    pub compiled: bool,
    pub available: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelerationPreference {
    Auto,
    Cpu,
    Cuda,
    Metal,
    DirectMl,
}

fn active_backend_registry() -> &'static Mutex<BTreeMap<String, String>> {
    ACTIVE_INFERENCE_BACKENDS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Records the provider that successfully initialized for a concrete workload. Diagnostics use
/// this instead of inferring activity from compile-time features or device presence.
pub fn record_active_inference_backend(workload: &str, backend: &str) {
    if let Ok(mut backends) = active_backend_registry().lock() {
        backends.insert(workload.to_string(), backend.to_string());
    }
}

pub fn clear_active_inference_backend(workload: &str) {
    if let Ok(mut backends) = active_backend_registry().lock() {
        backends.remove(workload);
    }
}

pub fn active_inference_backends() -> BTreeMap<String, String> {
    active_backend_registry()
        .lock()
        .map(|backends| backends.clone())
        .unwrap_or_default()
}

pub fn cpu_inference_threads() -> usize {
    (num_cpus::get() / 2).max(1)
}

pub fn cuda_device_id() -> i32 {
    std::env::var("PURSUE_CUDA_DEVICE")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0)
}

pub fn cuda_memory_limit_bytes() -> Option<usize> {
    std::env::var("PURSUE_CUDA_MEMORY_LIMIT_MB")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|mb| mb.saturating_mul(1024 * 1024))
}

pub fn cuda_memory_limit_label() -> String {
    cuda_memory_limit_bytes()
        .map(|bytes| format!("{} MiB arena", bytes / 1024 / 1024))
        .unwrap_or_else(|| "default arena".to_string())
}

pub fn acceleration_preference(force_cpu: bool) -> AccelerationPreference {
    if force_cpu {
        return AccelerationPreference::Cpu;
    }

    let raw = std::env::var("PURSUE_ACCELERATION")
        .or_else(|_| std::env::var("PURSUE_DEVICE"))
        .unwrap_or_else(|_| "auto".to_string())
        .to_lowercase();

    match raw.as_str() {
        "cpu" | "off" | "disabled" => AccelerationPreference::Cpu,
        "cuda" | "nvidia" => AccelerationPreference::Cuda,
        "metal" | "mps" | "apple" => AccelerationPreference::Metal,
        "directml" | "dml" | "windows" => AccelerationPreference::DirectMl,
        _ => AccelerationPreference::Auto,
    }
}

pub fn acceleration_backends() -> Vec<AccelerationBackend> {
    let directml_compiled = cfg!(feature = "directml");
    vec![
        AccelerationBackend {
            id: "cuda".to_string(),
            label: "NVIDIA CUDA".to_string(),
            compiled: cfg!(feature = "cuda"),
            available: candle_core::utils::cuda_is_available(),
            notes: if cfg!(feature = "cuda") {
                format!(
                    "Candle and ONNX CUDA execution providers are compiled in for device {} ({}).",
                    cuda_device_id(),
                    cuda_memory_limit_label()
                )
            } else {
                "Build with --features cuda to enable native NVIDIA GPU execution.".to_string()
            },
        },
        AccelerationBackend {
            id: "metal".to_string(),
            label: "Apple Metal/CoreML".to_string(),
            compiled: cfg!(feature = "metal"),
            available: candle_core::utils::metal_is_available(),
            notes: if cfg!(feature = "metal") {
                "Candle Metal and ONNX CoreML execution providers are compiled in.".to_string()
            } else {
                "Build with --features metal on Apple Silicon to enable native acceleration."
                    .to_string()
            },
        },
        AccelerationBackend {
            id: "directml".to_string(),
            label: "Windows DirectML".to_string(),
            compiled: directml_compiled,
            available: directml_compiled && cfg!(target_os = "windows"),
            notes: if directml_compiled && cfg!(target_os = "windows") {
                "ONNX DirectML is compiled in for broad Windows GPU fallback; Candle LLM remains CUDA/Metal/CPU."
                    .to_string()
            } else if cfg!(target_os = "windows") {
                "Build with --features directml to enable broad Windows GPU execution.".to_string()
            } else {
                "DirectML is only available on Windows.".to_string()
            },
        },
        AccelerationBackend {
            id: "cpu".to_string(),
            label: "CPU".to_string(),
            compiled: true,
            available: true,
            notes: format!(
                "Always available fallback using {} inference thread(s).",
                cpu_inference_threads()
            ),
        },
    ]
}

pub fn acceleration_recommendation() -> String {
    let backends = acceleration_backends();
    let cuda = backends.iter().find(|backend| backend.id == "cuda");
    let directml = backends.iter().find(|backend| backend.id == "directml");

    if cuda.is_some_and(|backend| backend.compiled && backend.available) {
        "Using NVIDIA CUDA first, with DirectML/CPU fallback where supported.".to_string()
    } else if cfg!(target_os = "windows")
        && cuda.is_some_and(|backend| backend.compiled)
        && directml.is_some_and(|backend| backend.compiled && backend.available)
    {
        "CUDA build installed, but no NVIDIA CUDA device/runtime was detected; using DirectML/CPU fallback."
            .to_string()
    } else if cfg!(target_os = "windows")
        && directml.is_some_and(|backend| backend.compiled && backend.available)
    {
        "Using standard Windows DirectML acceleration; install the CUDA build on NVIDIA GPU systems."
            .to_string()
    } else if cfg!(target_os = "windows") {
        "No Windows GPU provider is active; processing will use CPU fallback.".to_string()
    } else {
        "Using the best compiled native acceleration provider with CPU fallback.".to_string()
    }
}

pub fn gpu_acceleration_available() -> bool {
    acceleration_backends()
        .into_iter()
        .any(|backend| backend.id != "cpu" && backend.compiled && backend.available)
}

pub fn acceleration_summary() -> String {
    let active = acceleration_backends()
        .into_iter()
        .filter(|backend| backend.id != "cpu" && backend.compiled && backend.available)
        .map(|backend| backend.label)
        .collect::<Vec<_>>();

    if active.is_empty() {
        format!("CPU Only ({} threads)", cpu_inference_threads())
    } else {
        format!("{} + CPU fallback", active.join(" / "))
    }
}

pub fn candle_device_candidates(force_cpu: bool) -> Vec<(String, candle_core::Device)> {
    let preference = acceleration_preference(force_cpu);
    let mut candidates = Vec::new();

    let push_cuda = |candidates: &mut Vec<(String, candle_core::Device)>| {
        if candle_core::utils::cuda_is_available() {
            let device_id = cuda_device_id().max(0) as usize;
            match candle_core::Device::new_cuda(device_id) {
                Ok(device) => match cuda_kernel_smoke_test(&device) {
                    Ok(()) => candidates.push((
                        format!("CUDA:{} full GPU tensors; CPU fallback enabled", device_id),
                        device,
                    )),
                    Err(error) => log::warn!(
                        "CUDA device {device_id} initialized but failed the kernel smoke test \
                         (binary kernels likely built for a different GPU generation): {error}"
                    ),
                },
                Err(error) => log::warn!("CUDA reported available but device 0 failed: {error}"),
            }
        }
    };

    let push_metal = |candidates: &mut Vec<(String, candle_core::Device)>| {
        if candle_core::utils::metal_is_available() {
            match candle_core::Device::new_metal(0) {
                Ok(device) => match accelerator_kernel_smoke_test(&device) {
                    Ok(()) => candidates.push((
                        "Metal:0 full GPU tensors; CPU fallback enabled".to_string(),
                        device,
                    )),
                    Err(error) => log::warn!(
                        "Metal device 0 initialized but failed the kernel smoke test: {error}"
                    ),
                },
                Err(error) => log::warn!("Metal reported available but device 0 failed: {error}"),
            }
        }
    };

    match preference {
        AccelerationPreference::Cpu => {}
        AccelerationPreference::Cuda => {
            push_cuda(&mut candidates);
        }
        AccelerationPreference::Metal => {
            push_metal(&mut candidates);
        }
        AccelerationPreference::DirectMl => {}
        AccelerationPreference::Auto => {
            push_cuda(&mut candidates);
            push_metal(&mut candidates);
        }
    }

    candidates.push((
        format!("CPU offload/fallback ({} threads)", cpu_inference_threads()),
        candle_core::Device::Cpu,
    ));
    candidates
}

/// Launches a real kernel on the device before we commit to it. Device creation and
/// memcpy succeed even when the binary's CUDA kernels were compiled for a different GPU
/// generation ("no kernel image is available") — only an actual launch surfaces that, and
/// without this probe it used to surface mid-inference with no CPU fallback.
fn cuda_kernel_smoke_test(device: &candle_core::Device) -> candle_core::Result<()> {
    accelerator_kernel_smoke_test(device)
}

fn accelerator_kernel_smoke_test(device: &candle_core::Device) -> candle_core::Result<()> {
    let ones = candle_core::Tensor::ones((4, 4), candle_core::DType::F32, device)?;
    // sum_all runs a custom candle reduce kernel; to_scalar forces synchronization so any
    // deferred launch error is reported here.
    ones.sum_all()?.to_scalar::<f32>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_is_always_available() {
        let backends = acceleration_backends();
        assert!(backends
            .iter()
            .any(|backend| backend.id == "cpu" && backend.available && backend.compiled));
    }

    #[test]
    fn candle_candidates_always_end_with_cpu() {
        let candidates = candle_device_candidates(false);
        let (label, device) = candidates.last().expect("cpu fallback exists");
        assert!(label.contains("CPU"));
        assert!(matches!(device, candle_core::Device::Cpu));
    }

    #[test]
    fn recommendation_is_populated() {
        assert!(!acceleration_recommendation().is_empty());
    }
}
