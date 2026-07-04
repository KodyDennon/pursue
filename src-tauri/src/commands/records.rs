use crate::commands::{now, to_error, AppState};
use crate::db::records;
use crate::downloads::DownloadPartWriter;
use crate::library::IngestPartRequest;
use crate::models::{
    AppendDownloadChunkRequest, AppendDownloadChunkResponse, BeginDownloadItemRequest,
    BeginDownloadItemResponse, BulkDownloadItem, BulkDownloadReport, BulkDownloadStatus,
    DownloadJobWindow, DownloadResult, FailDownloadItemRequest, FinalizeDownloadItemRequest,
    ManualImportRequest, RecordFilter, RecordPage, RecordSummary, SyncReport,
    WarGovWebviewDownloadRequest,
};
use crate::sources::war_gov;
use crate::DownloadProgressWrite;
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Deserialize;
use sqlx::{Row, SqlitePool};
use std::path::Path;
use tauri::{Listener, Manager, State};
use tokio::sync::mpsc;
use uuid::Uuid;

const DOWNLOAD_PROGRESS_WRITE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
const DOWNLOAD_PROGRESS_WRITE_BYTE_DELTA: u64 = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum WarGovDownloadEvent {
    #[serde(rename = "headers")]
    Headers {
        status: u16,
        status_text: String,
        expected_size: Option<i64>,
        content_type: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
        reset_part: bool,
    },
    #[serde(rename = "chunk")]
    Chunk { offset: u64, bytes_base64: String },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { error: String },
}

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

    // Automatic healing: Reset failed items to 'queued' so they are retried
    sqlx::query(
        "UPDATE download_job_items SET status = 'queued', error = NULL, error_class = NULL WHERE status = 'failed'"
    )
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    // If a job was finished with errors, move it back to running so the worker picks it up
    sqlx::query(
        "UPDATE download_jobs SET status = 'running' WHERE status = 'completed_with_errors'",
    )
    .execute(&state.db)
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

