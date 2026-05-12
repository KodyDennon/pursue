PRAGMA foreign_keys = ON;

-- 1. Core Tactical Infrastructure
CREATE TABLE IF NOT EXISTS records (
    id TEXT PRIMARY KEY,
    stable_key TEXT,
    source_snapshot_id TEXT,
    content_hash TEXT,
    title TEXT NOT NULL,
    agency TEXT,
    release_date TEXT,
    incident_date TEXT,
    incident_location TEXT,
    document_url TEXT,
    local_path TEXT,
    file_type TEXT,
    source_type TEXT DEFAULT 'official',
    summary TEXT,
    intelligence_json TEXT,
    redaction_score REAL,
    analysis_status TEXT DEFAULT 'pending',
    analysis_error TEXT,
    thumbnail_path TEXT,
    removed_from_source_at TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_records_stable_key_source ON records(stable_key, source_type);
CREATE INDEX IF NOT EXISTS idx_records_document_url ON records(document_url);
CREATE INDEX IF NOT EXISTS idx_records_local_path ON records(local_path);

CREATE TABLE IF NOT EXISTS cases (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS case_records (
    case_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    notes TEXT,
    PRIMARY KEY (case_id, record_id),
    FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

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

-- 2. Source Ingestion & Snapshotting
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

-- 3. Artifact Lifecycle & Forensics
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

-- 4. Neural Extraction & Semantic Layers
CREATE TABLE IF NOT EXISTS analysis_results (
    record_id TEXT PRIMARY KEY,
    ocr_text TEXT,
    status TEXT DEFAULT 'pending',
    processed_at DATETIME,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS analysis_chunks (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    engine_name TEXT,
    model_version TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE IF NOT EXISTS analysis_chunks_fts USING fts5(
    chunk_id UNINDEXED, 
    record_id UNINDEXED, 
    title, 
    text, 
    entities
);

CREATE VIRTUAL TABLE IF NOT EXISTS vec_analysis_chunks USING vec0(
  chunk_id TEXT PRIMARY KEY,
  embedding FLOAT[384]
);

-- 5. Forensic Intelligence Graph
CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    description TEXT,
    UNIQUE(name, entity_type)
);

CREATE TABLE IF NOT EXISTS record_entities (
    record_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    confidence REAL DEFAULT 1.0,
    PRIMARY KEY (record_id, entity_id),
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS intelligence_fragments (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    fragment_type TEXT NOT NULL,
    text TEXT NOT NULL,
    confidence REAL NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE IF NOT EXISTS vec_intelligence_fragments USING vec0(
    fragment_id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

CREATE TABLE IF NOT EXISTS record_forensics (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    layer_type TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    bounding_box_json TEXT,
    metadata_json TEXT DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS intelligence_logs (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    user_prompt TEXT NOT NULL,
    thought_block TEXT,
    response_json TEXT NOT NULL,
    model_id TEXT NOT NULL,
    tokens_used INTEGER,
    execution_time_ms INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

-- 6. System Intelligence & Logistics
CREATE TABLE IF NOT EXISTS exports (
    id TEXT PRIMARY KEY,
    case_id TEXT NOT NULL,
    format TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- 7. Performance Optimization & Seed Data
CREATE INDEX IF NOT EXISTS idx_records_status ON records(analysis_status);
CREATE INDEX IF NOT EXISTS idx_chunks_record ON analysis_chunks(record_id);
CREATE INDEX IF NOT EXISTS idx_entities_record ON record_entities(record_id);
CREATE INDEX IF NOT EXISTS idx_fragments_record ON intelligence_fragments(record_id);
CREATE INDEX IF NOT EXISTS idx_forensics_record ON record_forensics(record_id);

INSERT OR IGNORE INTO app_settings (key, value_json, updated_at) 
VALUES ('ingestion_agent', '{"auto_sync": true, "auto_analyze": true}', CURRENT_TIMESTAMP);
