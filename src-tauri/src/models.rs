use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Record {
    pub id: String,
    pub title: String,
    pub agency: Option<String>,
    pub release_date: Option<String>,
    pub incident_date: Option<String>,
    pub incident_location: Option<String>,
    pub document_url: Option<String>,
    pub local_path: Option<String>,
    pub file_type: Option<String>,
    pub source_type: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordSummary {
    pub id: String,
    pub title: String,
    pub agency: Option<String>,
    pub release_date: Option<String>,
    pub incident_date: Option<String>,
    pub incident_location: Option<String>,
    pub document_url: Option<String>,
    pub local_path: Option<String>,
    pub file_type: Option<String>,
    pub source_type: String,
    pub summary: Option<String>,
    pub stable_key: Option<String>,
    pub content_hash: Option<String>,
    pub removed_from_source_at: Option<String>,
    pub artifact_sha256: Option<String>,
    pub artifact_size: Option<i64>,
    pub analysis_status: Option<String>,
    pub intelligence_json: Option<String>,
    pub redaction_score: Option<f64>,
    pub analysis_error: Option<String>,
    pub entity_count: i64,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordFilter {
    pub source_type: Option<String>,
    pub agency: Option<String>,
    pub local_only: Option<bool>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvRecord {
    #[serde(rename = "Redaction", default)]
    pub redaction: Option<String>,
    #[serde(rename = "Release Date", default)]
    pub release_date: Option<String>,
    #[serde(rename = "Title", default)]
    pub title: Option<String>,
    #[serde(rename = "Type", default)]
    pub doc_type: Option<String>,
    #[serde(rename = "Video Pairing", default)]
    pub video_pairing: Option<String>,
    #[serde(rename = "PDF Pairing", default)]
    pub pdf_pairing: Option<String>,
    #[serde(rename = "Description Blurb", default)]
    pub description: Option<String>,
    #[serde(rename = "DVIDS Video ID", default)]
    pub dvids_video_id: Option<String>,
    #[serde(rename = "Video Title", default)]
    pub video_title: Option<String>,
    #[serde(rename = "Agency", default)]
    pub agency: Option<String>,
    #[serde(rename = "Incident Date", default)]
    pub incident_date: Option<String>,
    #[serde(rename = "Incident Location", default)]
    pub incident_location: Option<String>,
    #[serde(rename = "PDF | Image Link", default)]
    pub document_url: Option<String>,
    #[serde(rename = "Modal Image", default)]
    pub modal_image: Option<String>,
    #[serde(rename = "Image Alt Text", default)]
    pub image_alt_text: Option<String>,
    #[serde(rename = "Image VIRIN", default)]
    pub image_virin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub change_type: String,
    pub title: String,
    pub document_url: Option<String>,
    pub stable_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub snapshot_id: String,
    pub upstream_url: String,
    pub fetched_at: String,
    pub content_hash: String,
    pub snapshot_path: String,
    pub record_count: usize,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    pub diffs: Vec<SnapshotDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub record_id: String,
    pub artifact_id: String,
    pub sha256: String,
    pub relative_path: String,
    pub byte_size: i64,
    pub skipped_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BulkDownloadStatus {
    pub id: String,
    pub status: String,
    pub total: i64,
    pub queued: i64,
    pub skipped: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancel_requested: i64,
    pub summary_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BulkDownloadItem {
    pub id: String,
    pub job_id: String,
    pub record_id: String,
    pub title: String,
    pub url: Option<String>,
    pub status: String,
    pub bytes_downloaded: i64,
    pub byte_size: Option<i64>,
    pub error: Option<String>,
    pub artifact_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDownloadReport {
    pub job: BulkDownloadStatus,
    pub items: Vec<BulkDownloadItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatus {
    pub app_data_dir: String,
    pub database_path: String,
    pub database_bytes: i64,
    pub library_path: String,
    pub snapshots_path: String,
    pub exports_path: String,
    pub total_records: i64,
    pub official_records: i64,
    pub manual_records: i64,
    pub downloadable_records: i64,
    pub local_records: i64,
    pub artifact_count: i64,
    pub artifact_bytes: i64,
    pub analyzed_records: i64,
    pub failed_analysis_records: i64,
    pub analysis_chunks: i64,
    pub vector_chunks: i64,
    pub entity_count: i64,
    pub case_count: i64,
    pub source_snapshots: i64,
    pub latest_snapshot_at: Option<String>,
    pub latest_snapshot_url: Option<String>,
    pub latest_snapshot_records: Option<i64>,
    pub active_download_jobs: i64,
    pub total_count: i64,
    pub total_size: i64,
    pub unanalyzed_count: i64,
    pub completed_count: i64,
    pub pending_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualImportRequest {
    pub path: String,
    pub title: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityHit {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub confidence: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnalysisChunk {
    pub id: String,
    pub record_id: String,
    pub chunk_index: i64,
    pub text: String,
    pub engine_name: Option<String>,
    pub model_version: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordAsset {
    pub id: String,
    pub record_id: String,
    pub asset_type: String,
    pub local_path: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub record_id: String,
    pub status: String,
    pub ocr_text: String,
    pub entities: Vec<EntityHit>,
    pub chunks_indexed: usize,
    pub engine: String,
    pub intelligence_json: Option<String>,
    pub assets: Vec<RecordAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    pub source_type: Option<String>,
    pub case_id: Option<String>,
    pub local_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub filters: Option<SearchFilters>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SearchResultItem {
    pub id: String,
    pub title: String,
    pub agency: Option<String>,
    pub release_date: Option<String>,
    pub document_url: Option<String>,
    pub local_path: Option<String>,
    pub summary: Option<String>,
    pub artifact_sha256: Option<String>,
    pub distance: f64,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub query: String,
    pub total: usize,
    pub results: Vec<SearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CaseSummary {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_at: String,
    pub record_count: i64,
    pub note_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCaseRequest {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseNotesRequest {
    pub case_id: String,
    pub record_id: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRecordToCaseRequest {
    pub case_id: String,
    pub record_id: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCaseRequest {
    pub case_id: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub export_id: String,
    pub case_id: String,
    pub format: String,
    pub relative_path: String,
    pub absolute_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecordForensics {
    pub id: String,
    pub record_id: String,
    pub layer_type: String,
    pub content: String,
    pub confidence: f64,
    pub bounding_box_json: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IntelligenceLog {
    pub id: String,
    pub record_id: Option<String>,
    pub system_prompt: String,
    pub user_prompt: String,
    pub thought_block: Option<String>,
    pub response_json: String,
    pub model_id: String,
    pub created_at: String,
}
