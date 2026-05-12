export interface RecordSummary {
	id: string;
	title: string;
	agency: string | null;
	release_date: string | null;
	incident_date: string | null;
	incident_location: string | null;
	document_url: string | null;
	local_path: string | null;
	file_type: string | null;
	source_type: string;
	summary: string | null;
	stable_key: string | null;
	content_hash: string | null;
	removed_from_source_at: string | null;
	artifact_sha256: string | null;
	artifact_size: number | null;
	analysis_status: string | null;
	intelligence_json: string | null;
	redaction_score: number | null;
	analysis_error: string | null;
	entity_count: number;
	thumbnail_path: string | null;
}

export type Record = RecordSummary;

export interface RecordFilter {
	source_type?: string | null;
	agency?: string | null;
	local_only?: boolean | null;
	query?: string | null;
}

export interface SnapshotDiff {
	change_type: string;
	title: string;
	document_url: string | null;
	stable_key: string;
}

export interface SyncReport {
	snapshot_id: string;
	upstream_url: string;
	fetched_at: string;
	content_hash: string;
	snapshot_path: string;
	record_count: number;
	added: number;
	changed: number;
	removed: number;
	diffs: SnapshotDiff[];
}

export interface DownloadResult {
	record_id: string;
	artifact_id: string;
	sha256: string;
	relative_path: string;
	byte_size: number;
	skipped_existing: boolean;
}

export interface BulkDownloadStatus {
	id: string;
	status: string;
	total: number;
	queued: number;
	skipped: number;
	completed: number;
	failed: number;
	cancel_requested: number;
	summary_json: string;
	created_at: string;
	updated_at: string;
}

export interface BulkDownloadItem {
	id: string;
	job_id: string;
	record_id: string;
	title: string;
	url: string | null;
	status: string;
	bytes_downloaded: number;
	byte_size: number | null;
	error: string | null;
	artifact_id: string | null;
	updated_at: string;
}

export interface BulkDownloadReport {
	job: BulkDownloadStatus;
	items: BulkDownloadItem[];
}

export interface DatabaseStatus {
	app_data_dir: string;
	database_path: string;
	library_path: string;
	snapshots_path: string;
	exports_path: string;
	total_records: number;
	official_records: number;
	manual_records: number;
	downloadable_records: number;
	local_records: number;
	artifact_count: number;
	artifact_bytes: number;
	analyzed_records: number;
	failed_analysis_records: number;
	analysis_chunks: number;
	vector_chunks: number;
	entity_count: number;
	case_count: number;
	source_snapshots: number;
	latest_snapshot_at: string | null;
	latest_snapshot_url: string | null;
	latest_snapshot_records: number | null;
	active_download_jobs: number;
	total_count?: number;
	total_size?: number;
}

export interface ManualImportRequest {
	path: string;
	title?: string | null;
	notes?: string | null;
}

export interface EntityHit {
	id: string;
	name: string;
	entity_type: string;
	confidence: number;
	source: string;
}

export interface RecordAsset {
	id: string;
	record_id: string;
	asset_type: string;
	local_path: string;
	mime_type: string | null;
	file_size: number | null;
	metadata_json: string | null;
	created_at: string;
}

export interface AnalysisReport {
	record_id: string;
	status: string;
	ocr_text: string;
	entities: EntityHit[];
	chunks_indexed: number;
	engine: string;
	intelligence_json?: string | null;
	assets: RecordAsset[];
}

export interface SearchRequest {
	query: string;
	filters?: {
		source_type?: string | null;
		case_id?: string | null;
		local_only?: boolean | null;
	} | null;
}

export interface SearchResultItem {
	id: string;
	title: string;
	agency: string | null;
	release_date: string | null;
	document_url: string | null;
	local_path: string | null;
	summary: string | null;
	artifact_sha256: string | null;
	distance: number;
	excerpt: string;
}

export interface SearchResults {
	query: string;
	total: number;
	results: SearchResultItem[];
}

export interface CaseSummary {
	id: string;
	title: string;
	description: string | null;
	created_at: string;
	record_count: number;
	note_count: number;
}

export interface ExportResult {
	export_id: string;
	case_id: string;
	format: string;
	relative_path: string;
	absolute_path: string;
}

export interface RecordForensics {
	id: string;
	record_id: string;
	layer_type: string;
	content: string;
	confidence: number;
	bounding_box_json: string | null;
	metadata_json: string | null;
	created_at: string;
}

export interface IntelligenceLog {
	id: string;
	record_id: string | null;
	system_prompt: string;
	user_prompt: string;
	thought_block: string | null;
	response_json: string;
	model_id: string;
	created_at: string;
}
