-- Add thumbnail_path to records
PRAGMA foreign_keys = ON;
ALTER TABLE records ADD COLUMN thumbnail_path TEXT;
