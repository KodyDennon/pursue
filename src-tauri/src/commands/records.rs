use crate::commands::{now, to_error, AppState};
use crate::db::records;
use crate::downloads::DownloadPartWriter;
use crate::models::{
    AppendDownloadChunkRequest, AppendDownloadChunkResponse, BeginDownloadItemRequest,
    BeginDownloadItemResponse, BulkDownloadItem, BulkDownloadReport, BulkDownloadStatus,
    DownloadJobWindow, DownloadResult, FailDownloadItemRequest, FinalizeDownloadItemRequest,
    ManualImportRequest, RecordFilter, RecordPage, RecordSummary, SyncReport,
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
pub async fn list_records_page(
    filter: Option<RecordFilter>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<RecordPage, String> {
    records::list_page(&state.db, filter, limit.unwrap_or(250), offset.unwrap_or(0))
        .await
        .map_err(to_error)
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
pub async fn queue_record_download(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let row = sqlx::query(
        r#"
        SELECT id, title, document_url, dvids_video_id, local_path
        FROM records
        WHERE id = ?
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(to_error)?
    .ok_or_else(|| format!("record not found: {id}"))?;
    let record_id = row.get::<String, _>("id");
    let title = row.get::<String, _>("title");
    let document_url = row.get::<Option<String>, _>("document_url");
    let dvids_video_id = row.get::<Option<String>, _>("dvids_video_id");
    let local_path = row.get::<Option<String>, _>("local_path");
    if local_path.is_some() {
        return Err("record already has a local artifact".to_string());
    }
    let url = downloadable_source_url(document_url.as_deref(), dvids_video_id.as_deref())
        .ok_or_else(|| "record has no downloadable source URL".to_string())?;
    let job_id = create_download_job(&state.db).await.map_err(to_error)?;
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
    sqlx::query(
        "UPDATE download_jobs SET status = 'running', total = 1, queued = 1, skipped = 0, updated_at = ? WHERE id = ?",
    )
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
pub async fn get_download_job_window(
    id: String,
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<DownloadJobWindow, String> {
    let job = sqlx::query_as::<_, BulkDownloadStatus>("SELECT * FROM download_jobs WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(to_error)?
        .ok_or_else(|| format!("download job not found: {id}"))?;
    let limit = limit.unwrap_or(75).clamp(1, 250);
    let offset = offset.unwrap_or(0).max(0);
    let total_items: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM download_job_items WHERE job_id = ?")
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map_err(to_error)?;
    let items = sqlx::query_as::<_, BulkDownloadItem>(
        r#"
        SELECT * FROM download_job_items
        WHERE job_id = ?
        ORDER BY
          CASE status
            WHEN 'downloading' THEN 0
            WHEN 'failed' THEN 1
            WHEN 'queued' THEN 2
            ELSE 3
          END,
          updated_at DESC,
          title ASC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(&id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)?;

    Ok(DownloadJobWindow {
        job,
        items,
        total_items,
    })
}

#[tauri::command]
pub async fn get_next_download_items(
    job_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<BulkDownloadItem>, String> {
    let limit = limit.unwrap_or(4).clamp(1, 16);
    sqlx::query_as::<_, BulkDownloadItem>(
        r#"
        SELECT * FROM download_job_items
        WHERE job_id = ? AND status IN ('queued', 'failed')
        ORDER BY retry_count ASC, updated_at ASC, title ASC
        LIMIT ?
        "#,
    )
    .bind(job_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)
}

#[tauri::command]
pub async fn begin_download_item(
    request: BeginDownloadItemRequest,
    state: State<'_, AppState>,
) -> Result<BeginDownloadItemResponse, String> {
    let writer = download_part_writer(&state, &request.item_id)
        .await
        .map_err(to_error)?;
    let offset = writer.offset().await.map_err(to_error)?;
    let part_path = writer.path().to_string_lossy().into_owned();
    let source_host = request
        .source_host
        .or_else(|| host_from_url(request.resolved_url.as_deref().unwrap_or(&request.url)));

    sqlx::query(
        r#"
        UPDATE download_job_items
        SET status = 'downloading',
            error = NULL,
            error_class = NULL,
            expected_size = COALESCE(?, expected_size),
            content_type = COALESCE(?, content_type),
            etag = COALESCE(?, etag),
            last_modified = COALESCE(?, last_modified),
            source_host = COALESCE(?, source_host),
            resolved_url = COALESCE(?, resolved_url),
            part_path = ?,
            bytes_downloaded = ?,
            last_progress_at = ?,
            updated_at = ?
        WHERE id = ? AND job_id = ?
        "#,
    )
    .bind(request.expected_size)
    .bind(request.content_type)
    .bind(request.etag)
    .bind(request.last_modified)
    .bind(source_host)
    .bind(request.resolved_url)
    .bind(&part_path)
    .bind(i64::try_from(offset).unwrap_or(i64::MAX))
    .bind(now())
    .bind(now())
    .bind(&request.item_id)
    .bind(&request.job_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    let cancel_requested: i64 =
        sqlx::query_scalar("SELECT cancel_requested FROM download_jobs WHERE id = ?")
            .bind(&request.job_id)
            .fetch_one(&state.db)
            .await
            .map_err(to_error)?;

    Ok(BeginDownloadItemResponse {
        offset,
        part_path,
        cancel_requested: cancel_requested != 0,
    })
}

#[tauri::command]
pub async fn append_download_chunk(
    request: AppendDownloadChunkRequest,
    state: State<'_, AppState>,
) -> Result<AppendDownloadChunkResponse, String> {
    let writer = download_part_writer(&state, &request.item_id)
        .await
        .map_err(to_error)?;
    let next_offset = writer
        .append(request.offset, &request.bytes)
        .await
        .map_err(to_error)?;

    sqlx::query(
        r#"
        UPDATE download_job_items
        SET bytes_downloaded = ?, last_progress_at = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(i64::try_from(next_offset).unwrap_or(i64::MAX))
    .bind(now())
    .bind(now())
    .bind(&request.item_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    Ok(AppendDownloadChunkResponse {
        offset: next_offset,
    })
}

#[tauri::command]
pub async fn finalize_download_item(
    request: FinalizeDownloadItemRequest,
    state: State<'_, AppState>,
) -> Result<DownloadResult, String> {
    let writer = download_part_writer(&state, &request.item_id)
        .await
        .map_err(to_error)?;
    let finalized = writer.finalize().await.map_err(to_error)?;
    if let Some(expected) = request.expected_size {
        if expected >= 0 && finalized.byte_size != expected {
            return Err(format!(
                "download size mismatch: expected {}, got {}",
                expected, finalized.byte_size
            ));
        }
    }

    let source_url = request.resolved_url.as_deref().unwrap_or(&request.url);
    let result = state
        .library
        .ingest_part_file(
            &state.db,
            &request.record_id,
            source_url,
            finalized.path,
            finalized.byte_size,
            finalized.sha256,
            request.content_type,
        )
        .await
        .map_err(to_error)?;

    sqlx::query(
        r#"
        UPDATE download_job_items
        SET status = 'completed',
            bytes_downloaded = ?,
            byte_size = ?,
            artifact_id = ?,
            error = NULL,
            error_class = NULL,
            updated_at = ?
        WHERE id = ? AND job_id = ?
        "#,
    )
    .bind(result.byte_size)
    .bind(result.byte_size)
    .bind(&result.artifact_id)
    .bind(now())
    .bind(&request.item_id)
    .bind(&request.job_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;
    refresh_download_job_counters(&state.db, &request.job_id)
        .await
        .map_err(to_error)?;

    Ok(result)
}

#[tauri::command]
pub async fn fail_download_item(
    request: FailDownloadItemRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let status = if request.error_class == "cancelled" {
        "cancelled"
    } else {
        "failed"
    };
    sqlx::query(
        r#"
        UPDATE download_job_items
        SET status = ?,
            error = ?,
            error_class = ?,
            retry_count = retry_count + ?,
            updated_at = ?
        WHERE id = ? AND job_id = ?
        "#,
    )
    .bind(status)
    .bind(request.error)
    .bind(request.error_class)
    .bind(if request.retryable { 1_i64 } else { 0_i64 })
    .bind(now())
    .bind(&request.item_id)
    .bind(&request.job_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;
    refresh_download_job_counters(&state.db, &request.job_id)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn reset_download_item_part(
    item_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let writer = download_part_writer(&state, &item_id)
        .await
        .map_err(to_error)?;
    if writer.path().exists() {
        tokio::fs::remove_file(writer.path())
            .await
            .map_err(to_error)?;
    }
    sqlx::query(
        r#"
        UPDATE download_job_items
        SET bytes_downloaded = 0, part_path = ?, last_progress_at = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(writer.path().to_string_lossy().into_owned())
    .bind(now())
    .bind(now())
    .bind(item_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;
    Ok(())
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

async fn download_part_writer(
    state: &State<'_, AppState>,
    item_id: &str,
) -> Result<DownloadPartWriter> {
    DownloadPartWriter::new(state.library.app_data_dir().join("download-parts"), item_id).await
}

async fn refresh_download_job_counters(db: &SqlitePool, job_id: &str) -> Result<()> {
    let (completed, failed, cancelled, remaining): (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
          COUNT(*) FILTER (WHERE status = 'completed'),
          COUNT(*) FILTER (WHERE status = 'failed'),
          COUNT(*) FILTER (WHERE status = 'cancelled'),
          COUNT(*) FILTER (WHERE status IN ('queued', 'downloading'))
        FROM download_job_items
        WHERE job_id = ?
        "#,
    )
    .bind(job_id)
    .fetch_one(db)
    .await?;

    let final_status = if remaining == 0 {
        if cancelled > 0 {
            Some("cancelled")
        } else if failed > 0 {
            Some("completed_with_errors")
        } else {
            Some("completed")
        }
    } else {
        None
    };

    let summary = serde_json::json!({
        "completed": completed,
        "failed": failed,
        "cancelled": cancelled,
        "remaining": remaining
    });

    if let Some(status) = final_status {
        sqlx::query(
            "UPDATE download_jobs SET completed = ?, failed = ?, status = ?, summary_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(completed)
        .bind(failed)
        .bind(status)
        .bind(summary.to_string())
        .bind(now())
        .bind(job_id)
        .execute(db)
        .await?;
    } else {
        sqlx::query(
            "UPDATE download_jobs SET completed = ?, failed = ?, summary_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(completed)
        .bind(failed)
        .bind(summary.to_string())
        .bind(now())
        .bind(job_id)
        .execute(db)
        .await?;
    }

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
pub async fn resolve_dvids_metadata(
    video_id: String,
    state: State<'_, AppState>,
    handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    use tauri::Listener;
    use tauri::Manager;
    use tokio::sync::oneshot;

    // Acquire permit to respect concurrency limit (prevents Akamai flagging)
    let _permit = state.dvids_semaphore.acquire().await.map_err(to_error)?;

    let window = handle
        .get_webview_window("dvids-resolver")
        .ok_or_else(|| "DVIDS resolver webview not found".to_string())?;

    let request_id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

    let handler_id = handle.listen(format!("dvids-resolved-{}", request_id), move |event| {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
            if let Ok(mut guard) = tx.lock() {
                if let Some(sender) = guard.take() {
                    let _ = sender.send(payload);
                }
            }
        }
    });

    let script = format!(
        r#"
        (async () => {{
            try {{
                const DVIDS_API_KEY = 'key-68bb60d16b35e';
                const res = await fetch(`https://api.dvidshub.net/asset?api_key=${{DVIDS_API_KEY}}&id=video:{video_id}&thumb_width=720`);
                if (!res.ok) throw new Error(`HTTP ${{res.status}}`);
                const data = await res.json();
                window.__TAURI__.emit('dvids-resolved-{request_id}', data);
            }} catch (e) {{
                window.__TAURI__.emit('dvids-resolved-{request_id}', {{ error: e.toString() }});
            }}
        }})()
        "#
    );

    window
        .eval(&script)
        .map_err(|e| format!("failed to execute script: {e}"))?;

    let result = tokio::time::timeout(std::time::Duration::from_secs(15), rx)
        .await
        .map_err(|_| "DVIDS resolution timed out".to_string())?
        .map_err(|_| "DVIDS resolution channel closed".to_string())?;

    handle.unlisten(handler_id);

    if let Some(error) = result.get("error") {
        return Err(error.as_str().unwrap_or("unknown error").to_string());
    }

    Ok(result)
}

fn downloadable_source_url(
    document_url: Option<&str>,
    dvids_video_id: Option<&str>,
) -> Option<String> {
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

fn host_from_url(raw: &str) -> Option<String> {
    url::Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}
