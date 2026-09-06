-- Migration 0067: Schema Alignment for Migration Commands
-- Aligns SQLite schema with migration.rs requirements:
-- 1. Creates `library_items` table and indexes queried by preview_migration, start_migration, and search_destination_tracks.
-- 2. Adds `dest_track_id` column to `migration_items` with synchronization to `destination_track_id`.
-- 3. Adds `external_id` and `source_service` columns to `playlists` for playlist-based migration lookups.
-- 4. Adds `credentials` and `service_name` columns to `accounts` for service account lookups in migration workflows.

-- ============================================================================
-- 1. Table: library_items
-- ============================================================================
CREATE TABLE IF NOT EXISTS library_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service TEXT,
    source_service TEXT,
    item_type TEXT NOT NULL DEFAULT 'track',
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    album TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    quality TEXT,
    raw_json TEXT,
    synced_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_library_items_source_service ON library_items(source_service);
CREATE INDEX IF NOT EXISTS idx_library_items_service ON library_items(service);
CREATE INDEX IF NOT EXISTS idx_library_items_external_id ON library_items(external_id);
CREATE INDEX IF NOT EXISTS idx_library_items_title ON library_items(title);
CREATE INDEX IF NOT EXISTS idx_library_items_artist ON library_items(artist);

CREATE TRIGGER IF NOT EXISTS trg_library_items_sync_service_ins
AFTER INSERT ON library_items
FOR EACH ROW
WHEN (NEW.source_service IS NULL AND NEW.service IS NOT NULL)
  OR (NEW.service IS NULL AND NEW.source_service IS NOT NULL)
BEGIN
    UPDATE library_items
    SET source_service = COALESCE(NEW.source_service, NEW.service),
        service = COALESCE(NEW.service, NEW.source_service)
    WHERE id = NEW.id;
END;

-- ============================================================================
-- 2. Table: migration_items - add dest_track_id and synchronize
-- ============================================================================
ALTER TABLE migration_items ADD COLUMN dest_track_id TEXT;

UPDATE migration_items
SET dest_track_id = destination_track_id
WHERE dest_track_id IS NULL AND destination_track_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_migration_items_dest_track_id ON migration_items(dest_track_id);

CREATE TRIGGER IF NOT EXISTS trg_migration_items_sync_dest_id_ins
AFTER INSERT ON migration_items
FOR EACH ROW
WHEN (NEW.dest_track_id IS NOT NULL AND NEW.destination_track_id IS NULL)
  OR (NEW.destination_track_id IS NOT NULL AND NEW.dest_track_id IS NULL)
BEGIN
    UPDATE migration_items
    SET dest_track_id = COALESCE(NEW.dest_track_id, NEW.destination_track_id),
        destination_track_id = COALESCE(NEW.destination_track_id, NEW.dest_track_id)
    WHERE id = NEW.id;
END;

-- ============================================================================
-- 3. Table: playlists - add external_id and source_service
-- ============================================================================
ALTER TABLE playlists ADD COLUMN external_id TEXT;
ALTER TABLE playlists ADD COLUMN source_service TEXT;

UPDATE playlists
SET external_id = service_playlist_id
WHERE external_id IS NULL AND service_playlist_id IS NOT NULL;

UPDATE playlists
SET source_service = (
    SELECT s.name
    FROM accounts a
    JOIN services s ON s.id = a.service_id
    WHERE a.id = playlists.account_id
)
WHERE source_service IS NULL AND account_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_playlists_external_id ON playlists(external_id);
CREATE INDEX IF NOT EXISTS idx_playlists_source_service ON playlists(source_service);

CREATE TRIGGER IF NOT EXISTS trg_playlists_sync_ins
AFTER INSERT ON playlists
FOR EACH ROW
BEGIN
    UPDATE playlists
    SET external_id = COALESCE(NEW.external_id, NEW.service_playlist_id),
        service_playlist_id = COALESCE(NEW.service_playlist_id, NEW.external_id),
        source_service = COALESCE(
            NEW.source_service,
            (SELECT s.name FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.id = NEW.account_id)
        )
    WHERE id = NEW.id
      AND (
          (NEW.external_id IS NULL AND NEW.service_playlist_id IS NOT NULL)
          OR (NEW.service_playlist_id IS NULL AND NEW.external_id IS NOT NULL)
          OR (NEW.source_service IS NULL AND NEW.account_id IS NOT NULL)
      );
END;

-- ============================================================================
-- 4. Table: accounts - add credentials and service_name
-- ============================================================================
ALTER TABLE accounts ADD COLUMN credentials TEXT;
ALTER TABLE accounts ADD COLUMN service_name TEXT;

UPDATE accounts
SET credentials = credentials_json
WHERE credentials IS NULL AND credentials_json IS NOT NULL;

UPDATE accounts
SET service_name = (
    SELECT name FROM services WHERE services.id = accounts.service_id
)
WHERE service_name IS NULL AND service_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_accounts_service_name ON accounts(service_name);

CREATE TRIGGER IF NOT EXISTS trg_accounts_sync_creds_ins
AFTER INSERT ON accounts
FOR EACH ROW
BEGIN
    UPDATE accounts
    SET credentials = COALESCE(NEW.credentials, NEW.credentials_json),
        credentials_json = COALESCE(NEW.credentials_json, NEW.credentials),
        service_name = COALESCE(NEW.service_name, (SELECT name FROM services WHERE services.id = NEW.service_id))
    WHERE id = NEW.id
      AND (
          (NEW.credentials IS NULL AND NEW.credentials_json IS NOT NULL)
          OR (NEW.credentials_json IS NULL AND NEW.credentials IS NOT NULL)
          OR (NEW.service_name IS NULL AND NEW.service_id IS NOT NULL)
      );
END;
