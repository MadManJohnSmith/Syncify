-- Migration 0113 (M01, issue #166): migration items declare their kind and
-- jobs record the collections they carry.
--
-- Until now migration_items was track-shaped only (0017): the engine walked
-- track favorites and playlist tracks, and a job could not say it was
-- migrating albums. M01 adds the favorite-album collection on the same job
-- pipeline, so every item carries the kind of collection it belongs to and
-- the job records the requested collections as a JSON array of the wire
-- names used by the capability matrix (M00: "tracks" | "albums" | "artists"
-- | "playlists").
--
-- Historical rows are track items by definition: before this migration the
-- engine could not create any other kind, so 'track' is the NOT NULL default
-- and NULL collections keeps meaning the legacy track-favorites flow (the
-- same scope rule as source_playlist_ids).

ALTER TABLE migration_items ADD COLUMN kind TEXT NOT NULL DEFAULT 'track';

ALTER TABLE migration_jobs ADD COLUMN collections TEXT;

-- Per-kind progress reads on the job detail view (the existing
-- idx_migration_items_job_status already serves status filters).
CREATE INDEX IF NOT EXISTS idx_migration_items_job_kind ON migration_items(job_id, kind);
