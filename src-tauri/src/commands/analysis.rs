use crate::commands::{AppState, to_error};
use crate::models::{
    AnalysisReport, RecordForensics, IntelligenceLog, SearchRequest, SearchResults,
};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn index_record(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    state.analysis.index_record(&app_handle, &id).await.map_err(to_error)
}

#[tauri::command]
pub async fn synthesize_intelligence(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    state.analysis.synthesize_intelligence(&app_handle, &id).await.map_err(to_error)
}

#[tauri::command]
pub async fn analyze_record(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    state.analysis.analyze_record(&app_handle, &id).await.map_err(to_error)
}

#[tauri::command]
pub async fn index_all_records(state: State<'_, AppState>, app_handle: AppHandle) -> Result<usize, String> {
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
        let mut completed_count = 0;
        for row in records {
            let id: String = row.get("id");
            let current_idx = completed_count + 1;
            
            let _ = handle.emit("analysis-progress", serde_json::json!({
                "current": current_idx,
                "total": count,
                "status": "processing",
                "record_id": id
            }));
            
            if let Err(e) = analysis.index_record(&handle, &id).await {
                 let _ = handle.emit("analysis-progress", serde_json::json!({
                     "status": "record-failed",
                     "record_id": id,
                     "error": format!("Indexing failed: {}", e)
                 }));
            }
            completed_count += 1;
        }

        let _ = handle.emit("analysis-progress", serde_json::json!({
            "current": count,
            "total": count,
            "status": "completed",
            "record_id": null
        }));
        
        analysis.set_busy(false);
    });

    Ok(count)
}

#[tauri::command]
pub async fn analyze_all_records(state: State<'_, AppState>, app_handle: AppHandle) -> Result<usize, String> {
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
    
    tauri::async_runtime::spawn(async move {
        use sqlx::Row;
        
        let mut completed_count = 0;
        for row in records {
            let id: String = row.get("id");
            let current_idx = completed_count + 1;
            
            // Emit start of record processing
            let _ = handle.emit("analysis-progress", serde_json::json!({
                "current": current_idx,
                "total": count,
                "status": "processing",
                "record_id": id
            }));
            
            // 1. Indexing Phase
            if let Err(e) = analysis.index_record(&handle, &id).await {
                 let _ = handle.emit("analysis-progress", serde_json::json!({
                     "status": "record-failed",
                     "record_id": id,
                     "error": format!("Indexing failed: {}", e)
                 }));
                 completed_count += 1; // Increment so we move to next in progress
                 continue; 
            }

            // 2. Synthesis Phase (Sequential for VRAM)
            match analysis.synthesize_intelligence(&handle, &id).await {
                Ok(_) => {
                    completed_count += 1;
                    let _ = handle.emit("analysis-progress", serde_json::json!({
                        "current": completed_count,
                        "total": count,
                        "status": "analyzing",
                        "record_id": id
                    }));
                },
                Err(e) => {
                    completed_count += 1;
                    let _ = handle.emit("analysis-progress", serde_json::json!({
                        "status": "record-failed",
                        "record_id": id,
                        "error": format!("Synthesis failed: {}", e)
                    }));
                }
            }
        }

        let _ = handle.emit("analysis-progress", serde_json::json!({
            "current": count,
            "total": count,
            "status": "completed",
            "record_id": null
        }));
        
        analysis.set_busy(false);
    });

    Ok(count)
}

#[tauri::command]
pub async fn reprocess_all_records(state: State<'_, AppState>, app_handle: AppHandle) -> Result<usize, String> {
    let pool = state.db.clone();
    
    // Get all records that have local content
    let records = sqlx::query("SELECT id FROM records WHERE local_path IS NOT NULL")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;
        
    for row in &records {
        use sqlx::Row;
        let id: String = row.get("id");
        state.analysis.clear_record_analysis(&id).await.map_err(to_error)?;
    }
    
    // Now trigger the standard analysis loop
    analyze_all_records(state, app_handle).await
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
    sqlx::query_as::<_, RecordForensics>("SELECT * FROM record_forensics WHERE record_id = ? ORDER BY confidence DESC")
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
    sqlx::query_as::<_, IntelligenceLog>("SELECT * FROM intelligence_logs WHERE record_id = ? ORDER BY created_at DESC")
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
