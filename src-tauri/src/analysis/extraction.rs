use anyhow::{anyhow, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::Manager;

use once_cell::sync::Lazy;

static LLAMA_BACKEND: Lazy<Result<LlamaBackend, String>> = Lazy::new(|| {
    LlamaBackend::init().map_err(|e| e.to_string())
});

use crate::commands::AppState;

pub struct IntelligenceExtractor {
    backend: &'static LlamaBackend,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractionConfig {
    pub preferred_model_path: Option<PathBuf>,
    pub fallback_model_path: Option<PathBuf>,
    pub force_cpu: bool,
}

impl IntelligenceExtractor {
    pub fn new() -> Result<Self> {
        let backend = LLAMA_BACKEND.as_ref()
            .map_err(|e| anyhow!("LlamaBackend initialization failed: {}", e))?;
        Ok(Self {
            backend,
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
        let model_path = config.preferred_model_path
            .or(config.fallback_model_path)
            .ok_or_else(|| anyhow!("No model path provided for forensics"))?;
            
        // For forensics, we use a specialized system prompt that encourages visual-to-text audit
        self.extract_metadata(app, record_id, model_path, text, Some(images)).await
    }

    pub async fn extract_metadata(
        &self, 
        app: &tauri::AppHandle, 
        record_id: &str, 
        model_path: PathBuf, 
        text: &str,
        images: Option<Vec<PathBuf>>
    ) -> Result<Value> {
        let backend = self.backend;
        let text = text.to_string();
        let handle = app.clone();
        let rid = record_id.to_string();
        let db = app.state::<AppState>().db.clone();

        // LLM inference is heavy, run on blocking thread
        tokio::task::spawn_blocking(move || {
            use tauri::Emitter;
            let model_params = LlamaModelParams::default();
            
            let model = match LlamaModel::load_from_file(backend, &model_path, &model_params) {
                Ok(m) => m,
                Err(e) => {
                    let _ = std::fs::remove_file(&model_path);
                    return Err(anyhow!("Intelligence model load failure ({:?}). Corrupted file purged. Re-initiate analysis to trigger a fresh download.", e));
                }
            };

            let ctx_params = LlamaContextParams::default();
            
            let mut ctx = model
                .new_context(backend, ctx_params)
                .map_err(|e| anyhow!("new_context failed: {:?}", e))?;

            // Signal beginning of reasoning phase
            let _ = handle.emit("analysis-progress", serde_json::json!({
                "status": "thought",
                "record_id": rid,
                "current": 0,
                "total": 1
            }));

            // Fetch dynamic persona modifier from settings
            let persona_modifier = match futures::executor::block_on(sqlx::query("SELECT value_json FROM app_settings WHERE key = 'intelligence_persona'")
                .fetch_optional(&db)) {
                    Ok(Some(row)) => {
                        let val: String = sqlx::Row::get(&row, "value_json");
                        serde_json::from_str::<String>(&val).unwrap_or_default()
                    },
                    _ => "".to_string(),
                };

            let image_context = if let Some(imgs) = &images {
                format!("Visual Context: {} asset(s) provided. RECONCILE TEXT AGAINST VISUALS. IDENTIFY REDACTIONS.", imgs.len())
            } else {
                "".to_string()
            };

            // Trigger thinking with <|think|> at the start of the system prompt.
            let system_prompt = format!(
                "Role: PURSUE Intelligence Engine - High-Fidelity Forensic OSINT Analyzer.\n\
                Directives:\n\
                1. NEUTRALITY: Maintain absolute forensic neutrality.\n\
                2. VISUAL RECONCILIATION: Compare provided OCR text against the visual file structure. Identify redactions.\n\
                3. ERROR CORRECTION: If OCR text contradicts visual layout, correct the text in the 'corrections' block.\n\
                4. REDACTION PROFILING: Analyze black blocks to infer hidden data types.\n\
                5. MODIFIER: {}\n\
                {}",
                persona_modifier,
                image_context
            );

            let user_prompt = format!(
                "Source Context:\n\
                {}\n\n\
                Extraction Schema:\n\
                {{\n\
                  \"incident_date\": \"YYYY-MM-DD\",\n\
                  \"location\": \"Location String\",\n\
                  \"agencies\": [\"List\"],\n\
                  \"object_description\": \"Detailed physical synthesis\",\n\
                  \"pilot_observations\": \"Direct sensor/testimony data\",\n\
                  \"redaction_summary\": \"Analysis of withheld data\",\n\
                  \"corrections\": [{{ \"original\": \"...\", \"corrected\": \"...\", \"reason\": \"...\" }}],\n\
                  \"redaction_profiles\": [{{ \"description\": \"...\", \"suspected_content\": \"...\", \"confidence\": 0.0 }}],\n\
                  \"intelligence_score\": 0.0\n\
                }}",
                text
            );

            let prompt = format!(
                "<|think|>\n<start_of_turn>system\n{}\n<end_of_turn>\n<start_of_turn>user\n{}\n<end_of_turn>\n<start_of_turn>thought\n",
                system_prompt,
                user_prompt
            );

            let tokens = model
                .str_to_token(&prompt, AddBos::Always)
                .map_err(|e| anyhow!("str_to_token failed: {:?}", e))?;

            let mut batch = LlamaBatch::get_one(&tokens).map_err(|e| anyhow!("{:?}", e))?;
            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("decode failed: {:?}", e))?;

            // Standardized Gemma 4 Sampling Parameters
            let mut sampler = LlamaSampler::chain_simple(vec![
                LlamaSampler::temp(1.0),
                LlamaSampler::top_p(0.95, 1),
                LlamaSampler::top_k(64),
            ]);
            
            let mut response = String::new();
            let mut n_cur = tokens.len() as i32;
            let mut decoder = encoding_rs::UTF_8.new_decoder();

            // 1. Thinking Phase: Generate internal reasoning
            for _ in 0..1024 { // Increased thinking budget
                let token = sampler.sample(&ctx, n_cur - 1);
                sampler.accept(token);
                if model.is_eog_token(token) { break; }
                let piece = model.token_to_piece(token, &mut decoder, true, None).map_err(|e| anyhow!("{:?}", e))?;
                response.push_str(&piece);
                let t_arr = [token];
                let mut batch = LlamaBatch::get_one(&t_arr).map_err(|e| anyhow!("{:?}", e))?;
                ctx.decode(&mut batch).map_err(|e| anyhow!("decode failed: {:?}", e))?;
                n_cur += 1;
            }

            // Transition to Final Extraction
            response.push_str("\n<end_of_turn>\n<start_of_turn>model\n");
            let next_prompt = "\n<end_of_turn>\n<start_of_turn>model\n";
            let next_tokens = model.str_to_token(next_prompt, AddBos::Never).map_err(|e| anyhow!("{:?}", e))?;
            let mut next_batch = LlamaBatch::get_one(&next_tokens).map_err(|e| anyhow!("{:?}", e))?;
            ctx.decode(&mut next_batch).map_err(|e| anyhow!("decode failed: {:?}", e))?;
            n_cur += next_tokens.len() as i32;

            let mut json_response = String::new();
            // 2. Generation Phase: Produce the structured intelligence report
            for _ in 0..1024 {
                let token = sampler.sample(&ctx, n_cur - 1);
                sampler.accept(token);
                if model.is_eog_token(token) { break; }
                let piece = model.token_to_piece(token, &mut decoder, true, None).map_err(|e| anyhow!("{:?}", e))?;
                json_response.push_str(&piece);
                let t_arr = [token];
                let mut batch = LlamaBatch::get_one(&t_arr).map_err(|e| anyhow!("{:?}", e))?;
                ctx.decode(&mut batch).map_err(|e| anyhow!("decode failed: {:?}", e))?;
                n_cur += 1;
            }

            let response_text = json_response.trim();
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
                if let Ok(mut val) = serde_json::from_str::<Value>(json_str) {
                    // Persist the full intelligence log including the thoughts
                    let log_id = uuid::Uuid::new_v4().to_string();
                    let created_at = chrono::Utc::now().to_rfc3339();
                    
                    let _ = futures::executor::block_on(sqlx::query(
                        "INSERT INTO intelligence_logs (id, record_id, system_prompt, user_prompt, thought_block, response_json, model_id, created_at) \
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(&log_id)
                    .bind(&rid)
                    .bind(&system_prompt)
                    .bind(&user_prompt)
                    .bind(&response)
                    .bind(json_str)
                    .bind(model_path.to_string_lossy().to_string())
                    .bind(&created_at)
                    .execute(&db));

                    if let Some(obj) = val.as_object_mut() {
                        obj.insert("thought_log".to_string(), json!(response));
                        obj.insert("log_id".to_string(), json!(log_id));
                    }
                    return Ok(val);
                }
            }

            // Fallback if JSON parsing fails
            Ok(json!({
                "raw_response": response,
                "error": "Failed to parse structured JSON from model",
                "thought_log": response
            }))
        })
        .await?
    }
}
