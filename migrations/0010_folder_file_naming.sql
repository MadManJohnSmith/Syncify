-- 0010_folder_file_naming.sql
-- Sprint 2: Folder structure and file naming template settings

CREATE TABLE IF NOT EXISTS folder_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    base_folder TEXT NOT NULL DEFAULT '',
    folder_template TEXT NOT NULL DEFAULT '{AlbumArtist}/{Album}',
    file_template TEXT NOT NULL DEFAULT '{TrackNumber:pad2} - {Title}',
    artist_separator TEXT NOT NULL DEFAULT ', ',
    replace_spaces_with TEXT,
    max_path_length INTEGER NOT NULL DEFAULT 255,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default singleton row
INSERT OR IGNORE INTO folder_settings (id, base_folder) VALUES (1, '');

-- Rollback:
-- DROP TABLE IF EXISTS folder_settings;
