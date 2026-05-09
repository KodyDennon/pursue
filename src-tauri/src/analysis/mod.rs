pub mod ocr;
pub mod pdf;

use anyhow::{anyhow, Result};
use regex::Regex;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

use crate::db::records;
use crate::library::LibraryManager;
use crate::models::{AnalysisReport, EntityHit};
use crate::search::{chunk_text, vectorize_text};

use self::ocr::OcrEngine;
use self::pdf::PdfAnalyzer;

pub struct AnalysisManager {
    db: SqlitePool,
    library: Arc<LibraryManager>,
    ocr: OcrEngine,
    pdf: PdfAnalyzer,
}

impl AnalysisManager {
    pub fn new(db: SqlitePool, library: Arc<LibraryManager>) -> Self {
        Self {
            db,
            library,
            ocr: OcrEngine::new(),
            pdf: PdfAnalyzer::new(),
        }
    }

    pub async fn analyze_record(&self, record_id: &str) -> Result<AnalysisReport> {
        let record = records::find_by_id(&self.db, record_id)
            .await?
            .ok_or_else(|| anyhow!("record not found: {record_id}"))?;
        let relative_path = record
            .local_path
            .as_deref()
            .ok_or_else(|| anyhow!("record has no local artifact; download or import evidence first"))?;
        let full_path = self.library.get_full_path(relative_path);
        if !full_path.exists() {
            return Err(anyhow!("local artifact is missing: {}", full_path.display()));
        }

        sqlx::query(
            r#"
            INSERT INTO analysis_results (record_id, ocr_text, status, processed_at)
            VALUES (?, '', 'processing', CURRENT_TIMESTAMP)
            ON CONFLICT(record_id) DO UPDATE SET status = 'processing', processed_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(record_id)
        .execute(&self.db)
        .await?;

        let (text, engine) = match self.extract_text(&full_path).await {
            Ok(result) => result,
            Err(error) => {
                sqlx::query(
                    r#"
                    INSERT INTO analysis_results (record_id, ocr_text, status, processed_at)
                    VALUES (?, ?, 'failed', CURRENT_TIMESTAMP)
                    ON CONFLICT(record_id) DO UPDATE SET ocr_text = excluded.ocr_text, status = 'failed', processed_at = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(record_id)
                .bind(error.to_string())
                .execute(&self.db)
                .await?;
                return Err(error);
            }
        };

        let context = format!(
            "{}\n{}\n{}\n{}\n{}",
            record.title,
            record.summary.as_deref().unwrap_or(""),
            record.agency.as_deref().unwrap_or(""),
            record.incident_location.as_deref().unwrap_or(""),
            text
        );
        let entities = extract_entities(&context);
        self.persist_entities(record_id, &entities).await?;
        let chunks_indexed = self.persist_chunks(record_id, &record.title, &text, &entities).await?;

        sqlx::query(
            r#"
            INSERT INTO analysis_results (record_id, ocr_text, status, processed_at)
            VALUES (?, ?, 'completed', CURRENT_TIMESTAMP)
            ON CONFLICT(record_id) DO UPDATE SET
                ocr_text = excluded.ocr_text,
                status = 'completed',
                processed_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(record_id)
        .bind(&text)
        .execute(&self.db)
        .await?;

        Ok(AnalysisReport {
            record_id: record_id.to_string(),
            status: "completed".to_string(),
            ocr_text: text,
            entities,
            chunks_indexed,
            engine,
        })
    }

    pub async fn get_analysis(&self, record_id: &str) -> Result<Option<AnalysisReport>> {
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT ocr_text, status FROM analysis_results WHERE record_id = ?",
        )
        .bind(record_id)
        .fetch_optional(&self.db)
        .await?;

        let Some((ocr_text, status)) = row else {
            return Ok(None);
        };

        let entities = load_entities(&self.db, record_id).await?;
        let chunks_indexed = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM analysis_chunks WHERE record_id = ?",
        )
        .bind(record_id)
        .fetch_one(&self.db)
        .await
        .unwrap_or(0);

        Ok(Some(AnalysisReport {
            record_id: record_id.to_string(),
            status,
            ocr_text,
            entities,
            chunks_indexed: usize::try_from(chunks_indexed).unwrap_or(0),
            engine: "stored".to_string(),
        }))
    }

    async fn extract_text(&self, path: &Path) -> Result<(String, String)> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match extension.as_str() {
            "pdf" => {
                let digital = self.pdf.extract_text(path).await?;
                if digital.trim().len() > 80 {
                    Ok((digital, "pdf-text".to_string()))
                } else {
                    let ocr = self.ocr.extract_text_from_scanned_pdf(path).await?;
                    if ocr.trim().is_empty() {
                        Err(anyhow!("PDF analysis produced no text"))
                    } else {
                        Ok((ocr, "ocrmypdf".to_string()))
                    }
                }
            }
            "txt" | "md" | "csv" | "json" => Ok((fs::read_to_string(path).await?, "text-file".to_string())),
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" => Ok((
                self.ocr.extract_text_from_image(path).await?,
                "tesseract".to_string(),
            )),
            _ => Err(anyhow!(
                "unsupported analysis file type `{}` for {}",
                extension,
                path.display()
            )),
        }
    }

    async fn persist_entities(&self, record_id: &str, entities: &[EntityHit]) -> Result<()> {
        sqlx::query("DELETE FROM record_entities WHERE record_id = ?")
            .bind(record_id)
            .execute(&self.db)
            .await?;

        for entity in entities {
            sqlx::query(
                r#"
                INSERT INTO entities (id, name, entity_type, description)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(name, entity_type) DO UPDATE SET description = excluded.description
                "#,
            )
            .bind(&entity.id)
            .bind(&entity.name)
            .bind(&entity.entity_type)
            .bind(&entity.source)
            .execute(&self.db)
            .await?;

            let entity_id = sqlx::query_scalar::<_, String>(
                "SELECT id FROM entities WHERE name = ? AND entity_type = ?",
            )
            .bind(&entity.name)
            .bind(&entity.entity_type)
            .fetch_one(&self.db)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO record_entities (record_id, entity_id, confidence)
                VALUES (?, ?, ?)
                ON CONFLICT(record_id, entity_id) DO UPDATE SET confidence = excluded.confidence
                "#,
            )
            .bind(record_id)
            .bind(entity_id)
            .bind(entity.confidence)
            .execute(&self.db)
            .await?;
        }

        Ok(())
    }

    async fn persist_chunks(
        &self,
        record_id: &str,
        title: &str,
        text: &str,
        entities: &[EntityHit],
    ) -> Result<usize> {
        sqlx::query("DELETE FROM analysis_chunks WHERE record_id = ?")
            .bind(record_id)
            .execute(&self.db)
            .await?;
        sqlx::query("DELETE FROM analysis_chunks_fts WHERE record_id = ?")
            .bind(record_id)
            .execute(&self.db)
            .await?;

        let chunks = chunk_text(text, 1800);
        let entity_text = entities
            .iter()
            .map(|entity| entity.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        for (index, chunk) in chunks.iter().enumerate() {
            let chunk_id = Uuid::new_v4().to_string();
            let vector_json = serde_json::to_string(&vectorize_text(chunk))?;
            sqlx::query(
                r#"
                INSERT INTO analysis_chunks (id, record_id, chunk_index, text, vector_json, created_at)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&chunk_id)
            .bind(record_id)
            .bind(i64::try_from(index).unwrap_or(0))
            .bind(chunk)
            .bind(vector_json)
            .bind(now())
            .execute(&self.db)
            .await?;

            sqlx::query(
                "INSERT INTO analysis_chunks_fts (chunk_id, record_id, title, text, entities) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&chunk_id)
            .bind(record_id)
            .bind(title)
            .bind(chunk)
            .bind(&entity_text)
            .execute(&self.db)
            .await?;
        }

        Ok(chunks.len())
    }
}

pub fn extract_entities(text: &str) -> Vec<EntityHit> {
    let mut entities = BTreeMap::<(String, String), EntityHit>::new();
    let agency_terms = [
        "AARO", "NASA", "FBI", "CIA", "DIA", "NSA", "NRO", "NORAD", "FAA", "USAF", "USN",
        "US Navy", "U.S. Navy", "Department of Defense", "DoD",
    ];
    let object_terms = [
        "orb", "sphere", "disc", "disk", "triangle", "cigar", "tic tac", "cylinder", "boomerang",
        "light", "metallic",
    ];
    let sensor_terms = [
        "radar", "FLIR", "infrared", "thermal", "satellite", "sonar", "camera", "electro-optical",
    ];

    for term in agency_terms {
        add_if_present(&mut entities, text, term, "agency", 0.94);
    }
    for term in object_terms {
        add_if_present(&mut entities, text, term, "object_shape", 0.72);
    }
    for term in sensor_terms {
        add_if_present(&mut entities, text, term, "sensor", 0.76);
    }

    let date_re = Regex::new(
        r"\b(?:\d{4}-\d{2}-\d{2}|\d{1,2}/\d{1,2}/\d{2,4}|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\.?\s+\d{1,2},?\s+\d{4})\b",
    )
    .expect("valid date regex");
    for hit in date_re.find_iter(text) {
        add_entity(&mut entities, hit.as_str(), "date", 0.82, "deterministic-date");
    }

    let file_re = Regex::new(r"\b[A-Za-z0-9_-]+\.(?:pdf|jpg|jpeg|png|tif|tiff|mov|mp4|csv|txt)\b")
        .expect("valid file regex");
    for hit in file_re.find_iter(text) {
        add_entity(&mut entities, hit.as_str(), "file_reference", 0.9, "deterministic-file-ref");
    }

    let location_re = Regex::new(
        r"\b(?:Nevada|New Mexico|Arizona|California|Texas|Virginia|Florida|Atlantic|Pacific|Kazakhstan|Papua New Guinea|Mexico|Middle East|United States)\b",
    )
    .expect("valid location regex");
    for hit in location_re.find_iter(text) {
        add_entity(&mut entities, hit.as_str(), "location", 0.78, "deterministic-location");
    }

    let name_re = Regex::new(r"\b[A-Z][a-z]{2,}\s+[A-Z][a-z]{2,}\b").expect("valid name regex");
    let ignored: HashSet<&str> = ["United States", "Middle East", "New Mexico", "Papua New"].into_iter().collect();
    for hit in name_re.find_iter(text) {
        if ignored.contains(hit.as_str()) {
            continue;
        }
        add_entity(&mut entities, hit.as_str(), "person_like", 0.52, "deterministic-name-pattern");
    }

    entities.into_values().collect()
}

async fn load_entities(pool: &SqlitePool, record_id: &str) -> Result<Vec<EntityHit>> {
    let rows = sqlx::query(
        r#"
        SELECT e.id, e.name, e.entity_type, e.description, re.confidence
        FROM entities e
        JOIN record_entities re ON re.entity_id = e.id
        WHERE re.record_id = ?
        ORDER BY e.entity_type, e.name
        "#,
    )
    .bind(record_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| EntityHit {
            id: sqlx::Row::get(&row, "id"),
            name: sqlx::Row::get(&row, "name"),
            entity_type: sqlx::Row::get(&row, "entity_type"),
            confidence: sqlx::Row::get::<f64, _>(&row, "confidence"),
            source: sqlx::Row::get::<Option<String>, _>(&row, "description")
                .unwrap_or_else(|| "entity-index".to_string()),
        })
        .collect())
}

