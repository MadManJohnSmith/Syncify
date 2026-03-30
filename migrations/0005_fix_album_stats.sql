-- Syncify Migration 0005: Fix Album Count in Stats
-- The albums table wasn't being populated by imports.
-- This migration fixes the library_stats VIEW to count unique albums from existing data.

-- Drop the old view
DROP VIEW IF EXISTS library_stats;

-- Recreate with improved album counting
-- Since albums table is empty, we use a workaround:
-- Count distinct album entries from track_sources via service APIs
-- For now, track the albums table count (will be 0) plus add proper logic

CREATE VIEW library_stats AS 
SELECT 
    (SELECT COUNT(*) FROM tracks) as total_tracks,
    (SELECT COUNT(*) FROM artists) as total_artists,
    -- Albums: use the albums table if populated, otherwise count distinct album references
    (SELECT CASE 
        WHEN (SELECT COUNT(*) FROM albums) > 0 THEN (SELECT COUNT(*) FROM albums)
        ELSE (SELECT COUNT(DISTINCT album_id) FROM tracks WHERE album_id IS NOT NULL)
     END) as total_albums,
    (SELECT COUNT(*) FROM downloads) as total_downloads,
    (SELECT COUNT(*) FROM download_queue WHERE status = 'queued') as queued_downloads,
    (SELECT COUNT(*) FROM download_queue WHERE status = 'downloading') as active_downloads,
    (SELECT COUNT(*) FROM library_entries) as library_entries,
    (SELECT COUNT(*) FROM playlists) as playlists,
    (SELECT COUNT(DISTINCT service_id) FROM track_sources) as services_with_data;
