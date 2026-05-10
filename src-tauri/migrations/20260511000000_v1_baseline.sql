PRAGMA foreign_keys = ON;

-- 1. Core Tactical Infrastructure
CREATE TABLE IF NOT EXISTS records (
    id TEXT PRIMARY KEY,
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
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

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

-- 2. Neural Extraction & Semantic Layers
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

-- 3. Forensic Intelligence Graph
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

-- 4. Artifact Lifecycle & Forensics
CREATE TABLE IF NOT EXISTS record_assets (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    asset_type TEXT NOT NULL,
    local_path TEXT NOT NULL,
    mime_type TEXT,
    file_size INTEGER,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
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

CREATE TABLE IF NOT EXISTS neural_thought_logs (
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

-- 5. System Intelligence & Logistics
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

-- 6. Performance Optimization & Seed Data
CREATE INDEX IF NOT EXISTS idx_records_status ON records(analysis_status);
CREATE INDEX IF NOT EXISTS idx_assets_record ON record_assets(record_id);
CREATE INDEX IF NOT EXISTS idx_chunks_record ON analysis_chunks(record_id);
CREATE INDEX IF NOT EXISTS idx_entities_record ON record_entities(record_id);
CREATE INDEX IF NOT EXISTS idx_fragments_record ON intelligence_fragments(record_id);
CREATE INDEX IF NOT EXISTS idx_forensics_record ON record_forensics(record_id);

INSERT OR IGNORE INTO app_settings (key, value_json, updated_at) 
VALUES ('ingestion_agent', '{"auto_sync": true, "auto_analyze": true}', CURRENT_TIMESTAMP);
