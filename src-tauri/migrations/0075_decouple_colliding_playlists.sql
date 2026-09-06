-- Migration 0075: Decouple colliding playlists in playlist_sources and playlists
-- TASK-78: Remediate collision where playlists with identical names but different
-- remote service_playlist_id were erroneously mapped to the same local playlist_id.

-- Step 1: Recreate independent entries in `playlists` for any `playlist_sources`
-- record that does not currently have its own matching (account_id, service_playlist_id) in `playlists`.
INSERT INTO playlists (
    account_id,
    service_playlist_id,
    name,
    description,
    owner_name,
    owner_id,
    is_public,
    is_collaborative,
    image_url,
    track_count,
    last_synced,
    created_at,
    updated_at,
    external_id,
    source_service
)
SELECT
    ps.account_id,
    ps.service_playlist_id,
    p.name,
    p.description,
    p.owner_name,
    p.owner_id,
    p.is_public,
    p.is_collaborative,
    p.image_url,
    0,
    COALESCE(ps.synced_at, p.last_synced),
    COALESCE(ps.synced_at, p.created_at, CURRENT_TIMESTAMP),
    CURRENT_TIMESTAMP,
    ps.service_playlist_id,
    COALESCE(p.source_service, (SELECT s.name FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.id = ps.account_id))
FROM playlist_sources ps
JOIN playlists p ON p.id = ps.playlist_id
WHERE ps.service_playlist_id IS NOT NULL
  AND TRIM(ps.service_playlist_id) != ''
  AND NOT EXISTS (
      SELECT 1 FROM playlists p2
      WHERE p2.account_id = ps.account_id
        AND p2.service_playlist_id = ps.service_playlist_id
  )
GROUP BY ps.account_id, ps.service_playlist_id;

-- Step 2: Decouple playlist_sources by pointing each record to its dedicated
-- playlist_id in `playlists` matching (account_id, service_playlist_id).
UPDATE playlist_sources
SET playlist_id = (
    SELECT p.id
    FROM playlists p
    WHERE p.account_id = playlist_sources.account_id
      AND p.service_playlist_id = playlist_sources.service_playlist_id
    LIMIT 1
)
WHERE playlist_id != (
    SELECT p.id
    FROM playlists p
    WHERE p.account_id = playlist_sources.account_id
      AND p.service_playlist_id = playlist_sources.service_playlist_id
    LIMIT 1
)
AND EXISTS (
    SELECT 1
    FROM playlists p
    WHERE p.account_id = playlist_sources.account_id
      AND p.service_playlist_id = playlist_sources.service_playlist_id
);

-- Step 3: Durable Recurrence-Prevention Triggers
-- Prevent future collisions where two distinct service_playlist_id on the same account
-- map to the same playlist_id.
CREATE TRIGGER IF NOT EXISTS trg_prevent_playlist_source_collision_ins
BEFORE INSERT ON playlist_sources
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM playlist_sources
    WHERE playlist_id = NEW.playlist_id
      AND account_id = NEW.account_id
      AND service_playlist_id != NEW.service_playlist_id
)
BEGIN
    SELECT RAISE(ABORT, 'Collision detected: playlist_id already mapped to a different service_playlist_id for this account');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_playlist_source_collision_upd
BEFORE UPDATE OF playlist_id, account_id, service_playlist_id ON playlist_sources
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM playlist_sources
    WHERE playlist_id = NEW.playlist_id
      AND account_id = NEW.account_id
      AND service_playlist_id != NEW.service_playlist_id
      AND id != NEW.id
)
BEGIN
    SELECT RAISE(ABORT, 'Collision detected: playlist_id already mapped to a different service_playlist_id for this account');
END;
