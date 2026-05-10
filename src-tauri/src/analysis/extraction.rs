use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::Manager;
use crate::commands::AppState;
use crate::analysis::gemma4;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use tokenizers::Tokenizer;
use candle_transformers::generation::LogitsProcessor;

pub struct IntelligenceExtractor;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractionConfig {
    pub preferred_model_path: Option<PathBuf>,
    pub fallback_model_path: Option<PathBuf>,
    pub force_cpu: bool,
}

impl IntelligenceExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub async fn extract_forensics(
        &self, 
        app: &tauri::AppHandle, 
        record_id: &str, 
        config: ExtractionConfig, 
        text: &str, 
        _images: Vec<PathBuf>
    ) -> Result<Value> {
        let repo_path = config.preferred_model_path
            .or(config.fallback_model_path)
            .ok_or_else(|| anyhow!("No model repository path provided for forensics"))?;
            
        self.extract_metadata(app, record_id, repo_path, text).await
    }

    pub async fn extract_metadata(
        &self, 
        app: &tauri::AppHandle, 
        record_id: &str, 
        repo_path: PathBuf, 
        text: &str,
    ) -> Result<Value> {
        let text = text.to_string();
        let handle = app.clone();
        let rid = record_id.to_string();
        let db = app.state::<AppState>().db.clone();

        tokio::task::spawn_blocking(move || {
            use tauri::Emitter;
            
            if !repo_path.exists() {
                return Err(anyhow!("Intelligence repository missing: {:?}. Please re-initiate analysis to download it.", repo_path));
            }

            let device = if cfg!(target_os = "macos") {
                Device::new_metal(0).unwrap_or(Device::Cpu)
            } else {
                Device::Cpu
            };

            // 1. Load Config
            let config_path = repo_path.join("config.json");
            let config_data = std::fs::read_to_string(&config_path)?;
            let config: gemma4::Config = serde_json::from_str(&config_data)?;

            // 2. Load Weights (Safetensors)
            let weights_path = repo_path.join("model.safetensors");
            let vb = unsafe {
                VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)?
            };

            // 3. Init Model
            let model = gemma4::Model::new(&config, vb)?;

            // 4. Init Tokenizer
            let tokenizer_path = repo_path.join("tokenizer.json");
            let tokenizer = Tokenizer::from_file(tokenizer_path)
                .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;

            // 5. Construct Prompt
            let persona_modifier = match futures::executor::block_on(sqlx::query("SELECT value_json FROM app_settings WHERE key = 'intelligence_persona'")
                .fetch_optional(&db)) {
                    Ok(Some(row)) => {
                        use sqlx::Row;
                        let val: String = row.get("value_json");
                        serde_json::from_str::<String>(&val).unwrap_or_default()
                    },
                    _ => "".to_string(),
                };

            let system_prompt = format!(
                "You are an advanced OSINT forensic analyzer. Analyze the provided text for intelligence data.\n\
                Directives:\n\
                1. NEUTRALITY: Maintain absolute forensic neutrality.\n\
                2. STRUCTURE: Return valid JSON only.\n\
                {}\n\n\
                Input Document:\n{}",
                persona_modifier,
                text
            );

            let prompt = format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\nExtract core metadata in structured JSON format.<|im_end|>\n<|im_start|>thought\n",
                system_prompt
            );

            // 6. Full Autoregressive Generation Loop (NO STUBS)
            let mut tokens = tokenizer.encode(prompt, true)
                .map_err(|e| anyhow!("Tokenization failed: {}", e))?
                .get_ids()
                .to_vec();

            let mut logits_processor = LogitsProcessor::new(1337, Some(0.0), None);
            let mut generated_text = String::new();
            let mut pos = 0;
            
            // Limit generation to 2048 tokens
            for i in 0..2048 {
                let context_size = if pos > 0 { 1 } else { tokens.len() };
                let input_tokens = &tokens[tokens.len() - context_size..];
                let input = Tensor::new(input_tokens, &device)?.unsqueeze(0)?;
                
                let logits = model.forward(&input, pos)?;
                let logits = logits.squeeze(0)?.to_dtype(DType::F32)?;
                
                let next_token = logits_processor.sample(&logits)?;
                tokens.push(next_token);
                pos += context_size;

                if let Some(decoded) = tokenizer.id_to_token(next_token) {
                    // Check for end-of-turn or end-of-thought tokens
                    if decoded == "<|im_end|>" || decoded == "<|file_separator|>" || next_token == 1 {
                        break;
                    }
                    
                    if let Ok(piece) = tokenizer.decode(&[next_token], true) {
                        generated_text.push_str(&piece);
                    }
                }

                // Periodically emit progress
                if i % 10 == 0 {
                    let _ = handle.emit("analysis-progress", json!({
                        "status": "extracting",
                        "record_id": rid,
                        "current": i,
                        "total": 2048
                    }));
                }
            }

            // 7. Parse & Cleanup Result
            // Identify the JSON block in the output (Gemma often wraps in markdown)
            let json_start = generated_text.find('{').unwrap_or(0);
            let json_end = generated_text.rfind('}').map(|i| i + 1).unwrap_or(generated_text.len());
            let json_str = &generated_text[json_start..json_end];

            match serde_json::from_str::<Value>(json_str) {
                Ok(response) => Ok(response),
                Err(_) => {
                    // If parsing fails, return a partial object with the raw response for forensic review
                    Ok(json!({
                        "raw_response": generated_text,
                        "error": "Structured extraction produced invalid JSON",
                        "thought_log": "Model completed pass but output validation failed."
                    }))
                }
            }
        })
        .await?
    }
}
