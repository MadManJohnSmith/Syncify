-- Migration 0040: Add preview_url to tracks
-- preview_url: 30-second audio clip for quick listening
ALTER TABLE tracks ADD COLUMN preview_url TEXT;
