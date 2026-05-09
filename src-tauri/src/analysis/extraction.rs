use anyhow::{anyhow, Result};
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::batch::LlamaBatch;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::context::params::LlamaContextParams;
use std::path::PathBuf;
use serde_json::{json, Value};

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

    pub async fn load_and_extract(
        &self,
        config: ExtractionConfig,
        text: &str,
    ) -> Result<Value> {
        // 1. Try preferred model
        if let Some(path) = &config.preferred_model_path {
            match self.extract_metadata(path.clone(), text).await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    eprintln!("Preferred model failed, falling back: {}", e);
                }
            }
        }

        // 2. Fallback to Deep/Draft model
        if let Some(path) = &config.fallback_model_path {
            return self.extract_metadata(path.clone(), text).await;
        }

        Err(anyhow!("No models available for extraction"))
    }

    pub async fn extract_metadata(&self, model_path: PathBuf, text: &str) -> Result<Value> {
        if !model_path.exists() {
            return Err(anyhow!("Model file not found at {}", model_path.display()));
        }
        
        let mut model_params = LlamaModelParams::default();
        // Use GPU by default if available (Metal on Mac)
        model_params.set_n_gpu_layers(100); 

        let model = LlamaModel::load_from_file(&self.backend, &model_path, &model_params)
            .map_err(|e| anyhow!("LlamaModel::load_from_file failed: {:?}", e))?;

        let ctx_params = LlamaContextParams::default();
        let mut ctx = model.new_context(&self.backend, ctx_params)
            .map_err(|_| anyhow!("failed to create llama context"))?;

        // Gemma 4 specific extraction prompt
        let prompt = format!(
            "<start_of_turn>user\nYou are an Elite Intelligence Analyst. Extract every detail from this UFO document into a single JSON object. 
Fields to include: incident_date, location, agencies, object_description, pilot_observations, redaction_summary.

Text:
{}
<end_of_turn>
<start_of_turn>model\n{{",
            text
        );

        // Actual Sampling Loop
        let mut tokens = Vec::new();
        let mut n_curr = 0;
        let mut batch = LlamaBatch::new(prompt.len() as usize, 1);
        
        // Tokenize prompt
        let prompt_tokens = model.tokenize(&prompt, true, false)?;
        for (i, &token) in prompt_tokens.iter().enumerate() {
            batch.add(token, i as i32, &[0], i == prompt_tokens.len() - 1);
        }

        ctx.decode(&mut batch).map_err(|_| anyhow!("decode failed"))?;

        let mut n_decode = 0;
        let mut result_text = String::new();

        while n_decode < 512 {
            let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
            let candidates_p = LlamaTokenDataArray::from_iter(candidates, false);
            let token = ctx.sample_token_greedy(candidates_p);

            if token == model.token_eos() {
                break;
            }

            let piece = model.token_to_piece(token, true)?;
            result_text.push_str(&piece);
            
            tokens.push(token);
            batch.clear();
            batch.add(token, n_curr as i32, &[0], true);
            
            ctx.decode(&mut batch).map_err(|_| anyhow!("decode failed"))?;
            n_curr += 1;
            n_decode += 1;
        }

        // Parse generated JSON
        let cleaned_json = result_text.trim();
        let val: Value = serde_json::from_str(cleaned_json)
            .unwrap_or_else(|_| json!({ "raw_text": result_text, "status": "partial_parse_failure" }));

        Ok(val)
    }
}
