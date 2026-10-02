-- Migration 0085: Schema alignment with real code usage (post-audit phase 1, item 1.2)
--
-- ONE migration for five audit findings:
--   (a) BD-5:  albums.animated_cover_path never existed; the animated-cover flow
--              (services/animated_cover.rs) probed it with pragma_table_info and
--              skipped association on EVERY run. Column is created here; the
--              runtime pragma probe stays as a resilience guard only.
--   (b) BD-4:  library_items (0067) was never populated, so preview_migration,
--              start_migration and search_destination_tracks always saw 0 rows.
--              Backfilled from track_sources -> tracks -> track_artists -> artists
--              -> albums. Rows missing external_id/title/artist (NOT NULL columns)
--              are omitted. One row per canonical track, with id = tracks.id
--              because the consumers join `playlist_tracks.track_id = library_items.id`
--              (src-tauri/src/commands/migration.rs:185, 255 and
--              tests/migration_schema_alignment_test.rs:217-229 pin that
--              invariant). The source service chosen per track is its best
--              available source (available DESC, quality_score DESC, id ASC).
--   (c) BD-10: sync_log (0002:238) is a dead table: zero INSERTs/SELECTs in the
--              codebase, only a DELETE in the full library reset. Dropped here;
--              that DELETE is removed from library.rs in the same change.
--   (d) BD-11: columns/tables no code ever writes. Per column: populate from a
--              real data source, or drop it and clean every reader:
--        * library_snapshots.total_size_bytes / tracks_with_lyrics /
--          tracks_lossless / tracks_hires -> real sources exist
--          (downloads.file_size_bytes, lyrics.content, tracks.audio_quality
--          canonical values from 0072). create_library_snapshot
--          (commands/dashboard.rs) now writes them.
--        * library_snapshots.metadata_excellent / metadata_good /
--          metadata_needs_work -> NO data source exists (the codebase has no
--          three-tier metadata-quality classification) and no reader beyond
--          the LibrarySnapshot struct. Dropped; struct + TS interface cleaned.
--        * downloads.only_available_on / not_streaming -> no writer anywhere
--          (exclusivity is never computed by any availability check), no
--          reader beyond the Download struct. Dropped. not_streaming carries
--          an inline CHECK (0004), which blocks DROP COLUMN, so downloads is
--          rebuilt without both columns; all remaining columns, constraints,
--          FKs and indexes are preserved.
--        * downloads.musicbrainz_release_id -> real source exists:
--          albums.musicbrainz_id IS the MusicBrainz Release ID (0002:66).
--          Backfilled via downloads.track_id -> tracks.album_id and kept
--          populated for future rows by AFTER INSERT / AFTER UPDATE OF track_id
--          triggers.
--        * library_entries.play_count / auto_download -> no writer (the app has
--          no playback tracking and no per-entry auto-download flag; the
--          LibraryEntry struct was the only reader). Dropped; struct cleaned.
--        * cache_stats (0016) -> zero INSERT/UPDATE from code; the UI showed
--          permanently-zero sizes seeded by 0016 and clear_cache zeroed
--          nothing real. Table dropped; get_cache_stats/clear_cache commands,
--          CacheStats model and their UI wiring are removed in the same change.
--   (e) BD-12: trg_tracks_sync_album_total_tracks_{ins,del,upd} existed only as
--              a Rust helper (services/enrichment.rs install_album_total_tracks_triggers)
--              with no production caller, so deleting tracks desynchronized
--              albums.total_tracks. Installed here verbatim (same names and
--              semantics, incl. the TASK-138 rule that stub albums keep their
--              declared count); the dead Rust helper is removed in the same
--              change.

-- ============================================================================
-- (a) BD-5: albums.animated_cover_path
-- ============================================================================
ALTER TABLE albums ADD COLUMN animated_cover_path TEXT;

