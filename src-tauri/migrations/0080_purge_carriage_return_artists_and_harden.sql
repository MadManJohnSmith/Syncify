-- Migration 0080: Purge Carriage Return Artists and Harden
-- TASK-133: Purge 14,631 contaminated artists containing raw carriage return '\r' (char 13),
--           line feeds (char 10), and technical credit prefixes ('Recording Engineer\r - Tony Castle',
--           'Synthesizer\r - Daft Punk'). Unify into canonical clean artists, reassign track_credits,
--           track_artists, and album_artists, purge unlinked residual records, and install recurrence prevention triggers.

-- ============================================================================
-- 1. Identify all contaminated artists containing \r (char 13) or \n (char 10)
-- ============================================================================
DROP TABLE IF EXISTS _contaminated_artists_0080;
CREATE TEMP TABLE _contaminated_artists_0080 AS
SELECT
    id,
    name,
    trim(ltrim(
        substr(name, CASE WHEN instr(name, char(13)) > 0 THEN instr(name, char(13)) ELSE instr(name, char(10)) END + 1),
        char(10) || char(13) || ' ' || char(9) || '-' || ':' || '–' || '—'
    )) AS clean_name,
    trim(substr(name, 1, CASE WHEN instr(name, char(13)) > 0 THEN instr(name, char(13)) ELSE instr(name, char(10)) END - 1)) AS extracted_role
FROM artists
WHERE instr(name, char(13)) > 0 OR instr(name, char(10)) > 0;

-- ============================================================================
-- 2. Build mapping to canonical target (existing canonical artist or ranked winner)
-- ============================================================================
DROP TABLE IF EXISTS _artist_remapping_0080;
CREATE TEMP TABLE _artist_remapping_0080 AS
WITH existing_canonical AS (
    SELECT id, LOWER(TRIM(name)) AS norm_name
    FROM artists
    WHERE instr(name, char(13)) = 0 AND instr(name, char(10)) = 0
),
ranked_contaminated AS (
    SELECT
        c.id,
        c.name,
        c.clean_name,
        c.extracted_role,
        ec.id AS existing_id,
        ROW_NUMBER() OVER (
            PARTITION BY LOWER(TRIM(c.clean_name))
            ORDER BY
                (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = c.id) DESC,
                (SELECT COUNT(*) FROM album_artists aa WHERE aa.artist_id = c.id) DESC,
                (SELECT COUNT(*) FROM track_credits tc WHERE tc.artist_id = c.id) DESC,
                c.id ASC
        ) as rn
    FROM _contaminated_artists_0080 c
    LEFT JOIN existing_canonical ec ON ec.norm_name = LOWER(TRIM(c.clean_name))
    WHERE c.clean_name != ''
)
SELECT
    rc.id AS source_id,
    COALESCE(
        rc.existing_id,
        (SELECT r2.id FROM ranked_contaminated r2 WHERE LOWER(TRIM(r2.clean_name)) = LOWER(TRIM(rc.clean_name)) AND r2.rn = 1)
    ) AS target_id,
    rc.clean_name,
    rc.extracted_role,
    CASE WHEN rc.existing_id IS NULL AND rc.rn = 1 THEN 1 ELSE 0 END AS is_winner_to_rename
FROM ranked_contaminated rc;

-- ============================================================================
-- 3. Consolidate metadata & service IDs onto target artists before deletion
-- ============================================================================
UPDATE artists
SET
    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
        SELECT src.musicbrainz_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.musicbrainz_id IS NOT NULL AND src.musicbrainz_id != ''
        LIMIT 1
    )),
    spotify_id = COALESCE(artists.spotify_id, (
        SELECT src.spotify_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.spotify_id IS NOT NULL AND src.spotify_id != ''
        LIMIT 1
    )),
    tidal_id = COALESCE(artists.tidal_id, (
        SELECT src.tidal_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.tidal_id IS NOT NULL AND src.tidal_id != ''
        LIMIT 1
    )),
    qobuz_id = COALESCE(artists.qobuz_id, (
        SELECT src.qobuz_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.qobuz_id IS NOT NULL AND src.qobuz_id != ''
        LIMIT 1
    )),
    image_url = COALESCE(artists.image_url, (
        SELECT src.image_url FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.image_url IS NOT NULL AND src.image_url != ''
        LIMIT 1
    )),
    favorite_at = COALESCE(artists.favorite_at, (
        SELECT src.favorite_at FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.favorite_at IS NOT NULL
        LIMIT 1
    )),
    is_favorite = MAX(
        COALESCE(artists.is_favorite, 0),
        COALESCE((
            SELECT MAX(src.is_favorite) FROM artists src
            JOIN _artist_remapping_0080 m ON m.source_id = src.id
            WHERE m.target_id = artists.id AND m.source_id != m.target_id
        ), 0)
    )
