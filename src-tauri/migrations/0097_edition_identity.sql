-- ED-01: additive identity ledger. Legacy track_sources remains the active resolver input.
-- No ISRC/title-based associations and no destructive rewrite of legacy mappings.
CREATE TABLE edition_recordings (
    id INTEGER PRIMARY KEY,
    musicbrainz_recording_id TEXT UNIQUE,
    evidence_status TEXT NOT NULL DEFAULT 'unknown'
      CHECK (evidence_status IN ('unknown', 'observed', 'verified')),
    provenance TEXT NOT NULL DEFAULT 'unknown'
);
CREATE TABLE edition_releases (
    id INTEGER PRIMARY KEY,
    service_id INTEGER REFERENCES services(id),
    provider_release_id TEXT,
    evidence_status TEXT NOT NULL DEFAULT 'unknown'
      CHECK (evidence_status IN ('unknown', 'observed', 'verified')),
    provenance TEXT NOT NULL DEFAULT 'unknown',
    UNIQUE(service_id, provider_release_id),
    UNIQUE(id, service_id),
    CHECK (provider_release_id IS NULL OR (service_id IS NOT NULL AND length(trim(provider_release_id)) > 0))
);
CREATE TABLE edition_provider_items (
    id INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id),
    service_track_id TEXT NOT NULL CHECK (length(trim(service_track_id)) > 0),
    release_id INTEGER,
    recording_id INTEGER REFERENCES edition_recordings(id),
    evidence_status TEXT NOT NULL DEFAULT 'unknown'
      CHECK (evidence_status IN ('unknown', 'observed', 'verified')),
    provenance TEXT NOT NULL DEFAULT 'unknown',
    UNIQUE(service_id, service_track_id),
    FOREIGN KEY(release_id, service_id) REFERENCES edition_releases(id, service_id)
);
CREATE INDEX idx_edition_items_release ON edition_provider_items(release_id);
CREATE INDEX idx_edition_items_recording ON edition_provider_items(recording_id);
CREATE TABLE edition_item_tracks (
    item_id INTEGER NOT NULL REFERENCES edition_provider_items(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    validation_status TEXT NOT NULL CHECK (validation_status IN ('unknown', 'observed', 'verified', 'needs_review')),
    provenance TEXT NOT NULL,
    PRIMARY KEY(item_id, track_id)
);
CREATE INDEX idx_edition_item_tracks_track ON edition_item_tracks(track_id);
-- Preserve collisions verbatim, including duplicate legacy rows for the same item.
CREATE TABLE edition_legacy_collisions (
    track_source_id INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL,
    service_track_id TEXT NOT NULL,
    track_id INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('multiple_tracks', 'duplicate_item'))
);
INSERT INTO edition_legacy_collisions(track_source_id, service_id, service_track_id, track_id, reason)
SELECT ts.id, ts.service_id, ts.service_track_id, ts.track_id,
       CASE WHEN (SELECT count(DISTINCT other.track_id) FROM track_sources other
                  WHERE other.service_id = ts.service_id AND other.service_track_id = ts.service_track_id) > 1
            THEN 'multiple_tracks' ELSE 'duplicate_item' END
FROM track_sources ts
WHERE (SELECT count(*) FROM track_sources other
       WHERE other.service_id = ts.service_id AND other.service_track_id = ts.service_track_id) > 1;
-- Only unambiguous legacy rows enter the new ledger; unknown is NOT verified.
INSERT INTO edition_provider_items(service_id, service_track_id, provenance)
SELECT ts.service_id, ts.service_track_id, 'legacy_track_sources'
FROM track_sources ts
WHERE length(trim(ts.service_track_id)) > 0
  AND NOT EXISTS (SELECT 1 FROM edition_legacy_collisions c WHERE c.track_source_id = ts.id);
INSERT INTO edition_item_tracks(item_id, track_id, validation_status, provenance)
SELECT item.id, ts.track_id, 'unknown', 'legacy_track_sources'
FROM track_sources ts
JOIN edition_provider_items item
  ON item.service_id = ts.service_id AND item.service_track_id = ts.service_track_id
WHERE NOT EXISTS (SELECT 1 FROM edition_legacy_collisions c WHERE c.track_source_id = ts.id);
