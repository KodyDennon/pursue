-- App Settings Table
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Initialize default settings
INSERT OR IGNORE INTO app_settings (key, value_json, updated_at) 
VALUES ('ingestion_agent', '{"auto_sync": true, "auto_analyze": true}', CURRENT_TIMESTAMP);
