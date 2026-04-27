-- 0015_historical_snapshots.sql
-- Sprint 4: Library snapshots for tracking growth over time

CREATE TABLE IF NOT EXISTS library_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_date TEXT NOT NULL DEFAULT (date('now')),
    total_tracks INTEGER NOT NULL DEFAULT 0,
    total_albums INTEGER NOT NULL DEFAULT 0,
    total_artists INTEGER NOT NULL DEFAULT 0,
    total_size_bytes INTEGER NOT NULL DEFAULT 0,
    tracks_with_lyrics INTEGER NOT NULL DEFAULT 0,
    tracks_lossless INTEGER NOT NULL DEFAULT 0,
    tracks_hires INTEGER NOT NULL DEFAULT 0,
    metadata_excellent INTEGER NOT NULL DEFAULT 0,
    metadata_good INTEGER NOT NULL DEFAULT 0,
    metadata_needs_work INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Unique constraint to prevent multiple snapshots per day
CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshots_date ON library_snapshots(snapshot_date);

-- Service health check cache table
CREATE TABLE IF NOT EXISTS service_health_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL UNIQUE,
    is_connected INTEGER NOT NULL DEFAULT 0,
    token_valid INTEGER NOT NULL DEFAULT 0,
    token_expires_at TEXT,
    last_checked TEXT NOT NULL DEFAULT (datetime('now')),
    error_message TEXT,
    rate_limit_remaining INTEGER,
    rate_limit_reset_at TEXT
);

-- Seed initial service health entries
INSERT OR IGNORE INTO service_health_cache (service_name) VALUES
    ('spotify'),
    ('qobuz'),
    ('tidal'),
    ('deezer'),
    ('soundcloud'),
    ('apple_music');

-- Rollback:
-- DROP INDEX IF EXISTS idx_snapshots_date;
-- DROP TABLE IF EXISTS library_snapshots;
-- DROP TABLE IF EXISTS service_health_cache;
