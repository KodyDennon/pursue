use serde::{Deserialize, Serialize};
use sysinfo::System;

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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub memory_usage_mb: u64,
    pub process_memory_mb: u64,
    pub timestamp: String,
}

pub fn get_system_stats() -> SystemStats {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let cpu_usage = sys.global_cpu_usage();
    let memory_usage_mb = (sys.total_memory() - sys.available_memory()) / 1024 / 1024;
    
    // Get process memory
    let pid = sysinfo::get_current_pid().ok();
    let process_memory_mb = if let Some(p) = pid {
        if let Some(process) = sys.process(p) {
            process.memory() / 1024 / 1024
        } else {
            0
        }
    } else {
        0
    };

    SystemStats {
        cpu_usage,
        memory_usage_mb,
        process_memory_mb,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}
