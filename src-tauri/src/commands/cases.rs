use crate::cases;
use crate::commands::{to_error, AppState};
use crate::exports;
use crate::models::{
    AddRecordToCaseRequest, CaseNotesRequest, CaseSummary, CreateCaseRequest, ExportCaseRequest,
    ExportResult,
};
use tauri::State;

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
