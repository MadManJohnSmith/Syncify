-- Migration 0086: keep library_items populated in continuous operation
-- (post-audit phase 4, item 4.1 — BD-4 continuation)
--
-- Phase 1 (0085) backfilled library_items from the catalog exactly once, but
-- NOTHING kept it populated afterwards (BD-4: zero writers in src-tauri), so
-- any track whose service identity (track_sources) was written after that
-- backfill never reached the migration feature — preview_migration,
-- start_migration and search_destination_track all read library_items.
--
-- Sync/import/favorites flows persist service identity into track_sources at
-- many write points (commands/favorites.rs, commands/url_import.rs,
-- commands/service.rs, commands/queue.rs, services/track_matcher.rs,
-- services/apple_music.rs, services/deezer.rs, services/qobuz.rs,
-- services/soundcloud.rs, services/spotify.rs, services/tidal.rs,
-- services/tidal_pipeline.rs, services/enrichment.rs). Rather than duplicating
-- an upsert at each of those call sites, the triggers below fire exactly at
-- them (every INSERT — including INSERT OR IGNORE and INSERT OR REPLACE —
-- every UPDATE of the identity/availability columns, and every DELETE) and
-- maintain the mirror row under the same rules as the 0085 backfill:
--   * one row per canonical track with library_items.id = tracks.id — the
--     invariant pinned by the consumers via
--     `playlist_tracks.track_id = library_items.id`
--     (tests/migration_schema_alignment_test.rs and
--     tests/schema_alignment_audit_0085_test.rs);
--   * the row always carries the track's BEST source, chosen with the
--     backfill's exact ordering (available DESC, quality_score DESC, id ASC),
--     so the mirrored (service, external_id) identity is unique per track and
--     cannot accumulate duplicates (track_sources is UNIQUE on both
--     (track_id, service_id) — 0002 — and (service_id, service_track_id) —
--     0064; the mirror row is unique by primary key);
--   * rows lacking external_id/title/artist are omitted, and the mirrored
--     statement (the track_sources write itself) still succeeds — a track
--     that gains its artist metadata later is mirrored by the next
--     identity write;
--   * when a track loses its last source (catalog dedup/repair deletes,
--     the full library reset) or is deleted outright (this includes the
--     ON DELETE CASCADE from tracks — fires with the app's
--     PRAGMA foreign_keys = ON), the mirror row is removed so no ghost rows
--     survive into migration jobs.
-- trg_tracks_sync_library_items_del covers track deletes even when foreign
-- key enforcement is off, in which case the cascade (and its trigger) never
-- runs.

-- ============================================================================
-- Shared body, expanded per trigger (NEW.track_id / OLD.track_id): upsert the
-- mirror row from the track's current best source, then drop it when the
-- track or its service identity is gone.
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_track_sources_sync_library_items_ins
AFTER INSERT ON track_sources
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
        WHERE ts.track_id = NEW.track_id
        ORDER BY ts.available DESC, ts.quality_score DESC, ts.id ASC
        LIMIT 1
    ) best
    JOIN services s ON s.id = best.service_id
    LEFT JOIN albums alb ON alb.id = t.album_id
    WHERE t.id = NEW.track_id
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

    DELETE FROM library_items
    WHERE id = NEW.track_id
      AND (NOT EXISTS (SELECT 1 FROM tracks WHERE id = NEW.track_id)
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = NEW.track_id));
END;

CREATE TRIGGER IF NOT EXISTS trg_track_sources_sync_library_items_upd
AFTER UPDATE OF track_id, service_id, service_track_id, available, quality_score ON track_sources
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
        WHERE ts.track_id = NEW.track_id
        ORDER BY ts.available DESC, ts.quality_score DESC, ts.id ASC
        LIMIT 1
    ) best
    JOIN services s ON s.id = best.service_id
    LEFT JOIN albums alb ON alb.id = t.album_id
    WHERE t.id = NEW.track_id
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

    DELETE FROM library_items
    WHERE id = NEW.track_id
      AND (NOT EXISTS (SELECT 1 FROM tracks WHERE id = NEW.track_id)
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = NEW.track_id));

    -- The identity moved away from OLD.track_id (catalog merges reassign
    -- track_sources.track_id): recompute its mirror row from the remaining
    -- sources, then drop it if nothing is left.
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
        WHERE ts.track_id = OLD.track_id
        ORDER BY ts.available DESC, ts.quality_score DESC, ts.id ASC
        LIMIT 1
    ) best
    JOIN services s ON s.id = best.service_id
    LEFT JOIN albums alb ON alb.id = t.album_id
    WHERE t.id = OLD.track_id
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

    DELETE FROM library_items
    WHERE id = OLD.track_id
      AND (NOT EXISTS (SELECT 1 FROM tracks WHERE id = OLD.track_id)
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = OLD.track_id));
END;

CREATE TRIGGER IF NOT EXISTS trg_track_sources_sync_library_items_del
AFTER DELETE ON track_sources
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
        WHERE ts.track_id = OLD.track_id
        ORDER BY ts.available DESC, ts.quality_score DESC, ts.id ASC
        LIMIT 1
    ) best
    JOIN services s ON s.id = best.service_id
    LEFT JOIN albums alb ON alb.id = t.album_id
    WHERE t.id = OLD.track_id
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

    DELETE FROM library_items
    WHERE id = OLD.track_id
      AND (NOT EXISTS (SELECT 1 FROM tracks WHERE id = OLD.track_id)
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = OLD.track_id));
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_library_items_del
AFTER DELETE ON tracks
FOR EACH ROW
BEGIN
    DELETE FROM library_items WHERE id = OLD.id;
END;
