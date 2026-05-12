use crate::common::now;
use crate::models::EntityHit;
use crate::search::{chunk_text, vectorize_text};
use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct PersistenceManager {
    db: SqlitePool,
}

impl PersistenceManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn persist_entities(&self, record_id: &str, entities: &[EntityHit]) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query("DELETE FROM record_entities WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;

        for entity in entities {
            sqlx::query(r#"INSERT INTO entities (id, name, entity_type, description) VALUES (?, ?, ?, ?) ON CONFLICT(name, entity_type) DO UPDATE SET description = excluded.description"#)
                .bind(&entity.id)
                .bind(&entity.name)
                .bind(&entity.entity_type)
                .bind(&entity.source)
                .execute(&mut *tx)
                .await?;

            let eid: String =
                sqlx::query_scalar("SELECT id FROM entities WHERE name = ? AND entity_type = ?")
                    .bind(&entity.name)
                    .bind(&entity.entity_type)
                    .fetch_one(&mut *tx)
                    .await?;

            sqlx::query(
                "INSERT INTO record_entities (record_id, entity_id, confidence) VALUES (?, ?, ?)",
            )
            .bind(record_id)
            .bind(eid)
            .bind(entity.confidence)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn persist_chunks(
        &self,
        record_id: &str,
        title: &str,
        text: &str,
        entities: &[EntityHit],
    ) -> Result<usize> {
        let mut tx = self.db.begin().await?;

        sqlx::query("DELETE FROM analysis_chunks WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM analysis_chunks_fts WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;

        let chunks = chunk_text(text, 1800);
        let etext = entities
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        for (i, chunk) in chunks.iter().enumerate() {
            let cid = Uuid::new_v4().to_string();
            let emb = vectorize_text(chunk).await?;
            let vblob: &[u8] =
                unsafe { std::slice::from_raw_parts(emb.as_ptr() as *const u8, emb.len() * 4) };

            sqlx::query("INSERT INTO analysis_chunks (id, record_id, chunk_index, text, engine_name, model_version, created_at) VALUES (?, ?, ?, ?, 'bge-small', 'v1.5', ?)")
                .bind(&cid).bind(record_id).bind(i as i64).bind(chunk).bind(now()).execute(&mut *tx).await?;

            sqlx::query("INSERT INTO vec_analysis_chunks (chunk_id, embedding) VALUES (?, ?)")
                .bind(&cid)
                .bind(vblob)
                .execute(&mut *tx)
                .await?;

            sqlx::query("INSERT INTO analysis_chunks_fts (chunk_id, record_id, title, text, entities) VALUES (?, ?, ?, ?, ?)")
                .bind(&cid).bind(record_id).bind(title).bind(chunk).bind(&etext).execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(chunks.len())
    }

    pub async fn persist_forensics(
        &self,
        record_id: &str,
        discoveries: &[crate::analysis::pdf::ForensicDiscovery],
    ) -> Result<()> {
        let mut tx = self.db.begin().await?;

        sqlx::query("DELETE FROM record_forensics WHERE record_id = ?")
            .bind(record_id)
            .execute(&mut *tx)
            .await?;

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
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
