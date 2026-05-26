-- Performance Optimization for initial list_records_page call
CREATE INDEX IF NOT EXISTS idx_records_created_at ON records(created_at);
CREATE INDEX IF NOT EXISTS idx_records_release_date ON records(release_date);

-- Optimization for group by and counts used in get_database_status
CREATE INDEX IF NOT EXISTS idx_records_agency ON records(agency);
CREATE INDEX IF NOT EXISTS idx_records_source_type ON records(source_type);
CREATE INDEX IF NOT EXISTS idx_artifacts_sha256 ON artifacts(sha256);
CREATE INDEX IF NOT EXISTS idx_record_assets_type ON record_assets(asset_type);
CREATE INDEX IF NOT EXISTS idx_analysis_results_status ON analysis_results(status);
