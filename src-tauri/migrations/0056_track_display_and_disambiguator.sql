-- Migration 0056: Track Display Title, Source Title, and File Disambiguator
-- Allows distinguishing distinct editions/remixes while preserving original upstream source title

ALTER TABLE tracks ADD COLUMN display_title TEXT;
ALTER TABLE tracks ADD COLUMN source_title TEXT;
ALTER TABLE tracks ADD COLUMN file_disambiguator TEXT;

ALTER TABLE downloads ADD COLUMN file_disambiguator TEXT;

-- Create index for quick lookup of disambiguated tracks
CREATE INDEX IF NOT EXISTS idx_tracks_display_title ON tracks(display_title);
CREATE INDEX IF NOT EXISTS idx_tracks_file_disambiguator ON tracks(file_disambiguator);
