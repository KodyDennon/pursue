pub mod diagnostics;
pub mod extraction;
pub mod gemma4;
pub mod indexer;
pub mod model_manager;
#[cfg(target_os = "macos")]
pub mod native_macos;
#[cfg(target_os = "windows")]
pub mod native_windows;
pub mod ocr;
pub mod pdf;
pub mod persistence;
pub mod registry;
pub mod thumbnails;

use anyhow::{anyhow, Result};
use regex::Regex;
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::Emitter;
use tauri_plugin_log::log::info;
use tokio::fs;
use uuid::Uuid;

use crate::analysis::diagnostics::{get_hardware_specs, IntelligenceTier};
use crate::analysis::extraction::{ExtractionConfig, IntelligenceExtractor};
use crate::analysis::indexer::TextExtractor;
use crate::analysis::model_manager::ModelManager;
use crate::analysis::persistence::PersistenceManager;
use crate::db::records;
use crate::library::LibraryManager;
use crate::models::{AnalysisReport, EntityHit, RecordAsset};

use self::ocr::OcrEngine;
use self::pdf::PdfAnalyzer;
use self::thumbnails::ThumbnailManager;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Semaphore;

pub struct AnalysisManager {
    db: SqlitePool,
    library: Arc<LibraryManager>,
    indexer: TextExtractor,
    persistence: PersistenceManager,
    extractor: IntelligenceExtractor,
    models: ModelManager,
    thumbnails: ThumbnailManager,
    is_analyzing: Arc<AtomicBool>,
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
            library: library.clone(),
            indexer: TextExtractor::new(ocr, pdf),
            persistence: PersistenceManager::new(db),
            extractor: IntelligenceExtractor::new().expect("failed to init Gemma backend"),
            models: ModelManager::new(&library),
            thumbnails: ThumbnailManager::new(),
            is_analyzing: Arc::new(AtomicBool::new(false)),
            write_semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.is_analyzing.load(Ordering::SeqCst)
    }

    pub fn set_busy(&self, busy: bool) {
        self.is_analyzing.store(busy, Ordering::SeqCst);
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
        force_ocr: bool,
        current: usize,
        total: usize,
    ) -> Result<AnalysisReport> {
        info!("Indexing record: {} ({}/{})", record_id, current, total);
        sqlx::query(
            "UPDATE records SET analysis_status = 'indexing', analysis_error = NULL WHERE id = ?",
        )
        .bind(record_id)
        .execute(&self.db)
        .await?;
        match self
            .index_record_inner(app, record_id, force_ocr, current, total)
            .await
        {
            Ok(report) => Ok(report),
            Err(error) => {
                let message = error.to_string();
                let _ = sqlx::query("UPDATE records SET analysis_status = 'failed', analysis_error = ? WHERE id = ?").bind(&message).bind(record_id).execute(&self.db).await;
                Err(error)
            }
        }
    }

    pub async fn synthesize_intelligence(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
    ) -> Result<AnalysisReport> {
        info!("Synthesizing intelligence for record: {}", record_id);
        sqlx::query("UPDATE records SET analysis_status = 'synthesizing', analysis_error = NULL WHERE id = ?").bind(record_id).execute(&self.db).await?;
        match self.synthesize_intelligence_inner(app, record_id).await {
            Ok(report) => Ok(report),
            Err(error) => {
                let message = error.to_string();
                let _ = sqlx::query("UPDATE records SET analysis_status = 'failed', analysis_error = ? WHERE id = ?").bind(&message).bind(record_id).execute(&self.db).await;
                Err(error)
            }
        }
    }

    async fn index_record_inner(
        &self,
        _app: &tauri::AppHandle,
        record_id: &str,
        force_ocr: bool,
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
        let (text, engine) = self.indexer.extract(_app, record_id, &full_path, force_ocr).await?;

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

        // 2. Persistence & Asset Extraction
        // ACQUIRE WRITE PERMIT: We serialize the database writing phase to prevent 'database is locked' errors.
        // OCR and rendering (the slow parts) were done above in parallel.
        let _permit = self.write_semaphore.acquire().await?;
        info!("[Analysis] Persistence permit acquired for {}. Saving results...", record_id);

        let asset_dir = self.library.get_full_path(&format!("assets/{}", record_id));
        fs::create_dir_all(&asset_dir).await?;
        let thumb_name = "thumb_main.png";
        let thumb_path = asset_dir.join(thumb_name);

        if self
            .thumbnails
            .generate_thumbnail(&full_path, &thumb_path)
            .await
            .is_ok()
        {
            let rel_thumb_path = format!("assets/{}/{}", record_id, thumb_name);
            let rel_thumb_path = self.library.encrypt_generated_asset(&rel_thumb_path).await?;
            let _ = sqlx::query("UPDATE records SET thumbnail_path = ? WHERE id = ?")
                .bind(&rel_thumb_path)
                .bind(record_id)
                .execute(&self.db)
                .await;
        }

        // PDF specialized extraction
        if full_path.extension().and_then(|e| e.to_str()) == Some("pdf") {
            // Forensic layers
            if let Ok(forensics) = self.indexer.pdf.extract_forensics(&full_path) {
                let _ = self
                    .persistence
                    .persist_forensics(record_id, &forensics)
                    .await;
            }
            // Images
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
                    let _ = sqlx::query("INSERT INTO record_assets (id, record_id, asset_type, local_path, mime_type, created_at) VALUES (?, ?, 'image', ?, ?, ?)")
                        .bind(&asset_id).bind(record_id).bind(&rel_path).bind(&mime).bind(crate::common::now()).execute(&self.db).await;
                }
            }
        }

        info!("[Analysis] Persisting foundation for {}: entities and chunks...", record_id);
        let entities = extract_entities(&text);
        self.persistence
            .persist_entities(record_id, &entities)
            .await?;
        
        let chunks_indexed = self
            .persistence
            .persist_chunks(record_id, &record.title, &text, &entities)
            .await?;

        // Raw OCR storage for synthesis phase
        sqlx::query(
            "INSERT INTO analysis_results (record_id, ocr_text, status, processed_at) \
             VALUES (?, ?, 'indexed', ?) \
             ON CONFLICT(record_id) DO UPDATE SET ocr_text = excluded.ocr_text, status = 'indexed', processed_at = excluded.processed_at"
        )
        .bind(record_id)
        .bind(&text)
        .bind(crate::common::now())
        .execute(&self.db)
        .await?;

        let redaction_score = self
            .indexer
            .ocr
            .analyze_redactions(&full_path)
            .unwrap_or(0.0);
        
        sqlx::query(
            "UPDATE records SET analysis_status = 'indexed', redaction_score = ? WHERE id = ?",
        )
        .bind(redaction_score)
        .bind(record_id)
        .execute(&self.db)
        .await?;

        info!("[Analysis] Foundation secured for {}: {} semantic associations mapped.", record_id, chunks_indexed);
        info!("[Analysis] Syncing intelligence graph for record {}... Done.", record_id);

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
        let res_row = sqlx::query("SELECT ocr_text FROM analysis_results WHERE record_id = ?")
            .bind(record_id)
            .fetch_one(&self.db)
            .await?;
        let text: String = res_row.get("ocr_text");
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
            image_paths.push(self.library.get_readable_artifact_path(&asset.local_path).await?);
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
        sqlx::query(
            "UPDATE records SET analysis_status = 'completed', intelligence_json = ? WHERE id = ?",
        )
        .bind(&intel_str)
        .bind(record_id)
        .execute(&self.db)
        .await?;

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

