-- Migration 0053: Track Source Availability & Provenance Tracking
-- Adds explicit availability status and diagnostic reason to track_sources

ALTER TABLE track_sources ADD COLUMN availability_status TEXT NOT NULL DEFAULT 'unknown_unchecked';
ALTER TABLE track_sources ADD COLUMN availability_reason TEXT;

CREATE INDEX IF NOT EXISTS idx_track_sources_availability ON track_sources(track_id, availability_status);
