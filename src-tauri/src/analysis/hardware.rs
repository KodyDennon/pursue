use serde::{Deserialize, Serialize};

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
}

pub fn cpu_inference_threads() -> usize {
    (num_cpus::get() / 2).max(1)
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
        _ => AccelerationPreference::Auto,
    }
}

pub fn acceleration_backends() -> Vec<AccelerationBackend> {
    vec![
        AccelerationBackend {
            id: "cuda".to_string(),
            label: "NVIDIA CUDA".to_string(),
            compiled: cfg!(feature = "cuda"),
            available: candle_core::utils::cuda_is_available(),
            notes: if cfg!(feature = "cuda") {
                "Candle and ONNX CUDA execution providers are compiled in.".to_string()
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
            compiled: cfg!(any(feature = "directml", target_os = "windows")),
            available: cfg!(target_os = "windows"),
            notes: if cfg!(target_os = "windows") {
                "ONNX DirectML is used for embeddings when available; Candle LLM remains CUDA/Metal/CPU."
                    .to_string()
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
            match candle_core::Device::new_cuda(0) {
                Ok(device) => candidates.push((
                    "CUDA:0 full GPU tensors; CPU fallback enabled".to_string(),
                    device,
                )),
                Err(error) => log::warn!("CUDA reported available but device 0 failed: {error}"),
            }
        }
    };

    let push_metal = |candidates: &mut Vec<(String, candle_core::Device)>| {
        if candle_core::utils::metal_is_available() {
            match candle_core::Device::new_metal(0) {
                Ok(device) => candidates.push((
                    "Metal:0 full GPU tensors; CPU fallback enabled".to_string(),
                    device,
                )),
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
}