-- ============================================================================
-- (b) BD-4: backfill library_items from the real catalog
-- ============================================================================
-- BEGIN library_items_backfill
WITH best_source AS (
    SELECT ts.id, ts.track_id, ts.service_id, ts.service_track_id
    FROM track_sources ts
    WHERE ts.id = (
        SELECT ts2.id
        FROM track_sources ts2
        WHERE ts2.track_id = ts.track_id
        ORDER BY ts2.available DESC, ts2.quality_score DESC, ts2.id ASC
        LIMIT 1
    )
),
candidates AS (
    SELECT
        t.id AS track_id,
        s.name AS service_name,
        bs.service_track_id AS external_id,
        t.title AS title,
        (
            SELECT a.name
            FROM track_artists ta
            JOIN artists a ON a.id = ta.artist_id
            WHERE ta.track_id = t.id
            ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END,
                     ta.artist_id ASC
            LIMIT 1
        ) AS artist_name,
        alb.title AS album_title,
        COALESCE(t.duration_ms, 0) AS duration_ms,
        t.audio_quality AS quality
    FROM tracks t
    JOIN best_source bs ON bs.track_id = t.id
    JOIN services s ON s.id = bs.service_id
    LEFT JOIN albums alb ON alb.id = t.album_id
)
INSERT INTO library_items (id, service, source_service, item_type, external_id,
                           title, artist, album, duration_ms, quality)
SELECT
    track_id,
    service_name,
    service_name,
    'track',
    external_id,
    title,
    artist_name,
    album_title,
    duration_ms,
    quality
FROM candidates
WHERE COALESCE(TRIM(external_id), '') != ''
  AND COALESCE(TRIM(title), '') != ''
  AND COALESCE(TRIM(artist_name), '') != ''
  AND NOT EXISTS (
      SELECT 1 FROM library_items li WHERE li.id = candidates.track_id
  );
-- END library_items_backfill

-- ============================================================================
-- (c) BD-10: sync_log is a dead table
-- ============================================================================
DROP TABLE IF EXISTS sync_log;

-- ============================================================================
-- (d) BD-11 part 1: library_snapshots — drop the three metadata tiers that no
--     code computes or reads. The remaining four unwritten columns get real
--     writers in commands/dashboard.rs (same change).
-- ============================================================================
ALTER TABLE library_snapshots DROP COLUMN metadata_excellent;
ALTER TABLE library_snapshots DROP COLUMN metadata_good;
ALTER TABLE library_snapshots DROP COLUMN metadata_needs_work;

-- ============================================================================
-- (d) BD-11 part 2: downloads — drop only_available_on/not_streaming.
--     not_streaming has an inline CHECK constraint (0004_production.sql:59),
--     which SQLite forbids dropping directly, so the table is rebuilt without
--     both columns. Column list, constraints and FKs mirror 0004 + 0052 +
--     0056 + 0060 + 0062 exactly, minus the two dropped columns.
--     library_stats (0005) reads downloads, so it is dropped before the
--     rebuild and recreated verbatim afterwards — otherwise the schema
--     re-parse after DROP TABLE fails with "error in view library_stats".
-- ============================================================================
DROP VIEW IF EXISTS library_stats;

CREATE TABLE downloads_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,
    source_service_id INTEGER REFERENCES services(id),
    file_path TEXT NOT NULL,
    file_format TEXT CHECK(file_format IN ('FLAC', 'ALAC', 'WAV', 'MP3', 'AAC', 'OGG', 'OPUS') OR file_format IS NULL),
    file_size_bytes INTEGER CHECK(file_size_bytes >= 0 OR file_size_bytes IS NULL),
    file_hash TEXT,
    bit_depth INTEGER CHECK(bit_depth IN (16, 24, 32) OR bit_depth IS NULL),
    sample_rate INTEGER CHECK(sample_rate > 0 OR sample_rate IS NULL),
    metadata_completeness INTEGER DEFAULT 0 CHECK(metadata_completeness >= 0 AND metadata_completeness <= 100),
    downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
    musicbrainz_release_id TEXT,
    updated_at TEXT,
    origin_service TEXT,
    origin_service_track_id TEXT,
    effective_service TEXT,
    effective_service_track_id TEXT,
    fallback_reason TEXT,
    match_method TEXT,
    match_confidence REAL,
    file_disambiguator TEXT,
    requested_quality TEXT,
    effective_quality TEXT,
    requested_format TEXT,
    effective_format TEXT,
    quality_decision TEXT,
    provider_fallback_used INTEGER NOT NULL DEFAULT 0,
    quality_fallback_used INTEGER NOT NULL DEFAULT 0,
    decision_reason TEXT,
    skip_reason TEXT
);

INSERT INTO downloads_new (
    id, track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    musicbrainz_release_id, updated_at, origin_service, origin_service_track_id,
    effective_service, effective_service_track_id, fallback_reason, match_method,
    match_confidence, file_disambiguator, requested_quality, effective_quality,
    requested_format, effective_format, quality_decision, provider_fallback_used,
    quality_fallback_used, decision_reason, skip_reason
)
SELECT
    id, track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    musicbrainz_release_id, updated_at, origin_service, origin_service_track_id,
    effective_service, effective_service_track_id, fallback_reason, match_method,
    match_confidence, file_disambiguator, requested_quality, effective_quality,
    requested_format, effective_format, quality_decision, provider_fallback_used,
    quality_fallback_used, decision_reason, skip_reason
