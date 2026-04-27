-- Add is_favorite column to tracks table
ALTER TABLE tracks ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_tracks_favorite ON tracks(is_favorite);
