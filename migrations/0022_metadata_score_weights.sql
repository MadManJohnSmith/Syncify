-- Add quality scoring weights to metadata_preferences
ALTER TABLE metadata_preferences ADD COLUMN weight_album  INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_preferences ADD COLUMN weight_isrc   INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_preferences ADD COLUMN weight_mb_id  INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_preferences ADD COLUMN weight_cover  INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_preferences ADD COLUMN weight_year   INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_preferences ADD COLUMN weight_genre  INTEGER NOT NULL DEFAULT 1;
