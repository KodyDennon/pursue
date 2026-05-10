use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::Manager;
use crate::commands::AppState;
use crate::analysis::gemma4;
use crate::common::now;

use candle_core::{DType, Tensor};
use candle_nn::VarBuilder;
use tokenizers::Tokenizer;
use candle_transformers::generation::LogitsProcessor;

pub struct GemmaContext {
    pub model: gemma4::Model,
    pub tokenizer: Tokenizer,
    pub repo_path: PathBuf,
}

pub struct IntelligenceExtractor {
    cache: std::sync::Arc<tokio::sync::Mutex<Option<GemmaContext>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractionConfig {
    pub preferred_model_path: Option<PathBuf>,
    pub fallback_model_path: Option<PathBuf>,
    pub force_cpu: bool,
}

impl IntelligenceExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cache: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    pub async fn extract_forensics(
        &self, 
        app: &tauri::AppHandle, 
        record_id: &str, 
        config: ExtractionConfig, 
        text: &str, 
        images: Vec<PathBuf>
    ) -> Result<Value> {
        let repo_path = config.preferred_model_path
            .or(config.fallback_model_path)
            .ok_or_else(|| anyhow!("No model repository path provided for forensics"))?;
            
        self.extract_metadata(app, record_id, repo_path, text, images).await
    }

