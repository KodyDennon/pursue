-- Migration for Intelligence Engine & Vector Search
PRAGMA foreign_keys = ON;

-- 1. Add intelligence fields to records
ALTER TABLE records ADD COLUMN intelligence_json TEXT; -- Gemma extracted structured data
ALTER TABLE records ADD COLUMN redaction_score REAL; -- 0.0 to 1.0 based on black box area
ALTER TABLE records ADD COLUMN analysis_status TEXT DEFAULT 'pending'; -- pending, processing, completed, failed
ALTER TABLE records ADD COLUMN analysis_error TEXT;

-- 2. Create Vector Table using sqlite-vec
-- We use 384 dimensions for all-MiniLM-L6-v2
CREATE VIRTUAL TABLE IF NOT EXISTS vec_analysis_chunks USING vec0(
  chunk_id TEXT PRIMARY KEY,
  embedding FLOAT[384]
);

-- 3. Add column to track which engine was used
ALTER TABLE analysis_chunks ADD COLUMN engine_name TEXT;
ALTER TABLE analysis_chunks ADD COLUMN model_version TEXT;

-- 4. Create a table for Intelligence Entities (Named Entities)
-- This allows fast filtering by Location, Person, or Object Type
CREATE TABLE IF NOT EXISTS extracted_entities (
  id TEXT PRIMARY KEY,
  record_id TEXT NOT NULL,
  entity_type TEXT NOT NULL, -- Location, Date, Person, Object, Organization
  entity_value TEXT NOT NULL,
  confidence REAL,
  FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entities_record ON extracted_entities(record_id);
CREATE INDEX IF NOT EXISTS idx_entities_type_value ON extracted_entities(entity_type, entity_value);
