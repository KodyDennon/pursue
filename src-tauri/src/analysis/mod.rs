pub mod diagnostics;
pub mod extraction;
pub mod gemma4;
pub mod model_manager;
#[cfg(target_os = "macos")]
pub mod native_macos;
#[cfg(target_os = "windows")]
pub mod native_windows;
pub mod ocr;
pub mod pdf;
pub mod thumbnails;

use anyhow::{anyhow, Result};
use regex::Regex;
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tauri_plugin_log::log::{info, warn};
use tokio::fs;
use uuid::Uuid;
use tauri::Emitter;

use crate::analysis::diagnostics::{get_hardware_specs, IntelligenceTier};
use crate::analysis::extraction::{ExtractionConfig, IntelligenceExtractor};
use crate::analysis::model_manager::ModelManager;
use crate::db::records;
use crate::library::LibraryManager;
use crate::models::{AnalysisReport, EntityHit, RecordAsset};
use crate::search::{chunk_text, vectorize_text};

use self::ocr::OcrEngine;
use self::pdf::PdfAnalyzer;
use self::thumbnails::ThumbnailManager;

pub struct AnalysisManager {
    db: SqlitePool,
    library: Arc<LibraryManager>,
    ocr: OcrEngine,
    pdf: PdfAnalyzer,
    extractor: IntelligenceExtractor,
    models: ModelManager,
    thumbnails: ThumbnailManager,
}

impl AnalysisManager {
    pub fn new(db: SqlitePool, library: Arc<LibraryManager>) -> Self {
        Self {
            db,
            library: library.clone(),
            ocr: OcrEngine::new(),
            pdf: PdfAnalyzer::new(),
            extractor: IntelligenceExtractor::new().expect("failed to init Gemma backend"),
            models: ModelManager::new(&library),
            thumbnails: ThumbnailManager::new(),
        }
    }

    /// Pre-provision required models in the background without blocking
    pub async fn provision_models(&self, app: &tauri::AppHandle) -> Result<()> {
        info!("Starting background model provisioning...");
        
        let _ = self.models.ensure_model(app, "bge-small", "bge-small-en-v1.5.onnx", 
            "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx")
            .await
            .map_err(|e| {
                warn!("Background: Failed to provision embedding model: {}", e);
                e
            });
            
        let _ = self.models.ensure_model(app, "tokenizer", "tokenizer.json", 
            "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json")
            .await
            .map_err(|e| {
                warn!("Background: Failed to provision tokenizer: {}", e);
                e
            });

        let specs = get_hardware_specs();
        let (model_id, preferred_model, preferred_url) = match specs.recommended_tier {
            IntelligenceTier::Elite => (
                "gemma-4-e4b", 
                "gemma-4-e4b", 
                "google/gemma-4-E4B-it"
            ),
            _ => (
                "gemma-4-e2b", 
                "gemma-4-e2b", 
                "google/gemma-4-E2B-it"
            ),
        };
        
        let _ = self.models.ensure_model(app, model_id, preferred_model, preferred_url)
            .await
            .map_err(|e| {
                warn!("Background: Failed to provision main intelligence model: {}", e);
                e
            });

        info!("Background model provisioning completed");
        Ok(())
    }

    pub async fn index_record(&self, app: &tauri::AppHandle, record_id: &str) -> Result<AnalysisReport> {
        info!("Indexing record: {}", record_id);
        sqlx::query("UPDATE records SET analysis_status = 'indexing', analysis_error = NULL WHERE id = ?").bind(record_id).execute(&self.db).await?;
        match self.index_record_inner(app, record_id).await {
            Ok(report) => Ok(report),
            Err(error) => {
                let message = error.to_string();
                let _ = sqlx::query("UPDATE records SET analysis_status = 'failed', analysis_error = ? WHERE id = ?").bind(&message).bind(record_id).execute(&self.db).await;
                Err(error)
            }
        }
    }

