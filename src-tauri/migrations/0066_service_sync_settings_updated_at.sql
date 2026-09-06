-- Migration 0066: Add updated_at column to service_sync_settings
ALTER TABLE service_sync_settings ADD COLUMN updated_at TEXT;
UPDATE service_sync_settings SET updated_at = CURRENT_TIMESTAMP WHERE updated_at IS NULL;
