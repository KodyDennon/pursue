use anyhow::Result;
use regex::Regex;
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};

use crate::db::records;
use crate::models::{EntityHit, RecordFilter, SearchRequest, SearchResultItem, SearchResults};

const VECTOR_DIMS: usize = 256;

pub async fn search(pool: &SqlitePool, request: SearchRequest) -> Result<SearchResults> {
    let query = request.query.trim().to_string();
    if query.is_empty() {
        return Ok(SearchResults {
            query,
            total: 0,
            results: Vec::new(),
        });
    }

    let query_vector = vectorize_text(&query);
    let query_tokens = tokenize(&query);
    let filters = request.filters;
    let candidate_records = records::list(
        pool,
        Some(RecordFilter {
            source_type: filters.as_ref().and_then(|f| f.source_type.clone()),
            agency: None,
            local_only: filters.as_ref().and_then(|f| f.local_only),
            query: None,
        }),
    )
    .await?;
    let allowed: HashSet<String> = candidate_records.iter().map(|record| record.id.clone()).collect();
    let record_map: HashMap<String, _> = candidate_records
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect();

    let rows = sqlx::query(
        r#"
        SELECT c.record_id, c.text, c.vector_json, r.title
        FROM analysis_chunks c
        JOIN records r ON r.id = c.record_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut scored: HashMap<String, (f64, String)> = HashMap::new();
    for row in rows {
        let record_id = row.get::<String, _>("record_id");
        if !allowed.contains(&record_id) {
            continue;
        }
        if let Some(case_id) = filters.as_ref().and_then(|f| f.case_id.as_deref()) {
            if !record_in_case(pool, case_id, &record_id).await? {
                continue;
            }
        }

        let text = row.get::<String, _>("text");
        let vector_json = row.get::<String, _>("vector_json");
        let vector = serde_json::from_str::<Vec<f32>>(&vector_json).unwrap_or_default();
        let vector_score = cosine(&query_vector, &vector);
        let keyword_score = keyword_score(&query_tokens, &text);
        let score = vector_score + keyword_score;
        if score <= 0.0 {
            continue;
        }

        let excerpt = excerpt(&text, &query_tokens);
        scored
            .entry(record_id)
            .and_modify(|existing| {
                if score > existing.0 {
                    *existing = (score, excerpt.clone());
                }
            })
            .or_insert((score, excerpt));
    }

    for (id, record) in &record_map {
        let haystack = format!(
            "{} {} {} {}",
            record.title,
            record.summary.as_deref().unwrap_or(""),
            record.agency.as_deref().unwrap_or(""),
            record.incident_location.as_deref().unwrap_or("")
        );
        let metadata_score = keyword_score(&query_tokens, &haystack);
        if metadata_score > 0.0 {
            scored
                .entry(id.clone())
                .and_modify(|existing| existing.0 += metadata_score)
                .or_insert((metadata_score, excerpt(&haystack, &query_tokens)));
        }
    }

    let mut results = Vec::new();
    for (record_id, (score, excerpt)) in scored {
        if let Some(record) = record_map.get(&record_id) {
            let matched_entities = matching_entities(pool, &record_id, &query_tokens).await?;
            results.push(SearchResultItem {
                record: record.clone(),
                score,
                excerpt,
                matched_entities,
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(50);

    Ok(SearchResults {
        query,
        total: results.len(),
        results,
    })
}

pub fn vectorize_text(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0_f32; VECTOR_DIMS];
    for token in tokenize(text) {
        let mut hash = 2166136261_u32;
        for byte in token.as_bytes() {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(16777619);
        }
        let index = usize::try_from(hash).unwrap_or(0) % VECTOR_DIMS;
        vector[index] += 1.0;
    }
    normalize(&mut vector);
    vector
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
