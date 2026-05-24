ALTER TABLE download_job_items ADD COLUMN expected_size INTEGER;
ALTER TABLE download_job_items ADD COLUMN content_type TEXT;
ALTER TABLE download_job_items ADD COLUMN etag TEXT;
ALTER TABLE download_job_items ADD COLUMN last_modified TEXT;
ALTER TABLE download_job_items ADD COLUMN part_path TEXT;
ALTER TABLE download_job_items ADD COLUMN error_class TEXT;
ALTER TABLE download_job_items ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_job_items ADD COLUMN source_host TEXT;
ALTER TABLE download_job_items ADD COLUMN last_progress_at TEXT;
ALTER TABLE download_job_items ADD COLUMN resolved_url TEXT;

CREATE INDEX IF NOT EXISTS idx_download_items_job_status_updated
ON download_job_items(job_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_download_items_job_recent
ON download_job_items(job_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_records_dashboard_page
ON records(source_type, removed_from_source_at, release_date DESC, created_at DESC, title ASC);

CREATE INDEX IF NOT EXISTS idx_records_download_queue
ON records(source_type, local_path, release_date DESC, created_at DESC);

UPDATE app_settings
SET value_json = '{"auto_sync":true,"auto_analyze":false}', updated_at = CURRENT_TIMESTAMP
WHERE key = 'ingestion_agent'
  AND value_json IN (
    '{"auto_sync": true, "auto_analyze": true}',
    '{"auto_sync":true,"auto_analyze":true}'
  );
