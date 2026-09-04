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

-- 2. Deduplicate and normalize tracks colliding on normalized ISRC
DROP TABLE IF EXISTS _isrc_dedup_map;
CREATE TEMP TABLE _isrc_dedup_map AS
WITH ranked AS (
  SELECT 
    id,
    spotify_id,
    UPPER(REPLACE(TRIM(isrc), '-', '')) AS norm_isrc,
    ROW_NUMBER() OVER (
      PARTITION BY UPPER(REPLACE(TRIM(isrc), '-', ''))
      ORDER BY 
        (SELECT count(*) FROM downloads d WHERE d.track_id = tracks.id) DESC,
        (isrc = UPPER(REPLACE(TRIM(isrc), '-', ''))) DESC,
        (SELECT count(*) FROM track_sources ts WHERE ts.track_id = tracks.id) DESC,
        id ASC
    ) AS rn
  FROM tracks
  WHERE isrc IS NOT NULL
)
SELECT 
  loser.id AS loser_id,
  winner.id AS winner_id,
  loser.spotify_id AS loser_spotify_id
FROM ranked loser
JOIN ranked winner ON loser.norm_isrc = winner.norm_isrc AND winner.rn = 1
WHERE loser.rn > 1;

-- 2a. Merge metadata and favorites into winner tracks
UPDATE tracks
SET 
  is_favorite = MAX(tracks.is_favorite, (SELECT COALESCE(MAX(l.is_favorite), 0) FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id)),
  favorite_at = COALESCE(tracks.favorite_at, (SELECT l.favorite_at FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.favorite_at IS NOT NULL ORDER BY l.favorite_at DESC LIMIT 1)),
  album_id = COALESCE(tracks.album_id, (SELECT l.album_id FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.album_id IS NOT NULL LIMIT 1)),
  musicbrainz_id = COALESCE(tracks.musicbrainz_id, (SELECT l.musicbrainz_id FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.musicbrainz_id IS NOT NULL LIMIT 1)),
  qobuz_id = COALESCE(tracks.qobuz_id, (SELECT l.qobuz_id FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.qobuz_id IS NOT NULL LIMIT 1)),
  genre = COALESCE(tracks.genre, (SELECT l.genre FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.genre IS NOT NULL LIMIT 1)),
  release_year = COALESCE(tracks.release_year, (SELECT l.release_year FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.release_year IS NOT NULL LIMIT 1))
WHERE id IN (SELECT winner_id FROM _isrc_dedup_map);

-- 2b. Null out loser unique fields so they don't block winner updates or index creation
UPDATE tracks SET spotify_id = NULL, isrc = NULL WHERE id IN (SELECT loser_id FROM _isrc_dedup_map);

UPDATE tracks
SET spotify_id = (
  SELECT m.loser_spotify_id 
  FROM _isrc_dedup_map m 
  WHERE m.winner_id = tracks.id AND m.loser_spotify_id IS NOT NULL 
  LIMIT 1
)
WHERE id IN (SELECT winner_id FROM _isrc_dedup_map)
  AND tracks.spotify_id IS NULL;

-- 2c. Deduplicate and reassign track_sources
DELETE FROM track_sources
WHERE id NOT IN (
  SELECT MIN(ts.id)
  FROM track_sources ts
  LEFT JOIN _isrc_dedup_map m ON ts.track_id = m.loser_id
  GROUP BY COALESCE(m.winner_id, ts.track_id), ts.service_id
);

UPDATE track_sources 
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = track_sources.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2d. Reassign playlist_tracks
UPDATE playlist_tracks 
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = playlist_tracks.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2e. Deduplicate and reassign library_entries
DELETE FROM library_entries
WHERE id NOT IN (
  SELECT MIN(le.id)
  FROM library_entries le
  LEFT JOIN _isrc_dedup_map m ON le.track_id = m.loser_id
  GROUP BY le.account_id, COALESCE(m.winner_id, le.track_id)
);

UPDATE library_entries 
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = library_entries.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2f. Deduplicate and reassign lyrics
DELETE FROM lyrics
WHERE id NOT IN (
  SELECT MIN(l.id)
  FROM lyrics l
  LEFT JOIN _isrc_dedup_map m ON l.track_id = m.loser_id
  GROUP BY COALESCE(m.winner_id, l.track_id), l.format
);

UPDATE lyrics 
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = lyrics.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2g. Deduplicate and reassign downloads
DELETE FROM downloads
WHERE id NOT IN (
  SELECT MIN(d.id)
  FROM downloads d
  LEFT JOIN _isrc_dedup_map m ON d.track_id = m.loser_id
  GROUP BY COALESCE(m.winner_id, d.track_id)
);

UPDATE downloads 
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = downloads.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2h. Deduplicate and reassign track_artists
DELETE FROM track_artists
WHERE rowid NOT IN (
  SELECT MIN(ta.rowid)
  FROM track_artists ta
  LEFT JOIN _isrc_dedup_map m ON ta.track_id = m.loser_id
  GROUP BY COALESCE(m.winner_id, ta.track_id), ta.artist_id, COALESCE(ta.role, 'primary')
);

UPDATE track_artists
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = track_artists.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2i. Deduplicate and reassign track_credits
DELETE FROM track_credits
WHERE rowid NOT IN (
  SELECT MIN(tc.rowid)
  FROM track_credits tc
  LEFT JOIN _isrc_dedup_map m ON tc.track_id = m.loser_id
  GROUP BY COALESCE(m.winner_id, tc.track_id), tc.artist_id, tc.role
);

UPDATE track_credits
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = track_credits.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2j. Reassign download_queue
UPDATE download_queue
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = download_queue.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2k. Deduplicate and reassign enrichment_progress
DELETE FROM enrichment_progress
WHERE id NOT IN (
  SELECT MIN(ep.id)
  FROM enrichment_progress ep
  LEFT JOIN _isrc_dedup_map m ON ep.track_id = m.loser_id
  GROUP BY COALESCE(m.winner_id, ep.track_id), ep.service
);

UPDATE enrichment_progress
SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = enrichment_progress.track_id)
WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map);

-- 2l. Delete loser tracks and drop temp table
DELETE FROM tracks WHERE id IN (SELECT loser_id FROM _isrc_dedup_map);
DROP TABLE IF EXISTS _isrc_dedup_map;

-- 2m. Normalize all remaining ISRCs
UPDATE tracks SET isrc = UPPER(REPLACE(TRIM(isrc), '-', '')) WHERE isrc IS NOT NULL;

-- 2n. Ensure case-insensitive unique index for ISRCs
DROP INDEX IF EXISTS idx_tracks_isrc_unique;
CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_isrc_unique ON tracks(isrc COLLATE NOCASE) WHERE isrc IS NOT NULL;

-- 3. Ensure unique index on origin in track_sources
DELETE FROM track_sources WHERE id NOT IN (SELECT MIN(id) FROM track_sources GROUP BY service_id, service_track_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_track_sources_service_track_unique ON track_sources(service_id, service_track_id);

-- 4. Fix default values for SoundCloud
UPDATE services SET max_quality = 'lossy' WHERE name = 'soundcloud';
UPDATE quality_preferences SET max_quality = 'lossy', preferred_format = 'mp3' WHERE service_name = 'soundcloud';
