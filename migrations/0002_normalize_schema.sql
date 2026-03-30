-- Syncify Unified Schema v2
-- Migration 0002: Normalize and optimize database structure
-- Combines:
--   - PROJECT_CONTEXT.md data model requirements
--   - Syncify-test library_database.py patterns (ISRC dedup, quality scoring)
--   - Additional optimizations for Syncify's orchestration needs

-- ==============================================
-- STEP 1: BACKUP OLD TABLES
-- ==============================================

ALTER TABLE imported_library RENAME TO _backup_imported_library;
ALTER TABLE downloads RENAME TO _backup_downloads;
ALTER TABLE service_accounts RENAME TO _backup_service_accounts;
ALTER TABLE service_favorites RENAME TO _backup_service_favorites;
ALTER TABLE track_availability RENAME TO _backup_track_availability;
ALTER TABLE lyrics RENAME TO _backup_lyrics;

-- Drop old indexes
DROP INDEX IF EXISTS idx_library_service;
DROP INDEX IF EXISTS idx_downloads_status;
DROP INDEX IF EXISTS idx_favorites_service;

-- ==============================================
-- STEP 2: CORE ENTITIES (Properly Normalized)
-- ==============================================

-- Services supported by Syncify
CREATE TABLE services (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,                    -- 'spotify', 'qobuz', 'tidal', etc.
    supports_download INTEGER DEFAULT 0,          -- Can we download from this service?
    max_quality TEXT,                             -- 'hires', 'lossless', 'lossy'
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- User accounts (multiple per service allowed)
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    display_name TEXT,
    email TEXT,
    is_active INTEGER DEFAULT 1,
    credentials_json TEXT,                        -- Encrypted tokens/cookies
    last_synced TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service_id, email)
);

-- Canonical artists (deduplicated)
CREATE TABLE artists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    musicbrainz_id TEXT UNIQUE,                   -- MusicBrainz Artist ID
    spotify_id TEXT,                              -- Spotify Artist ID for quick linking
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_artists_name ON artists(name);
CREATE INDEX idx_artists_mbid ON artists(musicbrainz_id);

-- Canonical albums (deduplicated)
CREATE TABLE albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    release_date TEXT,                            -- ISO date
    musicbrainz_id TEXT UNIQUE,                   -- MusicBrainz Release ID
    upc TEXT,                                     -- Universal Product Code
    total_tracks INTEGER,
    cover_art_url TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_albums_title ON albums(title);
CREATE INDEX idx_albums_mbid ON albums(musicbrainz_id);

-- Album-Artist relationship (many-to-many)
CREATE TABLE album_artists (
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    is_primary INTEGER DEFAULT 1,
    PRIMARY KEY (album_id, artist_id)
);

-- Canonical tracks (deduplicated by ISRC - the core dedup key)
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    album_id INTEGER REFERENCES albums(id) ON DELETE SET NULL,
    duration_ms INTEGER,
    track_number INTEGER,
    disc_number INTEGER DEFAULT 1,
    isrc TEXT UNIQUE,                             -- International Standard Recording Code (PRIMARY DEDUP KEY)
    musicbrainz_id TEXT,                          -- MusicBrainz Recording ID
    explicit INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_tracks_isrc ON tracks(isrc);
CREATE INDEX idx_tracks_title ON tracks(title);
CREATE INDEX idx_tracks_album ON tracks(album_id);

-- Track-Artist relationship (many-to-many with roles)
CREATE TABLE track_artists (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    role TEXT DEFAULT 'primary',                  -- 'primary', 'featured', 'composer', 'producer'
    PRIMARY KEY (track_id, artist_id, role)
);

-- ==============================================
-- STEP 3: SERVICE-SPECIFIC DATA
-- ==============================================

-- Track quality per service (where it's available and at what quality)
CREATE TABLE track_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    service_track_id TEXT NOT NULL,               -- ID on that service
    format TEXT,                                  -- 'FLAC', 'AAC', 'MP3'
    bit_depth INTEGER,                            -- 16, 24
    sample_rate INTEGER,                          -- 44100, 96000, 192000
    bitrate INTEGER,                              -- kbps for lossy
    quality_score INTEGER,                        -- Computed: higher = better
    available INTEGER DEFAULT 1,
    last_checked TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(track_id, service_id)
);
CREATE INDEX idx_track_sources_track ON track_sources(track_id);
CREATE INDEX idx_track_sources_service ON track_sources(service_id);
CREATE INDEX idx_track_sources_service_id ON track_sources(service_track_id);

-- ==============================================
-- STEP 4: USER DATA
-- ==============================================

-- User's library entries (liked/saved tracks from each account)
CREATE TABLE library_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    added_at TEXT,                                -- When user added to library
    is_liked INTEGER DEFAULT 1,                   -- Favorited?
    play_count INTEGER DEFAULT 0,
    auto_download INTEGER DEFAULT 0,              -- Auto-queue for download?
    UNIQUE(account_id, track_id)
);
CREATE INDEX idx_library_account ON library_entries(account_id);
CREATE INDEX idx_library_track ON library_entries(track_id);