/// Returns the added/changed/removed diffs from the most recent completed war.gov sync.
/// `SyncReport.diffs` already carries this for the sync that just ran; this command lets the
/// UI re-fetch the same information later (e.g. after a reload) without re-syncing.
#[tauri::command]
pub async fn get_latest_sync_diffs(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::SnapshotDiff>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 500);

    let snapshot_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM source_snapshots WHERE source_name = 'war.gov/UFO' AND status = 'completed' ORDER BY fetched_at DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(to_error)?;

    let Some(snapshot_id) = snapshot_id else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query(
        "SELECT change_type, title, document_url, stable_key FROM source_diffs WHERE snapshot_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&snapshot_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)?;

    Ok(rows
        .into_iter()
        .map(|row| crate::models::SnapshotDiff {
            change_type: row.get("change_type"),
            title: row.get("title"),
            document_url: row.get("document_url"),
            stable_key: row.get("stable_key"),
        })
        .collect())
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
        SELECT id, title, document_url, dvids_video_id, modal_image, local_path
        FROM records
        WHERE source_type = 'official'
        ORDER BY COALESCE(release_date, created_at) DESC, title ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(to_error)?;

    // One transaction for the whole queue-build instead of one commit per INSERT — for a
    // large archive this was previously thousands of individual round trips before the
    // download job even started.
    let mut tx = state.db.begin().await.map_err(to_error)?;
    let mut queued = 0_i64;
    let mut skipped = 0_i64;
    for row in &candidates {
        let record_id = row.get::<String, _>("id");
        let title = row.get::<String, _>("title");
        let document_url = row.get::<Option<String>, _>("document_url");
        let dvids_video_id = row.get::<Option<String>, _>("dvids_video_id");
        let modal_image = row.get::<Option<String>, _>("modal_image");
        let local_path = row.get::<Option<String>, _>("local_path");
        let url = downloadable_source_url(
            document_url.as_deref(),
            dvids_video_id.as_deref(),
            modal_image.as_deref(),
        );
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
        .execute(&mut *tx)
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
    .execute(&mut *tx)
    .await
    .map_err(to_error)?;
    tx.commit().await.map_err(to_error)?;

    Ok(job_id)
}

#[tauri::command]
pub async fn queue_record_download(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let row = sqlx::query(
        r#"
        SELECT id, title, document_url, dvids_video_id, modal_image, local_path
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
    let modal_image = row.get::<Option<String>, _>("modal_image");
    let local_path = row.get::<Option<String>, _>("local_path");
    if local_path.is_some() {
        return Err("record already has a local artifact".to_string());
    }
    let url = downloadable_source_url(
        document_url.as_deref(),
        dvids_video_id.as_deref(),
        modal_image.as_deref(),
    )
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
    remember_download_progress_write(&state, &request.item_id, offset).await;

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
    let bytes = BASE64
        .decode(&request.bytes_base64)
        .map_err(|e| format!("invalid download chunk payload: {e}"))?;
    let next_offset = writer
        .append(request.offset, &bytes)
        .await
        .map_err(to_error)?;

    if should_write_download_progress(&state, &request.item_id, next_offset).await {
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
    }

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
    remove_download_part_writer(&state, &request.item_id).await;
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
            IngestPartRequest {
                record_id: request.record_id.clone(),
                url: source_url.to_string(),
                part_path: finalized.path,
                byte_size: finalized.byte_size,
                sha256: finalized.sha256,
                media_type: request.content_type,
            },
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
pub async fn download_war_gov_item_with_webview(
    request: WarGovWebviewDownloadRequest,
    state: State<'_, AppState>,
    handle: tauri::AppHandle,
) -> Result<DownloadResult, String> {
    // Acquire permit to respect concurrency limit for the hidden webview
    let _permit = state.webview_semaphore.acquire().await.map_err(to_error)?;

    let source_url = request.resolved_url.as_deref().unwrap_or(&request.url);
    ensure_war_gov_url(source_url)?;
    let source_host = host_from_url(source_url);
    let writer = download_part_writer(&state, &request.item_id)
        .await
        .map_err(to_error)?;
    let mut offset = writer.offset().await.map_err(to_error)?;
    let part_path = writer.path().to_string_lossy().into_owned();

    sqlx::query(
        r#"
        UPDATE download_job_items
        SET status = 'downloading',
            error = NULL,
            error_class = NULL,
            content_type = COALESCE(?, content_type),
            source_host = COALESCE(?, source_host),
            resolved_url = COALESCE(?, resolved_url),
            part_path = ?,
            bytes_downloaded = ?,
            last_progress_at = ?,
            updated_at = ?
        WHERE id = ? AND job_id = ?
        "#,
    )
    .bind(&request.content_type)
    .bind(source_host)
    .bind(&request.resolved_url)
    .bind(&part_path)
    .bind(i64::try_from(offset).unwrap_or(i64::MAX))
    .bind(now())
    .bind(now())
    .bind(&request.item_id)
    .bind(&request.job_id)
    .execute(&state.db)
    .await
    .map_err(to_error)?;

    let window = handle
        .get_webview_window("war-gov-resolver")
        .ok_or_else(|| "WAR.gov resolver webview not found".to_string())?;
    let request_id = Uuid::new_v4().to_string();
    let event_name = format!("war-gov-download-{request_id}");
    let (tx, mut rx) = mpsc::unbounded_channel::<WarGovDownloadEvent>();
    let handler_id = handle.listen(event_name.clone(), move |event| {
        if let Ok(payload) = serde_json::from_str::<WarGovDownloadEvent>(event.payload()) {
            let _ = tx.send(payload);
        }
    });

    let script = build_war_gov_download_script(source_url, offset, &request_id);
    if let Err(error) = window.eval(&script) {
        handle.unlisten(handler_id);
        return Err(format!(
            "failed to execute WAR.gov download script: {error}"
        ));
    }

    let download_result = async {
        let mut expected_size: Option<i64> = None;
        let mut content_type = request.content_type.clone();
        // Throttle DB progress writes to at most one per 500ms/1MB; every chunk still hits
        // disk immediately via writer.append().
        let progress_write_interval = std::time::Duration::from_millis(500);
        let progress_write_byte_delta: u64 = 1024 * 1024;
        let mut last_progress_write_at = std::time::Instant::now();
        let mut last_progress_write_offset = offset;

        loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(120), rx.recv())
                .await
                .map_err(|_| "WAR.gov webview download timed out".to_string())?
                .ok_or_else(|| "WAR.gov webview download channel closed".to_string())?;

            match event {
                WarGovDownloadEvent::Headers {
                    status,
                    status_text,
                    expected_size: next_expected_size,
                    content_type: next_content_type,
                    etag,
                    last_modified,
                    reset_part,
                } => {
                    if status >= 400 {
                        return Err(format!("HTTP {status}: {status_text}"));
                    }
                    if reset_part {
                        writer.reset().await.map_err(to_error)?;
                        offset = 0;
                    }
                    expected_size = next_expected_size;
                    if next_content_type.is_some() {
                        content_type = next_content_type.clone();
                    }
                    sqlx::query(
                        r#"
                        UPDATE download_job_items
                        SET expected_size = COALESCE(?, expected_size),
                            content_type = COALESCE(?, content_type),
                            etag = COALESCE(?, etag),
                            last_modified = COALESCE(?, last_modified),
                            bytes_downloaded = ?,
                            last_progress_at = ?,
                            updated_at = ?
                        WHERE id = ? AND job_id = ?
                        "#,
                    )
                    .bind(expected_size)
                    .bind(&content_type)
                    .bind(etag)
                    .bind(last_modified)
                    .bind(i64::try_from(offset).unwrap_or(i64::MAX))
                    .bind(now())
                    .bind(now())
                    .bind(&request.item_id)
                    .bind(&request.job_id)
                    .execute(&state.db)
                    .await
                    .map_err(to_error)?;
                }
                WarGovDownloadEvent::Chunk {
                    offset: chunk_offset,
                    bytes_base64,
                } => {
                    let bytes = BASE64
                        .decode(bytes_base64)
                        .map_err(|e| format!("invalid WAR.gov chunk payload: {e}"))?;
                    offset = writer
                        .append(chunk_offset, &bytes)
                        .await
                        .map_err(to_error)?;

                    let elapsed_since_last_write = last_progress_write_at.elapsed();
                    let bytes_since_last_write = offset.saturating_sub(last_progress_write_offset);
                    if elapsed_since_last_write >= progress_write_interval
                        || bytes_since_last_write >= progress_write_byte_delta
                    {
                        sqlx::query(
                            r#"
                            UPDATE download_job_items
                            SET bytes_downloaded = ?, last_progress_at = ?, updated_at = ?
                            WHERE id = ? AND job_id = ?
                            "#,
                        )
                        .bind(i64::try_from(offset).unwrap_or(i64::MAX))
                        .bind(now())
                        .bind(now())
                        .bind(&request.item_id)
                        .bind(&request.job_id)
                        .execute(&state.db)
                        .await
                        .map_err(to_error)?;
                        last_progress_write_at = std::time::Instant::now();
                        last_progress_write_offset = offset;
                    }

                    // Pull the next chunk only now that this one is durably written — see
                    // build_war_gov_download_script for why this bounds the webview's memory
                    // use instead of letting it read arbitrarily far ahead.
                    if let Err(error) =
                        window.eval(build_war_gov_continue_download_script(&request_id))
                    {
                        return Err(format!(
                            "failed to continue WAR.gov download script: {error}"
                        ));
                    }
                }
                WarGovDownloadEvent::Done => break,
                WarGovDownloadEvent::Error { error } => return Err(error),
            }
        }

        let finalized = writer.finalize().await.map_err(to_error)?;
        remove_download_part_writer(&state, &request.item_id).await;
        if let Some(expected) = expected_size {
            if expected >= 0 && finalized.byte_size != expected {
                return Err(format!(
                    "download size mismatch: expected {}, got {}",
                    expected, finalized.byte_size
                ));
            }
        }
        state
            .library
            .ingest_part_file(
                &state.db,
                IngestPartRequest {
                    record_id: request.record_id.clone(),
                    url: source_url.to_string(),
                    part_path: finalized.path,
                    byte_size: finalized.byte_size,
                    sha256: finalized.sha256,
                    media_type: content_type,
                },
            )
            .await
            .map_err(to_error)
    }
    .await;

    handle.unlisten(handler_id);
    let result = download_result?;

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
    remove_download_part_writer(&state, &request.item_id).await;
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
    writer.reset().await.map_err(to_error)?;
    remember_download_progress_write(&state, &item_id, 0).await;
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
    let mut writers = state.download_writers.lock().await;
    if let Some(writer) = writers.get(item_id) {
        return Ok(writer.clone());
    }

    let writer =
        DownloadPartWriter::new(state.library.app_data_dir().join("download-parts"), item_id)
            .await?;
    writers.insert(item_id.to_string(), writer.clone());
    Ok(writer)
}

async fn remove_download_part_writer(state: &State<'_, AppState>, item_id: &str) {
    let mut writers = state.download_writers.lock().await;
    writers.remove(item_id);
    drop(writers);

    let mut progress = state.download_progress_writes.lock().await;
    progress.remove(item_id);
}

async fn remember_download_progress_write(state: &State<'_, AppState>, item_id: &str, offset: u64) {
    let mut progress = state.download_progress_writes.lock().await;
    progress.insert(
        item_id.to_string(),
        DownloadProgressWrite {
            offset,
            at: std::time::Instant::now(),
        },
    );
}

async fn should_write_download_progress(
    state: &State<'_, AppState>,
    item_id: &str,
    offset: u64,
) -> bool {
    let mut progress = state.download_progress_writes.lock().await;
    let now = std::time::Instant::now();
    let Some(previous) = progress.get_mut(item_id) else {
        progress.insert(
            item_id.to_string(),
            DownloadProgressWrite { offset, at: now },
        );
        return true;
    };

    let should_write = previous.at.elapsed() >= DOWNLOAD_PROGRESS_WRITE_INTERVAL
        || offset.saturating_sub(previous.offset) >= DOWNLOAD_PROGRESS_WRITE_BYTE_DELTA;
    if should_write {
        previous.offset = offset;
        previous.at = now;
    }
    should_write
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
    record_id: Option<String>,
    state: State<'_, AppState>,
    handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    use tokio::sync::oneshot;

    // Own semaphore, separate from webview_semaphore (used by actual file downloads) — a
    // burst of DVIDS metadata lookups shouldn't be able to starve real downloads sharing the
    // same hidden webview, or vice versa.
    let _permit = state.dvids_semaphore.acquire().await.map_err(to_error)?;

    let asset_type = dvids_asset_type_for_record(&state.db, record_id.as_deref()).await;

    let window = handle
        .get_webview_window("war-gov-resolver")
        .ok_or_else(|| "WAR.gov resolver webview not found".to_string())?;

    let mut last_error = "unknown error".to_string();

    for attempt in 1..=3 {
        if attempt > 1 {
            tauri_plugin_log::log::info!(
                "[DVIDS] Retrying resolution for {}: attempt {}",
                video_id,
                attempt
            );
            tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt)).await;
        }

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

        let script = build_dvids_resolver_script(&video_id, &request_id, asset_type);

        if let Err(e) = window.eval(&script) {
            handle.unlisten(handler_id);
            last_error = format!("failed to execute script: {e}");
            continue;
        }

        let result = match tokio::time::timeout(std::time::Duration::from_secs(20), rx).await {
            Ok(Ok(val)) => val,
            Ok(Err(_)) => {
                handle.unlisten(handler_id);
                last_error = "DVIDS resolution channel closed".to_string();
                continue;
            }
            Err(_) => {
                handle.unlisten(handler_id);
                last_error = format!("DVIDS resolution timed out (attempt {})", attempt);
                continue;
            }
        };

        handle.unlisten(handler_id);

        if let Some(error) = result.get("error") {
            last_error = error.as_str().unwrap_or("unknown error").to_string();
            if last_error.contains("429") || last_error.contains("network") {
                continue; // Retry on rate limit or network errors
            }
            return Err(last_error); // Permanent error
        }

        return Ok(result);
    }

    Err(format!(
        "DVIDS resolution failed after 3 attempts: {}",
        last_error
    ))
}

