-- Migration to align vector system and remove redundancy
PRAGMA foreign_keys = ON;

-- Remove vector_json from analysis_chunks as it's redundant with vec_analysis_chunks
-- Note: In SQLite, dropping a column requires recreating the table for older versions, 
-- but modern SQLite (3.35+) supports it directly.
ALTER TABLE analysis_chunks DROP COLUMN vector_json;
