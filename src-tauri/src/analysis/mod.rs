pub mod batch_processor;
pub mod diagnostics;
pub mod entities;
pub mod extraction;
pub mod gemma4;
pub mod hardware;
pub mod indexer;
pub mod model_manager;
pub mod ocr;
pub mod ocr_repair;
pub mod pdf;
pub mod persistence;
pub mod registry;
pub mod thumbnails;
pub mod verifier;

use anyhow::{anyhow, Context, Result};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::sync::Arc;
use tauri::Emitter;
use tauri_plugin_log::log::{error, info};
use tokio::fs;
use uuid::Uuid;

use crate::analysis::entities::extract_entities;
use crate::analysis::extraction::{ExtractionConfig, IntelligenceExtractor};
use crate::analysis::indexer::TextExtractor;
use crate::analysis::model_manager::ModelManager;
use crate::analysis::persistence::PersistenceManager;
use crate::db::analysis_repo::AnalysisRepository;
use crate::db::records;
use crate::library::LibraryManager;
use crate::models::{AnalysisReport, RecordAsset};

fn gemma4_bf16_repository_ready(path: &Path) -> bool {
    let config_is_gemma4 = std::fs::read_to_string(path.join("config.json"))
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|config| {
            config
                .get("model_type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .is_some_and(|model_type| model_type == "gemma4");
    config_is_gemma4
        && path.join("tokenizer.json").exists()
        && std::fs::read_dir(path)
            .map(|entries| {
                entries.filter_map(|entry| entry.ok()).any(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some("safetensors")
                })
            })
            .unwrap_or(false)
}

use self::ocr::OcrEngine;
use self::pdf::PdfAnalyzer;
use self::thumbnails::ThumbnailManager;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

// Keep semantic coverage high for large records while bounding pathological inputs. At the
// current 1200-character chunk size, this allows roughly 4.9M characters of searchable content
// per record before warning and capping.
const MAX_SEMANTIC_CHUNKS_PER_RECORD: usize = 4096;

pub struct AnalysisManager {
    db: SqlitePool,
    repo: AnalysisRepository,
    library: Arc<LibraryManager>,
    indexer: TextExtractor,
    persistence: PersistenceManager,
    extractor: IntelligenceExtractor,
    models: ModelManager,
    thumbnails: ThumbnailManager,
    is_analyzing: Arc<AtomicBool>,
    cancel_token: Arc<std::sync::Mutex<CancellationToken>>,
    // SERIALIZED WRITER: SQLite only allows one writer at a time.
    // We use a semaphore to ensure only one thread enters the persistence phase.
    write_semaphore: Arc<Semaphore>,
    // Serializes heavyweight page rendering/OCR/image extraction so batch work cannot multiply
    // per-page image buffers into process-wide memory pressure.
    heavy_analysis_semaphore: Arc<Semaphore>,
}

impl AnalysisManager {
    pub fn new(db: SqlitePool, library: Arc<LibraryManager>) -> Self {
        let ocr = OcrEngine::new();
        let pdf = PdfAnalyzer::new();
        Self {
            db: db.clone(),
            repo: AnalysisRepository::new(db.clone()),
            library: library.clone(),
            indexer: TextExtractor::new(ocr, pdf),
            persistence: PersistenceManager::new(db.clone()),
            extractor: IntelligenceExtractor::new().expect("failed to init Gemma backend"),
            models: ModelManager::new(&library).with_db(db.clone()),
            thumbnails: ThumbnailManager::new(),
            is_analyzing: Arc::new(AtomicBool::new(false)),
            cancel_token: Arc::new(std::sync::Mutex::new(CancellationToken::new())),
            write_semaphore: Arc::new(Semaphore::new(1)),
            heavy_analysis_semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.is_analyzing.load(Ordering::SeqCst)
    }

    pub fn set_busy(&self, busy: bool) {
        if busy {
            // Reset token when starting new work
            if let Ok(mut token) = self.cancel_token.lock() {
                *token = CancellationToken::new();
            }
        }
        self.is_analyzing.store(busy, Ordering::SeqCst);
    }

    pub fn get_cancel_token(&self) -> CancellationToken {
        self.cancel_token.lock().unwrap().clone()
    }

    pub async fn abort_analysis(&self) -> Result<()> {
        info!("[Analysis] ABORT REQUESTED");
        if let Ok(token) = self.cancel_token.lock() {
            token.cancel();
        }
        Ok(())
    }

    pub async fn provision_models(&self, app: &tauri::AppHandle) -> Result<()> {
        info!("Starting background model provisioning...");

        let registry = registry::get_model_registry();
        for model in registry {
            let source = model
                .download_url()
                .unwrap_or_else(|| model.repo_id.clone());
            self.models
                .ensure_model(
                    app,
                    &model.id,
                    model.filename.as_deref().unwrap_or(&model.id),
                    &source,
                    model.expected_bytes,
                    model.expected_sha256.as_deref(),
                )
                .await
                .with_context(|| format!("failed to provision required model {}", model.name))?;
        }

        info!("Background model provisioning completed");
        Ok(())
    }

    pub async fn index_record(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
        current: usize,
        total: usize,
    ) -> Result<AnalysisReport> {
        info!("Indexing record: {} ({}/{})", record_id, current, total);

        let permit = self.write_semaphore.acquire().await?;
        self.repo
            .update_analysis_status(record_id, "indexing", None)
            .await?;
        drop(permit);

        match self
            .index_record_inner(app, record_id, current, total)
            .await
        {
            Ok(report) => Ok(report),
            Err(e) => {
                let message = e.to_string();
                error!("[Analysis] Indexing failed for {}: {}", record_id, message);
                let permit = self.write_semaphore.acquire().await?;
                let _ = self
                    .repo
                    .update_analysis_status(record_id, "failed", Some(&message))
                    .await;
                drop(permit);
                Err(e)
            }
        }
    }

    pub async fn synthesize_intelligence(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
    ) -> Result<AnalysisReport> {
        info!("Synthesizing intelligence for record: {}", record_id);

        let permit = self.write_semaphore.acquire().await?;
        self.repo
            .update_analysis_status(record_id, "synthesizing", None)
            .await?;
        drop(permit);

        match self.synthesize_intelligence_inner(app, record_id).await {
            Ok(report) => Ok(report),
            Err(e) => {
                let message = e.to_string();
                error!("[Analysis] Synthesis failed for {}: {}", record_id, message);
                let permit = self.write_semaphore.acquire().await?;
                let _ = self
                    .repo
                    .update_analysis_status(record_id, "failed", Some(&message))
                    .await;
                drop(permit);
                Err(e)
            }
        }
    }

    async fn index_record_inner(
        &self,
        _app: &tauri::AppHandle,
        record_id: &str,
        current: usize,
        total: usize,
    ) -> Result<AnalysisReport> {
        let record = records::find_by_id(&self.db, record_id)
            .await?
            .ok_or_else(|| anyhow!("record not found"))?;
        let local_path = record
            .local_path
            .as_deref()
            .ok_or_else(|| anyhow!("record {} has no local_path", record_id))?;
        let full_path = self.library.get_readable_artifact_path(local_path).await?;

        // 1. OCR (Foundation)
        let _ = _app.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "extracting-foundation",
                "record_id": record_id,
                "current": current,
                "total": total
            }),
        );
        let heavy_permit = self.heavy_analysis_semaphore.acquire().await?;
        let extraction = self
            .indexer
            .extract(_app, record_id, &full_path, &record)
            .await?;
        let text = extraction.text;
        let engine = extraction.engine;
        let extraction_metadata = extraction.metadata;
        let mut analysis_warnings = extraction.warnings;

        info!("Foundation captured for {}: used {}", record_id, engine);

        if text.trim().is_empty() {
            tauri_plugin_log::log::warn!("[Analysis] Foundation extraction for {} resulted in empty text. No semantic chunks will be created.", record_id);
        }

        // ENGINE TRANSPARENCY: Report the specific OCR implementation used
        let _ = _app.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "foundation-indexed",
                "record_id": record_id,
                "engine": engine,
                "current": current,
                "total": total
            }),
        );

        // 2. Heavy Extraction (Outside lock)
        let asset_dir = self.library.get_full_path(&format!("assets/{}", record_id));
        fs::create_dir_all(&asset_dir).await?;
        let thumb_name = "thumb_main.png";
        let thumb_path = asset_dir.join(thumb_name);

        let mut thumbnail_rel_path = None;
        if self
            .thumbnails
            .generate_thumbnail(&full_path, &thumb_path)
            .await
            .is_ok()
        {
            let rel_path = format!("assets/{}/{}", record_id, thumb_name);
            thumbnail_rel_path = Some(self.library.encrypt_generated_asset(&rel_path).await?);
        }

        let mut pdf_forensics = Vec::new();
        let mut pdf_images = Vec::new();
        if is_pdf_path(&full_path) {
            if let Ok(forensics) = self.indexer.pdf.extract_forensics(&full_path) {
                pdf_forensics = forensics;
            }
            if let Ok(extracted) = self
                .indexer
                .pdf
                .extract_images(&full_path, &asset_dir)
                .await
            {
                for (filename, mime) in extracted {
                    let asset_id = Uuid::new_v4().to_string();
                    let rel_path = format!("assets/{}/{}", record_id, filename);
                    let rel_path = self.library.encrypt_generated_asset(&rel_path).await?;
                    pdf_images.push((asset_id, rel_path, mime));
                }
            } else {
                let warning = "PDF embedded image extraction was skipped after an error.";
                emit_analysis_warning(_app, record_id, warning);
                analysis_warnings.push(warning.to_string());
            }
        }

        let redaction_score = if is_pdf_path(&full_path) {
            match self
                .analyze_pdf_redactions_pagewise(_app, record_id, &full_path)
                .await
            {
                Ok((score, mut discoveries)) => {
                    pdf_forensics.append(&mut discoveries);
                    score
                }
                Err(error) => {
                    let warning = format!("PDF redaction scoring skipped: {error}");
                    emit_analysis_warning(_app, record_id, &warning);
                    analysis_warnings.push(warning);
                    0.0
                }
            }
        } else {
            self.indexer
                .ocr
                .analyze_redactions(&full_path)
                .unwrap_or(0.0)
        };
        drop(heavy_permit);

        let entities = extract_entities(&text);
        let mut chunks = crate::search::chunk_text(&text, 1200);
        if chunks.len() > MAX_SEMANTIC_CHUNKS_PER_RECORD {
            let warning = format!(
                "Semantic indexing capped at {} chunks out of {} extracted chunks.",
                MAX_SEMANTIC_CHUNKS_PER_RECORD,
                chunks.len()
            );
            emit_analysis_warning(_app, record_id, &warning);
            analysis_warnings.push(warning);
            chunks.truncate(MAX_SEMANTIC_CHUNKS_PER_RECORD);
        }
        let mut embeddings = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            embeddings.push(crate::search::vectorize_text(chunk).await?);
        }

        // 3. Persistence (Inside lock)
        let _permit = self.write_semaphore.acquire().await?;
        info!(
            "[Analysis] Persistence permit acquired for {}. Saving results...",
            record_id
        );

        if let Some(path) = thumbnail_rel_path {
            let _ = self.repo.save_thumbnail_path(record_id, &path).await;
        }

        let _ = self
            .persistence
            .persist_forensics(record_id, &pdf_forensics)
            .await;

        for (asset_id, rel_path, mime) in pdf_images {
            let _ = self
                .repo
                .insert_record_asset(&asset_id, record_id, "image", &rel_path, &mime)
                .await;
        }

        self.persistence
            .persist_entities(record_id, &entities)
            .await?;

        let chunks_indexed = self
            .persistence
            .persist_chunks(record_id, &record.title, &chunks, &embeddings, &entities)
            .await?;

        // Raw OCR storage for synthesis phase
        self.repo
            .save_ocr_result(
                record_id,
                &text,
                &engine,
                &extraction_metadata,
                &analysis_warnings,
            )
            .await?;

        self.repo
            .update_redaction_score(record_id, redaction_score)
            .await?;
        let warning_text = if analysis_warnings.is_empty() {
            None
        } else {
            Some(format!(
                "Partial analysis warnings: {}",
                analysis_warnings.join(" | ")
            ))
        };
        self.repo
            .update_analysis_status(record_id, "indexed", warning_text.as_deref())
            .await?;

        info!(
            "[Analysis] Foundation secured for {}: {} semantic associations mapped.",
            record_id, chunks_indexed
        );
        info!(
            "[Analysis] Syncing intelligence graph for record {}... Done.",
            record_id
        );

        drop(_permit); // Release the database write lock

        Ok(AnalysisReport {
            record_id: record_id.to_string(),
            status: "indexed".to_string(),
            ocr_text: text,
            entities,
            chunks_indexed,
            engine,
            intelligence_json: None,
            assets: Vec::new(),
            extraction_metadata_json: Some(extraction_metadata.to_string()),
            extraction_warnings_json: Some(
                serde_json::to_string(&analysis_warnings).unwrap_or_else(|_| "[]".to_string()),
            ),
        })
    }

    async fn synthesize_intelligence_inner(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
    ) -> Result<AnalysisReport> {
        let text = self.repo.get_ocr_text(record_id).await?;
        let assets =
            sqlx::query_as::<_, RecordAsset>("SELECT * FROM record_assets WHERE record_id = ?")
                .bind(record_id)
                .fetch_all(&self.db)
                .await?;

        let bf16_path = self.models.models_dir().join("gemma-4-e4b");
        let q4_path = self
            .models
            .models_dir()
            .join(crate::analysis::extraction::GEMMA4_Q4_FILENAME);
        let mmproj_path = self
            .models
            .models_dir()
            .join(crate::analysis::extraction::GEMMA4_MMPROJ_FILENAME);
        let bf16_ready = gemma4_bf16_repository_ready(&bf16_path);
        let bf16_accelerator_suitable =
            bf16_ready && crate::analysis::hardware::gemma4_bf16_accelerator_suitable().await;
        let q4_ready = std::fs::metadata(&q4_path)
            .map(|metadata| {
                metadata.is_file() && metadata.len() == crate::analysis::extraction::GEMMA4_Q4_BYTES
            })
            .unwrap_or(false)
            && !crate::analysis::verifier::is_model_corrupted(
                &q4_path,
                crate::analysis::extraction::GEMMA4_Q4_FILENAME,
            )
            .await;

        let mut image_paths = Vec::new();
        for asset in &assets {
            let ext = std::path::Path::new(&asset.local_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if asset.asset_type == "image"
                || matches!(
                    ext.as_str(),
                    "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff"
                )
            {
                if let Ok(path) = self
                    .library
                    .get_readable_artifact_path(&asset.local_path)
                    .await
                {
                    image_paths.push(path);
                }
            } else if asset.asset_type == "video"
                || matches!(ext.as_str(), "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm")
            {
                if let Ok(video_path) = self
                    .library
                    .get_readable_artifact_path(&asset.local_path)
                    .await
                {
                    let extracted = extract_video_keyframes_for_gemma(&video_path).await;
                    image_paths.extend(extracted);
                }
            }
        }

        // Native Gemma 4 image understanding (llama.cpp mtmd) needs the GGUF text model AND the
        // multimodal projector. When a record has images and both are present, run text synthesis
        // AND image analysis on the single shared Q4 GGUF (one resident llama.cpp model). This is
        // distinct from OCR — which only transcribes text — and never auto-cascades: it runs only
        // when this on-demand synthesis is invoked on an image-bearing record.
        let native_vision = !image_paths.is_empty()
            && q4_ready
            && crate::analysis::extraction::gemma4_mmproj_ready(&mmproj_path);

        let model_path = if native_vision {
            info!("[Analysis] Gemma 4 multimodal via native llama.cpp mtmd on the shared Q4 GGUF");
            q4_path
        } else if bf16_accelerator_suitable {
            info!(
                "[Analysis] Using existing full-precision Gemma 4 E4B on a high-memory accelerator"
            );
            bf16_path.clone()
        } else if q4_ready {
            info!("[Analysis] Using official Gemma 4 E4B QAT Q4_0 with adaptive GPU offload");
            q4_path
        } else if bf16_ready {
            return Err(anyhow!(
                "The existing Gemma 4 E4B BF16 cache is preserved, but this accelerator does not have the safe 20 GiB memory margin it requires. Download the official Gemma 4 E4B QAT Q4_0 model from Intelligence Setup."
            ));
        } else {
            return Err(anyhow!(
                "Gemma 4 E4B is not ready. Download the required official QAT Q4_0 model from Intelligence Setup. Gemma 3 and Gemma 2 are not valid fallbacks."
            ));
        };

        // One unified path: the extractor runs Gemma 4 text synthesis, and — when images are
        // present with the projector — the native mtmd multimodal synthesis on the same model.
        let intelligence_json = self
            .extractor
            .extract_forensics(
                app,
                record_id,
                ExtractionConfig {
                    preferred_model_path: Some(model_path),
                    fallback_model_path: None,
                    force_cpu: false,
                },
                &text,
                image_paths,
            )
            .await?;

        let intel_str = serde_json::to_string(&intelligence_json)?;
        let _permit = self.write_semaphore.acquire().await?;
        self.repo
            .save_intelligence_json(record_id, &intel_str)
            .await?;
        drop(_permit);

        self.get_analysis(record_id)
            .await?
            .ok_or_else(|| anyhow!("report missing"))
    }

    pub async fn get_analysis(&self, record_id: &str) -> Result<Option<AnalysisReport>> {
        let row = sqlx::query("SELECT r.intelligence_json, r.analysis_status, ar.ocr_text, ar.engine, ar.metadata_json, ar.warnings_json FROM records r LEFT JOIN analysis_results ar ON ar.record_id = r.id WHERE r.id = ?")
            .bind(record_id).fetch_optional(&self.db).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(AnalysisReport {
            record_id: record_id.to_string(),
            status: row.get("analysis_status"),
            ocr_text: row.get::<Option<String>, _>("ocr_text").unwrap_or_default(),
            entities: Vec::new(),
            chunks_indexed: 0,
            engine: row
                .get::<Option<String>, _>("engine")
                .unwrap_or_else(|| "stored".to_string()),
            intelligence_json: row.get("intelligence_json"),
            assets: Vec::new(),
            extraction_metadata_json: row.get::<Option<String>, _>("metadata_json"),
            extraction_warnings_json: row.get::<Option<String>, _>("warnings_json"),
        }))
    }
}

