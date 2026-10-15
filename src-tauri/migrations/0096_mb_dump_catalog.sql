-- Separate, local MusicBrainz dump catalog; never alters the online enrichment cache.
CREATE TABLE mb_dump_recording (
    recording_id INTEGER PRIMARY KEY,
    mbid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    artist_credit INTEGER NOT NULL,
    length INTEGER
);
CREATE TABLE mb_dump_recording_isrc (
    isrc_id INTEGER PRIMARY KEY,
    isrc TEXT NOT NULL,
    recording_id INTEGER NOT NULL,
    recording_mbid TEXT NOT NULL,
    UNIQUE(isrc, recording_mbid)
);
CREATE INDEX mb_dump_isrc_lookup ON mb_dump_recording_isrc(isrc);
CREATE TABLE mb_dump_artist (
    artist_id INTEGER PRIMARY KEY,
    mbid TEXT NOT NULL,
    name TEXT NOT NULL
);
CREATE TABLE mb_dump_artist_credit_name (
    artist_credit INTEGER NOT NULL,
    position INTEGER NOT NULL,
    artist INTEGER NOT NULL,
    name TEXT NOT NULL,
    join_phrase TEXT NOT NULL,
    PRIMARY KEY(artist_credit, position)
);
CREATE TABLE mb_dump_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    dump_path TEXT NOT NULL,
    dump_timestamp TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    recording_rows INTEGER NOT NULL DEFAULT 0,
    isrc_rows INTEGER NOT NULL DEFAULT 0,
    artist_rows INTEGER NOT NULL DEFAULT 0,
    artist_credit_name_rows INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (status IN ('importing', 'completed', 'failed')),
    error TEXT,
    imported_at TEXT
);
