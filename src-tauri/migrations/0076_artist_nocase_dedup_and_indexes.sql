-- Migration 0076: Case-Insensitive (NOCASE) Artist Deduplication, Unique Constraint and Reverse Indexes
-- TASK-83: Deduplicate case/whitespace variant artists, merge relational links and metadata,
--          enforce case-insensitive unique constraint, and establish inverse performance indexes.

-- ============================================================================
-- 1. Drop old binary unique index to allow restructuring and sanitization
-- ============================================================================
DROP INDEX IF EXISTS idx_artists_name_unique;

-- Support optional artist image_url column across schemas
ALTER TABLE artists ADD COLUMN image_url TEXT;

-- ============================================================================
-- 2. Build Deduplication Map for Artist Groups colliding on LOWER(TRIM(name))
--    Preserve loser metadata and external identifiers in temp table before clearing
-- ============================================================================
DROP TABLE IF EXISTS _artist_dedup_map;
CREATE TEMP TABLE _artist_dedup_map AS
WITH ranked_artists AS (
    SELECT
        a.id,
        a.name,
        a.musicbrainz_id,
        a.spotify_id,
        a.tidal_id,
        a.qobuz_id,
        a.image_url,
        a.favorite_at,
        a.is_favorite,
        ROW_NUMBER() OVER (
            PARTITION BY LOWER(TRIM(a.name))
            ORDER BY
                -- Prioritize artist with most linked tracks
                (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = a.id) DESC,
                -- Prioritize artist with most linked albums
                (SELECT COUNT(*) FROM album_artists aa WHERE aa.artist_id = a.id) DESC,
                -- Prioritize artist with populated external service identifiers
                ((CASE WHEN a.spotify_id IS NOT NULL AND a.spotify_id != '' THEN 1 ELSE 0 END) +
                 (CASE WHEN a.tidal_id IS NOT NULL AND a.tidal_id != '' THEN 1 ELSE 0 END) +
                 (CASE WHEN a.qobuz_id IS NOT NULL AND a.qobuz_id != '' THEN 1 ELSE 0 END) +
                 (CASE WHEN a.musicbrainz_id IS NOT NULL AND a.musicbrainz_id != '' THEN 1 ELSE 0 END)) DESC,
                -- Prioritize favorited artist
                a.is_favorite DESC,
                -- Deterministic tie-breaker: lowest canonical ID
                a.id ASC
        ) AS rn
    FROM artists a
)
SELECT
    loser.id AS loser_id,
    winner.id AS winner_id,
    loser.musicbrainz_id AS loser_musicbrainz_id,
    loser.spotify_id AS loser_spotify_id,
    loser.tidal_id AS loser_tidal_id,
    loser.qobuz_id AS loser_qobuz_id,
    loser.image_url AS loser_image_url,
    loser.favorite_at AS loser_favorite_at,
    loser.is_favorite AS loser_is_favorite
FROM ranked_artists loser
JOIN ranked_artists winner
    ON LOWER(TRIM(loser.name)) = LOWER(TRIM(winner.name))
   AND winner.rn = 1
WHERE loser.rn > 1;

-- ============================================================================
-- 3. Clear Unique Service Identifiers on Loser Artists before Merging
-- ============================================================================
UPDATE artists
SET musicbrainz_id = NULL, tidal_id = NULL, qobuz_id = NULL, spotify_id = NULL
WHERE id IN (SELECT loser_id FROM _artist_dedup_map);