impl AnalysisManager {
    async fn analyze_pdf_redactions_pagewise(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
        path: &std::path::Path,
    ) -> Result<(f32, Vec<crate::analysis::pdf::ForensicDiscovery>)> {
        let total_pages = self.indexer.pdf.page_count(path)?;
        let mut max_score = 0.0f32;
        let mut discoveries = Vec::new();
        for idx in 0..total_pages {
            let _ = app.emit(
                "analysis-progress",
                serde_json::json!({
                    "status": "extracting-foundation",
                    "record_id": record_id,
                    "step": format!("Redaction scoring page {} of {}", idx + 1, total_pages)
                }),
            );
            let page_img = self
                .indexer
                .pdf
                .render_page(path, idx, crate::analysis::pdf::PdfRenderOptions::default())
                .await?;
            if let Ok(analysis) = self
                .indexer
                .ocr
                .analyze_redactions_image_structured(&page_img)
            {
                if analysis.score > max_score {
                    max_score = analysis.score;
                }
                for region in analysis.regions {
                    discoveries.push(crate::analysis::pdf::ForensicDiscovery {
                        layer_type: "rendered_redaction_candidate".to_string(),
                        content: format!("Rendered black block candidate @ Page {}", idx + 1),
                        confidence: 0.6,
                        metadata: serde_json::json!({
                            "page": idx + 1,
                            "bbox": [region.x, region.y, region.width, region.height],
                            "source": "rendered_page_image",
                            "caveat": "Black blocks can be legitimate graphics or layout. Treat as a candidate requiring analyst review."
                        }),
                    });
                }
            }
        }
        Ok((max_score, discoveries))
    }