// DVIDS audio records fail to resolve under an "audio:" namespace in practice; "video:" is
// the only confirmed-working namespace for all asset types.
async fn dvids_asset_type_for_record(
    _pool: &SqlitePool,
    _record_id: Option<&str>,
) -> &'static str {
    "video"
}

fn build_dvids_resolver_script(video_id: &str, request_id: &str, asset_type: &str) -> String {
    let video_id = serde_json::to_string(video_id).unwrap_or_else(|_| "\"\"".to_string());
    let event_name = serde_json::to_string(&format!("dvids-resolved-{request_id}"))
        .unwrap_or_else(|_| "\"dvids-resolved-invalid\"".to_string());
    let asset_type = serde_json::to_string(asset_type).unwrap_or_else(|_| "\"video\"".to_string());

    format!(
        r#"
        (async () => {{
            const eventName = {event_name};
            const emitResult = async (payload) => {{
                console.log(`[DVIDS] Emitting result for ${{eventName}}`, payload);
                await window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {{
                    event: eventName,
                    payload
                }});
            }};
            try {{
                const DVIDS_API_KEY = 'key-68bb60d16b35e';
                const videoId = {video_id};
                const assetType = {asset_type};
                console.log(`[DVIDS] Resolving ${{assetType}}: ${{videoId}}`);
                const res = await fetch(`https://api.dvidshub.net/asset?api_key=${{DVIDS_API_KEY}}&id=${{assetType}}:${{encodeURIComponent(videoId)}}&thumb_width=720`);
                if (!res.ok) throw new Error(`HTTP ${{res.status}}: ${{res.statusText}}`);
                const data = await res.json();
                console.log(`[DVIDS] Success: ${{videoId}}`);
                await emitResult(data);
            }} catch (e) {{
                console.error(`[DVIDS] Error: ${{e.message}}`);
                await emitResult({{ error: e && e.stack ? e.stack : String(e) }});
            }}
        }})()
        "#
    )
}

