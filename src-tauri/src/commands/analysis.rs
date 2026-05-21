use crate::commands::{to_error, AppState};
use crate::models::{
    AnalysisReport, IntelligenceLog, RecordForensics, SearchRequest, SearchResults,
};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn index_record(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    let _ = app_handle.emit(
        "analysis-progress",
        serde_json::json!({
            "current": 1,
            "total": 1,
            "status": "extracting-foundation",
            "record_id": id
        }),
    );
    state
        .analysis
        .index_record(&app_handle, &id, 1, 1)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn synthesize_intelligence(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    let _ = app_handle.emit(
        "analysis-progress",
        serde_json::json!({
            "current": 1,
            "total": 1,
            "status": "synthesizing-start",
            "record_id": id
        }),
    );
    state
        .analysis
        .synthesize_intelligence(&app_handle, &id)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn analyze_record(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    // Phase 1: Foundation
    let _ = app_handle.emit(
        "analysis-progress",
        serde_json::json!({
            "current": 1,
            "total": 1,
            "status": "extracting-foundation",
            "record_id": id
        }),
    );
    state
        .analysis
        .index_record(&app_handle, &id, 1, 1)
        .await
        .map_err(to_error)?;

    // Phase 2: Synthesis
    let _ = app_handle.emit(
        "analysis-progress",
        serde_json::json!({
            "current": 1,
            "total": 1,
            "status": "synthesizing-start",
            "record_id": id
        }),
    );
    state
        .analysis
        .synthesize_intelligence(&app_handle, &id)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn index_all_records(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    crate::analysis::batch_processor::BatchProcessor::index_all_records(
        state.analysis.clone(),
        state.db.clone(),
        app_handle,
    )
    .await
}

#[tauri::command]
pub async fn analyze_all_records(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    crate::analysis::batch_processor::BatchProcessor::analyze_all_records(
        state.analysis.clone(),
        state.db.clone(),
        app_handle,
    )
    .await
}

#[tauri::command]
pub async fn reprocess_all_records(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    crate::analysis::batch_processor::BatchProcessor::reprocess_all_records(
        state.analysis.clone(),
        state.db.clone(),
        app_handle,
    )
    .await
}

#[tauri::command]
pub async fn synthesize_all_records(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    crate::analysis::batch_processor::BatchProcessor::synthesize_all_records(
        state.analysis.clone(),
        state.db.clone(),
        app_handle,
    )
    .await
}

#[tauri::command]
pub async fn get_record_chunks(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::AnalysisChunk>, String> {
    sqlx::query_as::<_, crate::models::AnalysisChunk>(
        "SELECT * FROM analysis_chunks WHERE record_id = ? ORDER BY chunk_index ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)
}

#[tauri::command]
pub async fn get_analysis_result(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<AnalysisReport>, String> {
    state.analysis.get_analysis(&id).await.map_err(to_error)
}

#[tauri::command]
pub async fn get_forensic_report(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<RecordForensics>, String> {
    sqlx::query_as::<_, RecordForensics>(
        "SELECT * FROM record_forensics WHERE record_id = ? ORDER BY confidence DESC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)
}

#[tauri::command]
pub async fn get_intelligence_logs(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<IntelligenceLog>, String> {
    sqlx::query_as::<_, IntelligenceLog>(
        "SELECT * FROM intelligence_logs WHERE record_id = ? ORDER BY created_at DESC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(to_error)
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
pub async fn check_neural_runtime_status(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<bool, String> {
    Ok(state.analysis.vision.is_provisioned(&app_handle).await)
}

#[tauri::command]
pub async fn provision_neural_runtime(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    state
        .analysis
        .vision
        .provision(&app_handle)
        .await
        .map_err(to_error)
}

#[tauri::command]
pub async fn get_model_registry() -> Vec<crate::analysis::registry::ModelDefinition> {
    crate::analysis::registry::get_model_registry()
}
