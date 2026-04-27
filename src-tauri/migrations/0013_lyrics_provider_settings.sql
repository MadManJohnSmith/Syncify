-- 0013_lyrics_provider_settings.sql
-- Sprint 3: Lyrics provider configuration and priority ordering

CREATE TABLE IF NOT EXISTS lyrics_provider_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL UNIQUE,
    provider_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 0,
    sync_level TEXT NOT NULL DEFAULT 'line',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed default provider priorities
-- Apple Music has syllable-level sync, others have line-level or none
INSERT OR IGNORE INTO lyrics_provider_settings (provider_id, provider_name, priority, sync_level) VALUES
    ('apple_music', 'Apple Music', 1, 'syllable'),
    ('lrclib', 'LRCLIB', 2, 'line'),
    ('netease', 'NetEase', 3, 'line'),
    ('genius', 'Genius', 4, 'none');

-- Index for quick lookups
CREATE INDEX IF NOT EXISTS idx_lyrics_providers_priority ON lyrics_provider_settings(priority);

-- Rollback:
-- DROP INDEX IF EXISTS idx_lyrics_providers_priority;
-- DROP TABLE IF EXISTS lyrics_provider_settings;
