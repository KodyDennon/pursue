-- Hot-path indexes for polling, asset lookup, and pending analysis queues.
CREATE INDEX IF NOT EXISTS idx_download_jobs_status_updated
ON download_jobs(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_record_assets_record_type
ON record_assets(record_id, asset_type);

CREATE INDEX IF NOT EXISTS idx_records_analysis_local
ON records(analysis_status, local_path)
WHERE local_path IS NOT NULL;