pub fn extract_entities(text: &str) -> Vec<EntityHit> {
    let mut entities = BTreeMap::<(String, String), EntityHit>::new();

    let patterns = [
        (r"(?i)\b(AARO|NASA|DOJ|FBI|CIA|DHS|FAA|NORAD)\b", "agency"),
        (
            r"(?i)\b(orb|sphere|tic-tac|cylinder|disc|triangle)\b",
            "shape",
        ),
        (r"(?i)\b(radar|ir|flir|sonar|visual|satellite)\b", "sensor"),
        (
            r"(?i)\b(hypersonic|transmedium|instantaneous acceleration)\b",
            "pattern",
        ),
    ];

    for (pat, ty) in patterns {
        if let Ok(re) = Regex::new(pat) {
            for mat in re.find_iter(text) {
                add_entity(&mut entities, mat.as_str(), ty, 0.85, "deterministic");
            }
        }
    }

    let date_re = Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap();
    for h in date_re.find_iter(text) {
        add_entity(&mut entities, h.as_str(), "date", 0.8, "deterministic");
    }

    entities.into_values().collect()
}

impl AnalysisManager {
    pub async fn analyze_record(
        &self,
        app: &tauri::AppHandle,
        record_id: &str,
    ) -> Result<AnalysisReport> {
        self.index_record(app, record_id, false, 1, 1).await?;
        self.synthesize_intelligence(app, record_id).await
    }
}

fn add_entity(e: &mut BTreeMap<(String, String), EntityHit>, n: &str, ty: &str, c: f64, s: &str) {
    let name = n.trim().to_string();
    if name.is_empty() {
        return;
    }
    e.entry((name.to_lowercase(), ty.to_string()))
        .or_insert(EntityHit {
            id: Uuid::new_v4().to_string(),
            name,
            entity_type: ty.to_string(),
            confidence: c,
            source: s.to_string(),
        });
}

impl AnalysisManager {
    pub async fn clear_record_analysis(&self, record_id: &str) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query("DELETE FROM analysis_results WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM vec_analysis_chunks WHERE chunk_id IN (SELECT id FROM analysis_chunks WHERE record_id = ?)").bind(record_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM analysis_chunks WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM analysis_chunks_fts WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM record_forensics WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM record_entities WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM intelligence_logs WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM record_assets WHERE record_id = ? AND asset_type != 'source'")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE records SET analysis_status = 'pending', intelligence_json = NULL, redaction_score = NULL WHERE id = ?").bind(record_id).execute(&mut *tx).await?;

        tx.commit().await?;
        Ok(())
    }
}
