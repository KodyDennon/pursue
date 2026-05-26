pub mod batch_processor;
pub mod diagnostics;
pub mod entities;
pub mod extraction;
pub mod gemma4;
pub mod indexer;
pub mod model_manager;
pub mod nn;
pub mod ocr;
pub mod pdf;
pub mod persistence;
pub mod registry;
pub mod thumbnails;
pub mod verifier;

use anyhow::{anyhow, Result};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tauri::Emitter;
use tauri_plugin_log::log::{error, info};
use tokio::fs;
use uuid::Uuid;

use crate::analysis::diagnostics::{get_hardware_specs, IntelligenceTier};
use crate::analysis::entities::extract_entities;
use crate::analysis::extraction::{ExtractionConfig, IntelligenceExtractor};
use crate::analysis::indexer::TextExtractor;
use crate::analysis::model_manager::ModelManager;
use crate::analysis::persistence::PersistenceManager;
use crate::db::analysis_repo::AnalysisRepository;
use crate::db::records;
use crate::library::LibraryManager;
use crate::models::{AnalysisReport, RecordAsset};

use self::ocr::OcrEngine;
use self::pdf::PdfAnalyzer;
use self::thumbnails::ThumbnailManager;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

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
            persistence: PersistenceManager::new(db),
            extractor: IntelligenceExtractor::new().expect("failed to init Gemma backend"),
            models: ModelManager::new(&library),
            thumbnails: ThumbnailManager::new(),
            is_analyzing: Arc::new(AtomicBool::new(false)),
            cancel_token: Arc::new(std::sync::Mutex::new(CancellationToken::new())),
            write_semaphore: Arc::new(Semaphore::new(1)),
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
        let specs = get_hardware_specs();

        for model in registry {
            // Only provision recommended intelligence tier
            if model.model_type == registry::ModelType::Intelligence {
                let is_elite = model.id == "gemma-4-e4b";
                if (is_elite && specs.recommended_tier != IntelligenceTier::Elite)
                    || (!is_elite && specs.recommended_tier == IntelligenceTier::Elite)
                {
                    continue;
                }
            }

            let _ = self
                .models
                .ensure_model(
                    app,
                    &model.id,
                    model.filename.as_deref().unwrap_or(&model.id),
                    &model.repo_id,
                )
                .await;
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
        let full_path = self
            .library
            .get_readable_artifact_path(record.local_path.as_ref().unwrap())
            .await?;

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
        let (text, engine) = self.indexer.extract(_app, record_id, &full_path).await?;

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
        if full_path.extension().and_then(|e| e.to_str()) == Some("pdf") {
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
            }
        }

        let entities = extract_entities(&text);
        let chunks = crate::search::chunk_text(&text, 1200);
        let mut embeddings = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            embeddings.push(crate::search::vectorize_text(chunk).await?);
        }

        let redaction_score = if full_path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            if let Ok(pages) = self.indexer.pdf.render_pdf_to_images(&full_path).await {
                let mut max_score = 0.0f32;
                for page_img in &pages {
                    if let Ok(score) = self.indexer.ocr.analyze_redactions_image(page_img) {
                        if score > max_score {
                            max_score = score;
                        }
                    }
                }
                max_score
            } else {
                0.0
            }
        } else {
            self.indexer
                .ocr
                .analyze_redactions(&full_path)
                .unwrap_or(0.0)
        };

        // 3. Persistence (Inside lock)
        let _permit = self.write_semaphore.acquire().await?;
        info!(
            "[Analysis] Persistence permit acquired for {}. Saving results...",
            record_id
        );

        if let Some(path) = thumbnail_rel_path {
            let _ = self.repo.save_thumbnail_path(record_id, &path).await;
        }

        if !pdf_forensics.is_empty() {
            let _ = self
                .persistence
                .persist_forensics(record_id, &pdf_forensics)
                .await;
        }

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
        self.repo.save_ocr_result(record_id, &text).await?;

        self.repo
            .update_redaction_score(record_id, redaction_score)
            .await?;
        self.repo
            .update_analysis_status(record_id, "indexed", None)
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

        let specs = get_hardware_specs();
        let preferred_model = if specs.recommended_tier == IntelligenceTier::Elite {
            "gemma-4-e4b"
        } else {
            "gemma-4-e2b"
        };
        let model_path = self.models.models_dir().join(preferred_model);

        let mut image_paths = Vec::new();
        for asset in assets.iter().filter(|a| a.asset_type == "image") {
            image_paths.push(
                self.library
                    .get_readable_artifact_path(&asset.local_path)
                    .await?,
            );
        }

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
        let row = sqlx::query("SELECT r.intelligence_json, r.analysis_status, ar.ocr_text FROM records r LEFT JOIN analysis_results ar ON ar.record_id = r.id WHERE r.id = ?")
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
            engine: "stored".to_string(),
            intelligence_json: row.get("intelligence_json"),
            assets: Vec::new(),
        }))
    }
}

impl AnalysisManager {
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
