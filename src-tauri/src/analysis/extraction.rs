use crate::analysis::gemma4;
use crate::commands::AppState;
use crate::common::now;
use anyhow::{anyhow, Result};
use log::debug;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use candle_core::{DType, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use tokenizers::Tokenizer;

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
        app: &AppHandle,
        record_id: &str,
        config: ExtractionConfig,
        text: &str,
        images: Vec<PathBuf>,
    ) -> Result<Value> {
        let repo_path = config
            .preferred_model_path
            .or(config.fallback_model_path)
            .ok_or_else(|| anyhow!("No model repository path provided for forensics"))?;

        self.extract_metadata(app, record_id, repo_path, text, images)
            .await
    }

    pub async fn extract_metadata(
        &self,
        app: &AppHandle,
        record_id: &str,
        repo_path: PathBuf,
        text: &str,
        images: Vec<PathBuf>,
    ) -> Result<Value> {
        debug!(
            "[Extraction] Starting metadata extraction for record: {}",
            record_id
        );
        let text_owned = text.to_string();
        let handle = app.clone();
        let rid = record_id.to_string();
        let db = app.state::<AppState>().db.clone();

        let mut cache = self.cache.lock().await;

        // 1. Ensure Model Readiness
        if cache.is_none() || cache.as_ref().unwrap().repo_path != repo_path {
            debug!("[Extraction] Loading model from: {:?}", repo_path);
            let _ = handle.emit(
                "analysis-progress",
                json!({
                    "status": "loading-model",
                    "record_id": rid,
                }),
            );
            *cache = Some(Self::load_context(&repo_path)?);
            debug!("[Extraction] Model loaded and cached.");
        }

        let ctx = cache.take().unwrap();

        let rid_clone = rid.clone();

        // 2. Retrieval-Augmented Context (Async)
        // For long documents, we prioritize fragments from THIS record.
        let fragments =
            crate::search::query_related_fragments_for_record(&db, &rid, &text_owned, 10)
                .await
                .unwrap_or_default();
        let related_context = if !fragments.is_empty() {
            format!(
                "\nCRITICAL CONTEXT FROM DOCUMENT CHUNKS:\n{}\n",
                fragments.join("\n---\n")
            )
        } else {
            "".to_string()
        };

        // 3. Optimize Input Text (Handle huge multi-page docs)
        // If text is too long, we take the head and tail, and rely on RAG for the middle.
        let processed_text = if text_owned.len() > 15000 {
            let head: String = text_owned.chars().take(8000).collect();
            let tail: String = text_owned
                .chars()
                .skip(text_owned.chars().count() - 4000)
                .collect();
            format!(
                "{}\n\n[... TRUNCATED MIDDLE - REFER TO CONTEXT FRAGMENTS ...]\n\n{}",
                head, tail
            )
        } else {
            text_owned
        };

        // 4. Inference Orchestration (spawn_blocking)
        debug!("[Extraction] Spawning inference task...");
        let result = tokio::task::spawn_blocking(move || {
            Self::run_inference(
                handle,
                rid_clone,
                ctx,
                processed_text,
                related_context,
                images,
            )
        })
        .await?;

        // 4. Restore Cache
        match result {
            Ok((val, thought, ctx_to_restore)) => {
                debug!("[Extraction] Inference completed successfully.");
                *cache = Some(ctx_to_restore);

                // 5. Post-process: Persist fragments & Neural Logs
                self.persist_result_fragments(&db, record_id, &val).await?;

                let log_id = uuid::Uuid::new_v4().to_string();
                let model_id = repo_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("gemma-4")
                    .to_string();

                sqlx::query("INSERT INTO intelligence_logs (id, record_id, system_prompt, user_prompt, thought_block, response_json, model_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(&log_id)
                    .bind(record_id)
                    .bind("Forensic Auditor System Prompt") // Should ideally pass the actual prompt
                    .bind("Perform forensic audit.")
                    .bind(&thought)
                    .bind(serde_json::to_string(&val).unwrap_or_default())
                    .bind(model_id)
                    .bind(now())
                    .execute(&db).await?;

                debug!("[Extraction] Logged to database. Done.");

                Ok(val)
            }
            Err(e) => {
                debug!("[Extraction] Inference task failed: {:?}", e);
                Err(e)
            }
        }
    }

    async fn persist_result_fragments(
        &self,
        db: &sqlx::SqlitePool,
        record_id: &str,
        response: &Value,
    ) -> Result<()> {
        if let Some(obs) = response.get("observations").and_then(|a| a.as_array()) {
            for item in obs {
                if let Some(txt) = item.as_str() {
                    let fid = uuid::Uuid::new_v4().to_string();
                    sqlx::query("INSERT INTO intelligence_fragments (id, record_id, fragment_type, text, confidence, created_at) VALUES (?, ?, 'observation', ?, 0.9, ?)")
                        .bind(&fid).bind(record_id).bind(txt).bind(now()).execute(db).await?;

                    if let Ok(emb) = crate::search::vectorize_text(txt).await {
                        let vblob: &[u8] = unsafe {
                            std::slice::from_raw_parts(emb.as_ptr() as *const u8, emb.len() * 4)
                        };
                        sqlx::query("INSERT INTO vec_intelligence_fragments (fragment_id, embedding) VALUES (?, ?)")
                            .bind(&fid).bind(vblob).execute(db).await?;
                    }
                }
            }
        }
        Ok(())
    }

    fn run_inference(
        handle: AppHandle,
        rid: String,
        ctx: GemmaContext,
        text: String,
        related_context: String,
        images: Vec<PathBuf>,
    ) -> Result<(Value, String, GemmaContext)> {
        let device = &ctx.model.device;
        let image_count = images.len();
        let forensic_audit_note = if image_count > 0 {
            format!(
                "AUDIT NOTICE: {} visual assets are attached. Perform a forensic comparison.",
                image_count
            )
        } else {
            "No visual assets attached.".to_string()
        };

        let system_prompt = format!(
            "You are the PURSUE Intelligence OS forensic auditor. \n\
            Directives:\n\
            1. STRUCTURE: Return valid JSON only.\n\
            2. AUDIT: {}\n\n\
            {}\n\n\
            Input Document:\n{}",
            forensic_audit_note, related_context, text
        );

        let prompt = format!(
            "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\nPerform forensic audit.<|im_end|>\n<|im_start|>thought\n",
            system_prompt
        );

        let mut tokens = ctx
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?
            .get_ids()
            .to_vec();

        let mut logits_processor = LogitsProcessor::new(1337, Some(0.0), None);
        let mut generated_text = String::new();
        let mut pos = 0;
        let mut kv_cache = vec![ctx.model.new_kv_cache(); ctx.model.layers.len()];

        let _ = handle.emit(
            "analysis-progress",
            json!({
                "status": "synthesizing-start",
                "record_id": rid
            }),
        );

        for i in 0..2048 {
            let context_size = if pos > 0 { 1 } else { tokens.len() };
            let input_tokens = &tokens[tokens.len() - context_size..];
            let input = Tensor::new(input_tokens, device)?.unsqueeze(0)?;

            let logits = ctx.model.forward(&input, pos, &mut kv_cache)?;
            let logits = logits.squeeze(0)?.to_dtype(DType::F32)?;

            let next_token = logits_processor.sample(&logits)?;
            tokens.push(next_token);
            pos += context_size;

            let mut piece_to_emit = None;
            if let Some(decoded) = ctx.tokenizer.id_to_token(next_token) {
                if decoded == "<|im_end|>" || next_token == 1 {
                    break;
                }
                if let Ok(piece) = ctx.tokenizer.decode(&[next_token], true) {
                    generated_text.push_str(&piece);
                    piece_to_emit = Some(piece);
                }
            }

            if i % 5 == 0 || piece_to_emit.is_some() {
                let _ = handle.emit(
                    "analysis-progress",
                    json!({
                        "status": "synthesizing",
                        "record_id": rid,
                        "token_index": i,
                        "token_limit": 2048,
                        "token_text": piece_to_emit
                    }),
                );
            }
        }

        let json_start = generated_text.find('{').unwrap_or(0);
        let thought = generated_text[..json_start].trim().to_string();

        let json_end = generated_text
            .rfind('}')
            .map(|i| i + 1)
            .unwrap_or(generated_text.len());
        let json_str = &generated_text[json_start..json_end];

        let val = match serde_json::from_str::<Value>(json_str) {
            Ok(v) => v,
            Err(_) => {
                json!({ "object_description": "Extraction failed", "raw_response": generated_text })
            }
        };

        Ok((val, thought, ctx))
    }

    fn load_context(repo_path: &PathBuf) -> Result<GemmaContext> {
        let device = if cfg!(target_os = "macos") {
            candle_core::Device::new_metal(0).unwrap_or(candle_core::Device::Cpu)
        } else {
            candle_core::Device::Cpu
        };

        let config_data = std::fs::read_to_string(repo_path.join("config.json"))?;
        let config_wrapper: gemma4::ConfigWrapper = serde_json::from_str(&config_data)?;
        let config = config_wrapper.extract().map_err(|e| anyhow!("{}", e))?;

        let mut safetensors_paths = Vec::new();
        for entry in std::fs::read_dir(repo_path)? {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("safetensors") {
                safetensors_paths.push(entry.path());
            }
        }
        safetensors_paths.sort();

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&safetensors_paths, DType::BF16, &device)?
        };
        let model = gemma4::Model::new(&config, vb)?;
        let tokenizer =
            Tokenizer::from_file(repo_path.join("tokenizer.json")).map_err(|e| anyhow!(e))?;

        Ok(GemmaContext {
            model,
            tokenizer,
            repo_path: repo_path.clone(),
        })
    }
}
