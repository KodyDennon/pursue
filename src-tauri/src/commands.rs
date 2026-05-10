use anyhow::{anyhow, Result};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::analysis::diagnostics;
use crate::analysis::diagnostics::HardwareSpecs;
use crate::analysis::model_manager::ModelManager;
use crate::analysis::AnalysisManager;
use crate::cases;
use crate::db::records;
use crate::exports;
use crate::library::LibraryManager;
use crate::models::{
    AddRecordToCaseRequest, AnalysisReport, BulkDownloadItem, BulkDownloadReport,
    BulkDownloadStatus, CaseNotesRequest, CaseSummary, CreateCaseRequest, DatabaseStatus,
    DownloadResult, ExportCaseRequest, ExportResult, ManualImportRequest, RecordFilter,
    RecordSummary, SearchRequest, SearchResults, SyncReport,
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
pub async fn get_database_status(state: State<'_, AppState>) -> Result<DatabaseStatus, String> {
    database_status(&state.db, &state.library)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn download_record(
    id: String,
    state: State<'_, AppState>,
) -> Result<DownloadResult, String> {
    download_one(&state.db, &state.library, &id)
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
        .get_full_path(relative_path)
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
    let job_id = create_download_job(&state.db).await.map_err(to_error)?;
    let db = state.db.clone();
    let library = state.library.clone();
    let job_id_for_task = job_id.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_download_job(db.clone(), library, &job_id_for_task).await {
            log::error!("Background download job failed: {}", error);
            let summary = serde_json::json!({ "error": error.to_string() });
            let _ = sqlx::query(
                "UPDATE download_jobs SET status = 'failed', summary_json = ?, updated_at = ? WHERE id = ?",
            )
            .bind(summary.to_string())
            .bind(now())
            .bind(&job_id_for_task)
            .execute(&db)
            .await;
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
    let temp_path = state.library.app_data_dir().join(format!("web-{}.txt", record_id));
    
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

#[tauri::command]
pub async fn analyze_record(
    id: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<AnalysisReport, String> {
    let manager = AnalysisManager::new(state.db.clone(), state.library.clone());
    manager.analyze_record(&app_handle, &id).await.map_err(to_error)
}

#[tauri::command]
pub async fn get_hardware_diagnostics() -> Result<HardwareSpecs, String> {
    Ok(diagnostics::get_hardware_specs())
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
    cases::create_case(&state.db, request)
        .await
        .map_err(to_error)
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

async fn database_status(db: &SqlitePool, library: &LibraryManager) -> Result<DatabaseStatus> {
    let latest_snapshot = sqlx::query(
        r#"
        SELECT fetched_at, upstream_url, record_count
        FROM source_snapshots
        WHERE status = 'completed'
        ORDER BY fetched_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await?;

    Ok(DatabaseStatus {
        app_data_dir: library.app_data_dir().to_string_lossy().into_owned(),
        database_path: library
            .app_data_dir()
            .join("pursue.db")
            .to_string_lossy()
            .into_owned(),
        library_path: library.library_dir().to_string_lossy().into_owned(),
        snapshots_path: library.snapshots_dir().to_string_lossy().into_owned(),
        exports_path: library.exports_dir().to_string_lossy().into_owned(),
        total_records: count_scalar(db, "SELECT COUNT(*) FROM records").await?,
        official_records: count_scalar(
            db,
            "SELECT COUNT(*) FROM records WHERE source_type = 'official'",
        )
        .await?,
        manual_records: count_scalar(
            db,
            "SELECT COUNT(*) FROM records WHERE source_type = 'manual'",
        )
        .await?,
        downloadable_records: count_scalar(
            db,
            "SELECT COUNT(*) FROM records WHERE source_type = 'official' AND document_url IS NOT NULL AND document_url != ''",
        )
        .await?,
        local_records: count_scalar(db, "SELECT COUNT(*) FROM records WHERE local_path IS NOT NULL")
            .await?,
        artifact_count: count_scalar(db, "SELECT COUNT(*) FROM artifacts").await?,
        artifact_bytes: count_scalar(db, "SELECT COALESCE(SUM(byte_size), 0) FROM artifacts")
            .await?,
        analyzed_records: count_scalar(
            db,
            "SELECT COUNT(*) FROM records WHERE analysis_status = 'completed'",
        )
        .await?,
        failed_analysis_records: count_scalar(
            db,
            "SELECT COUNT(*) FROM records WHERE analysis_status = 'failed'",
        )
        .await?,
        analysis_chunks: count_scalar(db, "SELECT COUNT(*) FROM analysis_chunks").await?,
        vector_chunks: count_scalar(db, "SELECT COUNT(*) FROM vec_analysis_chunks").await?,
        entity_count: count_scalar(db, "SELECT COUNT(*) FROM entities").await?,
        case_count: count_scalar(db, "SELECT COUNT(*) FROM cases").await?,
        source_snapshots: count_scalar(db, "SELECT COUNT(*) FROM source_snapshots").await?,
        latest_snapshot_at: latest_snapshot
            .as_ref()
            .map(|row| row.get::<String, _>("fetched_at")),
        latest_snapshot_url: latest_snapshot
            .as_ref()
            .map(|row| row.get::<String, _>("upstream_url")),
        latest_snapshot_records: latest_snapshot
            .as_ref()
            .map(|row| row.get::<i64, _>("record_count")),
        active_download_jobs: count_scalar(
            db,
            "SELECT COUNT(*) FROM download_jobs WHERE status IN ('queued', 'running')",
        )
        .await?,
    })
}

async fn count_scalar(db: &SqlitePool, sql: &str) -> Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>(sql).fetch_one(db).await?)
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
        let cancel_req =
            sqlx::query_scalar::<_, i64>("SELECT cancel_requested FROM download_jobs WHERE id = ?")
                .bind(job_id)
                .fetch_one(&db)
                .await?;
        if cancel_req != 0 {
            sqlx::query(
                "UPDATE download_jobs SET status = 'cancelled', updated_at = ? WHERE id = ?",
            )
            .bind(now())
            .bind(job_id)
            .execute(&db)
            .await?;
            return Ok(());
        }

        sqlx::query(
            "UPDATE download_job_items SET status = 'downloading', updated_at = ? WHERE id = ?",
        )
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

        // Small delay to be polite and avoid WAF/rate-limiting
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let status = if failed == 0 {
        "completed"
    } else {
        "completed_with_errors"
    };
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

#[tauri::command]
pub async fn check_model_status(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, bool>, String> {
    let manager = ModelManager::new(&state.library);
    let mut status = std::collections::HashMap::new();
    
    let model_files = vec![
        ("bge-small", "bge-small-en-v1.5.onnx"),
        ("tokenizer", "tokenizer.json"),
        ("gemma-2b", "gemma-4-e2b.gguf"),
        ("gemma-4b", "gemma-4-e4b.gguf"),
    ];

    for (id, filename) in model_files {
        let path = manager.models_dir().join(filename);
        status.insert(id.to_string(), path.exists());
    }

    Ok(status)
}

#[tauri::command]
pub async fn provision_model(
    id: String,
    url: String,
    name: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let manager = ModelManager::new(&state.library);
    manager
        .ensure_model(&app_handle, &id, &name, &url)
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(to_error)
}

fn to_error(error: impl std::fmt::Display) -> String {
    let msg = error.to_string();
    log::error!("Backend command failed: {}", msg);
    msg
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
