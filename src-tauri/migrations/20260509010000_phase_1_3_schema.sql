PRAGMA foreign_keys = ON;

ALTER TABLE records ADD COLUMN stable_key TEXT;
ALTER TABLE records ADD COLUMN source_snapshot_id TEXT;
ALTER TABLE records ADD COLUMN content_hash TEXT;
ALTER TABLE records ADD COLUMN removed_from_source_at TEXT;
ALTER TABLE records ADD COLUMN updated_at DATETIME DEFAULT CURRENT_TIMESTAMP;

CREATE UNIQUE INDEX IF NOT EXISTS idx_records_stable_key_source ON records(stable_key, source_type);
CREATE INDEX IF NOT EXISTS idx_records_document_url ON records(document_url);
CREATE INDEX IF NOT EXISTS idx_records_local_path ON records(local_path);

CREATE TABLE IF NOT EXISTS source_snapshots (
    id TEXT PRIMARY KEY,
    source_name TEXT NOT NULL,
    upstream_url TEXT NOT NULL,
    release_label TEXT,
    fetched_at TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    snapshot_path TEXT NOT NULL,
    record_count INTEGER NOT NULL,
    status TEXT NOT NULL,
    error TEXT
);

CREATE TABLE IF NOT EXISTS source_snapshot_records (
    snapshot_id TEXT NOT NULL,
    stable_key TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    title TEXT NOT NULL,
    document_url TEXT,
    record_json TEXT NOT NULL,
    PRIMARY KEY (snapshot_id, stable_key),
    FOREIGN KEY (snapshot_id) REFERENCES source_snapshots(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS source_diffs (
    id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL,
    stable_key TEXT NOT NULL,
    change_type TEXT NOT NULL,
    title TEXT NOT NULL,
    document_url TEXT,
    previous_hash TEXT,
    current_hash TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (snapshot_id) REFERENCES source_snapshots(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_source_diffs_snapshot ON source_diffs(snapshot_id, change_type);

CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    record_id TEXT,
    sha256 TEXT NOT NULL,
    original_filename TEXT,
    media_type TEXT,
    byte_size INTEGER NOT NULL,
    source_url TEXT,
    relative_path TEXT NOT NULL,
    source_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(sha256),
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_record ON artifacts(record_id);

CREATE TABLE IF NOT EXISTS download_jobs (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    total INTEGER NOT NULL DEFAULT 0,
    queued INTEGER NOT NULL DEFAULT 0,
    skipped INTEGER NOT NULL DEFAULT 0,
    completed INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    cancel_requested INTEGER NOT NULL DEFAULT 0,
    summary_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS download_job_items (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    title TEXT NOT NULL,
    url TEXT,
    status TEXT NOT NULL,
    bytes_downloaded INTEGER NOT NULL DEFAULT 0,
    byte_size INTEGER,
    error TEXT,
    artifact_id TEXT,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (job_id) REFERENCES download_jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE,
    FOREIGN KEY (artifact_id) REFERENCES artifacts(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_download_items_job ON download_job_items(job_id);

CREATE TABLE IF NOT EXISTS analysis_chunks (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    vector_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_analysis_chunks_record ON analysis_chunks(record_id);

CREATE VIRTUAL TABLE IF NOT EXISTS analysis_chunks_fts
USING fts5(chunk_id UNINDEXED, record_id UNINDEXED, title, text, entities);

CREATE TABLE IF NOT EXISTS case_notes (
    id TEXT PRIMARY KEY,
    case_id TEXT NOT NULL,
    record_id TEXT,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS exports (
    id TEXT PRIMARY KEY,
    case_id TEXT NOT NULL,
    format TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE
);