/// The `window` property name used to stash per-download read state (the stream reader and
/// current write offset) between separate `eval()` calls — see `build_war_gov_download_script`.
fn war_gov_download_state_key(request_id: &str) -> String {
    format!("__pursue_dl_{request_id}")
}

fn build_war_gov_download_script(source_url: &str, offset: u64, request_id: &str) -> String {
    let source_url = serde_json::to_string(source_url).unwrap_or_else(|_| "\"\"".to_string());
    let event_name = serde_json::to_string(&format!("war-gov-download-{request_id}"))
        .unwrap_or_else(|_| "\"war-gov-download-invalid\"".to_string());
    let state_key =
        serde_json::to_string(&war_gov_download_state_key(request_id)).unwrap_or_default();

    format!(
        r#"
        (async () => {{
            const eventName = {event_name};
            const sourceUrl = {source_url};
            const startOffset = {offset};
            const stateKey = {state_key};
            const emitResult = async (payload) => {{
                await window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {{
                    event: eventName,
                    payload
                }});
            }};
            const bytesToBase64 = (bytes) => {{
                let binary = '';
                const stride = 0x8000;
                for (let i = 0; i < bytes.length; i += stride) {{
                    binary += String.fromCharCode(...bytes.subarray(i, i + stride));
                }}
                return btoa(binary);
            }};
            const emitChunk = async (bytes, offset) => {{
                const maxChunk = 64 * 1024;
                let writeOffset = offset;
                for (let i = 0; i < bytes.length; i += maxChunk) {{
                    const slice = bytes.subarray(i, i + maxChunk);
                    await emitResult({{
                        kind: 'chunk',
                        offset: writeOffset,
                        bytes_base64: bytesToBase64(slice)
                    }});
                    writeOffset += slice.byteLength;
                }}
                return writeOffset;
            }};
            const expectedSizeFromHeaders = (res, baseOffset) => {{
                const contentRange = res.headers.get('content-range');
                const rangeMatch = contentRange && contentRange.match(/\/(\d+)$/);
                if (rangeMatch) return Number.parseInt(rangeMatch[1], 10);
                const contentLength = res.headers.get('content-length');
                if (!contentLength) return null;
                const parsedLength = Number.parseInt(contentLength, 10);
                if (!Number.isFinite(parsedLength)) return null;
                return res.status === 206 ? parsedLength + baseOffset : parsedLength;
            }};

            // Reads one chunk then returns; Rust pulls the next one only after this chunk is
            // flushed to disk. A self-driving read loop here let the webview read arbitrarily
            // far ahead of disk writes, which is what caused OOM on large downloads.
            window[stateKey + '_readNext'] = async () => {{
                const state = window[stateKey];
                if (!state) return;
                try {{
                    const next = await state.reader.read();
                    if (next.done) {{
                        await emitResult({{ kind: 'done' }});
                        delete window[stateKey];
                        delete window[stateKey + '_readNext'];
                        return;
                    }}
                    state.writeOffset = await emitChunk(next.value, state.writeOffset);
                }} catch (e) {{
                    await emitResult({{ kind: 'error', error: e && e.stack ? e.stack : String(e) }});
                    delete window[stateKey];
                    delete window[stateKey + '_readNext'];
                }}
            }};

            try {{
                const headers = {{}};
                if (startOffset > 0) headers.Range = `bytes=${{startOffset}}-`;
                let res = await fetch(sourceUrl, {{ headers, cache: 'no-store' }});

                let resetPart = startOffset > 0 && res.status === 200;
                let currentStartOffset = startOffset;

                if (res.status === 416) {{
                    // Range not satisfiable - local data exceeds remote size. Force reset.
                    resetPart = true;
                    currentStartOffset = 0;
                    res = await fetch(sourceUrl, {{ cache: 'no-store' }});
                }}

                const baseOffset = resetPart ? 0 : currentStartOffset;
                await emitResult({{
                    kind: 'headers',
                    status: res.status,
                    status_text: res.statusText,
                    expected_size: expectedSizeFromHeaders(res, baseOffset),
                    content_type: res.headers.get('content-type'),
                    etag: res.headers.get('etag'),
                    last_modified: res.headers.get('last-modified'),
                    reset_part: resetPart
                }});
                if (!res.ok && res.status !== 206) {{
                    await emitResult({{ kind: 'error', error: `HTTP ${{res.status}}: ${{res.statusText}}` }});
                    return;
                }}
                if (!res.body) throw new Error('Response body is not streamable');

                window[stateKey] = {{ reader: res.body.getReader(), writeOffset: baseOffset }};
                await window[stateKey + '_readNext']();
            }} catch (e) {{
                await emitResult({{ kind: 'error', error: e && e.stack ? e.stack : String(e) }});
            }}
        }})()
        "#
    )
}

