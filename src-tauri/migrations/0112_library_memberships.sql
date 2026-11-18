-- Migration 0112: pertenencia de biblioteca por cuenta, track y colección
-- (L00, issue #97; spec #96, ADR-0008 decisión 1).
--
-- `library_memberships` — LA pertenencia real: una fila por cuenta, Track
-- canónico y colección de origen (favorito, álbum, playlist o artista), con
-- el ítem del proveedor que la produjo y el `added_at` CRUDO del proveedor
-- (puede ser NULL o «1970-01-01»: el materializado los descarta). Las
-- colecciones sin fila propia (favoritos, compras e importación local)
-- dejan `collection_id` NULL; `playlist`/`album`/`artist` guardan el id
-- LOCAL de la colección (playlists.id / albums.id / artists.id) para que
-- el filtro por colección y el detalle entren por joins.
--
-- Decisiones que este esquema fija (informe de #96, decisión del propietario
-- del 2026-11-16):
-- - `library_items` (0067) NO es la pertenencia por cuenta: es el espejo de
--   una fila por Track con la mejor fuente del flujo de migración mbdump.
--   Ninguna consulta la lee como pertenencia; aquí se crea la real.
-- - El favorito deja de expandirse por release: la clave de identidad de la
--   pertenencia es (cuenta, track, colección) — la misma grabación produce
--   UNA pertenencia por cuenta y colección, no una por release. El índice
--   único usa `ifnull(collection_id, -1)` porque en SQLite los NULL son
--   distintos entre sí en un índice UNIQUE.
-- - La cuenta de una pista sale SOLO de sus pertenencias; no se infiere ni
--   se rellena con la cuenta activa (ADR-0007 decisión 1).
--
-- `tracks.library_added_at` — materialización del máximo `added_at` no nulo
-- y distinto de «1970-01-01» sobre las pertenencias vivas de la pista
-- (library_entries + playlist_tracks + library_memberships), con
-- `tracks.created_at` como respaldo. La mantienen el sync y el importador en
-- cada escritura de pertenencia (services/library_membership.rs); el orden
-- por fecha de añadido de la vista pasa a leerla.
--
-- Cuarentena (#72): esta migración no crea el estado de cuarentena — lo
-- escribe #72 y lo consume L09. El enganche queda en el modelo: las
-- pertenencias no se borran, así que una pertenencia retirada puede marcarse
-- en cuarentena sin perder el rastro.

CREATE TABLE library_memberships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    collection_type TEXT NOT NULL CHECK (collection_type IN
        ('favorite', 'album', 'playlist', 'artist', 'purchase', 'library')),
    collection_id INTEGER,
    provider_item TEXT,
    added_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Identidad de la pertenencia (NULL-safe para colecciones sin fila).
CREATE UNIQUE INDEX uq_library_memberships_identity
    ON library_memberships(account_id, track_id, collection_type, ifnull(collection_id, -1));
-- Índices mínimos de la vista (issue #97): pertenencias por track+fecha y
-- por cuenta+track; la colección para el filtro por colección.
CREATE INDEX idx_library_memberships_track_added
    ON library_memberships(track_id, added_at);
CREATE INDEX idx_library_memberships_account_track
    ON library_memberships(account_id, track_id);
CREATE INDEX idx_library_memberships_collection
    ON library_memberships(collection_type, collection_id);

-- Índice del materializado para ordenar por fecha de añadido.
ALTER TABLE tracks ADD COLUMN library_added_at TEXT;
CREATE INDEX idx_tracks_library_added_at ON tracks(library_added_at);

-- Los demás índices mínimos enumerados por el issue YA EXISTEN y no se
-- recrean: playlist_tracks(track_id) es idx_playlist_tracks_track (0006/0110),
-- track_sources(track_id) es idx_track_sources_track (0002), tracks(album_id)
-- es idx_tracks_album (0002), downloads(track_id) es idx_downloads_track
-- (0002/0085) y (account_id, track_id) en library_entries lo cubre el
-- índice único de su UNIQUE (0002). Falta el compuesto de artista:
CREATE INDEX idx_track_artists_artist_role ON track_artists(artist_id, role);

-- Backfill: la pertenencia existente se traslada a la tabla nueva para que
-- el upgrade no pierda ni la visibilidad ni la cuenta de lo ya importado.
-- library_entries: favorito (is_liked), compra (is_purchased) o entrada de
-- biblioteca sin colección (importación local y catálogo de playlists que
-- el motor escribe con is_liked=0).
INSERT OR IGNORE INTO library_memberships
    (account_id, track_id, collection_type, collection_id, provider_item, added_at)
SELECT account_id, track_id,
       CASE WHEN is_liked = 1 THEN 'favorite'
            WHEN is_purchased = 1 THEN 'purchase'
            ELSE 'library' END,
       NULL, NULL, added_at
FROM library_entries;

-- Playlists con dueño: la cuenta es la del dueño (la misma regla de la
-- vista). Las playlists locales (account_id NULL, PL00) no generan
-- pertenencia: su pertenencia viva sigue siendo playlist_tracks.
INSERT OR IGNORE INTO library_memberships
    (account_id, track_id, collection_type, collection_id, provider_item, added_at)
SELECT pl.account_id, pt.track_id, 'playlist', pl.id, NULL, pt.added_at
FROM playlist_tracks pt
JOIN playlists pl ON pl.id = pt.playlist_id
WHERE pl.account_id IS NOT NULL;

-- Materialización inicial de library_added_at sobre TODAS las pistas: el
-- máximo added_at vivo no nulo y distinto de 1970, con created_at como
-- respaldo. Misma expresión que mantiene services/library_membership.rs.
UPDATE tracks SET library_added_at = COALESCE(
    (SELECT MAX(m.added_at) FROM (
        SELECT le.added_at AS added_at FROM library_entries le WHERE le.track_id = tracks.id
        UNION ALL
        SELECT pt.added_at AS added_at FROM playlist_tracks pt WHERE pt.track_id = tracks.id
        UNION ALL
        SELECT lm.added_at AS added_at FROM library_memberships lm WHERE lm.track_id = tracks.id
    ) m WHERE m.added_at IS NOT NULL AND m.added_at NOT LIKE '1970-01-01%'),
    created_at);
