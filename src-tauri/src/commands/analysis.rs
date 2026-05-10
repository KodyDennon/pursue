use crate::analysis::AnalysisManager;
use crate::commands::{AppState, to_error};
use crate::models::{
    AnalysisReport, RecordForensics, IntelligenceLog, SearchRequest, SearchResults,
};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn analyze_record(
    id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalysisReport, String> {
    let manager = AnalysisManager::new(state.db.clone(), state.library.clone());
    
    let _ = app_handle.emit("analysis-progress", serde_json::json!({
        "current": 0,
        "total": 1,
        "status": "starting",
        "record_id": id
    }));

    let result = match manager.analyze_record(&app_handle, &id).await {
        Ok(res) => res,
        Err(e) => {
            let _ = app_handle.emit("analysis-progress", serde_json::json!({
                "current": 0,
                "total": 1,
                "status": "failed",
                "record_id": id,
                "error": e.to_string()
            }));
            return Err(e.to_string());
        }
    };
    
    let _ = app_handle.emit("analysis-progress", serde_json::json!({
        "current": 1,
        "total": 1,
        "status": "completed",
        "record_id": id
    }));

    Ok(result)
}

#[tauri::command]
pub async fn analyze_all_records(state: State<'_, AppState>, app_handle: AppHandle) -> Result<usize, String> {
    let pool = state.db.clone();
    let records = sqlx::query("SELECT id FROM records WHERE (analysis_status IS NULL OR analysis_status != 'completed') AND local_path IS NOT NULL")
        .fetch_all(&pool)
        .await
        .map_err(to_error)?;

    let count = records.len();
    if count == 0 {
        return Ok(0);
    }
    
    let library = state.library.clone();
    let handle = app_handle.clone();
    
    tauri::async_runtime::spawn(async move {
        use futures_util::StreamExt;
        
        let manager = std::sync::Arc::new(AnalysisManager::new(pool, library));
        let completed = std::sync::Arc::new(tokio::sync::Mutex::new(0_usize));
        
        let mut stream = futures_util::stream::iter(records)
            .map(|row| {
                use sqlx::Row;
                let id: String = row.get("id");
                let manager = manager.clone();
                let handle = handle.clone();
                let completed = completed.clone();
                
                async move {
                    match manager.analyze_record(&handle, &id).await {
                        Ok(_) => {},
                        Err(e) => {
                             let _ = handle.emit("analysis-progress", serde_json::json!({
                                "status": "record-failed",
                                "record_id": id,
                                "error": e.to_string()
                            }));
                        }
                    }
                    let mut c = completed.lock().await;
                    *c += 1;
                    let _ = handle.emit("analysis-progress", serde_json::json!({
                        "current": *c,
                        "total": count,
                        "status": "analyzing",
                        "record_id": id
                    }));
                }
            })
            .buffer_unordered(4);

        while let Some(_) = stream.next().await {}

        let _ = handle.emit("analysis-progress", serde_json::json!({
            "current": count,
            "total": count,
            "status": "completed",
            "record_id": null
        }));
    });

    Ok(count)
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