    /// Native Gemma 4 image understanding is ready when both the Q4 GGUF text model and its
    /// multimodal projector are present. No Python/torch runtime is involved — the projector is
    /// loaded via llama.cpp mtmd onto the same resident model used for text synthesis.
    pub async fn check_neural_runtime_status(&self) -> Result<bool> {
        let models_dir = self.models.models_dir();
        // Report readiness on the same completeness test the synthesis path uses, so the UI
        // cannot advertise vision as ready while a half-downloaded projector makes it fall
        // back to text-only.
        let q4_ready = models_dir
            .join(crate::analysis::extraction::GEMMA4_Q4_FILENAME)
            .exists();
        let mmproj_ready = crate::analysis::extraction::gemma4_mmproj_ready(
            &models_dir.join(crate::analysis::extraction::GEMMA4_MMPROJ_FILENAME),
        );
        Ok(q4_ready && mmproj_ready)
    }

    /// Ensure the Gemma 4 multimodal projector is downloaded (it pairs with the text GGUF).
    pub async fn provision_neural_runtime(&self, app: &tauri::AppHandle) -> Result<()> {
        let mmproj = registry::get_model_registry()
            .into_iter()
            .find(|model| model.id == "gemma-4-e4b-mmproj")
            .ok_or_else(|| {
                anyhow!("Gemma 4 vision projector is missing from the model registry")
            })?;
        let url = mmproj
            .download_url()
            .ok_or_else(|| anyhow!("Gemma 4 vision projector has no resolvable download URL"))?;
        self.models
            .ensure_model(
                app,
                &mmproj.id,
                mmproj.filename.as_deref().unwrap_or(&mmproj.id),
                &url,
                mmproj.expected_bytes,
                mmproj.expected_sha256.as_deref(),
            )
            .await
            .map(|_| ())
            .context("failed to provision the Gemma 4 vision projector")
    }

