ALTER TABLE records ADD COLUMN release_label TEXT;
ALTER TABLE records ADD COLUMN source_asset_class TEXT;
ALTER TABLE records ADD COLUMN dvids_video_id TEXT;
ALTER TABLE records ADD COLUMN video_title TEXT;
ALTER TABLE records ADD COLUMN video_pairing TEXT;
ALTER TABLE records ADD COLUMN pdf_pairing TEXT;
ALTER TABLE records ADD COLUMN modal_image TEXT;
ALTER TABLE records ADD COLUMN image_alt_text TEXT;
ALTER TABLE records ADD COLUMN image_virin TEXT;

CREATE INDEX IF NOT EXISTS idx_records_dvids_video_id ON records(dvids_video_id);
CREATE INDEX IF NOT EXISTS idx_records_source_asset_class ON records(source_asset_class);
