-- Migration 0045: S94 Favorites Sync & Relational Favorites Table
-- Supports multi-service favorites synchronization (Tidal, Qobuz, Spotify)

-- 1. Unified favorites table for cross-service tracking
CREATE TABLE IF NOT EXISTS favorites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
    service_id INTEGER REFERENCES services(id) ON DELETE CASCADE,
    item_type TEXT NOT NULL, -- 'track', 'album', 'artist'
    service_item_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artist_name TEXT,
    album_name TEXT,
    isrc TEXT,
    upc TEXT,
    image_url TEXT,
    favorited_at TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(account_id, item_type, service_item_id)
);

CREATE INDEX IF NOT EXISTS idx_favorites_account_type ON favorites(account_id, item_type);
CREATE INDEX IF NOT EXISTS idx_favorites_service_type ON favorites(service_id, item_type);
CREATE INDEX IF NOT EXISTS idx_favorites_service_item ON favorites(service_item_id);

-- 2. Favorites cache table for raw fast retrieval and freshness tracking
CREATE TABLE IF NOT EXISTS favorites_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL,
    item_type TEXT NOT NULL, -- 'track', 'album', 'artist'
    total_count INTEGER DEFAULT 0,
    last_synced_at TEXT DEFAULT CURRENT_TIMESTAMP,
    data_json TEXT,
    UNIQUE(service_name, item_type)
);

CREATE INDEX IF NOT EXISTS idx_favorites_cache_lookup ON favorites_cache(service_name, item_type);

-- 3. Add favorite flags and timestamps to albums and artists
ALTER TABLE albums ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE albums ADD COLUMN favorite_at TEXT;
CREATE INDEX IF NOT EXISTS idx_albums_favorite_at ON albums(is_favorite, favorite_at DESC);

ALTER TABLE artists ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE artists ADD COLUMN favorite_at TEXT;
CREATE INDEX IF NOT EXISTS idx_artists_favorite_at ON artists(is_favorite, favorite_at DESC);
