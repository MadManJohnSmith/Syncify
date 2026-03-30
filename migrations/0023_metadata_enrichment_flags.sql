-- Migration 0023: Metadata enrichment provider flags
-- Sprint 19 implementation

ALTER TABLE metadata_preferences ADD COLUMN enable_musicbrainz INTEGER NOT NULL DEFAULT 1;
ALTER TABLE metadata_preferences ADD COLUMN enable_lastfm INTEGER NOT NULL DEFAULT 0;
ALTER TABLE metadata_preferences ADD COLUMN enable_acoustid INTEGER NOT NULL DEFAULT 0;
