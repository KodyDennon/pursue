-- Migration: Forensic Intelligence Upgrade
-- Adds support for deep layered extraction, redaction profiling, and neural auditability.

CREATE TABLE IF NOT EXISTS record_forensics (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    layer_type TEXT NOT NULL, -- 'hidden_text', 'improper_redaction', 'neural_profile'
    content TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    bounding_box_json TEXT, -- JSON array of [x, y, w, h] if applicable
    metadata_json TEXT DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forensics_record ON record_forensics(record_id);

CREATE TABLE IF NOT EXISTS intelligence_logs (
    id TEXT PRIMARY KEY,
    record_id TEXT,
    system_prompt TEXT NOT NULL,
    user_prompt TEXT NOT NULL,
    thought_block TEXT,
    response_json TEXT NOT NULL,
    model_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_intel_logs_record ON intelligence_logs(record_id);
