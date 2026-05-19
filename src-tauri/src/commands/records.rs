use crate::commands::{now, to_error, AppState};
use crate::db::records;
use crate::models::{
    BulkDownloadItem, BulkDownloadReport, BulkDownloadStatus, DownloadResult, ManualImportRequest,
    RecordFilter, RecordSummary, SyncReport,
};
use crate::sources::war_gov;
use anyhow::Result;

use sqlx::{Row, SqlitePool};
use std::path::Path;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn sync_official_source_with_csv(
    csv: String,
    state: State<'_, AppState>,
) -> Result<SyncReport, String> {
    war_gov::sync_official_source_from_bytes(&state.db, &state.library, csv.as_bytes())
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
        SELECT id, title, document_url, local_path
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
        let url = row.get::<Option<String>, _>("document_url");
        let local_path = row.get::<Option<String>, _>("local_path");
        if local_path.is_some() || url.as_deref().unwrap_or("").is_empty() {
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
    sqlx::query(
        "UPDATE download_job_items SET status = ?, error = ?, updated_at = ? WHERE id = ?",
    )
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

    sqlx::query(
        "UPDATE download_jobs SET completed = ?, failed = ?, updated_at = ? WHERE id = ?",
    )
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
        let final_status = if failed == 0 { "completed" } else { "completed_with_errors" };
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
