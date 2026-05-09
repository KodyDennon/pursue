use anyhow::{anyhow, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

pub struct IntelligenceExtractor {
    backend: Arc<LlamaBackend>,
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
        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    pub async fn load_and_extract(&self, config: ExtractionConfig, text: &str) -> Result<Value> {
        let model_path = if let Some(path) = &config.preferred_model_path {
            if path.exists() {
                path.clone()
            } else if let Some(fallback) = &config.fallback_model_path {
                fallback.clone()
            } else {
                anyhow::bail!("Preferred model missing and no fallback provided");
            }
        } else {
            config
                .fallback_model_path
                .ok_or_else(|| anyhow!("No model path provided"))?
        };

        self.extract_metadata(model_path, text).await
    }

    pub async fn extract_metadata(&self, model_path: PathBuf, text: &str) -> Result<Value> {
        let backend = self.backend.clone();
        let text = text.to_string();

        // LLM inference is heavy, run on blocking thread
        tokio::task::spawn_blocking(move || {
            let model_params = LlamaModelParams::default();
            let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
                .map_err(|e| anyhow!("LlamaModel::load_from_file failed: {:?}", e))?;

            let ctx_params = LlamaContextParams::default();
            // Let it default to model's n_ctx
            let mut ctx = model
                .new_context(&backend, ctx_params)
                .map_err(|e| anyhow!("new_context failed: {:?}", e))?;

            let prompt = format!(
                "<start_of_turn>user\nExtract structured intelligence from this text as JSON. \
                Include: incident_date, location, agencies, object_description, pilot_observations, redaction_summary. \
                Text: {}\n<end_of_turn>\n<start_of_turn>model\n",
                text
            );

            let tokens = model
                .str_to_token(&prompt, AddBos::Always)
                .map_err(|e| anyhow!("str_to_token failed: {:?}", e))?;

            let mut batch = LlamaBatch::get_one(&tokens).map_err(|e| anyhow!("{:?}", e))?;
            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("decode failed: {:?}", e))?;

            let mut sampler = LlamaSampler::chain_simple(Vec::<LlamaSampler>::new());
            let mut response = String::new();
            let mut n_cur = tokens.len() as i32;
            let mut decoder = encoding_rs::UTF_8.new_decoder();

            for _ in 0..512 {
                let token = sampler.sample(&ctx, n_cur - 1);
                sampler.accept(token);

                if model.is_eog_token(token) {
                    break;
                }

                let piece = model.token_to_piece(token, &mut decoder, true, None).map_err(|e| anyhow!("{:?}", e))?;
                response.push_str(&piece);

                let single_token = [token];
                let mut batch = LlamaBatch::get_one(&single_token).map_err(|e| anyhow!("{:?}", e))?;
                ctx.decode(&mut batch)
                    .map_err(|e| anyhow!("decode failed: {:?}", e))?;
                n_cur += 1;
            }

            // Robust JSON extractor that handles markdown blocks and trailing text
            let response_text = response.trim();
            let mut balance = 0;
            let mut start_idx = None;
            let mut end_idx = None;
            
            // Strip markdown JSON block ticks if present
            let cleaned = if response_text.starts_with("```json") {
                response_text.trim_start_matches("```json").trim_end_matches("```").trim()
            } else if response_text.starts_with("```") {
                response_text.trim_start_matches("```").trim_end_matches("```").trim()
            } else {
                response_text
            };

            for (i, c) in cleaned.char_indices() {
                if c == '{' {
                    if balance == 0 {
                        start_idx = Some(i);
                    }
                    balance += 1;
                } else if c == '}' {
                    balance -= 1;
                    if balance == 0 && start_idx.is_some() {
                        end_idx = Some(i);
                        break;
                    }
                }
            }

            if let (Some(start), Some(end)) = (start_idx, end_idx) {
                let json_str = &cleaned[start..=end];
                if let Ok(val) = serde_json::from_str(json_str) {
                    return Ok(val);
                }
            }

            // Fallback if JSON parsing fails
            Ok(json!({
                "raw_response": response,
                "error": "Failed to parse structured JSON from model"
            }))
        })
        .await?
    }
}
