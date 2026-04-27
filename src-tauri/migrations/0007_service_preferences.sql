-- 0007_service_preferences.sql
-- Sprint 1: Service Preferences & Priorities
-- Purpose: Store per-service configuration for import priorities and auto-import settings

CREATE TABLE IF NOT EXISTS service_preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_name TEXT NOT NULL UNIQUE,
    priority INTEGER NOT NULL DEFAULT 0,
    auto_import_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Seed defaults (order matters for priority)
INSERT OR IGNORE INTO service_preferences (service_name, priority, auto_import_enabled) VALUES
    ('spotify', 1, FALSE),
    ('qobuz', 2, FALSE),
    ('tidal', 3, FALSE),
    ('deezer', 4, FALSE),
    ('soundcloud', 5, FALSE);

CREATE INDEX IF NOT EXISTS idx_service_prefs_priority ON service_preferences(priority);

-- Rollback:
-- DROP INDEX IF EXISTS idx_service_prefs_priority;
-- DROP TABLE IF EXISTS service_preferences;