    pub async fn synthesize_intelligence(&self, app: &tauri::AppHandle, record_id: &str) -> Result<AnalysisReport> {
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

    async fn index_record_inner(&self, _app: &tauri::AppHandle, record_id: &str) -> Result<AnalysisReport> {
        let record = records::find_by_id(&self.db, record_id).await?.ok_or_else(|| anyhow!("record not found: {record_id}"))?;
        let relative_path = record.local_path.as_deref().ok_or_else(|| anyhow!("record has no local artifact. please download it first."))?;
        let full_path = self.library.get_full_path(relative_path);
        let extension = full_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        // 1. OCR (Foundation)
        let _ = _app.emit("analysis-progress", serde_json::json!({
            "status": "extracting-foundation",
            "record_id": record_id
        }));
        let (text, engine) = self.extract_text(&full_path).await?;
        
        let _ = _app.emit("analysis-progress", serde_json::json!({
            "status": "indexing-vector",
            "record_id": record_id
        }));
        
        // Persist raw text immediately
        sqlx::query("INSERT INTO analysis_results (record_id, ocr_text, status, processed_at) VALUES (?, ?, 'indexed', ?) ON CONFLICT(record_id) DO UPDATE SET ocr_text = excluded.ocr_text, status = 'indexed', processed_at = excluded.processed_at")
            .bind(record_id).bind(&text).bind(crate::common::now()).execute(&self.db).await?;

        // 2. Asset & Thumbnail Extraction
        let mut assets = Vec::new();
        let asset_dir = self.library.get_full_path(&format!("assets/{}", record_id));
        fs::create_dir_all(&asset_dir).await?;

        let thumb_filename = "thumb_main.png";
        let thumb_path = asset_dir.join(thumb_filename);
        if self.thumbnails.generate_thumbnail(&full_path, &thumb_path).await.is_ok() {
            let rel_thumb = format!("assets/{}/{}", record_id, thumb_filename);
            sqlx::query("UPDATE records SET thumbnail_path = ? WHERE id = ?").bind(&rel_thumb).bind(record_id).execute(&self.db).await?;
        }

        if extension == "pdf" {
            if let Ok(extracted_images) = self.pdf.extract_images(&full_path, &asset_dir).await {
                for (filename, mime) in extracted_images {
                    let asset_id = Uuid::new_v4().to_string();
                    let rel_path = format!("assets/{}/{}", record_id, filename);
                    let size = fs::metadata(asset_dir.join(&filename)).await.map(|m| m.len() as i64).ok();
                    sqlx::query("INSERT INTO record_assets (id, record_id, asset_type, local_path, mime_type, file_size, created_at) VALUES (?, ?, 'image', ?, ?, ?, ?)")
                        .bind(&asset_id).bind(record_id).bind(&rel_path).bind(&mime).bind(size).bind(crate::common::now()).execute(&self.db).await?;
                    assets.push(RecordAsset { id: asset_id, record_id: record_id.to_string(), asset_type: "image".to_string(), local_path: rel_path, mime_type: Some(mime), file_size: size, metadata_json: None, created_at: crate::common::now() });
                }
            }
        }

        // 3. Forensic Layer Extraction (Rule-based)
        let forensics = if extension == "pdf" { self.pdf.extract_forensics(&full_path).unwrap_or_default() } else { Vec::new() };
        self.persist_forensics(record_id, &forensics).await?;

        // 4. Entity Extraction & Persistence
        let entities = extract_entities(&text);
        self.persist_entities(record_id, &entities).await?;

        // 5. Vector Indexing
        let chunks_indexed = self.persist_chunks(record_id, &record.title, &text, &entities).await?;
        
        let _ = _app.emit("analysis-progress", serde_json::json!({
            "status": "indexing-vector",
            "record_id": record_id,
            "chunk_count": chunks_indexed
        }));

        // 6. Final State Update
        let redaction_score = self.ocr.analyze_redactions(&full_path).unwrap_or(0.0);
        sqlx::query("UPDATE records SET analysis_status = 'indexed', redaction_score = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(redaction_score).bind(record_id).execute(&self.db).await?;

        Ok(AnalysisReport {
            record_id: record_id.to_string(),
            status: "indexed".to_string(),
            ocr_text: text,
            entities,
            chunks_indexed,
            engine,
            intelligence_json: None,
            assets,
        })
    }

    async fn synthesize_intelligence_inner(&self, app: &tauri::AppHandle, record_id: &str) -> Result<AnalysisReport> {
        let record = records::find_by_id(&self.db, record_id).await?.ok_or_else(|| anyhow!("record not found"))?;
        let res_row = sqlx::query("SELECT ocr_text FROM analysis_results WHERE record_id = ?").bind(record_id).fetch_one(&self.db).await?;
        let text: String = res_row.get("ocr_text");
        let assets = sqlx::query_as::<_, RecordAsset>("SELECT * FROM record_assets WHERE record_id = ?").bind(record_id).fetch_all(&self.db).await?;
        
        let specs = get_hardware_specs();
        let preferred_model = match specs.recommended_tier {
            IntelligenceTier::Elite => "gemma-4-e4b",
            _ => "gemma-4-e2b",
        };
        let verified_model_path = self.models.models_dir().join(preferred_model);
        
        let mut image_paths = Vec::new();
        for asset in &assets {
            if asset.asset_type == "image" {
                image_paths.push(self.library.get_full_path(&asset.local_path));
            }
        }

        let intelligence_json = self.extractor.extract_forensics(
            app,
            record_id,
            ExtractionConfig { 
                preferred_model_path: Some(verified_model_path), 
                fallback_model_path: Some(self.models.models_dir().join("gemma-4-e2b")), 
                force_cpu: !specs.gpu_acceleration_available 
            },
            &text,
            image_paths
        ).await?;

        let intel_str = serde_json::to_string(&intelligence_json)?;
        let summary = extraction_summary(&Some(intel_str.clone()));
        let extracted_loc = extraction_location(&Some(intel_str.clone()));

        // Smart location handling: only overwrite vague/unknown locations
        let current_location = record.incident_location.as_deref().unwrap_or("N/A");
        let final_location = if is_unspecified_location(current_location) {
            extracted_loc.or_else(|| Some(current_location.to_string()))
        } else {
            Some(current_location.to_string())
        };

        sqlx::query(
            "UPDATE records SET analysis_status = 'completed', intelligence_json = ?, summary = ?, incident_location = ?, analysis_error = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
            .bind(&intel_str).bind(summary).bind(final_location).bind(record_id)
            .execute(&self.db).await?;

        sqlx::query("UPDATE analysis_results SET status = 'completed', processed_at = ? WHERE record_id = ?")
            .bind(now()).bind(record_id).execute(&self.db).await?;

        self.get_analysis(record_id).await?.ok_or_else(|| anyhow!("failed to retrieve final report"))
    }

    pub async fn analyze_record(&self, app: &tauri::AppHandle, record_id: &str) -> Result<AnalysisReport> {
        // Legacy entry point: Run both in sequence for backward compatibility
        self.index_record(app, record_id).await?;
        self.synthesize_intelligence(app, record_id).await
    }

    pub async fn get_analysis(&self, record_id: &str) -> Result<Option<AnalysisReport>> {
        let row = sqlx::query("SELECT r.intelligence_json, r.analysis_status, r.redaction_score, ar.ocr_text FROM records r LEFT JOIN analysis_results ar ON ar.record_id = r.id WHERE r.id = ?")
        .bind(record_id).fetch_optional(&self.db).await?;
        let Some(row) = row else { return Ok(None); };
        let status: String = row.get::<Option<String>, _>("analysis_status").unwrap_or_else(|| "pending".to_string());
        let intelligence_json: Option<String> = row.get("intelligence_json");
        let ocr_text: String = row.get::<Option<String>, _>("ocr_text").unwrap_or_default();
        let entities = load_entities(&self.db, record_id).await?;
        let chunks_indexed = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM analysis_chunks WHERE record_id = ?").bind(record_id).fetch_one(&self.db).await.unwrap_or(0);
        let mut assets = sqlx::query_as::<_, RecordAsset>("SELECT * FROM record_assets WHERE record_id = ? ORDER BY created_at ASC").bind(record_id).fetch_all(&self.db).await?;
        for asset in &mut assets { asset.local_path = self.library.get_full_path(&asset.local_path).to_string_lossy().into_owned(); }
        Ok(Some(AnalysisReport { record_id: record_id.to_string(), status, ocr_text, entities, chunks_indexed: chunks_indexed as usize, engine: "stored".to_string(), intelligence_json, assets }))
    }


    async fn extract_text(&self, path: &Path) -> Result<(String, String)> {
        let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("").to_lowercase();
        match extension.as_str() {
            "pdf" => {
                let digital = self.pdf.extract_text(path).await?;
                if digital.trim().len() > 100 { 
                    Ok((digital, "pdf-text".to_string())) 
                } else {
                    // Digital text is sparse, trigger OS-native OCR (handles multi-page)
                    #[cfg(target_os = "macos")]
                    {
                        info!("Digital text sparse. Triggering macOS Vision OCR...");
                        if let Ok(text) = self::native_macos::extract_text_macos(path).await {
                            return Ok((text, "macos-vision-pdf".to_string()));
                        }
                    }
                    
                    #[cfg(target_os = "windows")]
                    {
                        info!("Digital text sparse. Triggering Windows Media OCR...");
                        if let Ok(text) = self::native_windows::extract_text_windows(path).await {
                            return Ok((text, "windows-ocr-pdf".to_string()));
                        }
                    }

                    // Pure Rust fallback for Linux or if OS-native fails
                    let text = self.ocr.extract_text_fallback(path).await?;
                    Ok((text, "rust-ocrs".to_string()))
                }
            }
            "txt" | "md" | "csv" | "json" => Ok((fs::read_to_string(path).await?, "text-file".to_string())),
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
                #[cfg(target_os = "macos")]
                if let Ok(text) = self::native_macos::extract_text_macos(path).await {
                    return Ok((text, "macos-vision-image".to_string()));
                }
                
                #[cfg(target_os = "windows")]
                if let Ok(text) = self::native_windows::extract_text_windows(path).await {
                    return Ok((text, "windows-ocr-image".to_string()));
                }

                // Pure Rust fallback
                let text = self.ocr.extract_text_fallback(path).await?;
                Ok((text, "rust-ocrs".to_string()))
            }
            _ => Err(anyhow!("unsupported type `{}`", extension)),
        }
    }

    async fn persist_entities(&self, record_id: &str, entities: &[EntityHit]) -> Result<()> {
        sqlx::query("DELETE FROM record_entities WHERE record_id = ?").bind(record_id).execute(&self.db).await?;
        for entity in entities {
            sqlx::query(r#"INSERT INTO entities (id, name, entity_type, description) VALUES (?, ?, ?, ?) ON CONFLICT(name, entity_type) DO UPDATE SET description = excluded.description"#)
            .bind(&entity.id).bind(&entity.name).bind(&entity.entity_type).bind(&entity.source)
            .execute(&self.db).await?;
            let eid: String = sqlx::query_scalar("SELECT id FROM entities WHERE name = ? AND entity_type = ?").bind(&entity.name).bind(&entity.entity_type).fetch_one(&self.db).await?;
            sqlx::query("INSERT INTO record_entities (record_id, entity_id, confidence) VALUES (?, ?, ?)").bind(record_id).bind(eid).bind(entity.confidence).execute(&self.db).await?;
        }
        Ok(())
    }

    async fn persist_chunks(&self, record_id: &str, title: &str, text: &str, entities: &[EntityHit]) -> Result<usize> {
        sqlx::query("DELETE FROM analysis_chunks WHERE record_id = ?").bind(record_id).execute(&self.db).await?;
        sqlx::query("DELETE FROM analysis_chunks_fts WHERE record_id = ?").bind(record_id).execute(&self.db).await?;
        let chunks = chunk_text(text, 1800);
        let etext = entities.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(" ");
        for (i, chunk) in chunks.iter().enumerate() {
            let cid = Uuid::new_v4().to_string();
            let emb = vectorize_text(chunk).await?;
            let vblob: &[u8] = unsafe { std::slice::from_raw_parts(emb.as_ptr() as *const u8, emb.len() * 4) };
            
            sqlx::query("INSERT INTO analysis_chunks (id, record_id, chunk_index, text, engine_name, model_version, created_at) VALUES (?, ?, ?, ?, 'bge-small', 'v1.5', ?)")
            .bind(&cid).bind(record_id).bind(i as i64).bind(chunk).bind(now()).execute(&self.db).await?;
            
            sqlx::query("INSERT INTO vec_analysis_chunks (chunk_id, embedding) VALUES (?, ?)")
            .bind(&cid).bind(vblob).execute(&self.db).await?;
            sqlx::query("INSERT INTO analysis_chunks_fts (chunk_id, record_id, title, text, entities) VALUES (?, ?, ?, ?, ?)")
            .bind(&cid).bind(record_id).bind(title).bind(chunk).bind(&etext).execute(&self.db).await?;
        }
        Ok(chunks.len())
    }

    async fn persist_forensics(&self, record_id: &str, discoveries: &[self::pdf::ForensicDiscovery]) -> Result<()> {
        sqlx::query("DELETE FROM record_forensics WHERE record_id = ?").bind(record_id).execute(&self.db).await?;
        for discovery in discoveries {
            let id = Uuid::new_v4().to_string();
            let bbox_json = discovery.metadata.get("bbox").map(|b| b.to_string());
            let mjson = serde_json::to_string(&discovery.metadata)?;
            sqlx::query(
                "INSERT INTO record_forensics (id, record_id, layer_type, content, confidence, bounding_box_json, metadata_json, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&id)
            .bind(record_id)
            .bind(&discovery.layer_type)
            .bind(&discovery.content)
            .bind(discovery.confidence as f64)
            .bind(bbox_json)
            .bind(mjson)
            .bind(now())
            .execute(&self.db)
            .await?;
        }
        Ok(())
    }
}

pub fn extract_entities(text: &str) -> Vec<EntityHit> {
    let mut entities = BTreeMap::<(String, String), EntityHit>::new();
    let terms = [("AARO", "agency"), ("NASA", "agency"), ("orb", "shape"), ("sphere", "shape"), ("radar", "sensor")];
    for (t, ty) in terms { add_if_present(&mut entities, text, t, ty, 0.9); }
    let re = Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap();
    for h in re.find_iter(text) { add_entity(&mut entities, h.as_str(), "date", 0.8, "deterministic"); }
    entities.into_values().collect()
}

async fn load_entities(pool: &SqlitePool, rid: &str) -> Result<Vec<EntityHit>> {
    let rows = sqlx::query("SELECT e.id, e.name, e.entity_type, e.description, re.confidence FROM entities e JOIN record_entities re ON re.entity_id = e.id WHERE re.record_id = ?")
    .bind(rid).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| EntityHit { id: r.get("id"), name: r.get("name"), entity_type: r.get("entity_type"), confidence: r.get("confidence"), source: r.get::<Option<String>, _>("description").unwrap_or_default() }).collect())
}

fn add_if_present(e: &mut BTreeMap<(String, String), EntityHit>, t: &str, term: &str, ty: &str, c: f64) {
    if Regex::new(&format!(r"(?i)\b{}\b", term)).unwrap().is_match(t) { add_entity(e, term, ty, c, "deterministic"); }
}

fn add_entity(e: &mut BTreeMap<(String, String), EntityHit>, n: &str, ty: &str, c: f64, s: &str) {
    let name = n.trim().to_string();
    if name.is_empty() { return; }
    e.entry((name.to_lowercase(), ty.to_string())).or_insert(EntityHit { id: Uuid::new_v4().to_string(), name, entity_type: ty.to_string(), confidence: c, source: s.to_string() });
}

fn now() -> String { crate::common::now() }

fn extraction_summary(j: &Option<String>) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(j.as_ref()?).ok()?;
    v.get("object_description").and_then(|d| d.as_str()).map(|d| d.to_string())
}

fn extraction_location(j: &Option<String>) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(j.as_ref()?).ok()?;
    v.get("location").and_then(|d| d.as_str()).map(|d| d.to_string())
}

fn is_unspecified_location(loc: &str) -> bool {
    let l = loc.to_lowercase();
    l == "n/a" || l == "unknown" || l == "global" || l == "none" || l.is_empty()
}
