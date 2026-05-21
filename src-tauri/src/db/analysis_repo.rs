use anyhow::Result;
use sqlx::{SqlitePool, Row};
use crate::common::now;

pub struct AnalysisRepository {
    db: SqlitePool,
}

impl AnalysisRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn update_analysis_status(&self, record_id: &str, status: &str, error: Option<&str>) -> Result<()> {
        sqlx::query(
            "UPDATE records SET analysis_status = ?, analysis_error = ? WHERE id = ?",
        )
        .bind(status)
        .bind(error)
        .bind(record_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_redaction_score(&self, record_id: &str, score: f32) -> Result<()> {
        sqlx::query(
            "UPDATE records SET redaction_score = ? WHERE id = ?",
        )
        .bind(score)
        .bind(record_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn save_ocr_result(&self, record_id: &str, text: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO analysis_results (record_id, ocr_text, status, processed_at) \
             VALUES (?, ?, 'indexed', ?) \
             ON CONFLICT(record_id) DO UPDATE SET ocr_text = excluded.ocr_text, status = 'indexed', processed_at = excluded.processed_at"
        )
        .bind(record_id)
        .bind(text)
        .bind(now())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn save_intelligence_json(&self, record_id: &str, json: &str) -> Result<()> {
        sqlx::query(
            "UPDATE records SET analysis_status = 'completed', intelligence_json = ? WHERE id = ?",
        )
        .bind(json)
        .bind(record_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn save_thumbnail_path(&self, record_id: &str, path: &str) -> Result<()> {
        sqlx::query("UPDATE records SET thumbnail_path = ? WHERE id = ?")
            .bind(path)
            .bind(record_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn insert_record_asset(&self, id: &str, record_id: &str, asset_type: &str, path: &str, mime: &str) -> Result<()> {
        sqlx::query("INSERT INTO record_assets (id, record_id, asset_type, local_path, mime_type, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id)
            .bind(record_id)
            .bind(asset_type)
            .bind(path)
            .bind(mime)
            .bind(now())
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn get_ocr_text(&self, record_id: &str) -> Result<String> {
        let row = sqlx::query("SELECT ocr_text FROM analysis_results WHERE record_id = ?")
            .bind(record_id)
            .fetch_one(&self.db)
            .await?;
        let text: String = row.get("ocr_text");
        Ok(text)
    }

    pub async fn clear_analysis_data(&self, record_id: &str) -> Result<()> {
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

    pub async fn clear_all_analysis_data(&self) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query("DELETE FROM analysis_results").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM vec_analysis_chunks").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM analysis_chunks").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM analysis_chunks_fts").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM record_forensics").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM record_entities").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM intelligence_logs").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM record_assets WHERE asset_type != 'source'").execute(&mut *tx).await?;
        
        sqlx::query("UPDATE records SET analysis_status = 'pending', intelligence_json = NULL, redaction_score = NULL")
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}
