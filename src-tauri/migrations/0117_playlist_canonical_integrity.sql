-- Migration 0117 (issue #74): corrección de evidencia y primaria canónica.
--
-- 0116 se publicó antes de integrar. Se corrige con otra migración, nunca
-- editando una versión ya aplicada:
--
-- 1. Un total local recompactado NO es evidencia de que el proveedor declaró
--    ese total. Las fuentes históricas quedan sin declaración hasta que una
--    re-expansión vuelva a pasar por el trigger de 0116.
-- 2. La fuente primaria se deriva de la playlist miembro más antigua. No se
--    guarda un FK a una playlist concreta: borrar o renombrar la primaria no
--    puede arrastrar el grupo entero ni dejar una primaria colgada.

UPDATE playlist_sources SET reported_track_count = NULL;

-- El padre se reconstruye sin ON DELETE CASCADE. Se staguean los miembros sin
-- FKs antes del DROP para que la cascada de 0116 no los borre.
CREATE TABLE _playlist_canonicals_0117_stage AS
SELECT id, normalized_name, content_hash, track_count, created_at, updated_at
FROM playlist_canonicals;

CREATE TABLE _playlist_canonical_members_0117_stage AS
SELECT canonical_id, playlist_id FROM playlist_canonical_members;

DROP TABLE playlist_canonicals;
DROP TABLE playlist_canonical_members;

CREATE TABLE playlist_canonicals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    normalized_name TEXT NOT NULL CHECK (length(trim(normalized_name)) > 0),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
    track_count INTEGER NOT NULL CHECK (track_count > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (normalized_name, content_hash)
);

INSERT INTO playlist_canonicals
    (id, normalized_name, content_hash, track_count, created_at, updated_at)
SELECT id, normalized_name, content_hash, track_count, created_at, updated_at
FROM _playlist_canonicals_0117_stage;

CREATE TABLE playlist_canonical_members (
    canonical_id INTEGER NOT NULL REFERENCES playlist_canonicals(id) ON DELETE CASCADE,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id)
);

INSERT INTO playlist_canonical_members (canonical_id, playlist_id)
SELECT canonical_id, playlist_id
FROM _playlist_canonical_members_0117_stage;

CREATE INDEX idx_playlist_canonical_members_canonical
    ON playlist_canonical_members(canonical_id);

DROP TABLE _playlist_canonicals_0117_stage;
DROP TABLE _playlist_canonical_members_0117_stage;
