-- Migration for Record Assets (Images, Videos)
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS record_assets (
  id TEXT PRIMARY KEY,
  record_id TEXT NOT NULL,
  asset_type TEXT NOT NULL, -- image, video, other
  local_path TEXT NOT NULL,
  mime_type TEXT,
  file_size INTEGER,
  metadata_json TEXT, -- Optional JSON metadata (dimensions, extraction info)
  created_at TEXT NOT NULL,
  FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_assets_record ON record_assets(record_id);
