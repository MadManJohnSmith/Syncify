-- 0011_duplicate_settings.sql
-- Sprint 2: Duplicate detection and handling settings

CREATE TABLE IF NOT EXISTS duplicate_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enable_detection INTEGER NOT NULL DEFAULT 1,
    prefer_higher_quality INTEGER NOT NULL DEFAULT 1,
    prefer_lossless INTEGER NOT NULL DEFAULT 1,
    replace_same_quality_different_source INTEGER NOT NULL DEFAULT 0,
    quality_threshold_kbps INTEGER NOT NULL DEFAULT 64,
    delete_duplicates_immediately INTEGER NOT NULL DEFAULT 0,
    move_to_trash INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default singleton row
INSERT OR IGNORE INTO duplicate_settings (id) VALUES (1);

-- Rollback:
-- DROP TABLE IF EXISTS duplicate_settings;
