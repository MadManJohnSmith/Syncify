-- Migration 0068: Sanitize Album and Track Titles Whitespace
-- TASK-134: Resolves whitespace collisions, trailing spaces, and consecutive internal spaces in albums and tracks.
-- 1. Normalizes newlines, carriage returns, and tabs in albums and tracks to single spaces.
-- 2. Collapses consecutive multiple spaces into a single space.
-- 3. Trims leading and trailing whitespace from albums.title and tracks.title.
-- 4. Deduplicates colliding album rows sharing the same artist and normalized title, reassigning tracks and album_artists.
-- 5. Installs durable recurrence-prevention triggers to sanitize future inserts and updates.

-- ============================================================================
-- 1. Whitespace Normalization (Albums & Tracks)
-- ============================================================================

-- Clean carriage returns, newlines, and tabs
UPDATE albums
SET title = REPLACE(REPLACE(REPLACE(title, char(13), ' '), char(10), ' '), char(9), ' ')
WHERE title LIKE '%' || char(13) || '%'
   OR title LIKE '%' || char(10) || '%'
   OR title LIKE '%' || char(9) || '%';

UPDATE tracks
SET title = REPLACE(REPLACE(REPLACE(title, char(13), ' '), char(10), ' '), char(9), ' ')
WHERE title LIKE '%' || char(13) || '%'
   OR title LIKE '%' || char(10) || '%'
   OR title LIKE '%' || char(9) || '%';

-- Collapse multiple consecutive spaces (iteratively)
UPDATE albums SET title = REPLACE(title, '        ', ' ') WHERE title LIKE '%        %';
UPDATE albums SET title = REPLACE(title, '    ', ' ') WHERE title LIKE '%    %';
UPDATE albums SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';
UPDATE albums SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';

UPDATE tracks SET title = REPLACE(title, '        ', ' ') WHERE title LIKE '%        %';
UPDATE tracks SET title = REPLACE(title, '    ', ' ') WHERE title LIKE '%    %';
UPDATE tracks SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';
UPDATE tracks SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';

-- Trim leading and trailing whitespace
UPDATE albums SET title = TRIM(title) WHERE title != TRIM(title);
UPDATE tracks SET title = TRIM(title) WHERE title != TRIM(title);

-- ============================================================================
-- 2. Deduplicate Colliding Albums
-- ============================================================================

DROP TABLE IF EXISTS _album_dedup_map;
CREATE TEMP TABLE _album_dedup_map AS
WITH ranked_albums AS (
    SELECT
        a.id,
        aa.artist_id,
        a.title,
        ROW_NUMBER() OVER (
            PARTITION BY aa.artist_id, LOWER(a.title)
            ORDER BY
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id) DESC,
                (a.tidal_id IS NOT NULL OR a.spotify_id IS NOT NULL OR a.qobuz_id IS NOT NULL) DESC,
                (a.cover_art_url IS NOT NULL) DESC,
                a.id ASC
        ) AS rn
    FROM albums a
    JOIN album_artists aa ON aa.album_id = a.id AND aa.is_primary = 1
)
SELECT
    loser.id AS loser_id,
    winner.id AS winner_id
FROM ranked_albums loser
JOIN ranked_albums winner
    ON loser.artist_id = winner.artist_id
   AND LOWER(loser.title) = LOWER(winner.title)
   AND winner.rn = 1
WHERE loser.rn > 1;

-- Also deduplicate unlinked albums with identical title
INSERT OR IGNORE INTO _album_dedup_map
WITH unlinked_ranked AS (
    SELECT
        a.id,
        a.title,
        ROW_NUMBER() OVER (
            PARTITION BY LOWER(a.title)
            ORDER BY
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id) DESC,
                (a.cover_art_url IS NOT NULL) DESC,
                a.id ASC
        ) AS rn
    FROM albums a
    WHERE NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.album_id = a.id)
)
SELECT
    loser.id AS loser_id,
    winner.id AS winner_id
