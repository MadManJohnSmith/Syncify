-- Migration 0037: Add tidal_id to albums table for robust deduplication
-- Created: 2026-04-26

-- Add tidal_id column
ALTER TABLE albums ADD COLUMN tidal_id TEXT;

-- Create partial unique index to allow NULLs for non-Tidal albums
-- while enforcing uniqueness for Tidal albums.
CREATE UNIQUE INDEX idx_albums_tidal_id ON albums(tidal_id) WHERE tidal_id IS NOT NULL;
