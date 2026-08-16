-- Migration 0050: Download Queue Source Identity & Precision Routing
-- Adds explicit provider, edition, and source tracking columns to download_queue

ALTER TABLE download_queue ADD COLUMN service_id INTEGER REFERENCES services(id);
ALTER TABLE download_queue ADD COLUMN service_name TEXT;
ALTER TABLE download_queue ADD COLUMN service_track_id TEXT;
ALTER TABLE download_queue ADD COLUMN service_album_id TEXT;
ALTER TABLE download_queue ADD COLUMN target_title TEXT;
ALTER TABLE download_queue ADD COLUMN target_artist TEXT;
ALTER TABLE download_queue ADD COLUMN target_album TEXT;
ALTER TABLE download_queue ADD COLUMN target_isrc TEXT;
ALTER TABLE download_queue ADD COLUMN smart_studio_origin INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_queue ADD COLUMN allow_fallback INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_download_queue_service_identity ON download_queue(service_name, service_track_id);
