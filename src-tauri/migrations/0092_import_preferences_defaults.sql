-- C-08: Defaults for NEW service synchronization rows. SQLite cannot ALTER COLUMN
-- DEFAULT, so reconstruct the table. Never infer user intent from current flags:
-- 0066 set updated_at on every existing row, even untouched seed rows.
-- Preserve all existing rows, IDs, timestamps and the AUTOINCREMENT high-water
-- mark; only future rows receive the requested default scope.
-- The preceding schema has no dependent foreign keys, triggers or custom indexes.

CREATE TEMP TABLE sync_settings_previous_sequence (seq INTEGER NOT NULL);
INSERT INTO sync_settings_previous_sequence (seq)
SELECT seq FROM sqlite_sequence WHERE name = 'service_sync_settings';

CREATE TABLE service_sync_settings_rebuilt (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL UNIQUE,
    sync_favorites BOOLEAN NOT NULL DEFAULT 1,
    sync_playlists BOOLEAN NOT NULL DEFAULT 1,
    sync_albums BOOLEAN NOT NULL DEFAULT 1,
    incremental_sync BOOLEAN NOT NULL DEFAULT 1,
    last_synced TEXT,
    sync_favorite_artists BOOLEAN NOT NULL DEFAULT 1,
    sync_purchases BOOLEAN NOT NULL DEFAULT 0,
    sync_library_history BOOLEAN NOT NULL DEFAULT 0,
    sync_include_appearances BOOLEAN NOT NULL DEFAULT 0,
    updated_at TEXT
);

INSERT INTO service_sync_settings_rebuilt (
    id, service_name, sync_favorites, sync_playlists, sync_albums,
    incremental_sync, last_synced, sync_favorite_artists, sync_purchases,
    sync_library_history, sync_include_appearances, updated_at
)
SELECT id, service_name, sync_favorites, sync_playlists, sync_albums,
       incremental_sync, last_synced, sync_favorite_artists, sync_purchases,
       sync_library_history, sync_include_appearances, updated_at
FROM service_sync_settings;

DROP TABLE service_sync_settings;
ALTER TABLE service_sync_settings_rebuilt RENAME TO service_sync_settings;
UPDATE sqlite_sequence
SET seq = MAX(seq, (SELECT seq FROM sync_settings_previous_sequence))
WHERE name = 'service_sync_settings';
DROP TABLE sync_settings_previous_sequence;

-- Apply C-08 default toggles to seed rows that were never synced
UPDATE service_sync_settings
SET sync_albums = 1, sync_favorite_artists = 1
WHERE last_synced IS NULL;
