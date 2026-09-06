-- Migration 0078: Album Stubs and Catalog Placeholder Purge
-- TASK-103: Hydration of ghost albums, marking stubs, preview classification, and purging corrupt placeholder tracks.

-- 1. Add is_stub column to albums (0 = fully populated or standard, 1 = stub with 0 tracks)
ALTER TABLE albums ADD COLUMN is_stub INTEGER NOT NULL DEFAULT 0;

-- 2. Add is_preview column to tracks (0 = full length >= 30s, 1 = preview / snippet < 30s)
ALTER TABLE tracks ADD COLUMN is_preview INTEGER NOT NULL DEFAULT 0;

-- 3. Create indexes for efficient filtering
CREATE INDEX IF NOT EXISTS idx_albums_is_stub ON albums(is_stub);
CREATE INDEX IF NOT EXISTS idx_tracks_is_preview ON tracks(is_preview);

-- 4. Mark is_preview = 1 for existing tracks with duration < 30s and > 0
UPDATE tracks
SET is_preview = 1
WHERE duration_ms > 0 AND duration_ms < 30000;

-- 5. Purge ghost and placeholder tracks (duration_ms = 0 or placeholder titles)
-- Clean referencing foreign-key tables first to guarantee referential integrity
DELETE FROM track_artists WHERE track_id IN (
    SELECT id FROM tracks
    WHERE (duration_ms = 0 AND (
        title IS NULL 
        OR TRIM(title) = '' 
        OR LOWER(TRIM(title)) = 'unavailable'
        OR LOWER(TRIM(title)) = 'unknown'
        OR LOWER(TRIM(title)) LIKE 'unknown%'
        OR LOWER(TRIM(title)) LIKE 'track %'
        OR LOWER(TRIM(title)) = 'track'
    ))
    OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
);

DELETE FROM track_sources WHERE track_id IN (
    SELECT id FROM tracks
    WHERE (duration_ms = 0 AND (
        title IS NULL 
        OR TRIM(title) = '' 
        OR LOWER(TRIM(title)) = 'unavailable'
        OR LOWER(TRIM(title)) = 'unknown'
        OR LOWER(TRIM(title)) LIKE 'unknown%'
        OR LOWER(TRIM(title)) LIKE 'track %'
        OR LOWER(TRIM(title)) = 'track'
    ))
    OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
);

DELETE FROM playlist_tracks WHERE track_id IN (
    SELECT id FROM tracks
    WHERE (duration_ms = 0 AND (
        title IS NULL 
        OR TRIM(title) = '' 
        OR LOWER(TRIM(title)) = 'unavailable'
        OR LOWER(TRIM(title)) = 'unknown'
        OR LOWER(TRIM(title)) LIKE 'unknown%'
        OR LOWER(TRIM(title)) LIKE 'track %'
        OR LOWER(TRIM(title)) = 'track'
    ))
    OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
);

DELETE FROM library_entries WHERE track_id IN (
    SELECT id FROM tracks
    WHERE (duration_ms = 0 AND (
        title IS NULL 
        OR TRIM(title) = '' 
        OR LOWER(TRIM(title)) = 'unavailable'
        OR LOWER(TRIM(title)) = 'unknown'
        OR LOWER(TRIM(title)) LIKE 'unknown%'
        OR LOWER(TRIM(title)) LIKE 'track %'
        OR LOWER(TRIM(title)) = 'track'
    ))
    OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
);

DELETE FROM download_queue WHERE track_id IN (
    SELECT id FROM tracks
    WHERE (duration_ms = 0 AND (
        title IS NULL 
        OR TRIM(title) = '' 
        OR LOWER(TRIM(title)) = 'unavailable'
        OR LOWER(TRIM(title)) = 'unknown'
        OR LOWER(TRIM(title)) LIKE 'unknown%'
        OR LOWER(TRIM(title)) LIKE 'track %'
        OR LOWER(TRIM(title)) = 'track'
    ))
    OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
);

