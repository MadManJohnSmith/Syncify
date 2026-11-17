-- Migration 0107: Library FTS v2 (L02, issue #99)
-- Amplía library_fts con género e ISRC además de título, artista y álbum.
-- La migración 0019 aplicada NO se edita: se recrea la tabla (rebuild) y se
-- reinstalan los triggers. El número 0107 (y no 0106) porque el PR #304
-- ya ocupa 0106 para sync_runs; sqlx no exige numeración contigua.
--
-- El ISRC se indexa normalizado: sin guiones y en mayúsculas, para que la
-- comparación exacta y el token FTS coincidan con la consulta normalizada
-- (misma normalización en Rust, ver commands/library_query.rs).

-- 1. Triggers de 0019 fuera antes de recrear la tabla.
DROP TRIGGER IF EXISTS library_fts_insert;
DROP TRIGGER IF EXISTS library_fts_delete;
DROP TRIGGER IF EXISTS library_fts_update;
DROP TRIGGER IF EXISTS library_fts_artist_link_insert;
DROP TRIGGER IF EXISTS library_fts_artist_link_delete;
DROP TRIGGER IF EXISTS library_fts_album_update;
DROP TRIGGER IF EXISTS library_fts_artist_update;

DROP TABLE IF EXISTS library_fts;

-- 2. Tabla nueva: el orden de columnas fija el orden de los pesos bm25
--    (título > artista > álbum > género > ISRC, ver library_query.rs).
CREATE VIRTUAL TABLE library_fts USING fts5(
    title,
    artist,
    album,
    genre,
    isrc,
    tokenize='porter unicode61'
);

-- 3. Rebuild inicial desde el esquema vigente (una fila por track).
INSERT INTO library_fts(rowid, title, artist, album, genre, isrc)
SELECT
    t.id,
    t.title,
    COALESCE(
        (SELECT GROUP_CONCAT(a.name, ' ')
         FROM track_artists ta
         JOIN artists a ON ta.artist_id = a.id
         WHERE ta.track_id = t.id),
        ''
    ) as artist_names,
    COALESCE(al.title, '') as album_title,
    COALESCE(t.genre, '') as genre,
    COALESCE(UPPER(REPLACE(t.isrc, '-', '')), '') as isrc
FROM tracks t
LEFT JOIN albums al ON t.album_id = al.id;

-- 4. Triggers de mantenimiento (ahora cubren género e ISRC).

-- 4.1 Tracks Insert
CREATE TRIGGER library_fts_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO library_fts(rowid, title, artist, album, genre, isrc)
    VALUES (
        NEW.id,
        NEW.title,
        '', -- sin artistas vinculados en el insert habitual
        (SELECT title FROM albums WHERE id = NEW.album_id),
        COALESCE(NEW.genre, ''),
        COALESCE(UPPER(REPLACE(NEW.isrc, '-', '')), '')
    );
END;

-- 4.2 Tracks Delete
CREATE TRIGGER library_fts_delete AFTER DELETE ON tracks BEGIN
    DELETE FROM library_fts WHERE rowid = OLD.id;
END;

-- 4.3 Tracks Update: título, álbum, género o ISRC
CREATE TRIGGER library_fts_update AFTER UPDATE OF title, album_id, genre, isrc ON tracks BEGIN
    UPDATE library_fts SET
        title = NEW.title,
        album = (SELECT title FROM albums WHERE id = NEW.album_id),
        genre = COALESCE(NEW.genre, ''),
        isrc = COALESCE(UPPER(REPLACE(NEW.isrc, '-', '')), '')
    WHERE rowid = NEW.id;
END;

-- 4.4 Vínculo de artistas (alta) -> actualiza la cadena de artistas
CREATE TRIGGER library_fts_artist_link_insert AFTER INSERT ON track_artists BEGIN
    UPDATE library_fts SET
        artist = COALESCE(
            (SELECT GROUP_CONCAT(a.name, ' ')
             FROM track_artists ta
             JOIN artists a ON ta.artist_id = a.id
             WHERE ta.track_id = NEW.track_id),
            ''
        )
    WHERE rowid = NEW.track_id;
END;

-- 4.5 Vínculo de artistas (baja) -> actualiza la cadena de artistas
CREATE TRIGGER library_fts_artist_link_delete AFTER DELETE ON track_artists BEGIN
    UPDATE library_fts SET
        artist = COALESCE(
            (SELECT GROUP_CONCAT(a.name, ' ')
             FROM track_artists ta
             JOIN artists a ON ta.artist_id = a.id
             WHERE ta.track_id = OLD.track_id),
            ''
        )
    WHERE rowid = OLD.track_id;
END;

-- 4.6 Título del álbum actualizado -> actualiza todas las pistas del álbum
CREATE TRIGGER library_fts_album_update AFTER UPDATE OF title ON albums BEGIN
    UPDATE library_fts SET album = NEW.title
    WHERE rowid IN (SELECT id FROM tracks WHERE album_id = NEW.id);
END;

-- 4.7 Nombre del artista actualizado -> actualiza sus pistas
CREATE TRIGGER library_fts_artist_update AFTER UPDATE OF name ON artists BEGIN
    UPDATE library_fts
    SET artist = COALESCE(
        (SELECT GROUP_CONCAT(a.name, ' ')
         FROM track_artists ta
         JOIN artists a ON ta.artist_id = a.id
         WHERE ta.track_id = library_fts.rowid),
        ''
    )
    WHERE rowid IN (
        SELECT track_id FROM track_artists WHERE artist_id = NEW.id
    );
END;
