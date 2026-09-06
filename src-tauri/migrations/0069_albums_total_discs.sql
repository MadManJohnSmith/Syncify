-- Migration 0069: Add total_discs column to albums table and backfill from tracks
-- TASK-139: Support DISCTOTAL / TPOS multidisc album tagging

ALTER TABLE albums ADD COLUMN total_discs INTEGER;

-- Backfill total_discs from existing tracks linked to each album
UPDATE albums
SET total_discs = (
    SELECT MAX(COALESCE(disc_number, 1))
    FROM tracks
    WHERE tracks.album_id = albums.id
)
WHERE total_discs IS NULL;

-- Durable recurrence-prevention triggers to keep total_discs in sync when tracks are inserted or disc_number updated
CREATE TRIGGER IF NOT EXISTS trg_albums_total_discs_after_track_insert
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.album_id IS NOT NULL AND NEW.disc_number IS NOT NULL
BEGIN
    UPDATE albums
    SET total_discs = MAX(COALESCE(total_discs, 1), NEW.disc_number)
    WHERE id = NEW.album_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_albums_total_discs_after_track_update
AFTER UPDATE OF album_id, disc_number ON tracks
FOR EACH ROW
WHEN NEW.album_id IS NOT NULL AND NEW.disc_number IS NOT NULL
BEGIN
    UPDATE albums
    SET total_discs = MAX(COALESCE(total_discs, 1), NEW.disc_number)
    WHERE id = NEW.album_id;
END;
