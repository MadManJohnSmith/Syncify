-- Migration 0039: Add industry fields to albums
-- label: Record label responsible for the release
ALTER TABLE albums ADD COLUMN label TEXT;
