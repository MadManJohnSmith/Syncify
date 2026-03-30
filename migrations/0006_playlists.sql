-- Add playlists support
-- Migration: 0006_playlists.sql

-- Playlists table - stores playlist metadata
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    service_playlist_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    owner_name TEXT,
    is_public INTEGER DEFAULT 1,
    is_collaborative INTEGER DEFAULT 0,
    image_url TEXT,
    track_count INTEGER DEFAULT 0,
    last_synced TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account_id, service_playlist_id)
);

-- Playlist tracks junction table
CREATE TABLE IF NOT EXISTS playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    added_at TEXT,
    UNIQUE(playlist_id, track_id)
);

-- Index for faster playlist lookups
CREATE INDEX IF NOT EXISTS idx_playlists_account ON playlists(account_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track ON playlist_tracks(track_id);
