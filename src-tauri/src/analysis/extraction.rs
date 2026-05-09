use anyhow::{anyhow, Result};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct IntelligenceExtractor {
    backend: LlamaBackend,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractionConfig {
    pub preferred_model_path: Option<PathBuf>,
    pub fallback_model_path: Option<PathBuf>,
    pub force_cpu: bool,
}

impl IntelligenceExtractor {
    pub fn new() -> Result<Self> {
        let backend = LlamaBackend::init()?;
        Ok(Self { backend })
    }

    pub async fn load_and_extract(&self, config: ExtractionConfig, text: &str) -> Result<Value> {
        if let Some(path) = &config.preferred_model_path {
            if path.exists() {
                return self.extract_metadata(path.clone(), text).await;
            }
        }
        if let Some(path) = &config.fallback_model_path {
            if path.exists() {
                return self.extract_metadata(path.clone(), text).await;
            }
        }
        Err(anyhow!("No models available for extraction"))
    }

    pub async fn extract_metadata(&self, model_path: PathBuf, _text: &str) -> Result<Value> {
        let model_params = LlamaModelParams::default();
        let _model = LlamaModel::load_from_file(&self.backend, &model_path, &model_params)
            .map_err(|e| anyhow!("LlamaModel::load_from_file failed: {:?}", e))?;

        // Placeholder for structured extraction logic
        Ok(json!({
            "incident_date": "2026-05-09",
            "location": "Global Surveillance",
            "agencies": ["AARO", "PURSUE"],
            "object_description": "Intelligence extraction engine online.",
            "pilot_observations": "Automated OSINT analysis in progress.",
            "redaction_summary": "No redactions identified by local brain."
        }))
    }
}
