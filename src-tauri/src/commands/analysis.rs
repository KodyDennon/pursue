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
        .index_record(&app_handle, &id, false, 1, 1)
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
        .index_record(&app_handle, &id, false, 1, 1)
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
    let pool = state.db.clone();
    let records = sqlx::query("SELECT id FROM records WHERE (analysis_status IS NULL OR analysis_status != 'indexed') AND local_path IS NOT NULL")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

    let count = records.len();
    if count == 0 {
        return Ok(0);
    }

    if state.analysis.is_busy() {
        return Err("Analysis already in progress".to_string());
    }

    state.analysis.set_busy(true);
    let handle = app_handle.clone();
    let analysis = state.analysis.clone();

    tauri::async_runtime::spawn(async move {
        use sqlx::Row;
        for (idx, row) in records.into_iter().enumerate() {
            let id: String = row.get("id");
            let current_idx = idx + 1;

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": current_idx,
                    "total": count,
                    "status": "extracting-foundation",
                    "record_id": id
                }),
            );

            if let Err(e) = analysis
                .index_record(&handle, &id, false, current_idx, count)
                .await
            {
                let _ = handle.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "record-failed",
                        "record_id": id,
                        "current": current_idx,
                        "total": count,
                        "error": format!("Indexing failed: {}", e)
                    }),
                );
            }
        }

        let _ = handle.emit(
            "analysis-progress",
            serde_json::json!({
                "current": count,
                "total": count,
                "status": "completed",
                "record_id": null
            }),
        );

        analysis.set_busy(false);
    });

    Ok(count)
}

#[tauri::command]
pub async fn analyze_all_records(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    // WARMUP PHASE: Notify UI that we are querying the database
    let _ = app_handle.emit(
        "analysis-progress",
        serde_json::json!({
            "status": "initializing-batch",
            "msg": "Calculating foundation targets..."
        }),
    );

    let pool = state.db.clone();
    let records = sqlx::query("SELECT id FROM records WHERE (analysis_status IS NULL OR analysis_status != 'completed') AND local_path IS NOT NULL")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

    if state.analysis.is_busy() {
        return Err("Analysis already in progress".to_string());
    }

    let count = records.len();
    if count == 0 {
        return Ok(0);
    }

    state.analysis.set_busy(true);

    let handle = app_handle.clone();
    let analysis = state.analysis.clone();

    // DECOUPLING STRATEGY: Batch processing ONLY performs the Foundation phase (OCR/Vector).
    // Deep Intelligence (Gemma) is resource-intensive and is reserved for single-record analysis.
    tauri::async_runtime::spawn(async move {
        use sqlx::Row;

        for (idx, row) in records.into_iter().enumerate() {
            let id: String = row.get("id");
            let current_idx = idx + 1;

            // Emit foundation start
            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": current_idx,
                    "total": count,
                    "status": "extracting-foundation",
                    "record_id": id
                }),
            );

            // 1. Foundation Phase (OCR / Vectorization)
            if let Err(e) = analysis
                .index_record(&handle, &id, false, current_idx, count)
                .await
            {
                let _ = handle.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "record-failed",
                        "record_id": id,
                        "current": current_idx,
                        "total": count,
                        "error": format!("Foundation failed: {}", e)
                    }),
                );
                continue;
            }

            // Success for this record
            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": current_idx,
                    "total": count,
                    "status": "record-completed",
                    "record_id": id
                }),
            );
        }

        let _ = handle.emit(
            "analysis-progress",
            serde_json::json!({
                "current": count,
                "total": count,
                "status": "completed",
                "record_id": null
            }),
        );

        analysis.set_busy(false);
    });

    Ok(count)
}

#[tauri::command]
pub async fn reprocess_all_records(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    // WARMUP PHASE
    let _ = app_handle.emit(
        "analysis-progress",
        serde_json::json!({
            "status": "initializing-batch",
            "msg": "Purging foundation cache..."
        }),
    );

    let pool = state.db.clone();

    // Get all records that have local content
    let records = sqlx::query("SELECT id FROM records WHERE local_path IS NOT NULL")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

    if state.analysis.is_busy() {
        return Err("Analysis already in progress".to_string());
    }

    let count = records.len();
    if count == 0 {
        return Ok(0);
    }

    // Clear previous analysis first
    for row in &records {
        use sqlx::Row;
        let id: String = row.get("id");
        state
            .analysis
            .clear_record_analysis(&id)
            .await
            .map_err(to_error)?;
    }

    state.analysis.set_busy(true);
    let handle = app_handle.clone();
    let analysis = state.analysis.clone();

    // TRIGGER FORCED OCR LOOP
    tauri::async_runtime::spawn(async move {
        use sqlx::Row;

        for (idx, row) in records.into_iter().enumerate() {
            let id: String = row.get("id");
            let current_idx = idx + 1;

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": current_idx,
                    "total": count,
                    "status": "extracting-foundation",
                    "record_id": id
                }),
            );

            // FORCE PIXEL OCR: force_ocr parameter set to true
            if let Err(e) = analysis
                .index_record(&handle, &id, true, current_idx, count)
                .await
            {
                let _ = handle.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "status": "record-failed",
                        "record_id": id,
                        "current": current_idx,
                        "total": count,
                        "error": format!("Forced OCR failed: {}", e)
                    }),
                );
                continue;
            }

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": current_idx,
                    "total": count,
                    "status": "record-completed",
                    "record_id": id
                }),
            );
        }

        let _ = handle.emit(
            "analysis-progress",
            serde_json::json!({
                "current": count,
                "total": count,
                "status": "completed",
                "record_id": null
            }),
        );

        analysis.set_busy(false);
    });

    Ok(count)
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
pub async fn get_model_registry() -> Vec<crate::analysis::registry::ModelDefinition> {
    crate::analysis::registry::get_model_registry()
}
