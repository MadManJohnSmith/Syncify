-- Remove duplicated playlists, keeping only the most recent one
DELETE FROM playlists 
WHERE id NOT IN (
    SELECT MAX(id) 
    FROM playlists 
    GROUP BY account_id, service_playlist_id
);

-- Create a unique index to enforce the constraint that was missing
-- This will make INSERT OR REPLACE work correctly in service.rs
CREATE UNIQUE INDEX IF NOT EXISTS idx_playlists_unique 
ON playlists(account_id, service_playlist_id);