FROM downloads;

DROP TABLE downloads;
ALTER TABLE downloads_new RENAME TO downloads;

CREATE INDEX idx_downloads_track ON downloads(track_id);
CREATE INDEX idx_downloads_hash ON downloads(file_hash);
CREATE INDEX idx_downloads_path ON downloads(file_path);
CREATE UNIQUE INDEX idx_downloads_unique_path ON downloads(file_path);
CREATE INDEX IF NOT EXISTS idx_downloads_quality_decision ON downloads(quality_decision);
CREATE INDEX IF NOT EXISTS idx_downloads_skip_reason ON downloads(skip_reason);

-- library_stats recreated verbatim from 0005_fix_album_stats.sql.
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

-- (d) BD-11 part 3: downloads.musicbrainz_release_id — backfill from
--     albums.musicbrainz_id (the MusicBrainz Release ID) and keep it populated
--     for future rows with the same canonical source.
UPDATE downloads
SET musicbrainz_release_id = (
    SELECT a.musicbrainz_id
    FROM tracks t
    JOIN albums a ON a.id = t.album_id
    WHERE t.id = downloads.track_id
)
WHERE track_id IS NOT NULL
  AND musicbrainz_release_id IS NULL;

CREATE TRIGGER IF NOT EXISTS trg_downloads_sync_musicbrainz_release_ins
AFTER INSERT ON downloads
FOR EACH ROW
WHEN NEW.track_id IS NOT NULL AND NEW.musicbrainz_release_id IS NULL
BEGIN
    UPDATE downloads
    SET musicbrainz_release_id = (
        SELECT a.musicbrainz_id
        FROM tracks t
        JOIN albums a ON a.id = t.album_id
        WHERE t.id = NEW.track_id
    )
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_downloads_sync_musicbrainz_release_upd
AFTER UPDATE OF track_id ON downloads
FOR EACH ROW
WHEN NEW.track_id IS NOT NULL AND NEW.musicbrainz_release_id IS NULL
BEGIN
    UPDATE downloads
    SET musicbrainz_release_id = (
        SELECT a.musicbrainz_id
        FROM tracks t
        JOIN albums a ON a.id = t.album_id
        WHERE t.id = NEW.track_id
    )
    WHERE id = NEW.id;
END;

-- ============================================================================
-- (d) BD-11 part 4: library_entries — drop play_count/auto_download (no
--     playback tracking and no per-entry auto-download writer exist).
-- ============================================================================
ALTER TABLE library_entries DROP COLUMN play_count;
ALTER TABLE library_entries DROP COLUMN auto_download;

-- ============================================================================
-- (d) BD-11 part 5: cache_stats — never written from code, cleared by a command
--     that only zeroed its own rows. Dropped together with its commands.
-- ============================================================================
DROP TABLE IF EXISTS cache_stats;

-- ============================================================================
-- (e) BD-12: install trg_tracks_sync_album_total_tracks_{ins,del,upd}
--     (verbatim from services/enrichment.rs install_album_total_tracks_triggers,
--     which had no production caller). Stub albums keep their declared count
--     (TASK-138); non-stub albums always reflect the real track count, so
--     deleting tracks can no longer desynchronize total_tracks.
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_album_total_tracks_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.album_id IS NOT NULL
BEGIN
    UPDATE albums
    SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id)
    WHERE id = NEW.album_id AND (is_stub != 1 OR is_stub IS NULL);
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_album_total_tracks_del
AFTER DELETE ON tracks
FOR EACH ROW
WHEN OLD.album_id IS NOT NULL
BEGIN
    UPDATE albums
    SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id)
    WHERE id = OLD.album_id AND (is_stub != 1 OR is_stub IS NULL);
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_album_total_tracks_upd
AFTER UPDATE OF album_id ON tracks
FOR EACH ROW
BEGIN
    UPDATE albums
    SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id)
    WHERE NEW.album_id IS NOT NULL AND id = NEW.album_id AND (is_stub != 1 OR is_stub IS NULL);

    UPDATE albums
    SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id)
    WHERE OLD.album_id IS NOT NULL AND id = OLD.album_id AND (is_stub != 1 OR is_stub IS NULL);
END;
