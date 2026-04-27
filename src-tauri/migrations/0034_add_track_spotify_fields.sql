ALTER TABLE tracks ADD COLUMN spotify_id TEXT;
ALTER TABLE tracks ADD COLUMN popularity INTEGER;
CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_spotify_id ON tracks(spotify_id) WHERE spotify_id IS NOT NULL;
