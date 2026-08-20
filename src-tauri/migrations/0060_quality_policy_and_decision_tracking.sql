-- Migration 0060: Quality Policy & Decision Tracking
-- Adds canonical quality decision, format and fallback provenance columns to downloads and download_queue

-- 1. Extend downloads table with canonical quality decision provenance
ALTER TABLE downloads ADD COLUMN requested_quality TEXT;
ALTER TABLE downloads ADD COLUMN effective_quality TEXT;
ALTER TABLE downloads ADD COLUMN requested_format TEXT;
ALTER TABLE downloads ADD COLUMN effective_format TEXT;
ALTER TABLE downloads ADD COLUMN quality_decision TEXT;
ALTER TABLE downloads ADD COLUMN provider_fallback_used INTEGER NOT NULL DEFAULT 0;
ALTER TABLE downloads ADD COLUMN quality_fallback_used INTEGER NOT NULL DEFAULT 0;
ALTER TABLE downloads ADD COLUMN decision_reason TEXT;

-- 2. Extend download_queue table with canonical quality decision provenance
ALTER TABLE download_queue ADD COLUMN requested_quality TEXT;
ALTER TABLE download_queue ADD COLUMN effective_quality TEXT;
ALTER TABLE download_queue ADD COLUMN requested_format TEXT;
ALTER TABLE download_queue ADD COLUMN effective_format TEXT;
ALTER TABLE download_queue ADD COLUMN quality_decision TEXT;
ALTER TABLE download_queue ADD COLUMN provider_fallback_used INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_queue ADD COLUMN quality_fallback_used INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_queue ADD COLUMN decision_reason TEXT;

-- 3. Create indexes for quality decision querying and telemetry
CREATE INDEX IF NOT EXISTS idx_downloads_quality_decision ON downloads(quality_decision);
CREATE INDEX IF NOT EXISTS idx_download_queue_quality_decision ON download_queue(quality_decision);
