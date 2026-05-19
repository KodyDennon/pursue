-- Standardize Virtual Table Sync via Triggers
-- This ensures that when a record's analysis is cleared or the record is deleted, 
-- the associated virtual table data (VEC0 and FTS5) is purged automatically.

-- 1. Analysis Chunks Cleanup
CREATE TRIGGER IF NOT EXISTS trg_cleanup_analysis_chunks_vec
AFTER DELETE ON analysis_chunks
FOR EACH ROW
BEGIN
    DELETE FROM vec_analysis_chunks WHERE chunk_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_cleanup_analysis_chunks_fts
AFTER DELETE ON analysis_chunks
FOR EACH ROW
BEGIN
    DELETE FROM analysis_chunks_fts WHERE chunk_id = OLD.id;
END;

-- 2. Intelligence Fragments Cleanup
CREATE TRIGGER IF NOT EXISTS trg_cleanup_intelligence_fragments_vec
AFTER DELETE ON intelligence_fragments
FOR EACH ROW
BEGIN
    DELETE FROM vec_intelligence_fragments WHERE fragment_id = OLD.id;
END;

-- 3. Record Assets (Physical Cleanup notification - App level handled, but DB integrity here)
CREATE TRIGGER IF NOT EXISTS trg_cleanup_records_cascade
AFTER DELETE ON records
FOR EACH ROW
BEGIN
    DELETE FROM analysis_results WHERE record_id = OLD.id;
END;
