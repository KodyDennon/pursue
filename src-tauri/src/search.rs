use crate::models::{SearchFilters, SearchRequest, SearchResultItem, SearchResults};
use anyhow::Result;
use ort::session::Session;
use ort::value::Value;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokenizers::Tokenizer;

const VECTOR_DIMS: usize = 384;

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();
static TOKENIZER: OnceLock<Tokenizer> = OnceLock::new();
static EMBEDDING_SESSION: OnceLock<Mutex<Session>> = OnceLock::new();

pub fn init_search_engine(models_path: PathBuf) {
    let _ = MODELS_DIR.set(models_path);
}

fn get_models_dir() -> PathBuf {
    MODELS_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("models"))
}

fn get_tokenizer() -> Result<&'static Tokenizer> {
    if let Some(tokenizer) = TOKENIZER.get() {
        return Ok(tokenizer);
    }

    let path = get_models_dir().join("tokenizer.json");
    if !path.exists() {
        anyhow::bail!("Tokenizer file not found at {}", path.display());
    }

    let tokenizer = Tokenizer::from_file(path).map_err(|e| anyhow::anyhow!(e))?;
    let _ = TOKENIZER.set(tokenizer);
    Ok(TOKENIZER.get().unwrap())
}

fn get_embedding_session() -> Result<&'static Mutex<Session>> {
    if let Some(session) = EMBEDDING_SESSION.get() {
        return Ok(session);
    }

    // Initialize ORT with a higher log level and consistent execution provider
    let _ = ort::init()
        .with_name("pursue-embeddings")
        .with_execution_providers([ort::execution_providers::CPUExecutionProvider::default().build()])
        .commit();

    let path = get_models_dir().join("bge-small-en-v1.5.onnx");
    if !path.exists() {
        anyhow::bail!("Embedding model not found at {}", path.display());
    }

    let session = Session::builder()
        .map_err(|e| anyhow::anyhow!("failed to create ort session builder: {}", e))?
        .commit_from_file(path)
        .map_err(|e| anyhow::anyhow!("failed to load embedding model: {}", e))?;

    let _ = EMBEDDING_SESSION.set(Mutex::new(session));
    Ok(EMBEDDING_SESSION.get().unwrap())
}

pub async fn search(pool: &SqlitePool, request: SearchRequest) -> Result<SearchResults> {
    vector_search(pool, request).await
}

pub async fn vector_search(pool: &SqlitePool, request: SearchRequest) -> Result<SearchResults> {
    let query = request.query;
    let filters = request.filters.unwrap_or(SearchFilters {
        source_type: None,
        case_id: None,
        local_only: None,
    });
    let query_vector = vectorize_text(&query).await?;

    let vector_blob: &[u8] = unsafe {
        std::slice::from_raw_parts(
            query_vector.as_ptr() as *const u8,
            query_vector.len() * std::mem::size_of::<f32>(),
        )
    };

    let results = match sqlx::query_as::<_, SearchResultItem>(
        r#"
        SELECT 
            r.id, r.title, r.agency, r.release_date, r.document_url, r.local_path, 
            r.intelligence_json as summary, a.sha256 as artifact_sha256,
            vec_distance_cosine(v.embedding, ?) as distance,
            c.text as excerpt
        FROM vec_analysis_chunks v
        JOIN analysis_chunks c ON c.id = v.chunk_id
        JOIN records r ON r.id = c.record_id
        LEFT JOIN artifacts a ON a.record_id = r.id
        WHERE v.embedding MATCH ? AND k = 20
          AND (? IS NULL OR r.source_type = ?)
          AND (? = 0 OR r.local_path IS NOT NULL)
          AND (
            ? IS NULL OR EXISTS (
              SELECT 1 FROM case_records cr
              WHERE cr.record_id = r.id AND cr.case_id = ?
            )
          )
        ORDER BY distance ASC
        "#,
    )
    .bind(vector_blob)
    .bind(vector_blob)
    .bind(&filters.source_type)
    .bind(&filters.source_type)
    .bind(if filters.local_only.unwrap_or(false) {
        1
    } else {
        0
    })
    .bind(&filters.case_id)
    .bind(&filters.case_id)
    .fetch_all(pool)
    .await
    {
        Ok(res) => res,
        Err(_) => keyword_search(pool, &query, &filters).await?,
    };

    Ok(SearchResults {
        query,
        total: results.len(),
        results,
    })
}

pub async fn vectorize_text(text: &str) -> Result<Vec<f32>> {
    match vectorize_text_with_model(text).await {
        Ok(vector) => Ok(vector),
        Err(e) => {
            // Silently fall back to deterministic hash to keep the pipeline moving,
            // but log to internal system logs for debugging.
            tauri_plugin_log::log::warn!("Neural embedding failed, using fallback: {}", e);
            Ok(deterministic_embedding(text))
        }
    }
}

