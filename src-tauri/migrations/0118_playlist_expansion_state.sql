-- Migration 0118 (issue #74): estado explícito de expansión y limpieza del upgrade.
--
-- 0117 invalidó la evidencia histórica, pero conservó membresías que ya no
-- debían seguir visibles. Al terminar el upgrade no queda ninguna canónica
-- hasta que una expansión vuelva a declarar y completar su contenido.
--
-- `expansion_complete` distingue:
--   NULL  → la fuente se acaba de reencontrar; aún no se completó la expansión;
--   0     → expansión incompleta o total desconocido;
--   1     → el llamador completó el recorrido y el declarado cuadra con lo local.
-- El trigger de 0116 conserva el total declarado; estos triggers solo reinician
-- el estado para que un sync nuevo no herede la completitud del anterior.

ALTER TABLE playlist_sources
    ADD COLUMN expansion_complete INTEGER
        CHECK (expansion_complete IS NULL OR expansion_complete IN (0, 1));

UPDATE playlist_sources SET expansion_complete = 0;

DELETE FROM playlist_canonical_members;
DELETE FROM playlist_canonicals;

CREATE TRIGGER trg_playlist_sources_expansion_started_ins
AFTER INSERT ON playlist_sources
FOR EACH ROW
BEGIN
    UPDATE playlist_sources SET expansion_complete = NULL WHERE id = NEW.id;
    DELETE FROM playlist_canonical_members WHERE playlist_id = NEW.playlist_id;
END;

CREATE TRIGGER trg_playlist_sources_expansion_started_upd
AFTER UPDATE OF playlist_id, synced_at ON playlist_sources
FOR EACH ROW
BEGIN
    UPDATE playlist_sources SET expansion_complete = NULL WHERE id = NEW.id;
    DELETE FROM playlist_canonical_members WHERE playlist_id = NEW.playlist_id;
END;

-- Cualquier mutación de contenido invalida la completitud y la membresía.
-- El importador vuelve a cerrar la expansión con recompactación; una alta local
-- desde otra ruta no puede dejar una copia fusionada con una huella vieja.
CREATE TRIGGER trg_playlist_tracks_expansion_dirty_ins
AFTER INSERT ON playlist_tracks
FOR EACH ROW
BEGIN
    UPDATE playlist_sources
    SET expansion_complete = NULL
    WHERE playlist_id = NEW.playlist_id;
    DELETE FROM playlist_canonical_members WHERE playlist_id = NEW.playlist_id;
END;

CREATE TRIGGER trg_playlist_tracks_expansion_dirty_del
AFTER DELETE ON playlist_tracks
FOR EACH ROW
BEGIN
    UPDATE playlist_sources
    SET expansion_complete = NULL
    WHERE playlist_id = OLD.playlist_id;
    DELETE FROM playlist_canonical_members WHERE playlist_id = OLD.playlist_id;
END;

CREATE TRIGGER trg_playlist_tracks_expansion_dirty_upd
AFTER UPDATE OF track_id ON playlist_tracks
FOR EACH ROW
BEGIN
    UPDATE playlist_sources
    SET expansion_complete = NULL
    WHERE playlist_id = NEW.playlist_id;
    DELETE FROM playlist_canonical_members WHERE playlist_id = NEW.playlist_id;
END;
