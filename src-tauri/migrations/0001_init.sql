-- Syncify Initial Schema
-- Migration 0001: Create core tables

-- Service accounts (Spotify, Qobuz, Tidal, etc.)
CREATE TABLE IF NOT EXISTS service_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL,
    account_id TEXT,
    credentials_json TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Imported library tracks from all services
CREATE TABLE IF NOT EXISTS imported_library (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL,
    service_track_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist TEXT,
    album TEXT,
    duration_ms INTEGER,
    metadata_json TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service_name, service_track_id)
);

-- Download queue and history
CREATE TABLE IF NOT EXISTS downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER REFERENCES imported_library(id),
    service_name TEXT NOT NULL,
    service_track_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    quality TEXT,
    local_path TEXT,
    error_message TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

-- User favorites from each service
CREATE TABLE IF NOT EXISTS service_favorites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL,
    service_track_id TEXT NOT NULL,
    favorited_at TEXT,
    metadata_json TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service_name, service_track_id)
);

-- Track availability across services
CREATE TABLE IF NOT EXISTS track_availability (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER REFERENCES imported_library(id),
    service_name TEXT NOT NULL,
    service_track_id TEXT,
    available INTEGER DEFAULT 1,
    quality_info TEXT,
    checked_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Lyrics storage (LRC, TTML, plain text)
CREATE TABLE IF NOT EXISTS lyrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER REFERENCES imported_library(id),
    format TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT,
    synced INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_library_service ON imported_library(service_name);
CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
CREATE INDEX IF NOT EXISTS idx_favorites_service ON service_favorites(service_name);
