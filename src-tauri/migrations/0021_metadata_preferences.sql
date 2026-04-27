-- Migration 0021: Metadata Preferences Table
-- Sprint 14 implementation

CREATE TABLE IF NOT EXISTS metadata_preferences (
  id INTEGER PRIMARY KEY DEFAULT 1,
  overwrite_on_reimport    BOOLEAN NOT NULL DEFAULT 0,
  preserve_custom_tags     BOOLEAN NOT NULL DEFAULT 1,
  multi_value_separator    TEXT    NOT NULL DEFAULT ';',
  write_releasetype        BOOLEAN NOT NULL DEFAULT 1,
  write_label              BOOLEAN NOT NULL DEFAULT 1,
  write_work_composer      BOOLEAN NOT NULL DEFAULT 0,
  write_musicbrainz_ids    BOOLEAN NOT NULL DEFAULT 1,
  write_download_source    BOOLEAN NOT NULL DEFAULT 0,
  write_download_date      BOOLEAN NOT NULL DEFAULT 0,
  write_only_available_on  BOOLEAN NOT NULL DEFAULT 0,
  write_not_available_streaming BOOLEAN NOT NULL DEFAULT 0,
  write_quality_score      BOOLEAN NOT NULL DEFAULT 0,
  write_lyrics_tags        BOOLEAN NOT NULL DEFAULT 0
);

-- Initialize the singleton row
INSERT OR IGNORE INTO metadata_preferences (id) VALUES (1);
