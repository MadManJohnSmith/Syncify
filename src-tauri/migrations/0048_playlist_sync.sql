-- Migration 0048: Playlist Sync & Cross-Service Sources
-- Adds playlist_sources table and indexing for bidirectional multi-service playlist sync

CREATE TABLE IF NOT EXISTS playlist_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    service_playlist_id TEXT NOT NULL,
    synced_at TEXT DEFAULT CURRENT_TIMESTAMP,
    snapshot_id TEXT,
    UNIQUE(account_id, service_playlist_id)
);

CREATE INDEX IF NOT EXISTS idx_playlist_sources_playlist ON playlist_sources(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_sources_lookup ON playlist_sources(service_id, service_playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_pos ON playlist_tracks(playlist_id, position);
