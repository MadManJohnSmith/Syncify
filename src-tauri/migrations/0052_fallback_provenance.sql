-- Migration 0052: Fallback Provenance and Edition Identity
ALTER TABLE download_queue ADD COLUMN origin_service TEXT;
ALTER TABLE download_queue ADD COLUMN origin_service_track_id TEXT;
ALTER TABLE download_queue ADD COLUMN effective_service TEXT;
ALTER TABLE download_queue ADD COLUMN effective_service_track_id TEXT;
ALTER TABLE download_queue ADD COLUMN fallback_reason TEXT;
ALTER TABLE download_queue ADD COLUMN match_method TEXT;
ALTER TABLE download_queue ADD COLUMN match_confidence REAL;

ALTER TABLE downloads ADD COLUMN origin_service TEXT;
ALTER TABLE downloads ADD COLUMN origin_service_track_id TEXT;
ALTER TABLE downloads ADD COLUMN effective_service TEXT;
ALTER TABLE downloads ADD COLUMN effective_service_track_id TEXT;
ALTER TABLE downloads ADD COLUMN fallback_reason TEXT;
ALTER TABLE downloads ADD COLUMN match_method TEXT;
ALTER TABLE downloads ADD COLUMN match_confidence REAL;
