use anyhow::{anyhow, Result};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::analysis::AnalysisManager;
use crate::cases;
use crate::db::records;
use crate::exports;
use crate::library::LibraryManager;
use crate::models::{
    AddRecordToCaseRequest, AnalysisReport, BulkDownloadItem, BulkDownloadReport,
    BulkDownloadStatus, CaseNotesRequest, CaseSummary, CreateCaseRequest, DownloadResult,
    ExportCaseRequest, ExportResult, ManualImportRequest, RecordFilter, RecordSummary,
    SearchRequest, SearchResults, SyncReport,
};
use crate::sources::war_gov;

pub struct AppState {
    pub db: SqlitePool,
    pub library: Arc<LibraryManager>,
}

#[tauri::command]
pub async fn sync_official_source(state: State<'_, AppState>) -> Result<SyncReport, String> {
    war_gov::sync_official_source(&state.db, &state.library)
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
pub async fn download_record(
    id: String,
    state: State<'_, AppState>,
) -> Result<DownloadResult, String> {
    download_one(&state.db, &state.library, &id).await.map_err(to_error)
}

#[tauri::command]
pub async fn download_missing_records(state: State<'_, AppState>) -> Result<String, String> {
    let job_id = create_download_job(&state.db).await.map_err(to_error)?;
    let db = state.db.clone();
    let failure_db = state.db.clone();
    let library = state.library.clone();
    let job_id_for_task = job_id.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_download_job(db, library, &job_id_for_task).await {
            let _ = mark_job_failed(&failure_db, &job_id_for_task, &error.to_string()).await;
        }
    });

    Ok(job_id)
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
        .or_else(|| path.file_stem().map(|name| name.to_string_lossy().into_owned()))
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
pub async fn analyze_record(
    id: String,
    state: State<'_, AppState>,
) -> Result<AnalysisReport, String> {
    let manager = AnalysisManager::new(state.db.clone(), state.library.clone());
    manager.analyze_record(&id).await.map_err(to_error)
}

