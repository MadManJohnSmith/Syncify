-- Migration 0116 (issue #74, ADR-0006 decisión 34): canónicas de playlist.
--
-- Una playlist de origen conserva su fila, su identidad remota y su contenido.
-- La canónica agrupa las copias con el mismo nombre normalizado y el mismo
-- conjunto exacto de Tracks. Así una fuente que diverge más adelante se puede
-- separar sin haber destruido el snapshot de ninguna otra fuente.
--
-- `playlist_sources.reported_track_count` congela el total declarado por el
-- proveedor ANTES de que la recompactación local sobrescriba `track_count`. Si
-- el declarado no coincide con las ocurrencias locales, la expansión está
-- incompleta y esa playlist no participa.

ALTER TABLE playlist_sources
    ADD COLUMN reported_track_count INTEGER
        CHECK (reported_track_count IS NULL OR reported_track_count >= 0);

-- Datos históricos: el único total declarado que sobrevive en una BD ya
-- recompactada es playlists.track_count. Se toma como evidencia disponible;
-- una desigualdad con las filas reales seguirá excluyendo la playlist.
UPDATE playlist_sources
SET reported_track_count = (
        SELECT p.track_count FROM playlists p WHERE p.id = playlist_sources.playlist_id
    );

CREATE TABLE playlist_canonicals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    normalized_name TEXT NOT NULL CHECK (length(trim(normalized_name)) > 0),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
    track_count INTEGER NOT NULL CHECK (track_count > 0),
    primary_playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (normalized_name, content_hash)
);

CREATE INDEX idx_playlist_canonicals_primary
    ON playlist_canonicals(primary_playlist_id);

CREATE TABLE playlist_canonical_members (
    canonical_id INTEGER NOT NULL REFERENCES playlist_canonicals(id) ON DELETE CASCADE,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id)
);

CREATE INDEX idx_playlist_canonical_members_canonical
    ON playlist_canonical_members(canonical_id);

-- Todos los importadores existentes escriben playlists.track_count con el
-- total del proveedor y después insertan/actualizan playlist_sources. Este
-- trigger captura ese total sin obligar a cada brazo de sync a conocer el
-- esquema canónico. La recompactación posterior solo toca playlists, así que
-- no puede borrar la evidencia de una expansión parcial.
CREATE TRIGGER trg_playlist_sources_capture_reported_ins
AFTER INSERT ON playlist_sources
FOR EACH ROW
BEGIN
    UPDATE playlist_sources
    SET reported_track_count = (
            SELECT p.track_count FROM playlists p WHERE p.id = NEW.playlist_id
        )
    WHERE id = NEW.id;
END;

CREATE TRIGGER trg_playlist_sources_capture_reported_upd
AFTER UPDATE OF playlist_id, synced_at ON playlist_sources
FOR EACH ROW
BEGIN
    UPDATE playlist_sources
    SET reported_track_count = (
            SELECT p.track_count FROM playlists p WHERE p.id = NEW.playlist_id
        )
    WHERE id = NEW.id;
END;