-- Playlists from all accounts
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    service_playlist_id TEXT,                     -- ID on source service
    name TEXT NOT NULL,
    description TEXT,
    is_public INTEGER DEFAULT 0,
    track_count INTEGER DEFAULT 0,
    last_synced TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_playlists_account ON playlists(account_id);

-- Playlist tracks (maintains order)
CREATE TABLE playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    added_at TEXT,
    UNIQUE(playlist_id, track_id)
);
CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);

-- ==============================================
-- STEP 5: DOWNLOADS & LOCAL FILES
-- ==============================================

-- Download queue
CREATE TABLE download_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    status TEXT DEFAULT 'queued',                 -- 'queued', 'downloading', 'complete', 'failed'
    priority INTEGER DEFAULT 50,                  -- 0-100, higher = sooner
    quality_preference TEXT,                      -- 'hires', 'lossless', 'any'
    progress_percent REAL DEFAULT 0.0,
    bytes_downloaded INTEGER DEFAULT 0,
    total_bytes INTEGER,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    completed_at TEXT
);
CREATE INDEX idx_download_queue_status ON download_queue(status);

-- Downloaded files (local library)
CREATE TABLE downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,
    source_service_id INTEGER REFERENCES services(id),
    file_path TEXT NOT NULL,
    file_format TEXT,                             -- 'FLAC', 'MP3', etc.
    file_size_bytes INTEGER,
    file_hash TEXT,                               -- SHA256 for duplicate detection
    bit_depth INTEGER,
    sample_rate INTEGER,
    metadata_completeness INTEGER DEFAULT 0,      -- 0-100 score
    downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
    only_available_on TEXT,                       -- Service name if exclusive
    not_streaming INTEGER DEFAULT 0               -- True if not on any streaming service
);
CREATE INDEX idx_downloads_track ON downloads(track_id);
CREATE INDEX idx_downloads_hash ON downloads(file_hash);

-- ==============================================
-- STEP 6: LYRICS
-- ==============================================

CREATE TABLE lyrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    format TEXT NOT NULL,                         -- 'ttml', 'lrc', 'plain'
    sync_level TEXT,                              -- 'syllable', 'word', 'line', 'none'
    source TEXT,                                  -- 'apple_ttml', 'netease', 'genius'
    content TEXT NOT NULL,
    language TEXT,
    embedded_in_file INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(track_id, format)
);
CREATE INDEX idx_lyrics_track ON lyrics(track_id);

-- ==============================================
-- STEP 7: SYNC & SETTINGS
-- ==============================================

-- Sync history for incremental updates
CREATE TABLE sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    sync_type TEXT,                               -- 'favorites', 'playlists', 'full'
    tracks_added INTEGER DEFAULT 0,
    tracks_removed INTEGER DEFAULT 0,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT,                                  -- 'running', 'complete', 'failed'
    error_message TEXT
);

-- Key-value settings
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- ==============================================
-- STEP 8: SEED DEFAULT SERVICES
-- ==============================================

INSERT INTO services (name, supports_download, max_quality) VALUES
    ('spotify', 0, 'lossy'),
    ('qobuz', 1, 'hires'),
    ('tidal', 1, 'hires'),
    ('deezer', 1, 'lossless'),
    ('soundcloud', 1, 'lossless'),
    ('apple_music', 0, 'lossless');

-- ==============================================
-- STEP 9: MIGRATE DATA FROM OLD TABLES
-- ==============================================

-- Migrate service_accounts → accounts
INSERT INTO accounts (service_id, credentials_json, created_at)
SELECT 
    (SELECT id FROM services WHERE name = _backup_service_accounts.service_name),
    _backup_service_accounts.credentials_json,
    _backup_service_accounts.created_at
FROM _backup_service_accounts
WHERE EXISTS (SELECT 1 FROM services WHERE name = _backup_service_accounts.service_name);

-- Migrate imported_library → tracks (deduplicate by title+artist combo for now)
-- Note: This creates basic tracks without ISRC; real imports will add ISRC
INSERT INTO tracks (title, duration_ms, created_at)
SELECT DISTINCT title, duration_ms, created_at
FROM _backup_imported_library;

-- Create artists from old data
INSERT INTO artists (name)
SELECT DISTINCT artist 
FROM _backup_imported_library 
WHERE artist IS NOT NULL AND artist != '';

-- Create track_artists links
INSERT INTO track_artists (track_id, artist_id, role)
SELECT t.id, a.id, 'primary'
FROM _backup_imported_library oil
JOIN tracks t ON t.title = oil.title
JOIN artists a ON a.name = oil.artist
WHERE oil.artist IS NOT NULL;

-- Create track_sources from old service mappings
INSERT INTO track_sources (track_id, service_id, service_track_id)
SELECT 
    t.id,
    s.id,
    oil.service_track_id
FROM _backup_imported_library oil
JOIN tracks t ON t.title = oil.title
JOIN services s ON s.name = oil.service_name;

-- ==============================================
-- STEP 10: CLEANUP (Optional - run after verification)
-- ==============================================

-- Uncomment these after verifying migration:
-- DROP TABLE _backup_imported_library;
-- DROP TABLE _backup_downloads;
-- DROP TABLE _backup_service_accounts;
-- DROP TABLE _backup_service_favorites;
-- DROP TABLE _backup_track_availability;
-- DROP TABLE _backup_lyrics;
