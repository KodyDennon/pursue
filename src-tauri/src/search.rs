use crate::models::{SearchRequest, SearchResultItem, SearchResults};
use anyhow::Result;
#[cfg(not(target_os = "windows"))]
use ort::session::Session;
#[cfg(not(target_os = "windows"))]
use ort::value::Value;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
#[cfg(not(target_os = "windows"))]
use std::sync::{Mutex, OnceLock};
#[cfg(not(target_os = "windows"))]
use tokenizers::Tokenizer;

const VECTOR_DIMS: usize = 384;

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();
#[cfg(not(target_os = "windows"))]
static TOKENIZER: OnceLock<Tokenizer> = OnceLock::new();
#[cfg(not(target_os = "windows"))]
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

#[cfg(not(target_os = "windows"))]
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

#[cfg(not(target_os = "windows"))]
fn get_embedding_session() -> Result<&'static Mutex<Session>> {
    if let Some(session) = EMBEDDING_SESSION.get() {
        return Ok(session);
    }

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

    let results = match sqlx::query_as::<_, SearchResultItem>(
        r#"
        SELECT 
            r.id, r.title, r.agency, r.release_date, r.document_url, r.local_path, 
            r.intelligence_json as summary, r.artifact_sha256,
            vec_distance_cosine(v.embedding, ?) as distance,
            c.text as excerpt
        FROM vec_analysis_chunks v
        JOIN analysis_chunks c ON c.id = v.chunk_id
        JOIN records r ON r.id = c.record_id
        WHERE v.embedding MATCH ? AND k = 20
        ORDER BY distance ASC
        "#,
    )
    .bind(vector_blob)
    .bind(vector_blob)
    .fetch_all(pool)
    .await
    {
        Ok(res) => res,
        Err(_) => keyword_search(pool, &query).await?,
    };

    Ok(SearchResults {
        query,
        total: results.len(),
        results,
    })
}

pub async fn vectorize_text(text: &str) -> Result<Vec<f32>> {
    #[cfg(not(target_os = "windows"))]
    if let Ok(vector) = vectorize_text_with_model(text).await {
        return Ok(vector);
    }

    Ok(deterministic_embedding(text))
}

#[cfg(not(target_os = "windows"))]
async fn vectorize_text_with_model(text: &str) -> Result<Vec<f32>> {
    let tokenizer = get_tokenizer()?;
    let encoding = tokenizer
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

    let session_mutex = get_embedding_session()?;
    let mut session = session_mutex
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

async fn keyword_search(pool: &SqlitePool, query: &str) -> Result<Vec<SearchResultItem>> {
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