#[tauri::command]
pub async fn get_analysis_result(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<AnalysisReport>, String> {
    let manager = AnalysisManager::new(state.db.clone(), state.library.clone());
    manager.get_analysis(&id).await.map_err(to_error)
}

#[tauri::command]
pub async fn search(
    request: SearchRequest,
    state: State<'_, AppState>,
) -> Result<SearchResults, String> {
    crate::search::search(&state.db, request)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn list_cases(state: State<'_, AppState>) -> Result<Vec<CaseSummary>, String> {
    cases::list_cases(&state.db).await.map_err(to_error)
}

#[tauri::command]
pub async fn create_case(
    request: CreateCaseRequest,
    state: State<'_, AppState>,
) -> Result<CaseSummary, String> {
    cases::create_case(&state.db, request).await.map_err(to_error)
}

#[tauri::command]
pub async fn update_case_notes(
    request: CaseNotesRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    cases::update_case_notes(&state.db, request)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn add_record_to_case(
    request: AddRecordToCaseRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    cases::add_record_to_case(&state.db, request)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn export_case(
    request: ExportCaseRequest,
    state: State<'_, AppState>,
) -> Result<ExportResult, String> {
    exports::export_case(&state.db, &state.library, request)
        .await
        .map_err(to_error)
}

async fn download_one(
    db: &SqlitePool,
    library: &LibraryManager,
    record_id: &str,
) -> Result<DownloadResult> {
    let record = records::find_by_id(db, record_id)
        .await?
        .ok_or_else(|| anyhow!("record not found: {record_id}"))?;
    let url = record
        .document_url
        .as_deref()
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .ok_or_else(|| anyhow!("record has no downloadable URL"))?;
    library.ingest_from_url(db, record_id, url).await
}

async fn create_download_job(db: &SqlitePool) -> Result<String> {
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

async fn run_download_job(
    db: SqlitePool,
    library: Arc<LibraryManager>,
    job_id: &str,
) -> Result<()> {
    let candidates = sqlx::query(
        r#"
        SELECT id, title, document_url, local_path
        FROM records
        WHERE source_type = 'official'
        ORDER BY COALESCE(release_date, created_at) DESC, title ASC
        "#,
    )
    .fetch_all(&db)
    .await?;

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
        .bind(job_id)
        .bind(record_id)
        .bind(title)
        .bind(url)
        .bind(now())
        .execute(&db)
        .await?;
    }

    sqlx::query(
        "UPDATE download_jobs SET status = 'running', total = ?, queued = ?, skipped = ?, updated_at = ? WHERE id = ?",
    )
    .bind(i64::try_from(candidates.len()).unwrap_or(0))
    .bind(queued)
    .bind(skipped)
    .bind(now())
    .bind(job_id)
    .execute(&db)
    .await?;

    let items = sqlx::query_as::<_, BulkDownloadItem>(
        "SELECT * FROM download_job_items WHERE job_id = ? AND status = 'queued' ORDER BY title ASC",
    )
    .bind(job_id)
    .fetch_all(&db)
    .await?;

    let mut completed = 0_i64;
    let mut failed = 0_i64;
    for item in items {
        if cancel_requested(&db, job_id).await? {
            sqlx::query("UPDATE download_jobs SET status = 'cancelled', updated_at = ? WHERE id = ?")
                .bind(now())
                .bind(job_id)
                .execute(&db)
                .await?;
            return Ok(());
        }

        sqlx::query("UPDATE download_job_items SET status = 'downloading', updated_at = ? WHERE id = ?")
            .bind(now())
            .bind(&item.id)
            .execute(&db)
            .await?;

        match download_one(&db, &library, &item.record_id).await {
            Ok(result) => {
                completed += 1;
                sqlx::query(
                    r#"
                    UPDATE download_job_items
                    SET status = 'completed', bytes_downloaded = ?, byte_size = ?, artifact_id = ?, updated_at = ?
                    WHERE id = ?
                    "#,
                )
                .bind(result.byte_size)
                .bind(result.byte_size)
                .bind(result.artifact_id)
                .bind(now())
                .bind(&item.id)
                .execute(&db)
                .await?;
            }
            Err(error) => {
                failed += 1;
                sqlx::query(
                    "UPDATE download_job_items SET status = 'failed', error = ?, updated_at = ? WHERE id = ?",
                )
                .bind(error.to_string())
                .bind(now())
                .bind(&item.id)
                .execute(&db)
                .await?;
            }
        }

        sqlx::query(
            "UPDATE download_jobs SET completed = ?, failed = ?, updated_at = ? WHERE id = ?",
        )
        .bind(completed)
        .bind(failed)
        .bind(now())
        .bind(job_id)
        .execute(&db)
        .await?;
    }

    let status = if failed == 0 { "completed" } else { "completed_with_errors" };
    let summary = serde_json::json!({
        "completed": completed,
        "failed": failed,
        "skipped": skipped,
        "queued": queued
    });
    sqlx::query(
        "UPDATE download_jobs SET status = ?, summary_json = ?, completed = ?, failed = ?, updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(summary.to_string())
    .bind(completed)
    .bind(failed)
    .bind(now())
    .bind(job_id)
    .execute(&db)
    .await?;
    Ok(())
}

async fn cancel_requested(db: &SqlitePool, job_id: &str) -> Result<bool> {
    let value = sqlx::query_scalar::<_, i64>(
        "SELECT cancel_requested FROM download_jobs WHERE id = ?",
    )
    .bind(job_id)
    .fetch_one(db)
    .await?;
    Ok(value != 0)
}

async fn mark_job_failed(db: &SqlitePool, job_id: &str, error: &str) -> Result<()> {
    let summary = serde_json::json!({ "error": error });
    sqlx::query(
        "UPDATE download_jobs SET status = 'failed', summary_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(summary.to_string())
    .bind(now())
    .bind(job_id)
    .execute(db)
    .await?;
    Ok(())
}

fn to_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
