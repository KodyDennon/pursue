-- Records table
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
    source_type TEXT DEFAULT 'official', -- 'official' or 'personal'
    summary TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Entities table (Knowledge Graph)
CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL, -- 'person', 'location', 'agency', 'object_shape', 'sensor', 'propulsion'
    description TEXT,
    UNIQUE(name, entity_type)
);

-- Relational links
CREATE TABLE IF NOT EXISTS record_entities (
    record_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    confidence REAL DEFAULT 1.0,
    PRIMARY KEY (record_id, entity_id),
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
);

-- Cases
CREATE TABLE IF NOT EXISTS cases (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Records in Cases
CREATE TABLE IF NOT EXISTS case_records (
    case_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    notes TEXT,
    PRIMARY KEY (case_id, record_id),
    FOREIGN KEY (case_id) REFERENCES cases(id) ON DELETE CASCADE,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);

-- Analysis Results (OCR etc)
CREATE TABLE IF NOT EXISTS analysis_results (
    record_id TEXT PRIMARY KEY,
    ocr_text TEXT,
    vector_id TEXT,
    status TEXT DEFAULT 'pending', -- 'pending', 'processing', 'completed', 'failed'
    processed_at DATETIME,
    FOREIGN KEY (record_id) REFERENCES records(id) ON DELETE CASCADE
);
