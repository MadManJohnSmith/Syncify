-- LY-12b: retain the user's explicit deletion even when tags or sidecars remain.
-- The '*' marker suppresses every format; a format restoration overrides only that format.
CREATE TABLE lyrics_deletion_marks (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    format TEXT NOT NULL CHECK (length(format) > 0),
    PRIMARY KEY (track_id, format)
);
CREATE TABLE lyrics_restored_formats (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    format TEXT NOT NULL CHECK (length(format) > 0 AND format <> '*'),
    PRIMARY KEY (track_id, format)
);