    pub async fn analyze_record(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
    ) -> Result<AnalysisReport> {
        self.index_record(app, record_id, 1, 1).await?;
        self.synthesize_intelligence(app, record_id).await
    }

    pub async fn clear_record_analysis(&self, record_id: &str) -> Result<()> {
        let _permit = self.write_semaphore.acquire().await?;
        self.repo.clear_analysis_data(record_id).await?;
        drop(_permit);
        Ok(())
    }

    pub async fn clear_all_analysis(&self) -> Result<()> {
        let _permit = self.write_semaphore.acquire().await?;
        info!("[Analysis] Initiating BULK PURGE of all intelligence data...");
        self.repo.clear_all_analysis_data().await?;
        info!("[Analysis] Bulk purge complete. Database neutralized.");

        // Optional: Vacuum to reclaim space and optimize
        let _ = sqlx::query("PRAGMA incremental_vacuum")
            .execute(&self.db)
            .await;

        drop(_permit);
        Ok(())
    }
}

fn is_pdf_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn emit_analysis_warning(app: &tauri::AppHandle, record_id: &str, warning: &str) {
    let _ = app.emit(
        "analysis-warning",
        serde_json::json!({
            "record_id": record_id,
            "warning": warning
        }),
    );
}

async fn probe_video_duration(video_path: &std::path::Path) -> Option<f64> {
    let mut cmd = tokio::process::Command::new("ffprobe");
    cmd.arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(video_path);

    if let Ok(output) = crate::common::hide_console(&mut cmd).output().await {
        if output.status.success() {
            let str_out = String::from_utf8_lossy(&output.stdout);
            if let Ok(sec) = str_out.trim().parse::<f64>() {
                if sec > 0.0 {
                    return Some(sec);
                }
            }
        }
    }
    None
}