fn add_if_present(
    entities: &mut BTreeMap<(String, String), EntityHit>,
    text: &str,
    term: &str,
    entity_type: &str,
    confidence: f64,
) {
    let pattern = format!(r"(?i)\b{}\b", regex::escape(term));
    if Regex::new(&pattern).expect("valid term regex").is_match(text) {
        add_entity(entities, term, entity_type, confidence, "deterministic-term");
    }
}

fn add_entity(
    entities: &mut BTreeMap<(String, String), EntityHit>,
    name: &str,
    entity_type: &str,
    confidence: f64,
    source: &str,
) {
    let name = name.trim().to_string();
    if name.is_empty() {
        return;
    }
    let key = (name.to_lowercase(), entity_type.to_string());
    entities.entry(key).or_insert_with(|| EntityHit {
        id: Uuid::new_v4().to_string(),
        name,
        entity_type: entity_type.to_string(),
        confidence,
        source: source.to_string(),
    });
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::extract_entities;

    #[test]
    fn extracts_deterministic_entities() {
        let entities = extract_entities("AARO radar saw a triangle over Nevada on 2026-01-01.");
        assert!(entities.iter().any(|entity| entity.name == "AARO"));
        assert!(entities.iter().any(|entity| entity.entity_type == "sensor"));
        assert!(entities.iter().any(|entity| entity.entity_type == "date"));
    }
}