/// Pulls the next chunk for an in-progress download started by `build_war_gov_download_script`.
/// Rust calls this (a fresh, separate `eval()`) only after fully processing the previously
/// emitted chunk, which is what bounds how far the webview's network read can run ahead of
/// Rust's disk-write pace.
fn build_war_gov_continue_download_script(request_id: &str) -> String {
    let state_key =
        serde_json::to_string(&war_gov_download_state_key(request_id)).unwrap_or_default();
    format!(
        r#"
        (async () => {{
            const fn = window[{state_key} + '_readNext'];
            if (fn) await fn();
        }})()
        "#
    )
}

fn downloadable_source_url(
    document_url: Option<&str>,
    dvids_video_id: Option<&str>,
    modal_image: Option<&str>,
) -> Option<String> {
    let document_url = document_url
        .map(str::trim)
        .filter(|url| !url.is_empty());
    let dvids_video_id = dvids_video_id.map(str::trim).filter(|id| !id.is_empty());

    if document_url.is_some() && dvids_video_id.is_some() {
        // `document_with_dvids` records (source_asset_class in war_gov.rs) have both a document
        // and a paired DVIDS video; only the document is queued today, so the video is silently
        // never downloaded. Not fixed here (would need modeling a record as two download
        // candidates) but at least made observable.
        tauri_plugin_log::log::warn!(
            "record has both document_url and dvids_video_id ({:?}); only the document will be queued, the DVIDS asset is skipped",
            dvids_video_id
        );
    }

    document_url
        .map(str::to_string)
        .or_else(|| dvids_video_id.map(|id| format!("dvids://asset/{id}")))
        .or_else(|| {
            // Thumbnail-only fallback for image records whose "PDF | Image Link" column is
            // empty but "Modal Image" is populated. Never preferred over a real document_url.
            modal_image
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_string)
        })
}

