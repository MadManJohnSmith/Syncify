-- Migration 0088: library_items follows catalog edits + albums keep their
-- declared release total
--
-- (a) BD-2 — the mirror went stale when the catalog row itself changed.
-- 0086/0087 mirror a track when its service identity or its artist link is
-- written, but nothing mirrored the *edits* of an already-mirrored track: the
-- corrections flows (commands/metadata.rs) rewrite tracks.title,
-- tracks.album_id, tracks.duration_ms, tracks.audio_quality and albums.title
-- and left the mirror with the values the track had at import time. Both
-- migration consumers read the mirror — preview_migration /
-- start_migration (commands/migration.rs) and search_destination_track — so a
-- renamed track was previewed, matched and transferred under its stale title,
-- and a track moved to another album kept the old album name.
--
-- New triggers:
--   * trg_tracks_sync_library_items_upd: re-runs the 0086 upsert body for the
--     edited track under the same rules (best source by available DESC,
--     quality_score DESC, id ASC; rows without external_id/title/artist are
--     skipped; a track that stops qualifying loses its mirror row);
--   * trg_albums_sync_library_items_upd: republishes the album title of every
--     track linked to the renamed album.
--
-- (b) BD-5 — the declared release total is not the number of imported tracks.
-- The 0085 triggers overwrote albums.total_tracks with COUNT(*) on every
-- insert/delete/update of a track row, so importing one track of a 20-track
-- release left the album advertising 1 of 20 (services/spotify.rs writes the
-- release total into the album upsert before the tracks are inserted, so the
-- triggers always won).
--
-- albums.total_tracks now keeps meaning "the total the release declares" and
-- albums.local_track_count carries "how many of them are imported here" — the
-- two values BD-5 asks to separate. A third column, declared_total_tracks,
-- records which of the two the album really declares (0 = never declared, so
-- total_tracks stays derived), because the importers only ever write
-- albums.total_tracks and a total that disagrees with the local count is what
-- tells a declaration from a derivation. Albums that do declare a release
-- total keep it through partial imports, exactly like the merge path already
-- does (commands/library.rs:3429-3437: "a declared count still describes the
-- album and must survive").
--
-- The documented stub rule (TASK-138: a stub keeps its declared count) is
-- unchanged: total_tracks is never derived for is_stub = 1, and stubs get a
-- truthful local_track_count like everybody else.

-- ============================================================================
-- (a) BD-2: mirror catalog edits into library_items
-- ============================================================================
-- `IS NOT`, not `IS NOT NEW.title`: SQLite evaluates the NULL-safe comparison
-- of an expression with itself as always false, so that form silently disabled
-- the trigger.
CREATE TRIGGER IF NOT EXISTS trg_albums_sync_library_items_upd
AFTER UPDATE OF title ON albums
FOR EACH ROW
WHEN NEW.title IS NOT OLD.title
BEGIN
    UPDATE library_items
    SET album = NEW.title,
        updated_at = CURRENT_TIMESTAMP
    WHERE id IN (SELECT id FROM tracks WHERE album_id = NEW.id);
END;

DROP TRIGGER IF EXISTS trg_tracks_sync_library_items_upd;

CREATE TRIGGER trg_tracks_sync_library_items_upd
AFTER UPDATE OF title, album_id, duration_ms, audio_quality ON tracks
FOR EACH ROW
BEGIN
    INSERT INTO library_items (id, service, source_service, item_type, external_id,
                               title, artist, album, duration_ms, quality)
    SELECT
        t.id,
        s.name,
        s.name,
        'track',
        best.service_track_id,
        t.title,
        (
            SELECT a.name
            FROM track_artists ta
            JOIN artists a ON a.id = ta.artist_id
            WHERE ta.track_id = t.id
            ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END,
                     ta.artist_id ASC
            LIMIT 1
        ),
        alb.title,
        COALESCE(t.duration_ms, 0),
        t.audio_quality
    FROM tracks t
    JOIN (
        SELECT ts.service_track_id, ts.service_id
        FROM track_sources ts
        WHERE ts.track_id = NEW.id
        ORDER BY ts.available DESC, ts.quality_score DESC, ts.id ASC
        LIMIT 1
    ) best
    JOIN services s ON s.id = best.service_id
    LEFT JOIN albums alb ON alb.id = t.album_id
    WHERE t.id = NEW.id
      AND COALESCE(TRIM(best.service_track_id), '') != ''
      AND COALESCE(TRIM(t.title), '') != ''
      AND EXISTS (
          SELECT 1
          FROM track_artists ta2
          JOIN artists a2 ON a2.id = ta2.artist_id
          WHERE ta2.track_id = t.id AND COALESCE(TRIM(a2.name), '') != ''
      )
    ON CONFLICT(id) DO UPDATE SET
        service = excluded.service,
        source_service = excluded.source_service,
        item_type = excluded.item_type,
        external_id = excluded.external_id,
        title = excluded.title,
        artist = excluded.artist,
        album = excluded.album,
        duration_ms = excluded.duration_ms,
        quality = excluded.quality,
        synced_at = CURRENT_TIMESTAMP,
        updated_at = CURRENT_TIMESTAMP;

    -- Same qualification the insert above applies: a track that no longer has
    -- a usable identity or title loses its mirror row.
    DELETE FROM library_items
    WHERE id = NEW.id
      AND (COALESCE(TRIM((SELECT title FROM tracks WHERE id = NEW.id)), '') = ''
        OR COALESCE(TRIM((SELECT service_track_id
                          FROM track_sources
                          WHERE track_id = NEW.id
                          ORDER BY available DESC, quality_score DESC, id ASC
                          LIMIT 1)), '') = ''
        OR NOT EXISTS (
            SELECT 1
            FROM track_artists ta
            JOIN artists a ON a.id = ta.artist_id
            WHERE ta.track_id = NEW.id AND COALESCE(TRIM(a.name), '') != ''
        ));
END;

-- ============================================================================
-- (b) BD-5: declared release total vs imported track count
-- ============================================================================
-- Two columns instead of one overloaded one:
--   declared_total_tracks — the release total as the service publishes it.
--       0 means "never declared", and then total_tracks is derived.
--   local_track_count      — how many of those tracks are imported here.
-- total_tracks keeps the value the rest of the app reads: the declared total
-- when the album declares one, the local count when it does not.
--
-- The importers write the release total through albums.total_tracks and no
-- importer can be made to write a second column, so the declaration is
-- captured there: an album row whose total_tracks differs from its local count
-- was given a number by a service, not derived by a trigger.
ALTER TABLE albums ADD COLUMN declared_total_tracks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE albums ADD COLUMN local_track_count INTEGER NOT NULL DEFAULT 0;

UPDATE albums
SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id);

