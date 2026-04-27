-- 0016_advanced_settings.sql
-- Sprint 5: Advanced settings for logging, workers, cache, matching, network, debug

CREATE TABLE IF NOT EXISTS advanced_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    -- Logging settings
    log_level TEXT NOT NULL DEFAULT 'info',
    log_to_file INTEGER NOT NULL DEFAULT 1,
    log_file_max_size_mb INTEGER NOT NULL DEFAULT 50,
    log_file_retention_days INTEGER NOT NULL DEFAULT 30,
    -- Worker settings
    max_concurrent_downloads INTEGER NOT NULL DEFAULT 3,
    max_concurrent_imports INTEGER NOT NULL DEFAULT 2,
    worker_timeout_seconds INTEGER NOT NULL DEFAULT 300,
    -- Cache settings
    cache_enabled INTEGER NOT NULL DEFAULT 1,
    cache_max_size_mb INTEGER NOT NULL DEFAULT 500,
    cache_ttl_hours INTEGER NOT NULL DEFAULT 168,
    -- Matching settings
    fuzzy_match_threshold REAL NOT NULL DEFAULT 0.85,
    use_acoustic_fingerprinting INTEGER NOT NULL DEFAULT 1,
    prefer_exact_matches INTEGER NOT NULL DEFAULT 1,
    -- Network settings
    request_timeout_seconds INTEGER NOT NULL DEFAULT 30,
    max_retries INTEGER NOT NULL DEFAULT 3,
    retry_delay_seconds INTEGER NOT NULL DEFAULT 5,
    use_proxy INTEGER NOT NULL DEFAULT 0,
    proxy_url TEXT,
    -- Debug settings
    debug_mode INTEGER NOT NULL DEFAULT 0,
    verbose_api_logging INTEGER NOT NULL DEFAULT 0,
    -- Timestamps
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default singleton row
INSERT OR IGNORE INTO advanced_settings (id) VALUES (1);

-- Cache stats table for tracking cache usage
CREATE TABLE IF NOT EXISTS cache_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cache_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    item_count INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    miss_count INTEGER NOT NULL DEFAULT 0,
    last_updated TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed initial cache types
INSERT OR IGNORE INTO cache_stats (cache_type) VALUES
    ('artwork'),
    ('metadata'),
    ('lyrics'),
    ('api_responses');

-- Rollback:
-- DROP TABLE IF EXISTS cache_stats;
-- DROP TABLE IF EXISTS advanced_settings;