WHERE id IN (SELECT target_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- Clear service identifiers on source losers before deletion to prevent unique constraints
UPDATE artists
SET musicbrainz_id = NULL, tidal_id = NULL, qobuz_id = NULL, spotify_id = NULL
WHERE id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- ============================================================================
-- 4. Reassign track_credits
-- ============================================================================
DELETE FROM track_credits
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM track_credits tc2
      JOIN _artist_remapping_0080 m ON m.source_id = track_credits.artist_id
      WHERE tc2.track_id = track_credits.track_id
        AND tc2.artist_id = m.target_id
        AND tc2.role = track_credits.role
  );

UPDATE track_credits
SET artist_id = (SELECT target_id FROM _artist_remapping_0080 WHERE source_id = track_credits.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- ============================================================================
-- 5. Reassign track_artists
-- ============================================================================
DELETE FROM track_artists
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM track_artists ta2
      JOIN _artist_remapping_0080 m ON m.source_id = track_artists.artist_id
      WHERE ta2.track_id = track_artists.track_id
        AND ta2.artist_id = m.target_id
        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
  );

UPDATE track_artists
SET artist_id = (SELECT target_id FROM _artist_remapping_0080 WHERE source_id = track_artists.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- ============================================================================
-- 6. Reassign album_artists
-- ============================================================================
DELETE FROM album_artists
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM album_artists aa2
      JOIN _artist_remapping_0080 m ON m.source_id = album_artists.artist_id
      WHERE aa2.album_id = album_artists.album_id
        AND aa2.artist_id = m.target_id
  );

UPDATE album_artists
SET artist_id = (SELECT target_id FROM _artist_remapping_0080 WHERE source_id = album_artists.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- ============================================================================
-- 7. Delete merged source artists
-- ============================================================================
DELETE FROM artists
WHERE id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- ============================================================================
-- 8. Rename winner artists to their clean names
-- ============================================================================
UPDATE artists
SET name = (SELECT clean_name FROM _artist_remapping_0080 WHERE source_id = artists.id)
WHERE id IN (SELECT source_id FROM _artist_remapping_0080 WHERE is_winner_to_rename = 1);

-- ============================================================================
-- 9. Purge residual unlinked artists containing \r or \n
-- ============================================================================
DELETE FROM track_credits
WHERE artist_id IN (
    SELECT id FROM artists
    WHERE (instr(name, char(13)) > 0 OR instr(name, char(10)) > 0)
      AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
      AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id)
);

DELETE FROM artists
WHERE (instr(name, char(13)) > 0 OR instr(name, char(10)) > 0)
  AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
  AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id);

DROP TABLE IF EXISTS _artist_remapping_0080;
DROP TABLE IF EXISTS _contaminated_artists_0080;

-- ============================================================================
-- 10. Recurrence prevention triggers
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS trg_artists_reject_control_chars_ins
BEFORE INSERT ON artists
FOR EACH ROW
WHEN instr(NEW.name, char(13)) > 0 
  OR instr(NEW.name, char(10)) > 0
  OR instr(NEW.name, char(9)) > 0
BEGIN
    SELECT RAISE(ABORT, 'Rejected artist name containing carriage return or line breaks');
END;

CREATE TRIGGER IF NOT EXISTS trg_artists_reject_control_chars_upd
BEFORE UPDATE OF name ON artists
FOR EACH ROW
WHEN instr(NEW.name, char(13)) > 0 
  OR instr(NEW.name, char(10)) > 0
  OR instr(NEW.name, char(9)) > 0
BEGIN
    SELECT RAISE(ABORT, 'Rejected artist name containing carriage return or line breaks');
END;
