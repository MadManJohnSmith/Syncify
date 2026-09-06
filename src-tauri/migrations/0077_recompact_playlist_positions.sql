-- Migration 0077: Recompact Playlist Track Positions and Reconcile Track Count
-- TASK-79: Recompact playlist positions to strictly 1-indexed, sequential, and gap-free (1, 2, 3... N).
--          Atomically reconcile playlists.track_count to match exact track count in playlist_tracks.

-- 1. Create a temporary staging table with PRIMARY KEY on id for O(1) indexed lookups
--    ordered by existing position ASC, and added_at ASC, id ASC as deterministic tie-breakers.
DROP TABLE IF EXISTS _playlist_tracks_recompact;
CREATE TEMP TABLE _playlist_tracks_recompact (
    id INTEGER PRIMARY KEY,
    new_pos INTEGER NOT NULL
);

INSERT INTO _playlist_tracks_recompact (id, new_pos)
SELECT
    id,
    ROW_NUMBER() OVER (
        PARTITION BY playlist_id
        ORDER BY position ASC, added_at ASC, id ASC
    )
FROM playlist_tracks;

-- 2. Stage existing positions to unique negative values to avoid UNIQUE(playlist_id, position) collisions.
UPDATE playlist_tracks
SET position = -(id + 1);

-- 3. Reassign positions to strictly 1-indexed canonical values from staging table.
UPDATE playlist_tracks
SET position = (
    SELECT r.new_pos
    FROM _playlist_tracks_recompact r
    WHERE r.id = playlist_tracks.id
);

-- 4. Clean up temporary table.
DROP TABLE IF EXISTS _playlist_tracks_recompact;

-- 5. Atomically reconcile playlists.track_count to match exact COUNT(*) from playlist_tracks.
UPDATE playlists
SET track_count = (
    SELECT COUNT(*)
    FROM playlist_tracks
    WHERE playlist_tracks.playlist_id = playlists.id
);
