-- Migration 0036: Add spotify_id to albums and enforce unique constraints for S70
ALTER TABLE albums ADD COLUMN spotify_id TEXT;

-- Unique partial index for album spotify_id
CREATE UNIQUE INDEX IF NOT EXISTS idx_albums_spotify_id ON albums(spotify_id) WHERE spotify_id IS NOT NULL;

-- Unique partial index for track ISRC (required for ON CONFLICT in S70)
CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_isrc_unique ON tracks(isrc) WHERE isrc IS NOT NULL;

-- Unique index for artist name (required for ON CONFLICT in S70)
CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_name_unique ON artists(name);
