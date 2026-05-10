-- Migration for Intelligence Graph & Neural Auditability
PRAGMA foreign_keys = ON;

-- 1. Create Neural Thought Logs table for transparency
-- Stores the AI's "internal monologue" and specific model parameters used
CREATE TABLE IF NOT EXISTS neural_thought_logs (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    user_prompt TEXT NOT NULL,
    thought_block TEXT, -- For models that support "reasoning" or "thinking" tags
    response_json TEXT NOT NULL,
    model_id TEXT NOT NULL,
    tokens_used INTEGER,
    execution_time_ms INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_neural_thoughts_record ON neural_thought_logs(record_id);

-- 2. Create Intelligence Fragments for the Pattern Graph
-- This allows cross-record correlation (e.g. "Find all records where AI detected 'Site B'")
CREATE VIRTUAL TABLE IF NOT EXISTS vec_intelligence_fragments USING vec0(
    fragment_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE TABLE IF NOT EXISTS intelligence_fragments (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    fragment_type TEXT NOT NULL, -- 'observation', 'pattern', 'entity_correlation'
    text TEXT NOT NULL,
    confidence REAL NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_intel_fragments_record ON intelligence_fragments(record_id);

-- 3. Standardize Analysis Results status
-- Adding 'indexed' to differentiate between OCR completion and Full AI Synthesis
-- We can't easily add constraints to existing status columns in SQLite without recreation,
-- but we will enforce this in the application logic.
