use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum IntelligenceTier {
    Standard, // 2B models, CPU-only
    Advanced, // 2B models, GPU-accelerated
    Elite,    // 4B+ models, High-VRAM GPU
}

pub fn now() -> String {
    Utc::now().to_rfc3339()
}

pub fn to_error(error: impl std::fmt::Display) -> String {
    let msg = error.to_string();
    log::error!("Backend error: {}", msg);
    msg
}
