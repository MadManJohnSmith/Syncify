-- 0012_audio_processing_settings.sql
-- Sprint 2: Audio processing settings (ReplayGain, transcoding, embedding)

CREATE TABLE IF NOT EXISTS audio_processing_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    replay_gain_mode TEXT NOT NULL DEFAULT 'off',
    target_loudness_lufs REAL NOT NULL DEFAULT -14.0,
    transcode_enabled INTEGER NOT NULL DEFAULT 0,
    transcode_format TEXT NOT NULL DEFAULT 'mp3',
    transcode_bitrate INTEGER NOT NULL DEFAULT 320,
    keep_original_after_transcode INTEGER NOT NULL DEFAULT 1,
    embed_lyrics INTEGER NOT NULL DEFAULT 1,
    embed_artwork INTEGER NOT NULL DEFAULT 1,
    artwork_max_size INTEGER NOT NULL DEFAULT 1200,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Insert default singleton row
INSERT OR IGNORE INTO audio_processing_settings (id) VALUES (1);

-- Rollback:
-- DROP TABLE IF EXISTS audio_processing_settings;
