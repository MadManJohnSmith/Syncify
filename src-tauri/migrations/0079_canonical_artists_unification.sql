-- Migration 0079: Canonical Artists Unification & Purge
-- TASK-105: Case-insensitive (NOCASE) artist unification, HTML entity unescaping (&amp;),
--           purging garbage artist names, removing pure orphan artists, and recurrence prevention.

-- ============================================================================
-- 1. Deduplicate colliding artists based on LOWER(TRIM(REPLACE(name, '&amp;', '&')))
--    Prioritize survivor:
--      1. Most linked tracks
--      2. Most linked albums
--      3. Populated external service IDs
--      4. is_favorite = 1
--      5. Mixed case over all-lowercase
--      6. Deterministic tie-breaker: lowest canonical ID
-- ============================================================================
DROP TABLE IF EXISTS _artist_dedup_map_0079;
CREATE TEMP TABLE _artist_dedup_map_0079 AS
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
            PARTITION BY LOWER(TRIM(REPLACE(a.name, '&amp;', '&')))
            ORDER BY
                (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = a.id) DESC,
                (SELECT COUNT(*) FROM album_artists aa WHERE aa.artist_id = a.id) DESC,
                ((CASE WHEN a.spotify_id IS NOT NULL AND a.spotify_id != '' THEN 1 ELSE 0 END) +
                 (CASE WHEN a.tidal_id IS NOT NULL AND a.tidal_id != '' THEN 1 ELSE 0 END) +
                 (CASE WHEN a.qobuz_id IS NOT NULL AND a.qobuz_id != '' THEN 1 ELSE 0 END) +
                 (CASE WHEN a.musicbrainz_id IS NOT NULL AND a.musicbrainz_id != '' THEN 1 ELSE 0 END)) DESC,
                COALESCE(a.is_favorite, 0) DESC,
                (CASE WHEN a.name != LOWER(a.name) THEN 1 ELSE 0 END) DESC,
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
    ON LOWER(TRIM(REPLACE(loser.name, '&amp;', '&'))) = LOWER(TRIM(REPLACE(winner.name, '&amp;', '&')))
   AND winner.rn = 1
WHERE loser.rn > 1;

-- Clear unique service identifiers on losers prior to merging
UPDATE artists
SET musicbrainz_id = NULL, tidal_id = NULL, qobuz_id = NULL, spotify_id = NULL
WHERE id IN (SELECT loser_id FROM _artist_dedup_map_0079);

-- Consolidate metadata, service IDs, and favorite state onto winners
UPDATE artists
SET
    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
        SELECT m.loser_musicbrainz_id FROM _artist_dedup_map_0079 m
        WHERE m.winner_id = artists.id AND m.loser_musicbrainz_id IS NOT NULL AND m.loser_musicbrainz_id != ''
        LIMIT 1
    )),
    spotify_id = COALESCE(artists.spotify_id, (
        SELECT m.loser_spotify_id FROM _artist_dedup_map_0079 m
        WHERE m.winner_id = artists.id AND m.loser_spotify_id IS NOT NULL AND m.loser_spotify_id != ''
        LIMIT 1
    )),
    tidal_id = COALESCE(artists.tidal_id, (
        SELECT m.loser_tidal_id FROM _artist_dedup_map_0079 m
        WHERE m.winner_id = artists.id AND m.loser_tidal_id IS NOT NULL AND m.loser_tidal_id != ''
        LIMIT 1
    )),
    qobuz_id = COALESCE(artists.qobuz_id, (
        SELECT m.loser_qobuz_id FROM _artist_dedup_map_0079 m
        WHERE m.winner_id = artists.id AND m.loser_qobuz_id IS NOT NULL AND m.loser_qobuz_id != ''
        LIMIT 1
    )),
    image_url = COALESCE(artists.image_url, (
        SELECT m.loser_image_url FROM _artist_dedup_map_0079 m
        WHERE m.winner_id = artists.id AND m.loser_image_url IS NOT NULL AND m.loser_image_url != ''
        LIMIT 1
    )),
    favorite_at = COALESCE(artists.favorite_at, (
        SELECT m.loser_favorite_at FROM _artist_dedup_map_0079 m
        WHERE m.winner_id = artists.id AND m.loser_favorite_at IS NOT NULL
        LIMIT 1
    )),
    is_favorite = MAX(
        COALESCE(artists.is_favorite, 0),
        COALESCE((
            SELECT MAX(m.loser_is_favorite) FROM _artist_dedup_map_0079 m
            WHERE m.winner_id = artists.id
        ), 0)
    ),
    name = TRIM(REPLACE(artists.name, '&amp;', '&'))
WHERE id IN (SELECT winner_id FROM _artist_dedup_map_0079);

-- Reassign track_artists relationships (preventing composite PK conflicts)
DELETE FROM track_artists
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079)
  AND EXISTS (
      SELECT 1 FROM track_artists ta2
      JOIN _artist_dedup_map_0079 m ON m.loser_id = track_artists.artist_id
      WHERE ta2.track_id = track_artists.track_id
        AND ta2.artist_id = m.winner_id
        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
  );

