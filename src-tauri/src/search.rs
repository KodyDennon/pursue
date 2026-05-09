use anyhow::{anyhow, Result};
use ort::{Session, SessionBuilder, Value as OrtValue};
use std::sync::Arc;
use lazy_static::lazy_static;
use tokenizers::Tokenizer;

const VECTOR_DIMS: usize = 384; 

lazy_static! {
    static ref TOKENIZER: Tokenizer = Tokenizer::from_file("models/tokenizer.json").expect("failed to load tokenizer");
    static ref EMBEDDING_SESSION: Arc<Session> = {
        Session::builder()
            .expect("failed to create ort session builder")
            .with_model_from_file("models/bge-small-en-v1.5.onnx")
            .expect("failed to load embedding model")
            .into()
    };
}

    let query_vector = vectorize_text(&query);
    let vector_blob = zerocopy::AsBytes::as_bytes(query_vector.as_slice());

    let results = sqlx::query_as::<_, SearchResultItem>(
        r#"
        SELECT 
            r.*, 
            vec_cos_distance(c.embedding, ?) as distance,
            c.text as excerpt
        FROM analysis_chunks c
        JOIN records r ON r.id = c.record_id
        WHERE c.embedding MATCH ? AND k = 50
        ORDER BY distance ASC
        "#
    )
    .bind(vector_blob)
    .bind(vector_blob)
    .fetch_all(pool)
    .await?;

    Ok(SearchResults {
        query,
        total: results.len(),
        results,
    })
}

pub fn vectorize_text(text: &str) -> Vec<f32> {
    let encoding = TOKENIZER.encode(text, true).expect("failed to encode text");
    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&id| id as i64).collect();
    let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&id| id as i64).collect();

    let input_ids_tensor = ort::inputs![ort::Tensor::from_array((&[1, input_ids.len()], input_ids)).unwrap()].unwrap();
    // ... run ORT ...
    vec![0.0; VECTOR_DIMS] // This will be fully implemented in the final pass
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

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter()
        .zip(b.iter())
        .map(|(left, right)| f64::from(left * right))
        .sum::<f64>()
}

fn keyword_score(tokens: &[String], text: &str) -> f64 {
    let lowered = text.to_lowercase();
    tokens
        .iter()
        .filter(|token| lowered.contains(token.as_str()))
        .map(|token| 1.0 + (token.len() as f64 / 20.0))
        .sum()
}

fn tokenize(text: &str) -> Vec<String> {
    Regex::new(r"[A-Za-z0-9][A-Za-z0-9_-]{1,}")
        .expect("valid tokenizer regex")
        .find_iter(text)
        .map(|hit| hit.as_str().to_lowercase())
        .collect()
}

fn excerpt(text: &str, tokens: &[String]) -> String {
    let lower = text.to_lowercase();
    let first_match = tokens
        .iter()
        .filter_map(|token| lower.find(token).map(|index| (index, token)))
        .min_by_key(|(index, _)| *index)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let start = first_match.saturating_sub(120);
    let end = (first_match + 320).min(text.len());
    text[start..end].replace('\n', " ")
}

async fn record_in_case(pool: &SqlitePool, case_id: &str, record_id: &str) -> Result<bool> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM case_records WHERE case_id = ? AND record_id = ?",
    )
    .bind(case_id)
    .bind(record_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

async fn matching_entities(
    pool: &SqlitePool,
    record_id: &str,
    query_tokens: &[String],
) -> Result<Vec<EntityHit>> {
    let rows = sqlx::query(
        r#"
        SELECT e.id, e.name, e.entity_type, re.confidence
        FROM entities e
        JOIN record_entities re ON re.entity_id = e.id
        WHERE re.record_id = ?
        "#,
    )
    .bind(record_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let name = row.get::<String, _>("name");
            let lower = name.to_lowercase();
            if !query_tokens.iter().any(|token| lower.contains(token)) {
                return None;
            }
            Some(EntityHit {
                id: row.get("id"),
                name,
                entity_type: row.get("entity_type"),
                confidence: row.get::<f64, _>("confidence"),
                source: "entity-index".to_string(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{chunk_text, vectorize_text};

    #[test]
    fn vectorize_is_stable() {
        assert_eq!(vectorize_text("AARO sensor").len(), 256);
        assert_eq!(vectorize_text("AARO sensor"), vectorize_text("AARO sensor"));
    }

    #[test]
    fn chunks_text_without_empty_chunks() {
        let chunks = chunk_text("one\n\ntwo\nthree", 6);
        assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
    }
}
