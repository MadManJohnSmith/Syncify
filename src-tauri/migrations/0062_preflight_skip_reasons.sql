-- Migration 0062: Preflight Skip Reasons and Selection Reconciliation
-- Adds skip_reason column to download_queue and downloads tables to persist explicit exclusion reasons

ALTER TABLE download_queue ADD COLUMN skip_reason TEXT;
ALTER TABLE downloads ADD COLUMN skip_reason TEXT;

CREATE INDEX IF NOT EXISTS idx_download_queue_skip_reason ON download_queue(skip_reason);
CREATE INDEX IF NOT EXISTS idx_downloads_skip_reason ON downloads(skip_reason);
