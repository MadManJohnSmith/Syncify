-- 0014_lyrics_config.sql
-- Sprint 3: Global lyrics configuration settings (singleton pattern)

CREATE TABLE IF NOT EXISTS lyrics_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    min_sync_level TEXT NOT NULL DEFAULT 'line',
    preferred_language TEXT NOT NULL DEFAULT 'en',
    storage_format TEXT NOT NULL DEFAULT 'lrc',
    auto_fetch_on_import INTEGER NOT NULL DEFAULT 1,
    retry_failed INTEGER NOT NULL DEFAULT 1,
    retry_frequency TEXT NOT NULL DEFAULT 'weekly',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default singleton row
INSERT OR IGNORE INTO lyrics_config (id) VALUES (1);

-- Rollback:
-- DROP TABLE IF EXISTS lyrics_config;
