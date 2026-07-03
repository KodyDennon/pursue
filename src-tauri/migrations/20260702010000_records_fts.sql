-- Full-text index for record search (title/summary/agency/incident_location). The queries in
-- db/records.rs previously matched via `lower(col) LIKE '%'||lower(?)||'%'`, which cannot use a
-- B-tree index regardless of what indexes exist (leading wildcard defeats any index) — every
-- list/search call was a full table scan. External-content FTS5 table keyed on SQLite's
-- built-in rowid (records.id is TEXT, not usable directly as an FTS5 content_rowid).
CREATE VIRTUAL TABLE IF NOT EXISTS records_fts USING fts5(
    title,
    summary,
    agency,
    incident_location,
    content = 'records',
    content_rowid = 'rowid'
);

-- Backfill rows that existed before this migration ran.
INSERT INTO records_fts(rowid, title, summary, agency, incident_location)
SELECT rowid, title, summary, agency, incident_location FROM records;

-- Keep records_fts in sync automatically regardless of which code path writes to `records`
-- (war.gov sync upsert, manual import, web ingest, etc.) — trigger-based sync means no Rust
-- call site needs to remember to also update the index.
CREATE TRIGGER IF NOT EXISTS trg_records_fts_insert AFTER INSERT ON records BEGIN
    INSERT INTO records_fts(rowid, title, summary, agency, incident_location)
    VALUES (new.rowid, new.title, new.summary, new.agency, new.incident_location);
END;

CREATE TRIGGER IF NOT EXISTS trg_records_fts_delete AFTER DELETE ON records BEGIN
    INSERT INTO records_fts(records_fts, rowid, title, summary, agency, incident_location)
    VALUES ('delete', old.rowid, old.title, old.summary, old.agency, old.incident_location);
END;

CREATE TRIGGER IF NOT EXISTS trg_records_fts_update AFTER UPDATE ON records BEGIN
    INSERT INTO records_fts(records_fts, rowid, title, summary, agency, incident_location)
    VALUES ('delete', old.rowid, old.title, old.summary, old.agency, old.incident_location);
    INSERT INTO records_fts(rowid, title, summary, agency, incident_location)
    VALUES (new.rowid, new.title, new.summary, new.agency, new.incident_location);
END;
