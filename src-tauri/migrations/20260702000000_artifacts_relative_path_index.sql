-- Supports joining artifacts to records via content-addressed relative_path instead of the
-- clobberable artifacts.record_id column (two different records that happen to download
-- byte-identical content previously fought over a single record_id, silently hiding the
-- artifact/SHA-256 for whichever record lost).
CREATE INDEX IF NOT EXISTS idx_artifacts_relative_path ON artifacts(relative_path);
