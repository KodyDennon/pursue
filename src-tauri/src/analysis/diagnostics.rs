use serde::{Deserialize, Serialize};
use sysinfo::{Components, Disks, Networks, System, Users};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum IntelligenceTier {
    Draft, // Basic OCR + Heuristic Extraction
    Deep,  // PaddleOCR + Gemma 4 E2B (8GB RAM)
    Elite, // PaddleOCR + Gemma 4 E4B/26B (16GB+ RAM)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HardwareSpecs {
    pub total_memory_gb: u64,
    pub available_memory_gb: u64,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub os_info: String,
    pub gpu_acceleration_available: bool,
    pub recommended_tier: IntelligenceTier,
}

pub fn get_hardware_specs() -> HardwareSpecs {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_memory_gb = sys.total_memory() / 1024 / 1024 / 1024;
    let available_memory_gb = sys.available_memory() / 1024 / 1024 / 1024;
    let cpu_cores = sys.cpus().len();
    let cpu_brand = if !sys.cpus().is_empty() {
        sys.cpus()[0].brand().to_string()
    } else {
        "Unknown".to_string()
    };

    let os_info = format!(
        "{} {}",
        System::name().unwrap_or_default(),
        System::os_version().unwrap_or_default()
    );

    // Basic heuristic for GPU acceleration detection
    // In a real implementation, we'd check for CUDA, Metal, or DirectML explicitly
    let gpu_acceleration_available = cfg!(target_os = "macos") || cfg!(target_os = "windows");

    let recommended_tier = if total_memory_gb >= 16 {
        IntelligenceTier::Elite
    } else if total_memory_gb >= 8 {
        IntelligenceTier::Deep
    } else {
        IntelligenceTier::Draft
    };

    HardwareSpecs {
        total_memory_gb,
        available_memory_gb,
        cpu_cores,
        cpu_brand,
        os_info,
        gpu_acceleration_available,
        recommended_tier,
    }
}
