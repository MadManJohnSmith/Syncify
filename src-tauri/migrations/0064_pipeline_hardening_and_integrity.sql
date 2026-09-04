-- Migration 0064: Pipeline Hardening and Integrity Constraints
-- 1. Modify constraint on playlist_tracks: migrate from UNIQUE(playlist_id, track_id) to UNIQUE(playlist_id, position)
CREATE TABLE playlist_tracks_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    added_at TEXT,
    UNIQUE(playlist_id, position)
);

INSERT OR IGNORE INTO playlist_tracks_new (id, playlist_id, track_id, position, added_at)
SELECT id, playlist_id, track_id, position, added_at FROM playlist_tracks;

DROP TABLE playlist_tracks;

ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks;

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track ON playlist_tracks(track_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_pos ON playlist_tracks(playlist_id, position);

-- 2. Ensure case-insensitive unique index for ISRCs
DROP INDEX IF EXISTS idx_tracks_isrc_unique;
CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_isrc_unique ON tracks(isrc COLLATE NOCASE) WHERE isrc IS NOT NULL;

-- 3. Ensure unique index on origin in track_sources
DELETE FROM track_sources WHERE id NOT IN (SELECT MIN(id) FROM track_sources GROUP BY service_id, service_track_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_track_sources_service_track_unique ON track_sources(service_id, service_track_id);

-- 4. Fix default values for SoundCloud
UPDATE services SET max_quality = 'lossy' WHERE name = 'soundcloud';
UPDATE quality_preferences SET max_quality = 'lossy', preferred_format = 'mp3' WHERE service_name = 'soundcloud';