    pub async fn extract_metadata(
        &self, 
        app: &tauri::AppHandle, 
        record_id: &str, 
        repo_path: PathBuf, 
        text: &str,
        images: Vec<PathBuf>,
    ) -> Result<Value> {
        let text = text.to_string();
        let handle = app.clone();
        let rid = record_id.to_string();
        let db = app.state::<AppState>().db.clone();
        
        let cache_clone = self.cache.clone();
        let repo_path_clone = repo_path.clone();

        tokio::task::spawn_blocking(move || {
            use tauri::Emitter;
            let mut cache = futures::executor::block_on(cache_clone.lock());
            
            // 1. Check if model is already loaded
            if cache.is_none() || cache.as_ref().unwrap().repo_path != repo_path_clone {
                let _ = handle.emit("analysis-progress", json!({
                    "status": "loading-model",
                    "record_id": rid,
                }));
                *cache = Some(Self::load_context(&repo_path_clone)?);
            }
            let ctx = cache.as_ref().unwrap();
            let device = &ctx.model.device;

            // 2. Retrieval-Augmented Context (Intelligence Graph)
            let related_context = match futures::executor::block_on(crate::search::query_related_fragments(&db, &text, 5)) {
                Ok(fragments) if !fragments.is_empty() => {
                    format!("\nCRITICAL CONTEXT FROM VAULT:\n{}\n", fragments.join("\n---\n"))
                },
                _ => "".to_string(),
            };

            // 3. Multimodal Contextualization
            let image_count = images.len();
            let forensic_audit_note = if image_count > 0 {
                format!("AUDIT NOTICE: {} visual assets are attached. Perform a forensic comparison between the OCR text and the visual artifacts.", image_count)
            } else {
                "No visual assets attached. Rely on OCR data for analysis.".to_string()
            };

            // 3. Construct System Persona
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
                "You are the PURSUE Intelligence OS, an advanced forensic auditor for sensitive documentation. \n\
                Directives:\n\
                1. NEUTRALITY: Maintain absolute forensic neutrality.\n\
                2. STRUCTURE: Return valid JSON only.\n\
                3. AUDIT: {}\n\
                {}\n\n\
                {}\n\n\
                Input Document:\n{}",
                forensic_audit_note,
                persona_modifier,
                related_context,
                text
            );

            let prompt = format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\nPerform a full forensic audit and extract intelligence metadata. Highlight any suspicious patterns or redaction failures.<|im_end|>\n<|im_start|>thought\n",
                system_prompt
            );

            let _ = handle.emit("analysis-progress", json!({
                "status": "synthesizing-start",
                "record_id": rid
            }));

            // 4. Autoregressive Generation with KV-Caching
            let mut tokens = ctx.tokenizer.encode(prompt, true)
                .map_err(|e| anyhow!("Tokenization failed: {}", e))?
                .get_ids()
                .to_vec();

            let mut logits_processor = LogitsProcessor::new(1337, Some(0.0), None);
            let mut generated_text = String::new();
            let mut pos = 0;
            
            // Initial KV Cache: 1 per layer (Gemma 4 has 42 layers in Elite, 26 in E2B)
            let num_layers = ctx.model.layers.len();
            let mut kv_cache = vec![gemma4::KVCache::new(); num_layers];
            
            for i in 0..2048 {
                let context_size = if pos > 0 { 1 } else { tokens.len() };
                let input_tokens = &tokens[tokens.len() - context_size..];
                let input = Tensor::new(input_tokens, device)?.unsqueeze(0)?;
                
                let logits = ctx.model.forward(&input, pos, &mut kv_cache)?;
                let logits = logits.squeeze(0)?.to_dtype(DType::F32)?;
                
                let next_token = logits_processor.sample(&logits)?;
                tokens.push(next_token);
                pos += context_size;

                if let Some(decoded) = ctx.tokenizer.id_to_token(next_token) {
                    if decoded == "<|im_end|>" || decoded == "<|file_separator|>" || next_token == 1 {
                        break;
                    }
                    if let Ok(piece) = ctx.tokenizer.decode(&[next_token], true) {
                        generated_text.push_str(&piece);
                    }
                }

                if i % 20 == 0 {
                    let _ = handle.emit("analysis-progress", json!({
                        "status": "synthesizing",
                        "record_id": rid,
                        "token_index": i,
                        "token_limit": 2048
                    }));
                }
            }

            // 5. Audit Logging (Neural Thought Log)
            let log_id = uuid::Uuid::new_v4().to_string();
            let _ = futures::executor::block_on(sqlx::query("INSERT INTO neural_thought_logs (id, record_id, system_prompt, user_prompt, thought_block, response_json, model_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&log_id).bind(&rid).bind(&system_prompt).bind("metadata-extraction").bind(Option::<String>::None).bind(&generated_text).bind(repo_path_clone.to_string_lossy().to_string()).bind(now())
                .execute(&db));

            // 6. Parse JSON Result
            let json_start = generated_text.find('{').unwrap_or(0);
            let json_end = generated_text.rfind('}').map(|i| i + 1).unwrap_or(generated_text.len());
            let json_str = &generated_text[json_start..json_end];

            match serde_json::from_str::<Value>(json_str) {
                Ok(response) => {
                    // 7. Persist Intelligence Fragments for future RAG
                    if let Some(obs) = response.get("observations").and_then(|a| a.as_array()) {
                        for item in obs {
                            if let Some(txt) = item.as_str() {
                                let fid = uuid::Uuid::new_v4().to_string();
                                let _ = futures::executor::block_on(sqlx::query("INSERT INTO intelligence_fragments (id, record_id, fragment_type, text, confidence, created_at) VALUES (?, ?, 'observation', ?, 0.9, ?)")
                                    .bind(&fid).bind(&rid).bind(txt).bind(now()).execute(&db));
                                
                                // Vectorize fragment
                                if let Ok(emb) = futures::executor::block_on(crate::search::vectorize_text(txt)) {
                                    let vblob: &[u8] = unsafe { std::slice::from_raw_parts(emb.as_ptr() as *const u8, emb.len() * 4) };
                                    let _ = futures::executor::block_on(sqlx::query("INSERT INTO vec_intelligence_fragments (fragment_id, embedding) VALUES (?, ?)")
                                        .bind(&fid).bind(vblob).execute(&db));
                                }
                            }
                        }
                    }
                    Ok(response)
                },
                Err(_) => {
                    Ok(json!({
                        "object_description": "Failed to parse structured response",
                        "raw_response": generated_text,
                        "error": "Structured extraction produced invalid JSON",
                        "thought_log": "Model completed pass but output validation failed."
                    }))
                }
            }
        })
        .await?
    }

    fn load_context(repo_path: &PathBuf) -> Result<GemmaContext> {
        if !repo_path.exists() {
            return Err(anyhow!("Intelligence repository missing: {:?}.", repo_path));
        }

        let device = if cfg!(target_os = "macos") {
            candle_core::Device::new_metal(0).unwrap_or(candle_core::Device::Cpu)
        } else {
            candle_core::Device::Cpu
        };

        // 1. Load Config
        let config_path = repo_path.join("config.json");
        let config_data = std::fs::read_to_string(&config_path)?;
        let config_wrapper: gemma4::ConfigWrapper = serde_json::from_str(&config_data)?;
        let config = config_wrapper.extract().map_err(|e| anyhow!("{}", e))?;

        // 2. Load Weights
        let mut safetensors_paths = Vec::new();
        for entry in std::fs::read_dir(&repo_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
                safetensors_paths.push(path);
            }
        }
        if safetensors_paths.is_empty() {
            return Err(anyhow!("No .safetensors files found in {:?}", repo_path));
        }
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&safetensors_paths, DType::F32, &device)?
        };

        // 3. Init Model
        let model = gemma4::Model::new(&config, vb)?;

        // 4. Init Tokenizer
        let tokenizer_path = repo_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;

        Ok(GemmaContext {
            model,
            tokenizer,
            repo_path: repo_path.clone(),
        })
    }
}