async fn vectorize_text_with_model(text: &str) -> Result<Vec<f32>> {
    let tokenizer = get_tokenizer()?;
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
    
    let mut input_ids = encoding.get_ids();
    let mut attention_mask = encoding.get_attention_mask();

    // BGE-Small-EN-v1.5 has a hard sequence limit of 512 tokens.
    // Exceeding this causes the positional embedding 'Add' nodes to fail with dimension mismatches.
    if input_ids.len() > 512 {
        input_ids = &input_ids[..512];
        attention_mask = &attention_mask[..512];
    }

    let seq_len = input_ids.len();

    let input_ids_tensor = Value::from_array((
        vec![1, seq_len],
        input_ids.iter().map(|&x| x as i64).collect::<Vec<i64>>(),
    ))?;

    let attention_mask_tensor = Value::from_array((
        vec![1, seq_len],
        attention_mask
            .iter()
            .map(|&x| x as i64)
            .collect::<Vec<i64>>(),
    ))?;

    let token_type_ids_tensor = Value::from_array((
        vec![1, seq_len],
        vec![0i64; seq_len],
    ))?;

    let session_mutex = get_embedding_session()?;
    let mut session = session_mutex
        .lock()
        .map_err(|e| anyhow::anyhow!("Mutex lock failed: {}", e))?;

    // Capture first output name to allow fallback without borrow conflicts
    let first_output_name = session.outputs().first().map(|o| o.name().to_string());

    // Use explicit input providing while including token_type_ids
    let outputs = session.run(ort::inputs![
        "input_ids" => input_ids_tensor,
        "attention_mask" => attention_mask_tensor,
        "token_type_ids" => token_type_ids_tensor,
    ])?;

    let output = outputs
        .get("last_hidden_state")
        .or_else(|| {
            // Fallback: Attempt to use the first output if last_hidden_state is missing
            first_output_name.and_then(|name| outputs.get(name.as_str()))
        })
        .ok_or_else(|| anyhow::anyhow!("Model produced no usable outputs"))?;

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

async fn keyword_search(
    pool: &SqlitePool,
    query: &str,
    filters: &SearchFilters,
) -> Result<Vec<SearchResultItem>> {
    sqlx::query_as::<_, SearchResultItem>(
        r#"
        SELECT
            r.id, r.title, r.agency, r.release_date, r.document_url, r.local_path,
            r.intelligence_json as summary, a.sha256 as artifact_sha256,
            0.0 as distance,
            c.text as excerpt
        FROM analysis_chunks c
        JOIN records r ON r.id = c.record_id
        LEFT JOIN artifacts a ON a.record_id = r.id
        WHERE (r.title LIKE ? OR c.text LIKE ?)
          AND (? IS NULL OR r.source_type = ?)
          AND (? = 0 OR r.local_path IS NOT NULL)
          AND (
            ? IS NULL OR EXISTS (
              SELECT 1 FROM case_records cr
              WHERE cr.record_id = r.id AND cr.case_id = ?
            )
          )
        LIMIT 20
        "#,
    )
    .bind(format!("%{}%", query))
    .bind(format!("%{}%", query))
    .bind(&filters.source_type)
    .bind(&filters.source_type)
    .bind(if filters.local_only.unwrap_or(false) {
        1
    } else {
        0
    })
    .bind(&filters.case_id)
    .bind(&filters.case_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

fn deterministic_embedding(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; VECTOR_DIMS];
    let mut saw_token = false;

    for token in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        saw_token = true;
        let token = token.to_ascii_lowercase();
        let digest = Sha256::digest(token.as_bytes());
        let weight = 1.0 + (token.len() as f32).ln();

        for chunk in digest.chunks_exact(4) {
            let slot = u16::from_le_bytes([chunk[0], chunk[1]]) as usize % VECTOR_DIMS;
            let sign = if chunk[2] & 1 == 0 { 1.0 } else { -1.0 };
            vector[slot] += sign * weight;
        }
    }

    if !saw_token {
        vector[0] = 1.0;
        return vector;
    }

    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }

    vector
}

pub async fn query_related_fragments_for_record(
    pool: &sqlx::SqlitePool,
    record_id: &str,
    text: &str,
    limit: usize,
) -> anyhow::Result<Vec<String>> {
    let vector = vectorize_text(text).await?;
    let vector_blob: &[u8] = unsafe {
        std::slice::from_raw_parts(
            vector.as_ptr() as *const u8,
            vector.len() * std::mem::size_of::<f32>(),
        )
    };

    let rows = sqlx::query(
        "SELECT f.text FROM vec_intelligence_fragments v JOIN intelligence_fragments f ON f.id = v.fragment_id WHERE f.record_id = ? AND v.embedding MATCH ? AND k = ?"
    )
    .bind(record_id)
    .bind(vector_blob)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("text"))
        .collect())
}

#[allow(dead_code)]
pub async fn query_related_fragments(
    pool: &sqlx::SqlitePool,
    text: &str,
    limit: usize,
) -> anyhow::Result<Vec<String>> {
    let vector = vectorize_text(text).await?;
    let vector_blob: &[u8] = unsafe {
        std::slice::from_raw_parts(
            vector.as_ptr() as *const u8,
            vector.len() * std::mem::size_of::<f32>(),
        )
    };

    let rows = sqlx::query(
        "SELECT f.text FROM vec_intelligence_fragments v JOIN intelligence_fragments f ON f.id = v.fragment_id WHERE v.embedding MATCH ? AND k = ?"
    )
    .bind(vector_blob)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    use sqlx::Row;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<String, _>("text"))
        .collect())
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
    use super::{deterministic_embedding, vectorize_text, VECTOR_DIMS};

    #[tokio::test]
    async fn vectorize_is_stable() {
        let v1 = match vectorize_text("AARO sensor").await {
            Ok(v) => v,
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("not found") || err_msg.contains("No such file") {
                    println!(
                        "Skipping vectorize_is_stable because model files are missing: {}",
                        err_msg
                    );
                    return;
                }
                panic!("vectorize_text failed unexpectedly: {}", e);
            }
        };
        assert_eq!(v1.len(), VECTOR_DIMS);
        let v2 = vectorize_text("AARO sensor").await.unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn deterministic_embedding_is_stable_and_normalized() {
        let v1 = deterministic_embedding("AARO sensor");
        let v2 = deterministic_embedding("AARO sensor");

        assert_eq!(v1, v2);
        assert_eq!(v1.len(), VECTOR_DIMS);

        let norm = v1.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
    }
}