async fn extract_video_keyframes_for_gemma(
    video_path: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let temp_dir =
        std::env::temp_dir().join(format!("pursue-gemma-frames-{}", uuid::Uuid::new_v4()));
    if tokio::fs::create_dir_all(&temp_dir).await.is_err() {
        return Vec::new();
    }

    let duration = probe_video_duration(video_path).await.unwrap_or(30.0);
    let offsets = [
        (duration * 0.1).clamp(0.5, 2.0),
        duration * 0.35,
        duration * 0.65,
        (duration * 0.90).min(duration - 0.5),
    ];
    let timestamps: Vec<String> = offsets
        .iter()
        .map(|s| {
            let total = s.max(0.0) as u64;
            let hrs = total / 3600;
            let mins = (total % 3600) / 60;
            let secs = total % 60;
            format!("{:02}:{:02}:{:02}", hrs, mins, secs)
        })
        .collect();

    let mut extracted_paths = Vec::new();

    for (idx, ts) in timestamps.iter().enumerate() {
        let frame_path = temp_dir.join(format!("gemma_frame_{idx}.png"));
        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.arg("-ss")
            .arg(ts)
            .arg("-i")
            .arg(video_path)
            .arg("-vframes")
            .arg("1")
            .arg("-f")
            .arg("image2")
            .arg("-y")
            .arg(&frame_path);

        if let Ok(output) = crate::common::hide_console(&mut cmd).output().await {
            if output.status.success() && frame_path.exists() {
                extracted_paths.push(frame_path);
            }
        }
    }

    extracted_paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pdf_path() {
        assert!(is_pdf_path(std::path::Path::new("document.pdf")));
        assert!(is_pdf_path(std::path::Path::new("document.PDF")));
        assert!(!is_pdf_path(std::path::Path::new("document.png")));
    }
}