-- Rows written before this migration: a total that disagrees with the local
-- count could only come from the service metadata (the 0085 triggers kept them
-- equal), so that is the declaration to keep.
UPDATE albums
SET declared_total_tracks = total_tracks
WHERE total_tracks IS NOT NULL
  AND total_tracks > 0
  AND total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id);

CREATE INDEX IF NOT EXISTS idx_albums_local_track_count ON albums(local_track_count);

CREATE TRIGGER IF NOT EXISTS trg_albums_capture_declared_total_tracks_ins
AFTER INSERT ON albums
FOR EACH ROW
WHEN NEW.total_tracks IS NOT NULL
  AND NEW.total_tracks > 0
  AND NEW.total_tracks != NEW.local_track_count
BEGIN
    UPDATE albums
    SET declared_total_tracks = NEW.total_tracks
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_albums_capture_declared_total_tracks_upd
AFTER UPDATE OF total_tracks ON albums
FOR EACH ROW
WHEN NEW.total_tracks IS NOT NULL
  AND NEW.total_tracks > 0
  AND NEW.total_tracks != NEW.local_track_count
  AND (OLD.declared_total_tracks = 0 OR NEW.total_tracks != OLD.declared_total_tracks)
BEGIN
    UPDATE albums
    SET declared_total_tracks = NEW.total_tracks
    WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS trg_tracks_sync_album_total_tracks_ins;
DROP TRIGGER IF EXISTS trg_tracks_sync_album_total_tracks_del;
DROP TRIGGER IF EXISTS trg_tracks_sync_album_total_tracks_upd;

CREATE TRIGGER trg_tracks_sync_album_total_tracks_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.album_id IS NOT NULL
BEGIN
    UPDATE albums
    SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id)
    WHERE id = NEW.album_id;

    UPDATE albums
    SET total_tracks = CASE WHEN declared_total_tracks > 0
                            THEN declared_total_tracks
                            ELSE (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id) END
    WHERE id = NEW.album_id
      AND (is_stub != 1 OR is_stub IS NULL);
END;

CREATE TRIGGER trg_tracks_sync_album_total_tracks_del
AFTER DELETE ON tracks
FOR EACH ROW
WHEN OLD.album_id IS NOT NULL
BEGIN
    UPDATE albums
    SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id)
    WHERE id = OLD.album_id;

    UPDATE albums
    SET total_tracks = CASE WHEN declared_total_tracks > 0
                            THEN declared_total_tracks
                            ELSE (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id) END
    WHERE id = OLD.album_id
      AND (is_stub != 1 OR is_stub IS NULL);
END;

CREATE TRIGGER trg_tracks_sync_album_total_tracks_upd
AFTER UPDATE OF album_id ON tracks
FOR EACH ROW
BEGIN
    UPDATE albums
    SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id)
    WHERE NEW.album_id IS NOT NULL AND id = NEW.album_id;

    UPDATE albums
    SET total_tracks = CASE WHEN declared_total_tracks > 0
                            THEN declared_total_tracks
                            ELSE (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id) END
    WHERE NEW.album_id IS NOT NULL
      AND id = NEW.album_id
      AND (is_stub != 1 OR is_stub IS NULL);

    UPDATE albums
    SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id)
    WHERE OLD.album_id IS NOT NULL AND id = OLD.album_id;

    UPDATE albums
    SET total_tracks = CASE WHEN declared_total_tracks > 0
                            THEN declared_total_tracks
                            ELSE (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id) END
    WHERE OLD.album_id IS NOT NULL
      AND id = OLD.album_id
      AND (is_stub != 1 OR is_stub IS NULL);
END;
