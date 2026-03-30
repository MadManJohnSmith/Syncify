-- Syncify Schema Enhancements
-- Migration 0003: Add missing columns and indexes from specialist review

-- ==============================================
-- 1. ADD MISSING COLUMNS
-- ==============================================

-- AcoustID fingerprint for audio matching
ALTER TABLE tracks ADD COLUMN acoustid_fingerprint TEXT;

-- MusicBrainz release ID for downloads (canonical release reference)
ALTER TABLE downloads ADD COLUMN musicbrainz_release_id TEXT;

-- Updated timestamps for sync conflict detection
ALTER TABLE tracks ADD COLUMN updated_at TEXT;
ALTER TABLE artists ADD COLUMN updated_at TEXT;
ALTER TABLE albums ADD COLUMN updated_at TEXT;
ALTER TABLE downloads ADD COLUMN updated_at TEXT;

-- ==============================================
-- 2. ADD COMPOSITE INDEXES FOR PERFORMANCE
-- ==============================================

-- Best quality source lookup (most common query pattern)
CREATE INDEX idx_track_sources_best ON track_sources(track_id, available DESC, quality_score DESC);

-- Library entries: find liked tracks for an account
CREATE INDEX idx_library_liked ON library_entries(account_id, is_liked) WHERE is_liked = 1;

-- Downloads: find by hash for duplicate detection
CREATE INDEX idx_downloads_path ON downloads(file_path);

-- Tracks: find by acoustid fingerprint
CREATE INDEX idx_tracks_acoustid ON tracks(acoustid_fingerprint) WHERE acoustid_fingerprint IS NOT NULL;

-- Download queue: priority ordering
CREATE INDEX idx_queue_priority ON download_queue(status, priority DESC, created_at);

-- ==============================================
-- 3. ADD TRIGGERS FOR updated_at
-- ==============================================

CREATE TRIGGER tracks_updated AFTER UPDATE ON tracks 
BEGIN UPDATE tracks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id; END;

CREATE TRIGGER artists_updated AFTER UPDATE ON artists 
BEGIN UPDATE artists SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id; END;

CREATE TRIGGER albums_updated AFTER UPDATE ON albums 
BEGIN UPDATE albums SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id; END;

CREATE TRIGGER downloads_updated AFTER UPDATE ON downloads 
BEGIN UPDATE downloads SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id; END;

-- ==============================================
-- 4. ADD UNIQUE CONSTRAINT ON file_path
-- ==============================================

-- Note: SQLite doesn't support ADD CONSTRAINT, so we use unique index
CREATE UNIQUE INDEX idx_downloads_unique_path ON downloads(file_path);
