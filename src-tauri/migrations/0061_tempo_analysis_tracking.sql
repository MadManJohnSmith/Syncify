-- Migration 0061: Tempo Analysis and Confidence Tracking
-- Adds columns to track tempo analysis confidence, provenance source, and analysis timestamp.

ALTER TABLE tracks ADD COLUMN tempo_confidence REAL;
ALTER TABLE tracks ADD COLUMN tempo_source TEXT;
ALTER TABLE tracks ADD COLUMN tempo_analyzed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_tracks_tempo_confidence ON tracks(tempo_confidence);
CREATE INDEX IF NOT EXISTS idx_tracks_tempo_source ON tracks(tempo_source);
