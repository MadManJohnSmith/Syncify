-- Migration 0087: keep library_items populated when the artist link lands
-- (post-audit phase 4, item 4.1 — BD-4 continuation)
--
-- 0086 mirrors every track whose service identity is written to
-- `track_sources`, but it can only mirror a track that is already complete:
-- `library_items.artist` is NOT NULL (0067), so a track without a
-- `track_artists` row is skipped. That is not a transient state in the
-- import flows:
--
--   * services/spotify.rs:1238-1278 links the artists with a bounded retry
--     and, when the retries are exhausted, logs the failure and writes the
--     `track_sources` identity anyway — the track lands in the catalog
--     without an artist;
--   * the re-syncs that would heal it are `INSERT OR IGNORE` writes
--     (commands/favorites.rs:979): the identity row already exists, so the
--     statement changes nothing and no 0086 trigger fires;
--
-- so such a track never reaches library_items and the migration feature
-- (preview_migration / start_migration / search_destination_track) cannot
-- see it, no matter how many syncs run.
--
-- These triggers mirror the second write point of a track's metadata — the
-- artist link — under the exact rules of the 0085 backfill and of 0086:
--   * the mirror row appears as soon as the track is complete, with no
--     further identity write required;
--   * re-linking artists refreshes the mirrored attribution (the enrichment
--     flow at commands/metadata.rs:202-208 deletes and re-inserts
--     `track_artists`);
--   * a track that loses its last artist link loses its mirror row too, so
--     library_items never advertises an artist that is no longer there.
--
-- The body is 0086's, expanded per trigger for the affected track: upsert the
-- mirror row from the track's current best source
-- (available DESC, quality_score DESC, id ASC), then drop it when the track,
-- its service identity or its artist link is gone.
--
-- The writes that dominate the sync cost nothing extra: in every import flow
-- `track_artists` is written BEFORE `track_sources`, so the upsert below
-- selects no best source and inserts nothing.

-- ============================================================================
-- AFTER INSERT: the track just became complete.
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_track_artists_sync_library_items_ins
AFTER INSERT ON track_artists
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
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = NEW.track_id)
        OR NOT EXISTS (SELECT 1
                       FROM track_artists ta3
                       JOIN artists a3 ON a3.id = ta3.artist_id
                       WHERE ta3.track_id = NEW.track_id
                         AND COALESCE(TRIM(a3.name), '') != ''));
END;

-- ============================================================================
-- AFTER UPDATE: the link moved between tracks and/or changed artist or role,
-- so both the old and the new track are re-mirrored (the artist the row now
-- reports is the one picked by the backfill ordering).
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_track_artists_sync_library_items_upd
AFTER UPDATE OF track_id, artist_id, role ON track_artists
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
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = NEW.track_id)
        OR NOT EXISTS (SELECT 1
                       FROM track_artists ta3
                       JOIN artists a3 ON a3.id = ta3.artist_id
                       WHERE ta3.track_id = NEW.track_id
                         AND COALESCE(TRIM(a3.name), '') != ''));

    -- The link left OLD.track_id: recompute its mirror from what is left.
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
      AND OLD.track_id != NEW.track_id
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
      AND OLD.track_id != NEW.track_id
      AND (NOT EXISTS (SELECT 1 FROM tracks WHERE id = OLD.track_id)
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = OLD.track_id)
        OR NOT EXISTS (SELECT 1
                       FROM track_artists ta3
                       JOIN artists a3 ON a3.id = ta3.artist_id
                       WHERE ta3.track_id = OLD.track_id
                         AND COALESCE(TRIM(a3.name), '') != ''));
END;

-- ============================================================================
-- AFTER DELETE: the track may have lost its last artist link.
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_track_artists_sync_library_items_del
AFTER DELETE ON track_artists
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
        OR NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = OLD.track_id)
        OR NOT EXISTS (SELECT 1
                       FROM track_artists ta3
                       JOIN artists a3 ON a3.id = ta3.artist_id
                       WHERE ta3.track_id = OLD.track_id
                         AND COALESCE(TRIM(a3.name), '') != ''));
END;