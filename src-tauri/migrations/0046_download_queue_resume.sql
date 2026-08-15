-- Migration 0046: Download Queue Resume and Manual Ordering Support
-- Adds position, staging_path, resumable, and last_error columns to download_queue

ALTER TABLE download_queue ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_queue ADD COLUMN staging_path TEXT;
ALTER TABLE download_queue ADD COLUMN resumable INTEGER NOT NULL DEFAULT 1;
ALTER TABLE download_queue ADD COLUMN last_error TEXT;

-- Create index for deterministic FIFO + Priority + Position ordering
CREATE INDEX IF NOT EXISTS idx_download_queue_order ON download_queue(status, priority DESC, position ASC, created_at ASC);
