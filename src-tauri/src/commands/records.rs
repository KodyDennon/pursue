use crate::commands::{now, to_error, AppState};
use crate::db::records;
use crate::models::{
    BulkDownloadItem, BulkDownloadReport, BulkDownloadStatus, DownloadResult, ManualImportRequest,
    RecordFilter, RecordSummary, SyncReport,
};
use crate::sources::war_gov;
use anyhow::{anyhow, Result};

use serde_json::Value;
use sqlx::{Row, SqlitePool};
use std::path::Path;
use tauri::State;
use uuid::Uuid;

const DVIDS_API_KEY: &str = "key-68bb60d16b35e";

#[tauri::command]
pub async fn sync_official_source_with_csv(
    csv: String,
    upstream_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<SyncReport, String> {
    let upstream_url = upstream_url.as_deref().unwrap_or(war_gov::WAR_GOV_CSV_URL);
    let report = war_gov::sync_official_source_from_bytes_with_url(
        &state.db,
        &state.library,
        csv.as_bytes(),
        upstream_url,
    )
    .await
    .map_err(to_error)?;
    war_gov::repair_official_record_identities(&state.db)
        .await
        .map_err(to_error)?;
    Ok(report)
}

#[tauri::command]
pub async fn repair_official_source_records(state: State<'_, AppState>) -> Result<usize, String> {
    war_gov::repair_official_record_identities(&state.db)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn list_records(
    filter: Option<RecordFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<RecordSummary>, String> {
    records::list(&state.db, filter).await.map_err(to_error)
}

#[tauri::command]
pub async fn get_record(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<RecordSummary>, String> {
    records::find_summary_by_id(&state.db, &id)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn get_record_artifact_path(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let record = records::find_by_id(&state.db, &id)
        .await
        .map_err(to_error)?
        .ok_or_else(|| format!("record not found: {id}"))?;
    let relative_path = record
        .local_path
        .as_deref()
        .ok_or_else(|| "record has no local artifact".to_string())?;
    Ok(state
        .library
        .get_readable_artifact_path(relative_path)
        .await
        .map_err(to_error)?
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub async fn download_record_with_bytes(
    id: String,
    url: String,
    bytes: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<DownloadResult, String> {
    state
        .library
        .ingest_from_bytes(&state.db, &id, &url, &bytes)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn download_missing_records(state: State<'_, AppState>) -> Result<String, String> {
    // Check for existing active job
    let active_job: Option<String> = sqlx::query_scalar(
        "SELECT id FROM download_jobs WHERE status IN ('running', 'queued') ORDER BY updated_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(to_error)?;

    if let Some(id) = active_job {
        return Ok(id);
    }

    let job_id = create_download_job(&state.db).await.map_err(to_error)?;

    // Prepare items but DON'T start a Rust thread.
    // The frontend will drive the download loop.
    let candidates = sqlx::query(
        r#"
        SELECT id, title, document_url, dvids_video_id, local_path
        FROM records
        WHERE source_type = 'official'
        ORDER BY COALESCE(release_date, created_at) DESC, title ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(to_error)?;

    let mut queued = 0_i64;
    let mut skipped = 0_i64;
    for row in &candidates {
        let record_id = row.get::<String, _>("id");
        let title = row.get::<String, _>("title");
        let document_url = row.get::<Option<String>, _>("document_url");
        let dvids_video_id = row.get::<Option<String>, _>("dvids_video_id");
        let local_path = row.get::<Option<String>, _>("local_path");
        let url = downloadable_source_url(document_url.as_deref(), dvids_video_id.as_deref());
        if local_path.is_some() || url.is_none() {
            skipped += 1;
            continue;
        }
        queued += 1;
        sqlx::query(
            r#"
            INSERT INTO download_job_items (id, job_id, record_id, title, url, status, updated_at)
            VALUES (?, ?, ?, ?, ?, 'queued', ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&job_id)
        .bind(record_id)
        .bind(title)
        .bind(url)
        .bind(now())
        .execute(&state.db)
        .await
        .map_err(to_error)?;
    }

    sqlx::query(
        "UPDATE download_jobs SET status = 'running', total = ?, queued = ?, skipped = ?, updated_at = ? WHERE id = ?",
    )
    .bind(i64::try_from(candidates.len()).unwrap_or(0))
    .bind(queued)
    .bind(skipped)
    .bind(now())
    .bind(&job_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    Ok(job_id)
}

#[tauri::command]
pub async fn update_download_item_status(
    item_id: String,
    status: String,
    error: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    sqlx::query("UPDATE download_job_items SET status = ?, error = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(error)
        .bind(now())
        .bind(item_id)
        .execute(&state.db)
        .await
        .map_err(to_error)?;
    Ok(())
}

#[tauri::command]
pub async fn ingest_downloaded_bytes(
    job_id: String,
    item_id: String,
    record_id: String,
    url: String,
    bytes: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<DownloadResult, String> {
    let result = state
        .library
        .ingest_from_bytes(&state.db, &record_id, &url, &bytes)
        .await
        .map_err(to_error)?;

    // Update item as completed
    sqlx::query(
        r#"
        UPDATE download_job_items
        SET status = 'completed', bytes_downloaded = ?, byte_size = ?, artifact_id = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(result.byte_size)
    .bind(result.byte_size)
    .bind(&result.artifact_id)
    .bind(now())
    .bind(&item_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    // Update job counters
    let (completed, failed): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE status = 'completed'), COUNT(*) FILTER (WHERE status = 'failed') FROM download_job_items WHERE job_id = ?"
    )
    .bind(&job_id)
    .fetch_one(&state.db)
    .await
    .map_err(to_error)?;

    sqlx::query("UPDATE download_jobs SET completed = ?, failed = ?, updated_at = ? WHERE id = ?")
        .bind(completed)
        .bind(failed)
        .bind(now())
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(to_error)?;

    // Check if job is finished
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM download_job_items WHERE job_id = ? AND status IN ('queued', 'downloading')"
    )
    .bind(&job_id)
    .fetch_one(&state.db)
    .await
    .map_err(to_error)?;

    if remaining == 0 {
        let final_status = if failed == 0 {
            "completed"
        } else {
            "completed_with_errors"
        };
        let summary = serde_json::json!({
            "completed": completed,
            "failed": failed,
        });
        sqlx::query(
            "UPDATE download_jobs SET status = ?, summary_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(final_status)
        .bind(summary.to_string())
        .bind(now())
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(to_error)?;
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_bulk_download_status(
    id: String,
    state: State<'_, AppState>,
) -> Result<BulkDownloadReport, String> {
    let job = sqlx::query_as::<_, BulkDownloadStatus>("SELECT * FROM download_jobs WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(to_error)?
        .ok_or_else(|| format!("download job not found: {id}"))?;
    let items = sqlx::query_as::<_, BulkDownloadItem>(
        "SELECT * FROM download_job_items WHERE job_id = ? ORDER BY updated_at DESC, title ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)?;
    Ok(BulkDownloadReport { job, items })
}

#[tauri::command]
pub async fn cancel_bulk_download(id: String, state: State<'_, AppState>) -> Result<(), String> {
    sqlx::query("UPDATE download_jobs SET cancel_requested = 1, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(to_error)?;
    Ok(())
}

#[tauri::command]
pub async fn import_manual_file(
    request: ManualImportRequest,
    state: State<'_, AppState>,
) -> Result<RecordSummary, String> {
    let path = Path::new(&request.path);
    let title = request
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::trim)
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .ok_or_else(|| "manual import requires a title or filename".to_string())?;
    let record_id = Uuid::new_v4().to_string();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let stable_key = format!("manual:{record_id}");
    sqlx::query(
        r#"
        INSERT INTO records (
            id, title, file_type, source_type, summary, stable_key, content_hash
        )
        VALUES (?, ?, ?, 'manual', ?, ?, ?)
        "#,
    )
    .bind(&record_id)
    .bind(&title)
    .bind(&extension)
    .bind(&request.notes)
    .bind(&stable_key)
    .bind(&stable_key)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    state
        .library
        .ingest_manual_file(&state.db, &record_id, path)
        .await
        .map_err(to_error)?;

    records::find_summary_by_id(&state.db, &record_id)
        .await
        .map_err(to_error)?
        .ok_or_else(|| "manual record disappeared after import".to_string())
}

#[tauri::command]
pub async fn ingest_web_page(
    url: String,
    state: State<'_, AppState>,
) -> Result<RecordSummary, String> {
    let record_id = Uuid::new_v4().to_string();
    let temp_path = state
        .library
        .app_data_dir()
        .join(format!("web-{}.txt", record_id));

    crate::sources::web::scrape_and_save(&url, &temp_path)
        .await
        .map_err(to_error)?;

    let stable_key = format!("web:{}", record_id);
    sqlx::query(
        r#"
        INSERT INTO records (
            id, title, file_type, source_type, document_url, stable_key, content_hash
        )
        VALUES (?, ?, 'txt', 'manual', ?, ?, ?)
        "#,
    )
    .bind(&record_id)
    .bind(&url)
    .bind(&url)
    .bind(&stable_key)
    .bind(&stable_key)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    state
        .library
        .ingest_manual_file(&state.db, &record_id, &temp_path)
        .await
        .map_err(to_error)?;

    let _ = tokio::fs::remove_file(&temp_path).await;

    records::find_summary_by_id(&state.db, &record_id)
        .await
        .map_err(to_error)?
        .ok_or_else(|| "web record disappeared after import".to_string())
}

pub async fn create_download_job(db: &SqlitePool) -> Result<String> {
    let job_id = Uuid::new_v4().to_string();
    let now = now();
    sqlx::query(
        "INSERT INTO download_jobs (id, status, created_at, updated_at) VALUES (?, 'queued', ?, ?)",
    )
    .bind(&job_id)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;
    Ok(job_id)
}

#[tauri::command]
pub async fn proxy_fetch_url(url: String, state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let client = state.library.client();

    if let Some(asset_id) = url.strip_prefix("dvids://asset/") {
        return fetch_dvids_asset_bytes(client, asset_id)
            .await
            .map_err(to_error);
    }

    if url.contains("war.gov") {
        let _ = client.get("https://www.war.gov/UFO/").send().await;
    }

    let response = client
        .get(&url)
        .header(reqwest::header::REFERER, "https://www.war.gov/UFO/")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .header(reqwest::header::ACCEPT, "*/*")
        .send()
        .await
        .map_err(to_error)?;

    if !response.status().is_success() {
        return Err(format!(
            "proxy fetch failed with status {}: {}",
            response.status(),
            url
        ));
    }

    let bytes = response.bytes().await.map_err(to_error)?;

    Ok(bytes.to_vec())
}

fn downloadable_source_url(document_url: Option<&str>, dvids_video_id: Option<&str>) -> Option<String> {
    document_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_string)
        .or_else(|| {
            dvids_video_id
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(|id| format!("dvids://asset/{id}"))
        })
}

async fn fetch_dvids_asset_bytes(client: &reqwest::Client, asset_id: &str) -> Result<Vec<u8>> {
    let metadata_url = format!(
        "https://api.dvidshub.net/asset?api_key={DVIDS_API_KEY}&id=video:{asset_id}&thumb_width=720"
    );
    let metadata = client
        .get(&metadata_url)
        .header(reqwest::header::REFERER, "https://www.war.gov/UFO/")
        .send()
        .await?;
    if !metadata.status().is_success() {
        return Err(anyhow!(
            "DVIDS metadata fetch failed with status {} for asset {}",
            metadata.status(),
            asset_id
        ));
    }

    let payload = metadata.json::<Value>().await?;
    let asset_url = select_dvids_file_url(&payload)
        .ok_or_else(|| anyhow!("DVIDS asset {} did not include a downloadable media file", asset_id))?;

    let response = client
        .get(&asset_url)
        .header(reqwest::header::REFERER, "https://www.war.gov/UFO/")
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "DVIDS media fetch failed with status {} for asset {}",
            response.status(),
            asset_id
        ));
    }

    Ok(response.bytes().await?.to_vec())
}

fn select_dvids_file_url(payload: &Value) -> Option<String> {
    let mut candidates = Vec::new();
    collect_dvids_file_candidates(payload, &mut candidates);
    candidates
        .into_iter()
        .max_by_key(|candidate| candidate.0)
        .map(|candidate| candidate.1)
}

fn collect_dvids_file_candidates(value: &Value, candidates: &mut Vec<(i64, String)>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_dvids_file_candidates(item, candidates);
            }
        }
        Value::Object(map) => {
            if let Some(url) = ["src", "url", "download_url", "file"]
                .iter()
                .find_map(|key| map.get(*key).and_then(Value::as_str))
                .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
            {
                let media_type = map
                    .get("type")
                    .or_else(|| map.get("mime_type"))
                    .or_else(|| map.get("mime"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let height = map
                    .get("height")
                    .or_else(|| map.get("h"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let score = if media_type.contains("mp4") {
                    20_000
                } else if media_type.contains("video") {
                    10_000
                } else if media_type.contains("audio") {
                    8_000
                } else {
                    1_000
                } + height;
                candidates.push((score, url.to_string()));
            }

            for item in map.values() {
                collect_dvids_file_candidates(item, candidates);
            }
        }
        _ => {}
    }
}
