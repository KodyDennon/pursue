use crate::models::{SearchRequest, SearchResultItem, SearchResults};
use anyhow::Result;
use lazy_static::lazy_static;
use ort::session::Session;
use ort::value::Value;
use sqlx::{Row, SqlitePool};
use std::sync::Mutex;
use tokenizers::Tokenizer;

const VECTOR_DIMS: usize = 384;

lazy_static! {
    static ref TOKENIZER: Tokenizer =
        Tokenizer::from_file("models/tokenizer.json").expect("failed to load tokenizer");
    static ref EMBEDDING_SESSION: Mutex<Session> = {
        let session = Session::builder()
            .expect("failed to create ort session builder")
            .commit_from_file("models/bge-small-en-v1.5.onnx")
            .expect("failed to load embedding model");
        Mutex::new(session)
    };
}

pub async fn search(pool: &SqlitePool, request: SearchRequest) -> Result<SearchResults> {
    vector_search(pool, request.query).await
}

pub async fn vector_search(pool: &SqlitePool, query: String) -> Result<SearchResults> {
    let query_vector = vectorize_text(&query).await?;

    let vector_blob: &[u8] = unsafe {
        std::slice::from_raw_parts(
            query_vector.as_ptr() as *const u8,
            query_vector.len() * std::mem::size_of::<f32>(),
        )
    };

    // Fallback search if vector extension is not loaded or fails
    // In a real production app, we would handle the lack of sqlite-vec gracefully.
    // For now, we'll try to use it but fallback to keyword search if it errors.
    
    let results = match sqlx::query_as::<_, SearchResultItem>(
        r#"
        SELECT 
            r.id, r.title, r.agency, r.release_date, r.document_url, r.local_path, 
            r.intelligence_json as summary, r.artifact_sha256,
            vec_distance_cosine(c.embedding, ?) as distance,
            c.text as excerpt
        FROM analysis_chunks c
        JOIN records r ON r.id = c.record_id
        WHERE c.embedding MATCH ? AND k = 20
        ORDER BY distance ASC
        "#,
    )
    .bind(vector_blob)
    .bind(vector_blob)
    .fetch_all(pool)
    .await {
        Ok(res) => res,
        Err(_) => {
            // Fallback to simple keyword search
            sqlx::query_as::<_, SearchResultItem>(
                r#"
                SELECT 
                    r.id, r.title, r.agency, r.release_date, r.document_url, r.local_path, 
                    r.intelligence_json as summary, r.artifact_sha256,
                    0.0 as distance,
                    c.text as excerpt
                FROM analysis_chunks c
                JOIN records r ON r.id = c.record_id
                WHERE r.title LIKE ? OR c.text LIKE ?
                LIMIT 20
                "#,
            )
            .bind(format!("%{}%", query))
            .bind(format!("%{}%", query))
            .fetch_all(pool)
            .await?
        }
    };

    Ok(SearchResults {
        query,
        total: results.len(),
        results,
    })
}

pub async fn vectorize_text(text: &str) -> Result<Vec<f32>> {
    let encoding = TOKENIZER
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
    let input_ids = encoding.get_ids();
    let attention_mask = encoding.get_attention_mask();

    let input_ids_tensor = Value::from_array((
        vec![1, input_ids.len()],
        input_ids.iter().map(|&x| x as i64).collect::<Vec<i64>>(),
    ))?;

    let attention_mask_tensor = Value::from_array((
        vec![1, attention_mask.len()],
        attention_mask
            .iter()
            .map(|&x| x as i64)
            .collect::<Vec<i64>>(),
    ))?;

    let mut session = EMBEDDING_SESSION
        .lock()
        .map_err(|e| anyhow::anyhow!("Mutex lock failed: {}", e))?;
    let outputs = session.run(ort::inputs![
        "input_ids" => input_ids_tensor,
        "attention_mask" => attention_mask_tensor,
    ])?;

    let output = outputs
        .get("last_hidden_state")
        .ok_or_else(|| anyhow::anyhow!("failed to get last_hidden_state"))?;

    let (shape, data) = output.try_extract_tensor::<f32>()?;

    // Mean pooling
    let seq_len = shape[1] as usize;
    let hidden_size = shape[2] as usize;

    let mut mean_vec = vec![0.0f32; hidden_size];
    for i in 0..seq_len {
        for j in 0..hidden_size {
            mean_vec[j] += data[i * hidden_size + j];
        }
    }

    for x in mean_vec.iter_mut() {
        *x /= seq_len as f32;
    }

    Ok(mean_vec)
}

pub fn chunk_text(text: &str, target_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for paragraph in text.split('\n') {
        if current.len() + paragraph.len() > target_chars && !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
            current.clear();
        }
        current.push_str(paragraph);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::{vectorize_text, VECTOR_DIMS};

    #[tokio::test]
    async fn vectorize_is_stable() {
        let v1 = vectorize_text("AARO sensor").await.unwrap();
        assert_eq!(v1.len(), VECTOR_DIMS);
        let v2 = vectorize_text("AARO sensor").await.unwrap();
        assert_eq!(v1, v2);
    }
}
