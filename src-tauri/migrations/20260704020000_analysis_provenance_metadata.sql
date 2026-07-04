ALTER TABLE analysis_results ADD COLUMN engine TEXT;
ALTER TABLE analysis_results ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE analysis_results ADD COLUMN warnings_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX IF NOT EXISTS idx_analysis_results_engine
ON analysis_results(engine);
