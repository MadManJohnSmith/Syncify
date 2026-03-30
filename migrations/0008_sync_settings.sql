-- 0008_sync_settings.sql
-- Sprint 1: Sync Settings & Per-Service Sync Configuration
-- Purpose: Global sync settings and per-service sync toggles

-- Global sync settings (singleton pattern with id = 1)
CREATE TABLE IF NOT EXISTS sync_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    auto_sync_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    sync_interval_value INTEGER NOT NULL DEFAULT 60,
    sync_interval_unit TEXT NOT NULL DEFAULT 'minutes',
    sync_on_startup BOOLEAN NOT NULL DEFAULT FALSE,
    background_download BOOLEAN NOT NULL DEFAULT TRUE,
    max_concurrent_downloads INTEGER NOT NULL DEFAULT 3,
    rate_limit_delay_ms INTEGER NOT NULL DEFAULT 500,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Singleton row
INSERT OR IGNORE INTO sync_settings (id) VALUES (1);

-- Per-service sync settings
CREATE TABLE IF NOT EXISTS service_sync_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL UNIQUE,
    sync_favorites BOOLEAN NOT NULL DEFAULT TRUE,
    sync_playlists BOOLEAN NOT NULL DEFAULT TRUE,
    sync_albums BOOLEAN NOT NULL DEFAULT FALSE,
    incremental_sync BOOLEAN NOT NULL DEFAULT TRUE,
    last_synced TEXT
);

-- Seed per-service sync settings
INSERT OR IGNORE INTO service_sync_settings (service_name) VALUES
    ('spotify'), ('qobuz'), ('tidal'), ('deezer'), ('soundcloud');

-- Rollback:
-- DROP TABLE IF EXISTS service_sync_settings;
-- DROP TABLE IF EXISTS sync_settings;