UPDATE track_artists
SET artist_id = (SELECT winner_id FROM _artist_dedup_map_0079 WHERE loser_id = track_artists.artist_id)
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079);

-- Reassign album_artists relationships (preventing composite PK conflicts)
DELETE FROM album_artists
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079)
  AND EXISTS (
      SELECT 1 FROM album_artists aa2
      JOIN _artist_dedup_map_0079 m ON m.loser_id = album_artists.artist_id
      WHERE aa2.album_id = album_artists.album_id
        AND aa2.artist_id = m.winner_id
  );

UPDATE album_artists
SET artist_id = (SELECT winner_id FROM _artist_dedup_map_0079 WHERE loser_id = album_artists.artist_id)
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079);

-- Reassign track_credits relationships (preventing composite PK conflicts)
DELETE FROM track_credits
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079)
  AND EXISTS (
      SELECT 1 FROM track_credits tc2
      JOIN _artist_dedup_map_0079 m ON m.loser_id = track_credits.artist_id
      WHERE tc2.track_id = track_credits.track_id
        AND tc2.artist_id = m.winner_id
        AND tc2.role = track_credits.role
  );

UPDATE track_credits
SET artist_id = (SELECT winner_id FROM _artist_dedup_map_0079 WHERE loser_id = track_credits.artist_id)
WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079);

-- Delete loser artists
DELETE FROM artists WHERE id IN (SELECT loser_id FROM _artist_dedup_map_0079);
DROP TABLE IF EXISTS _artist_dedup_map_0079;

-- Clean residual HTML entities on non-colliding names
UPDATE artists
SET name = TRIM(REPLACE(name, '&amp;', '&'))
WHERE name LIKE '%&amp;%';

-- ============================================================================
-- 2. Purge Garbage Artists
--    - Empty names
--    - 'Unknown' and 'Unknown Artist'
--    - Literal escapes '\P', '\\P'
--    - Unlinked 'Various' (not 'Various Artists')
-- ============================================================================
DELETE FROM track_artists
WHERE artist_id IN (
    SELECT id FROM artists
    WHERE TRIM(name) = ''
       OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\p', '\\p')
       OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id))
);

DELETE FROM album_artists
WHERE artist_id IN (
    SELECT id FROM artists
    WHERE TRIM(name) = ''
       OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\p', '\\p')
       OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id))
);

DELETE FROM track_credits
WHERE artist_id IN (
    SELECT id FROM artists
    WHERE TRIM(name) = ''
       OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\p', '\\p')
       OR LOWER(TRIM(name)) = 'various'
);

DELETE FROM artists
WHERE TRIM(name) = ''
   OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\p', '\\p')
   OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id));

-- ============================================================================
-- 3. Purge Pure Orphan Artists
--    - No tracks in track_artists
--    - No albums in album_artists
--    - No is_favorite (is_favorite = 0 or NULL)
--    - No external service IDs (spotify_id, tidal_id, qobuz_id, musicbrainz_id)
-- ============================================================================
DROP TABLE IF EXISTS _orphan_artists_to_purge;
CREATE TEMP TABLE _orphan_artists_to_purge AS
SELECT a.id FROM artists a
WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = a.id)
  AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = a.id)
  AND COALESCE(a.is_favorite, 0) = 0
  AND (a.spotify_id IS NULL OR a.spotify_id = '')
  AND (a.tidal_id IS NULL OR a.tidal_id = '')
  AND (a.qobuz_id IS NULL OR a.qobuz_id = '')
  AND (a.musicbrainz_id IS NULL OR a.musicbrainz_id = '');

DELETE FROM track_credits WHERE artist_id IN (SELECT id FROM _orphan_artists_to_purge);
DELETE FROM artists WHERE id IN (SELECT id FROM _orphan_artists_to_purge);
DROP TABLE IF EXISTS _orphan_artists_to_purge;

-- ============================================================================
-- 4. Uniqueness Constraints, Indexes & Recurrence Prevention
-- ============================================================================
-- Ensure all names are strictly trimmed
UPDATE artists SET name = TRIM(name) WHERE name != TRIM(name);

-- Canonical NOCASE Unique Index on LOWER(TRIM(name))
CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_canonical_name_unique ON artists(LOWER(TRIM(name)));

-- Triggers to reject garbage artist names on future INSERT or UPDATE
CREATE TRIGGER IF NOT EXISTS trg_artists_reject_garbage_ins
BEFORE INSERT ON artists
FOR EACH ROW
WHEN TRIM(NEW.name) = ''
  OR LOWER(TRIM(NEW.name)) IN ('unknown', 'unknown artist', '\p', '\\p')
BEGIN
    SELECT RAISE(ABORT, 'Rejected garbage or empty artist name');
END;

CREATE TRIGGER IF NOT EXISTS trg_artists_reject_garbage_upd
BEFORE UPDATE OF name ON artists
FOR EACH ROW
WHEN TRIM(NEW.name) = ''
  OR LOWER(TRIM(NEW.name)) IN ('unknown', 'unknown artist', '\p', '\\p')
BEGIN
    SELECT RAISE(ABORT, 'Rejected garbage or empty artist name');
END;
