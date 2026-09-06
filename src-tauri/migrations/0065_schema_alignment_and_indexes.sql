-- Migration 0065: Schema alignment (artists.qobuz_id) and critical performance indexes.
--
-- 1. Schema alignment:
--    Adds artists.qobuz_id (mirrors tracks.qobuz_id in 0043 and albums.qobuz_id in 0063).
--    Fixes runtime failures when querying artist services across providers.
--
-- 2. Performance indexes:
--    idx_artists_qobuz_id: Fast lookup and unique constraint for Qobuz artist IDs.
--    idx_download_queue_track_id: Prevents full table scan on queue lookups by track.
--    idx_track_artists_artist_id: Accelerates track-artist relationship joins.
--    idx_album_artists_artist_id: Accelerates discography and artist album views.
--    idx_tracks_qobuz_id: Accelerates track lookups by Qobuz ID.

ALTER TABLE artists ADD COLUMN qobuz_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_qobuz_id ON artists(qobuz_id) WHERE qobuz_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_download_queue_track_id ON download_queue(track_id);
CREATE INDEX IF NOT EXISTS idx_track_artists_artist_id ON track_artists(artist_id);
CREATE INDEX IF NOT EXISTS idx_album_artists_artist_id ON album_artists(artist_id);
CREATE INDEX IF NOT EXISTS idx_tracks_qobuz_id ON tracks(qobuz_id) WHERE qobuz_id IS NOT NULL;