-- ============================================================================
-- 4. Merge Richer Metadata, Service IDs, Favorites, and Sanitized Name into Winners
-- ============================================================================
UPDATE artists
SET
    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
        SELECT m.loser_musicbrainz_id FROM _artist_dedup_map m
        WHERE m.winner_id = artists.id AND m.loser_musicbrainz_id IS NOT NULL AND m.loser_musicbrainz_id != ''
        LIMIT 1
    )),
    spotify_id = COALESCE(artists.spotify_id, (
        SELECT m.loser_spotify_id FROM _artist_dedup_map m
        WHERE m.winner_id = artists.id AND m.loser_spotify_id IS NOT NULL AND m.loser_spotify_id != ''
        LIMIT 1
    )),
    tidal_id = COALESCE(artists.tidal_id, (
        SELECT m.loser_tidal_id FROM _artist_dedup_map m
        WHERE m.winner_id = artists.id AND m.loser_tidal_id IS NOT NULL AND m.loser_tidal_id != ''
        LIMIT 1
    )),
    qobuz_id = COALESCE(artists.qobuz_id, (
        SELECT m.loser_qobuz_id FROM _artist_dedup_map m
        WHERE m.winner_id = artists.id AND m.loser_qobuz_id IS NOT NULL AND m.loser_qobuz_id != ''
        LIMIT 1
    )),
    image_url = COALESCE(artists.image_url, (
        SELECT m.loser_image_url FROM _artist_dedup_map m
        WHERE m.winner_id = artists.id AND m.loser_image_url IS NOT NULL AND m.loser_image_url != ''
        LIMIT 1
    )),
    favorite_at = COALESCE(artists.favorite_at, (
        SELECT m.loser_favorite_at FROM _artist_dedup_map m
        WHERE m.winner_id = artists.id AND m.loser_favorite_at IS NOT NULL
        LIMIT 1
    )),
    is_favorite = MAX(
        artists.is_favorite,
        COALESCE((
            SELECT MAX(m.loser_is_favorite) FROM _artist_dedup_map m
            WHERE m.winner_id = artists.id
        ), 0)
    ),
    name = TRIM(artists.name)
WHERE id IN (SELECT winner_id FROM _artist_dedup_map);

-- ============================================================================
-- 5. Reassign Junction Table References (Safely Handling Composite Primary Keys)
-- ============================================================================

-- 5a. Track Artists (track_id, artist_id, role)
DELETE FROM track_artists
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map)
  AND EXISTS (
      SELECT 1 FROM track_artists ta2
      JOIN _artist_dedup_map m ON m.loser_id = track_artists.artist_id
      WHERE ta2.track_id = track_artists.track_id
        AND ta2.artist_id = m.winner_id
        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
  );

UPDATE track_artists
SET artist_id = (SELECT winner_id FROM _artist_dedup_map WHERE loser_id = track_artists.artist_id)
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map);

-- 5b. Album Artists (album_id, artist_id)
DELETE FROM album_artists
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map)
  AND EXISTS (
      SELECT 1 FROM album_artists aa2
      JOIN _artist_dedup_map m ON m.loser_id = album_artists.artist_id
      WHERE aa2.album_id = album_artists.album_id
        AND aa2.artist_id = m.winner_id
  );

UPDATE album_artists
SET artist_id = (SELECT winner_id FROM _artist_dedup_map WHERE loser_id = album_artists.artist_id)
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map);

-- 5c. Track Credits (track_id, artist_id, role)
DELETE FROM track_credits
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map)
  AND EXISTS (
      SELECT 1 FROM track_credits tc2
      JOIN _artist_dedup_map m ON m.loser_id = track_credits.artist_id
      WHERE tc2.track_id = track_credits.track_id
        AND tc2.artist_id = m.winner_id
        AND tc2.role = track_credits.role
  );

UPDATE track_credits
SET artist_id = (SELECT winner_id FROM _artist_dedup_map WHERE loser_id = track_credits.artist_id)
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map);

-- ============================================================================
-- 6. Delete Loser Artist Records and Purge Temp Dedup Map
-- ============================================================================
DELETE FROM artists WHERE id IN (SELECT loser_id FROM _artist_dedup_map);
DROP TABLE IF EXISTS _artist_dedup_map;

-- Sanitize all existing artist names (trim extraneous whitespace)
UPDATE artists SET name = TRIM(name) WHERE name != TRIM(name);

-- ============================================================================
-- 7. Create Case-Insensitive Unique Index & Secondary Performance Indexes
-- ============================================================================
CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_name_unique_nocase ON artists(name COLLATE NOCASE);

CREATE INDEX IF NOT EXISTS idx_track_artists_artist ON track_artists(artist_id);
CREATE INDEX IF NOT EXISTS idx_album_artists_artist ON album_artists(artist_id);
CREATE INDEX IF NOT EXISTS idx_track_credits_artist ON track_credits(artist_id);

-- ============================================================================
-- 8. Recurrence Prevention Triggers: Automatic Name Trimming
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS trg_artists_sanitize_name_ins
AFTER INSERT ON artists
FOR EACH ROW
WHEN NEW.name != TRIM(NEW.name)
BEGIN
    UPDATE artists SET name = TRIM(name) WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_artists_sanitize_name_upd
AFTER UPDATE OF name ON artists
FOR EACH ROW
WHEN NEW.name != TRIM(NEW.name)
BEGIN
    UPDATE artists SET name = TRIM(name) WHERE id = NEW.id;
END;
