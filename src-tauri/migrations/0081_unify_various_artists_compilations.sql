-- Migration 0081: Unify Various Artists Compilations
-- TASK-137: Normalize and unify compilation artist variants ('Various Interprets', 'Unknown',
--           'Unknown Artist', 'VA', 'V.A.', 'Various') into canonical 'Various Artists'.
--           Reassign album_artists, track_artists, and track_credits.
--           Add is_compilation to albums and mark compilation albums.
--           Purge residual unlinked obsolete artists and install recurrence prevention triggers.

-- ============================================================================
-- 1. Ensure is_compilation column exists on albums table
-- ============================================================================
ALTER TABLE albums ADD COLUMN is_compilation INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_albums_is_compilation ON albums(is_compilation);

-- ============================================================================
-- 2. Identify source variants to unify into Various Artists
-- ============================================================================
DROP TABLE IF EXISTS _compilation_artist_sources_0081;
CREATE TEMP TABLE _compilation_artist_sources_0081 AS
SELECT a.id AS source_id, a.name
FROM artists a
WHERE LOWER(TRIM(a.name)) IN (
    'various interprets',
    'various interpret',
    'unknown artist',
    'unknown',
    'v.a.',
    'va',
    'v/a',
    'various'
);

-- Ensure canonical 'Various Artists' exists in artists table if variants exist
INSERT OR IGNORE INTO artists (name)
SELECT 'Various Artists'
WHERE EXISTS (SELECT 1 FROM _compilation_artist_sources_0081);

DROP TABLE IF EXISTS _canonical_va_0081;
CREATE TEMP TABLE _canonical_va_0081 AS
SELECT id AS canonical_id FROM artists
WHERE LOWER(TRIM(name)) = 'various artists'
ORDER BY id ASC
LIMIT 1;

-- Exclude canonical Various Artists from sources if it matched 'various'
DELETE FROM _compilation_artist_sources_0081
WHERE source_id IN (SELECT canonical_id FROM _canonical_va_0081);

-- ============================================================================
-- 3. Consolidate metadata & clear external IDs on source records before reassigning
-- ============================================================================
UPDATE artists
SET
    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
        SELECT src.musicbrainz_id FROM artists src
        WHERE src.id IN (SELECT source_id FROM _compilation_artist_sources_0081)
          AND src.musicbrainz_id IS NOT NULL AND src.musicbrainz_id != ''
        LIMIT 1
    )),
    spotify_id = COALESCE(artists.spotify_id, (
        SELECT src.spotify_id FROM artists src
        WHERE src.id IN (SELECT source_id FROM _compilation_artist_sources_0081)
          AND src.spotify_id IS NOT NULL AND src.spotify_id != ''
        LIMIT 1
    )),
    tidal_id = COALESCE(artists.tidal_id, (
        SELECT src.tidal_id FROM artists src
        WHERE src.id IN (SELECT source_id FROM _compilation_artist_sources_0081)
          AND src.tidal_id IS NOT NULL AND src.tidal_id != ''
        LIMIT 1
    )),
    qobuz_id = COALESCE(artists.qobuz_id, (
        SELECT src.qobuz_id FROM artists src
        WHERE src.id IN (SELECT source_id FROM _compilation_artist_sources_0081)
          AND src.qobuz_id IS NOT NULL AND src.qobuz_id != ''
        LIMIT 1
    ))
WHERE id = (SELECT canonical_id FROM _canonical_va_0081);

UPDATE artists
SET musicbrainz_id = NULL, spotify_id = NULL, tidal_id = NULL, qobuz_id = NULL
WHERE id IN (SELECT source_id FROM _compilation_artist_sources_0081);

-- ============================================================================
-- 4. Reassign album_artists relationships (preventing composite PK conflicts)
-- ============================================================================
DELETE FROM album_artists
WHERE artist_id IN (SELECT source_id FROM _compilation_artist_sources_0081)
  AND EXISTS (
      SELECT 1 FROM album_artists aa2
      WHERE aa2.album_id = album_artists.album_id
        AND aa2.artist_id = (SELECT canonical_id FROM _canonical_va_0081)
  );

UPDATE album_artists
SET artist_id = (SELECT canonical_id FROM _canonical_va_0081)
WHERE artist_id IN (SELECT source_id FROM _compilation_artist_sources_0081);

-- ============================================================================
-- 5. Reassign track_artists relationships (preventing composite PK conflicts)
-- ============================================================================
DELETE FROM track_artists
WHERE artist_id IN (SELECT source_id FROM _compilation_artist_sources_0081)
  AND EXISTS (
      SELECT 1 FROM track_artists ta2
      WHERE ta2.track_id = track_artists.track_id
        AND ta2.artist_id = (SELECT canonical_id FROM _canonical_va_0081)
        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
  );

UPDATE track_artists
SET artist_id = (SELECT canonical_id FROM _canonical_va_0081)
WHERE artist_id IN (SELECT source_id FROM _compilation_artist_sources_0081);

-- ============================================================================
-- 6. Reassign track_credits relationships (preventing composite PK conflicts)
-- ============================================================================
DELETE FROM track_credits
WHERE artist_id IN (SELECT source_id FROM _compilation_artist_sources_0081)
  AND EXISTS (
      SELECT 1 FROM track_credits tc2
      WHERE tc2.track_id = track_credits.track_id
        AND tc2.artist_id = (SELECT canonical_id FROM _canonical_va_0081)
        AND tc2.role = track_credits.role
  );

UPDATE track_credits
SET artist_id = (SELECT canonical_id FROM _canonical_va_0081)
WHERE artist_id IN (SELECT source_id FROM _compilation_artist_sources_0081);

-- ============================================================================
-- 7. Mark compilation flag on albums associated with canonical Various Artists
-- ============================================================================
UPDATE albums
SET is_compilation = 1
WHERE id IN (
    SELECT aa.album_id FROM album_artists aa
    WHERE aa.artist_id = (SELECT canonical_id FROM _canonical_va_0081)
);

-- ============================================================================
-- 8. Purge residual unlinked obsolete artists
-- ============================================================================
DELETE FROM artists
WHERE id IN (SELECT source_id FROM _compilation_artist_sources_0081)
  AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id)
  AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
  AND NOT EXISTS (SELECT 1 FROM track_credits tc WHERE tc.artist_id = artists.id);

DROP TABLE IF EXISTS _compilation_artist_sources_0081;
DROP TABLE IF EXISTS _canonical_va_0081;

-- ============================================================================
-- 9. Recurrence Prevention Triggers
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS trg_artists_reject_va_variants_ins
BEFORE INSERT ON artists
FOR EACH ROW
WHEN LOWER(TRIM(NEW.name)) IN ('various interprets', 'various interpret', 'v.a.', 'va', 'v/a')
BEGIN
    SELECT RAISE(ABORT, 'Rejected compilation artist variant: use canonical Various Artists');
END;

CREATE TRIGGER IF NOT EXISTS trg_artists_reject_va_variants_upd
BEFORE UPDATE OF name ON artists
FOR EACH ROW
WHEN LOWER(TRIM(NEW.name)) IN ('various interprets', 'various interpret', 'v.a.', 'va', 'v/a')
BEGIN
    SELECT RAISE(ABORT, 'Rejected compilation artist variant: use canonical Various Artists');
END;

CREATE TRIGGER IF NOT EXISTS trg_album_artists_set_compilation_ins
AFTER INSERT ON album_artists
FOR EACH ROW
WHEN NEW.artist_id = (SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' LIMIT 1)
BEGIN
    UPDATE albums SET is_compilation = 1 WHERE id = NEW.album_id;
END;
