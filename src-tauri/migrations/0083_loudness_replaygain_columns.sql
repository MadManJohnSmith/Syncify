-- 0083_loudness_replaygain_columns.sql
-- TASK-73: ReplayGain and EBU R128 Loudness Normalization metadata support
-- Adds loudness and ReplayGain 2.0 columns to tracks table for fluid mixing and Sweet Fades.

ALTER TABLE tracks ADD COLUMN loudness REAL;
ALTER TABLE tracks ADD COLUMN replaygain_track_gain TEXT;
ALTER TABLE tracks ADD COLUMN replaygain_track_peak TEXT;
ALTER TABLE tracks ADD COLUMN replaygain_album_gain TEXT;
ALTER TABLE tracks ADD COLUMN replaygain_album_peak TEXT;

-- Create index on loudness for filtering and queries
CREATE INDEX IF NOT EXISTS idx_tracks_loudness ON tracks(loudness);