FROM unlinked_ranked loser
JOIN unlinked_ranked winner
    ON LOWER(loser.title) = LOWER(winner.title)
   AND winner.rn = 1
WHERE loser.rn > 1;

-- Clear unique identifiers on loser albums before merging into winners
UPDATE albums
SET tidal_id = NULL, spotify_id = NULL, qobuz_id = NULL, musicbrainz_id = NULL
WHERE id IN (SELECT loser_id FROM _album_dedup_map);

-- Merge richer metadata and favorites from losers into winners
UPDATE albums
SET
    cover_art_url = COALESCE(albums.cover_art_url, (SELECT l.cover_art_url FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.cover_art_url IS NOT NULL LIMIT 1)),
    release_date = COALESCE(albums.release_date, (SELECT l.release_date FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.release_date IS NOT NULL LIMIT 1)),
    total_tracks = COALESCE(albums.total_tracks, (SELECT l.total_tracks FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.total_tracks IS NOT NULL LIMIT 1)),
    tidal_id = COALESCE(albums.tidal_id, (SELECT l.tidal_id FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.tidal_id IS NOT NULL LIMIT 1)),
    spotify_id = COALESCE(albums.spotify_id, (SELECT l.spotify_id FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.spotify_id IS NOT NULL LIMIT 1)),
    qobuz_id = COALESCE(albums.qobuz_id, (SELECT l.qobuz_id FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.qobuz_id IS NOT NULL LIMIT 1)),
    musicbrainz_id = COALESCE(albums.musicbrainz_id, (SELECT l.musicbrainz_id FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.musicbrainz_id IS NOT NULL LIMIT 1)),
    upc = COALESCE(albums.upc, (SELECT l.upc FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.upc IS NOT NULL LIMIT 1)),
    label = COALESCE(albums.label, (SELECT l.label FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id AND l.label IS NOT NULL LIMIT 1)),
    is_favorite = MAX(albums.is_favorite, COALESCE((SELECT MAX(l.is_favorite) FROM albums l JOIN _album_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = albums.id), 0))
WHERE id IN (SELECT winner_id FROM _album_dedup_map);

-- Reassign track foreign keys from loser albums to winner albums
UPDATE tracks
SET album_id = (SELECT winner_id FROM _album_dedup_map WHERE loser_id = tracks.album_id)
WHERE album_id IN (SELECT loser_id FROM _album_dedup_map);

-- Reassign album_artists links to winner albums
INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary)
SELECT m.winner_id, aa.artist_id, aa.is_primary
FROM album_artists aa
JOIN _album_dedup_map m ON aa.album_id = m.loser_id;

-- Delete loser links and loser album rows
DELETE FROM album_artists WHERE album_id IN (SELECT loser_id FROM _album_dedup_map);
DELETE FROM albums WHERE id IN (SELECT loser_id FROM _album_dedup_map);

DROP TABLE IF EXISTS _album_dedup_map;

-- ============================================================================
-- 3. Recurrence Prevention Triggers
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_albums_sanitize_title_ins
AFTER INSERT ON albums
FOR EACH ROW
WHEN NEW.title != TRIM(NEW.title)
BEGIN
    UPDATE albums SET title = TRIM(title) WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_albums_sanitize_title_upd
AFTER UPDATE OF title ON albums
FOR EACH ROW
WHEN NEW.title != TRIM(NEW.title)
BEGIN
    UPDATE albums SET title = TRIM(title) WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_sanitize_title_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.title != TRIM(NEW.title)
BEGIN
    UPDATE tracks SET title = TRIM(title) WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_sanitize_title_upd
AFTER UPDATE OF title ON tracks
FOR EACH ROW
WHEN NEW.title != TRIM(NEW.title)
BEGIN
    UPDATE tracks SET title = TRIM(title) WHERE id = NEW.id;
END;
