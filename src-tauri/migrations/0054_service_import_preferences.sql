-- 0054_service_import_preferences.sql
-- S126B: Per-service import preferences & unified sync capabilities
-- Adds granular import preference flags to service_sync_settings

ALTER TABLE service_sync_settings ADD COLUMN sync_favorite_artists BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE service_sync_settings ADD COLUMN sync_purchases BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE service_sync_settings ADD COLUMN sync_library_history BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE service_sync_settings ADD COLUMN sync_include_appearances BOOLEAN NOT NULL DEFAULT 0;
