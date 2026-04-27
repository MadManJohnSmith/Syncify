-- Migration 0030: Add downloaded_tracks to library_snapshots
-- This allows historical tracking of downloaded files without using proxy columns

ALTER TABLE library_snapshots
ADD COLUMN downloaded_tracks INTEGER NOT NULL DEFAULT 0;