fn host_from_url(raw: &str) -> Option<String> {
    url::Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}

fn ensure_war_gov_url(raw: &str) -> Result<(), String> {
    let url = url::Url::parse(raw).map_err(|e| format!("invalid WAR.gov URL: {e}"))?;
    match url.host_str().map(str::to_ascii_lowercase).as_deref() {
        Some("war.gov") | Some("www.war.gov") => Ok(()),
        _ => Err(format!(
            "WAR.gov webview downloader rejected non-WAR.gov URL: {raw}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_dvids_resolver_script, build_war_gov_continue_download_script,
        build_war_gov_download_script, downloadable_source_url, dvids_asset_type_for_record,
    };

    #[tokio::test]
    async fn dvids_asset_type_always_resolves_to_video_pending_real_api_verification() {
        // Regression test: this previously returned "audio" for dvids_audio records, based on
        // an unverified assumption about DVIDS's namespace scheme that broke real audio
        // downloads (Apollo/Gemini mission audio) in production. Locks in the revert to
        // always-"video" until someone confirms the real DVIDS audio namespace.
        let pool = crate::db::test_pool().await.expect("test pool");
        sqlx::query(
            "INSERT INTO records (id, title, source_type, source_asset_class) VALUES ('rec-audio', 'Audio Record', 'official', 'dvids_audio')",
        )
        .execute(&pool)
        .await
        .expect("seed audio record");

        assert_eq!(
            dvids_asset_type_for_record(&pool, Some("rec-audio")).await,
            "video"
        );
        assert_eq!(dvids_asset_type_for_record(&pool, None).await, "video");
        assert_eq!(
            dvids_asset_type_for_record(&pool, Some("does-not-exist")).await,
            "video"
        );
    }

    #[test]
    fn dvids_resolver_script_uses_internal_event_ipc_instead_of_global_tauri_api() {
        let script = build_dvids_resolver_script("1006087", "request-1", "video");

        assert!(script.contains("window.__TAURI_INTERNALS__.invoke('plugin:event|emit'"));
        assert!(script.contains("\"dvids-resolved-request-1\""));
        assert!(script.contains("\"1006087\""));
        assert!(!script.contains("window.__TAURI__.emit"));
    }

    #[test]
    fn dvids_resolver_script_json_escapes_ids_before_injecting_javascript() {
        let script =
            build_dvids_resolver_script("100\";throw new Error('x')//", "request-2", "video");

        assert!(script.contains(r#""100\";throw new Error('x')//""#));
        assert!(!script.contains("const videoId = 100\";throw"));
    }

    #[test]
    fn dvids_resolver_script_requests_the_audio_namespace_for_audio_assets() {
        let video_script = build_dvids_resolver_script("1006087", "request-3", "video");
        let audio_script = build_dvids_resolver_script("1006087", "request-4", "audio");

        assert!(video_script.contains("id=${assetType}:") && video_script.contains("\"video\""));
        assert!(audio_script.contains("id=${assetType}:") && audio_script.contains("\"audio\""));
    }

    #[test]
    fn dvids_asset_url_is_used_when_record_has_no_document_url() {
        assert_eq!(
            downloadable_source_url(None, Some("1006087"), None).as_deref(),
            Some("dvids://asset/1006087")
        );
    }

    #[test]
    fn modal_image_is_a_last_resort_fallback_never_preferred_over_a_real_document() {
        assert_eq!(
            downloadable_source_url(None, None, Some("https://www.war.gov/thumb.jpg")).as_deref(),
            Some("https://www.war.gov/thumb.jpg")
        );
        assert_eq!(
            downloadable_source_url(
                Some("https://www.war.gov/doc.pdf"),
                None,
                Some("https://www.war.gov/thumb.jpg")
            )
            .as_deref(),
            Some("https://www.war.gov/doc.pdf")
        );
        assert_eq!(downloadable_source_url(None, None, None), None);
    }

    #[test]
    fn war_gov_download_script_uses_the_discovered_url_without_hardcoded_release_assets() {
        let discovered = "https://www.war.gov/medialink/ufo/dynamic-release/dynamic-file.pdf";
        let script = build_war_gov_download_script(discovered, 1024, "download-1");

        assert!(script.contains("window.__TAURI_INTERNALS__.invoke('plugin:event|emit'"));
        assert!(script.contains("\"war-gov-download-download-1\""));
        assert!(script
            .contains("\"https://www.war.gov/medialink/ufo/dynamic-release/dynamic-file.pdf\""));
        assert!(script.contains("Range"));
        assert!(!script.contains("052226"));
        assert!(!script.contains("release_02"));
        assert!(!script.contains("DOW-UAP-D017"));
    }

    #[test]
    fn war_gov_download_script_reads_only_one_chunk_before_waiting_to_be_pulled_again() {
        // Regression test for the "RangeError: Out of memory" seen in production on large
        // video downloads: the script must NOT contain a self-driving `while (true)` read
        // loop — it should read exactly one chunk via readNext() and then return, relying on
        // build_war_gov_continue_download_script to be eval()'d again for the next one.
        let script = build_war_gov_download_script("https://www.war.gov/file.mp4", 0, "dl-2");

        assert!(!script.contains("while (true)"), "must not self-loop over reads");
        assert!(script.contains("_readNext"));
        assert!(script.contains("state.reader.read()"));
        // Only one `.read()` call in the whole script — confirms the loop was really removed,
        // not just renamed.
        assert_eq!(script.matches(".read()").count(), 1);
    }

    #[test]
    fn war_gov_continue_download_script_targets_the_same_state_key_as_the_initial_script() {
        let initial = build_war_gov_download_script("https://www.war.gov/file.mp4", 0, "dl-3");
        let continue_script = build_war_gov_continue_download_script("dl-3");

        assert!(initial.contains("__pursue_dl_dl-3"));
        assert!(continue_script.contains("__pursue_dl_dl-3"));
        assert!(continue_script.contains("_readNext"));
    }
}