UPDATE downloads SET track_id = NULL WHERE track_id IN (
    SELECT id FROM tracks
    WHERE (duration_ms = 0 AND (
        title IS NULL 
        OR TRIM(title) = '' 
        OR LOWER(TRIM(title)) = 'unavailable'
        OR LOWER(TRIM(title)) = 'unknown'
        OR LOWER(TRIM(title)) LIKE 'unknown%'
        OR LOWER(TRIM(title)) LIKE 'track %'
        OR LOWER(TRIM(title)) = 'track'
    ))
    OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
);

DELETE FROM tracks
WHERE (duration_ms = 0 AND (
    title IS NULL 
    OR TRIM(title) = '' 
    OR LOWER(TRIM(title)) = 'unavailable'
    OR LOWER(TRIM(title)) = 'unknown'
    OR LOWER(TRIM(title)) LIKE 'unknown%'
    OR LOWER(TRIM(title)) LIKE 'track %'
    OR LOWER(TRIM(title)) = 'track'
))
OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'));

-- 6. Mark is_stub = 1 for albums with 0 tracks, and is_stub = 0 for albums with >= 1 track
UPDATE albums
SET is_stub = 1
WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);

UPDATE albums
SET is_stub = 0
WHERE id IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);

-- 7. Reclassify any residual tracks falsely marked as 'enriched' with 0 duration or preview status
UPDATE tracks
SET enrichment_status = 'pending'
WHERE enrichment_status = 'enriched'
  AND (duration_ms = 0 OR is_preview = 1);

-- 8. Recurrence Prevention: Triggers for automatic stub and preview maintenance

-- Trigger: When a track is inserted with album_id, automatically clear is_stub on the album
CREATE TRIGGER IF NOT EXISTS trg_tracks_clear_album_stub_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.album_id IS NOT NULL
BEGIN
    UPDATE albums SET is_stub = 0 WHERE id = NEW.album_id AND is_stub = 1;
END;

-- Trigger: When a track is deleted, re-evaluate if the album has become a stub
CREATE TRIGGER IF NOT EXISTS trg_tracks_set_album_stub_del
AFTER DELETE ON tracks
FOR EACH ROW
WHEN OLD.album_id IS NOT NULL
BEGIN
    UPDATE albums SET is_stub = 1 
    WHERE id = OLD.album_id 
      AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = OLD.album_id);
END;

-- Trigger: When track album_id is updated, clear stub on new album and mark old album if empty
CREATE TRIGGER IF NOT EXISTS trg_tracks_album_stub_upd
AFTER UPDATE OF album_id ON tracks
FOR EACH ROW
BEGIN
    UPDATE albums SET is_stub = 0 WHERE id = NEW.album_id AND is_stub = 1;
    UPDATE albums SET is_stub = 1 
    WHERE id = OLD.album_id 
      AND OLD.album_id IS NOT NULL 
      AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = OLD.album_id);
END;

-- Trigger: Auto-set is_preview on insert when duration < 30s
CREATE TRIGGER IF NOT EXISTS trg_tracks_is_preview_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.duration_ms > 0 AND NEW.duration_ms < 30000 AND (NEW.is_preview IS NULL OR NEW.is_preview = 0)
BEGIN
    UPDATE tracks SET is_preview = 1 WHERE id = NEW.id;
END;

-- Trigger: Auto-set is_preview on update of duration_ms when duration < 30s
CREATE TRIGGER IF NOT EXISTS trg_tracks_is_preview_upd
AFTER UPDATE OF duration_ms ON tracks
FOR EACH ROW
WHEN NEW.duration_ms > 0 AND NEW.duration_ms < 30000 AND (NEW.is_preview IS NULL OR NEW.is_preview = 0)
BEGIN
    UPDATE tracks SET is_preview = 1 WHERE id = NEW.id;
END;

-- Trigger: Auto-clear is_preview if duration is updated to >= 30s
CREATE TRIGGER IF NOT EXISTS trg_tracks_clear_preview_upd
AFTER UPDATE OF duration_ms ON tracks
FOR EACH ROW
WHEN (NEW.duration_ms >= 30000 OR NEW.duration_ms <= 0 OR NEW.duration_ms IS NULL) AND NEW.is_preview = 1
BEGIN
    UPDATE tracks SET is_preview = 0 WHERE id = NEW.id;
END;
