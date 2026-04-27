-- Migration 0032_add_title_duration_index.sql
-- Supports tolerant duplicate fallback on title + duration for tracks without ISRC

CREATE INDEX IF NOT EXISTS idx_tracks_title_duration
    ON tracks(title, duration_ms)
    WHERE isrc IS NULL;
