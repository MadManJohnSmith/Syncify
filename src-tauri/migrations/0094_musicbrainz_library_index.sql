-- Library-scoped positive MusicBrainz recordings. Never cache misses here.
-- The ISRC key is uppercase with hyphens/spaces removed; the application
-- validates that the key still belongs to a local track on every read.
CREATE TABLE mb_library_index (
    isrc TEXT PRIMARY KEY,
    mbid TEXT NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_mb_library_index_updated_at ON mb_library_index(updated_at);
