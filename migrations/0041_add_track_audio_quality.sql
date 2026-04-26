-- Migration 0041: Add audio_quality to tracks
-- audio_quality: Streaming quality (e.g., LOSSLESS, HI_RES, MASTER)
ALTER TABLE tracks ADD COLUMN audio_quality TEXT;
