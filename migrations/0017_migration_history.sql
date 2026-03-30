-- Migration: 0017_migration_history.sql
-- Sprint 6: Migration history tracking and templates

-- Migration Jobs Table
CREATE TABLE IF NOT EXISTS migration_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    source_service TEXT NOT NULL,
    destination_service TEXT NOT NULL,
    source_playlist_ids TEXT, -- JSON array of playlist IDs
    options TEXT NOT NULL, -- JSON MigrationOptions
    status TEXT NOT NULL DEFAULT 'pending', -- pending, running, completed, failed, cancelled
    total_items INTEGER NOT NULL DEFAULT 0,
    completed_items INTEGER NOT NULL DEFAULT 0,
    failed_items INTEGER NOT NULL DEFAULT 0,
    skipped_items INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME,
    completed_at DATETIME,
    error_message TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Migration Items Table (individual tracks being migrated)
CREATE TABLE IF NOT EXISTS migration_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES migration_jobs(id) ON DELETE CASCADE,
    source_track_id TEXT NOT NULL,
    source_track_title TEXT NOT NULL,
    source_track_artist TEXT NOT NULL,
    source_track_album TEXT,
    source_playlist_id TEXT,
    source_playlist_name TEXT,
    destination_track_id TEXT,
    match_confidence REAL,
    match_method TEXT, -- isrc, fingerprint, metadata, manual
    status TEXT NOT NULL DEFAULT 'pending', -- pending, matched, transferred, failed, skipped
    error_message TEXT,
    processed_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Migration Templates Table
CREATE TABLE IF NOT EXISTS migration_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    source_service TEXT NOT NULL,
    destination_service TEXT NOT NULL,
    options TEXT NOT NULL, -- JSON MigrationOptions
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_migration_jobs_status ON migration_jobs(status);
CREATE INDEX IF NOT EXISTS idx_migration_jobs_services ON migration_jobs(source_service, destination_service);
CREATE INDEX IF NOT EXISTS idx_migration_items_job_id ON migration_items(job_id);
CREATE INDEX IF NOT EXISTS idx_migration_items_status ON migration_items(status);
CREATE INDEX IF NOT EXISTS idx_migration_items_job_status ON migration_items(job_id, status);

-- Insert some default templates
INSERT OR IGNORE INTO migration_templates (name, description, source_service, destination_service, options)
VALUES 
    ('Spotify to Qobuz (Best Match)', 'Migrate Spotify playlists to Qobuz with strict matching', 'spotify', 'qobuz', 
     '{"match_threshold":0.85,"skip_unmatched":true,"create_playlists":true,"merge_existing":false,"download_matched":true}'),
    ('Tidal to Qobuz (Lenient)', 'Migrate Tidal library with lenient matching', 'tidal', 'qobuz',
     '{"match_threshold":0.70,"skip_unmatched":false,"create_playlists":true,"merge_existing":true,"download_matched":true}'),
    ('Spotify to Tidal', 'Direct Spotify to Tidal migration', 'spotify', 'tidal',
     '{"match_threshold":0.80,"skip_unmatched":true,"create_playlists":true,"merge_existing":false,"download_matched":false}');
