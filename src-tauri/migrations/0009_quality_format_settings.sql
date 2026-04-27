-- 0009_quality_format_settings.sql
-- Sprint 2: Quality preferences per streaming service

CREATE TABLE IF NOT EXISTS quality_preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL UNIQUE,
    max_quality TEXT NOT NULL DEFAULT 'lossless',
    preferred_format TEXT NOT NULL DEFAULT 'flac',
    fallback_quality TEXT NOT NULL DEFAULT 'high',
    fallback_format TEXT NOT NULL DEFAULT 'mp3',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed default quality preferences for each service
INSERT OR IGNORE INTO quality_preferences (service_name, max_quality, preferred_format, fallback_quality, fallback_format) VALUES
    ('spotify', 'high', 'ogg', 'medium', 'ogg'),
    ('qobuz', 'hires', 'flac', 'lossless', 'flac'),
    ('tidal', 'master', 'flac', 'hifi', 'flac'),
    ('deezer', 'lossless', 'flac', 'high', 'mp3'),
    ('soundcloud', 'high', 'mp3', 'medium', 'mp3');

-- Index for quick lookups
CREATE INDEX IF NOT EXISTS idx_quality_preferences_service ON quality_preferences(service_name);

-- Rollback:
-- DROP INDEX IF EXISTS idx_quality_preferences_service;
-- DROP TABLE IF EXISTS quality_preferences;
