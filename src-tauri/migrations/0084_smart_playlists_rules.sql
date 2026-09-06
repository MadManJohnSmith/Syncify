-- Migration 0084: Smart Playlists Rules and Persistence
-- TASK-21: Persistencia Real y Generación Dinámica de Smart Playlists en PlaylistView.vue

ALTER TABLE playlists ADD COLUMN is_smart INTEGER DEFAULT 0;
ALTER TABLE playlists ADD COLUMN rules_json TEXT;

CREATE INDEX IF NOT EXISTS idx_playlists_is_smart ON playlists(is_smart);
