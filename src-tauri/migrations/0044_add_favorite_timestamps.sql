-- Add favorite_at timestamp to tracks
ALTER TABLE tracks ADD COLUMN favorite_at TEXT;
CREATE INDEX IF NOT EXISTS idx_tracks_favorite_at ON tracks(is_favorite, favorite_at DESC);
