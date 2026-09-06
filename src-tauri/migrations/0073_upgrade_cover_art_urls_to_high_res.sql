-- Migration 0073: Upgrade Tidal cover art URLs to high-resolution (1280x1280)
-- TASK-146: Replace low-resolution /320x320.jpg thumbnails with standard /1280x1280.jpg high-res artwork

-- ============================================================================
-- 1. Batch Upgrade of Existing Records
-- ============================================================================

-- Upgrade album cover artwork URLs from 320x320 thumbnail to 1280x1280 high-resolution
UPDATE albums
SET cover_art_url = REPLACE(cover_art_url, '/320x320.jpg', '/1280x1280.jpg')
WHERE cover_art_url LIKE '%/320x320.jpg%';

-- Upgrade favorites image URLs if any match 320x320
UPDATE favorites
SET image_url = REPLACE(image_url, '/320x320.jpg', '/1280x1280.jpg')
WHERE image_url LIKE '%/320x320.jpg%';

-- Upgrade playlists image URLs if any match 320x320
UPDATE playlists
SET image_url = REPLACE(image_url, '/320x320.jpg', '/1280x1280.jpg')
WHERE image_url LIKE '%/320x320.jpg%';

-- Note: The 'tracks' table does not have a cover_url column in Syncify's schema;
-- track artwork is normalized via album_id foreign key to albums(id).cover_art_url.

-- ============================================================================
-- 2. Recurrence Prevention Triggers
-- ============================================================================

-- Ensure any future insert with 320x320 thumbnail is upgraded to 1280x1280 automatically
CREATE TRIGGER IF NOT EXISTS trg_upgrade_album_cover_art_url_insert
AFTER INSERT ON albums
FOR EACH ROW
WHEN NEW.cover_art_url LIKE '%/320x320.jpg%'
BEGIN
    UPDATE albums
    SET cover_art_url = REPLACE(NEW.cover_art_url, '/320x320.jpg', '/1280x1280.jpg')
    WHERE id = NEW.id;
END;

-- Ensure any future update with 320x320 thumbnail is upgraded to 1280x1280 automatically
CREATE TRIGGER IF NOT EXISTS trg_upgrade_album_cover_art_url_update
AFTER UPDATE OF cover_art_url ON albums
FOR EACH ROW
WHEN NEW.cover_art_url LIKE '%/320x320.jpg%'
BEGIN
    UPDATE albums
    SET cover_art_url = REPLACE(NEW.cover_art_url, '/320x320.jpg', '/1280x1280.jpg')
    WHERE id = NEW.id;
END;
