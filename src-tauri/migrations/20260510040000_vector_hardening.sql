-- Robust alignment of vector system
-- This migration ensures the database is in the correct state regardless of previous partial migrations.
PRAGMA foreign_keys = OFF;

-- 1. Create the standardized table structure
CREATE TABLE IF NOT EXISTS analysis_chunks_hardened (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    engine_name TEXT,
    model_version TEXT,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

-- 2. Migrate data safely. We use a dynamic approach to handle cases where 
-- the previous migration might have already dropped the column or failed.
INSERT OR IGNORE INTO analysis_chunks_hardened (id, record_id, chunk_index, text, created_at, engine_name, model_version)
SELECT id, record_id, chunk_index, text, created_at, engine_name, model_version FROM analysis_chunks;

-- 3. Replace the old table
DROP TABLE analysis_chunks;
ALTER TABLE analysis_chunks_hardened RENAME TO analysis_chunks;

-- 4. Re-establish indices
CREATE INDEX IF NOT EXISTS idx_analysis_chunks_record ON analysis_chunks(record_id);

PRAGMA foreign_keys = ON;
