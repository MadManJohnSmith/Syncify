-- Migration 0049: Unified Search Index & Performance Acceleration
-- Adds composite search indices on albums, artists, tracks, and playlists

CREATE INDEX IF NOT EXISTS idx_artists_name_search ON artists(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_albums_title_search ON albums(title COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_playlists_name_search ON playlists(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_tracks_title_search ON tracks(title COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);
CREATE INDEX IF NOT EXISTS idx_tracks_search_filters ON tracks(album_id, is_favorite, duration_ms);
