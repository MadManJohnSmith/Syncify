-- no-transaction
-- Migration 0108: PL00 — Modelo de playlist local (#153, spec padre #152)
--
-- La playlist pasa a ser una entidad local canónica:
--   * playlists.account_id pasa a nullable: una playlist local no es de ninguna
--     cuenta (borrar la cuenta no puede llevársela por CASCADE).
--   * playlists.local_uuid: identidad estable e independiente del nombre para
--     las locales (renombrar no cambia la identidad).
--   * playlists.deleted_at: columna de tombstone. El borrado con tombstone y su
--     semántica frente al sync llegan en fases posteriores de #152; esta
--     migración añade la columna y las lecturas ya la filtran.
--   * playlist_tracks se reconstruye garantizando la unicidad por
--     (playlist_id, position) (ya presente desde 0064) y sanea posiciones
--     canónicas 1..N sin huecos sobre bases históricas. La misma pista puede
--     aparecer varias veces en la misma playlist.
--
-- Las locales existentes (service_playlist_id LIKE 'local_%', hoy en cuenta 1)
-- se reasignan sin cuenta y con local_uuid generado.
--
-- Reconstruye las tres tablas porque SQLite no puede relajar un NOT NULL con
-- ALTER TABLE. Recrea índices y triggers que mueren con cada DROP. El orden es
-- seguro con las FK activas y dentro de una transacción (el migrador de sqlx
-- para SQLite envuelve cada migración en una aunque el fichero declare
-- -- no-transaction): primero se staguean los hijos SIN claves foráneas,
-- porque dropear la tabla padre con FK activas borraría sus filas y dispararía
-- el ON DELETE CASCADE de playlist_tracks y playlist_sources. Con el staging,
-- el DROP de los hijos es seguro y el del padre no deja nada a quien cascada.
-- Los ids no cambian, así que la integridad referencial se conserva. El índice
-- UNIQUE(account_id, service_playlist_id) solo gobierna a las playlists con
-- cuenta: SQLite trata los NULL como distintos, y la identidad de las locales
-- es local_uuid (índice único parcial).

-- La vista library_stats (0085) referencia `playlists`: el DROP de la tabla la
-- deja rota y el ALTER TABLE RENAME posterior re-parsea las vistas y fallaría.
-- Se recrea al final, ya con el filtro de tombstones.
DROP VIEW IF EXISTS library_stats;

-- Staging de las tres tablas SIN FKs ni índices (CREATE TABLE AS SELECT).
CREATE TABLE _pl00_playlists_stage AS
SELECT id, account_id, service_playlist_id, name, description, owner_name, owner_id,
       is_public, is_collaborative, image_url, track_count, last_synced,
       external_id, source_service, is_smart, rules_json, created_at, updated_at
FROM playlists;

CREATE TABLE _pl00_playlist_tracks_stage AS
SELECT id, playlist_id, track_id, position, added_at
FROM playlist_tracks;

CREATE TABLE _pl00_playlist_sources_stage AS
SELECT id, playlist_id, account_id, service_id, service_playlist_id, synced_at, snapshot_id
FROM playlist_sources;

-- Con los hijos ya stagueados, dropearlos y dropear el padre no cascada nada.
DROP TABLE playlist_tracks;
DROP TABLE playlist_sources;
DROP TABLE playlists;

CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
    service_playlist_id TEXT,
    name TEXT NOT NULL,
    description TEXT,
    owner_name TEXT,
    owner_id TEXT,
    is_public INTEGER DEFAULT 0,
    is_collaborative INTEGER DEFAULT 0,
    image_url TEXT,
    track_count INTEGER DEFAULT 0,
    last_synced TEXT,
    local_uuid TEXT,
    deleted_at TEXT,
    external_id TEXT,
    source_service TEXT,
    is_smart INTEGER DEFAULT 0,
    rules_json TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Copia 1:1 conservando ids; las locales se reasignan a continuación.
INSERT INTO playlists (
    id, account_id, service_playlist_id, name, description, owner_name, owner_id,
    is_public, is_collaborative, image_url, track_count, last_synced,
    local_uuid, deleted_at, external_id, source_service, is_smart, rules_json,
    created_at, updated_at
)
SELECT
    id, account_id, service_playlist_id, name, description, owner_name, owner_id,
    is_public, is_collaborative, image_url, track_count, last_synced,
    NULL, NULL, external_id, source_service, is_smart, rules_json,
    created_at, updated_at
FROM _pl00_playlists_stage;

-- Las locales existentes (local_%, hoy en cuenta 1) se reasignan sin cuenta,
-- sin rastro de servicio y con UUID v4 formado en SQL (una sola llamada a
-- randomblob por fila en la subconsulta: versión '4' en el tercer grupo y
-- variante 8/9/a/b en el cuarto).
UPDATE playlists
SET account_id = NULL,
    source_service = NULL,
    local_uuid = lower(
        substr(seed.h, 1, 8) || '-' || substr(seed.h, 9, 4) || '-4' ||
        substr(seed.h, 13, 3) || '-' ||
        substr('89ab', seed.v + 1, 1) || substr(seed.h, 17, 3) || '-' ||
        substr(seed.h, 21, 12)
    )
FROM (
    SELECT id, hex(randomblob(16)) AS h, (abs(random()) % 4) AS v
    FROM playlists
    WHERE substr(service_playlist_id, 1, 6) = 'local_'
) AS seed
WHERE playlists.id = seed.id;

-- Índices recreados (0002, 0035, 0049, 0067, 0084) + identidad local y tombstone.
CREATE INDEX IF NOT EXISTS idx_playlists_account ON playlists(account_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_playlists_unique ON playlists(account_id, service_playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlists_name_search ON playlists(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_playlists_external_id ON playlists(external_id);
CREATE INDEX IF NOT EXISTS idx_playlists_source_service ON playlists(source_service);
CREATE INDEX IF NOT EXISTS idx_playlists_is_smart ON playlists(is_smart);
CREATE UNIQUE INDEX IF NOT EXISTS idx_playlists_local_uuid ON playlists(local_uuid) WHERE local_uuid IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_playlists_deleted_at ON playlists(deleted_at);

-- Trigger de 0067, recreado verbatim (muere con el DROP de la tabla). Con
-- account_id NULL la rama de source_service no dispara (guard account_id IS NOT NULL).
CREATE TRIGGER IF NOT EXISTS trg_playlists_sync_ins
AFTER INSERT ON playlists
FOR EACH ROW
BEGIN
    UPDATE playlists
    SET external_id = COALESCE(NEW.external_id, NEW.service_playlist_id),
        service_playlist_id = COALESCE(NEW.service_playlist_id, NEW.external_id),
        source_service = COALESCE(
            NEW.source_service,
            (SELECT s.name FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.id = NEW.account_id)
        )
    WHERE id = NEW.id
      AND (
          (NEW.external_id IS NULL AND NEW.service_playlist_id IS NOT NULL)
          OR (NEW.service_playlist_id IS NULL AND NEW.external_id IS NOT NULL)
          OR (NEW.source_service IS NULL AND NEW.account_id IS NOT NULL)
      );
END;

-- playlist_tracks reconstruida: unicidad por (playlist_id, position) y
-- posiciones canónicas 1..N sin huecos (orden de 0077) sobre las filas
-- históricas. No descarta ninguna pista: las repeticiones del mismo track
-- son legales en el modelo nuevo; solo las colisiones de posición (que una
-- base sana no tiene) se sanea asignando el orden canónico.
CREATE TABLE playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    added_at TEXT,
    UNIQUE(playlist_id, position)
);

INSERT INTO playlist_tracks (id, playlist_id, track_id, position, added_at)
SELECT
    id,
    playlist_id,
    track_id,
    ROW_NUMBER() OVER (
        PARTITION BY playlist_id
        ORDER BY position ASC, added_at ASC, id ASC
    ),
    added_at
FROM _pl00_playlist_tracks_stage;

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track ON playlist_tracks(track_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_pos ON playlist_tracks(playlist_id, position);

-- playlist_sources reconstruida verbatim de 0048 (no cambia de esquema, pero
-- murió con el DROP ordenado de arriba y se restaura desde su staging).
CREATE TABLE playlist_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    service_playlist_id TEXT NOT NULL,
    synced_at TEXT DEFAULT CURRENT_TIMESTAMP,
    snapshot_id TEXT,
    UNIQUE(account_id, service_playlist_id)
);

INSERT INTO playlist_sources (id, playlist_id, account_id, service_id, service_playlist_id, synced_at, snapshot_id)
SELECT id, playlist_id, account_id, service_id, service_playlist_id, synced_at, snapshot_id
FROM _pl00_playlist_sources_stage;

CREATE INDEX IF NOT EXISTS idx_playlist_sources_playlist ON playlist_sources(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_sources_lookup ON playlist_sources(service_id, service_playlist_id);

-- Triggers de 0075, recreados verbatim (mueren con el DROP de playlist_sources).
CREATE TRIGGER IF NOT EXISTS trg_prevent_playlist_source_collision_ins
BEFORE INSERT ON playlist_sources
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM playlist_sources
    WHERE playlist_id = NEW.playlist_id
      AND account_id = NEW.account_id
      AND service_playlist_id != NEW.service_playlist_id
)
BEGIN
    SELECT RAISE(ABORT, 'Collision detected: playlist_id already mapped to a different service_playlist_id for this account');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_playlist_source_collision_upd
BEFORE UPDATE OF playlist_id, account_id, service_playlist_id ON playlist_sources
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM playlist_sources
    WHERE playlist_id = NEW.playlist_id
      AND account_id = NEW.account_id
      AND service_playlist_id != NEW.service_playlist_id
      AND id != NEW.id
)
BEGIN
    SELECT RAISE(ABORT, 'Collision detected: playlist_id already mapped to a different service_playlist_id for this account');
END;

-- library_stats recreada verbatim de 0085; el único cambio es el filtro de
-- tombstones en el recuento de playlists.
CREATE VIEW library_stats AS
SELECT
    (SELECT COUNT(*) FROM tracks) as total_tracks,
    (SELECT COUNT(*) FROM artists) as total_artists,
    -- Albums: use the albums table if populated, otherwise count distinct album references
    (SELECT CASE
        WHEN (SELECT COUNT(*) FROM albums) > 0 THEN (SELECT COUNT(*) FROM albums)
        ELSE (SELECT COUNT(DISTINCT album_id) FROM tracks WHERE album_id IS NOT NULL)
     END) as total_albums,
    (SELECT COUNT(*) FROM downloads) as total_downloads,
    (SELECT COUNT(*) FROM download_queue WHERE status = 'queued') as queued_downloads,
    (SELECT COUNT(*) FROM download_queue WHERE status = 'downloading') as active_downloads,
    (SELECT COUNT(*) FROM library_entries) as library_entries,
    (SELECT COUNT(*) FROM playlists WHERE deleted_at IS NULL) as playlists,
    (SELECT COUNT(DISTINCT service_id) FROM track_sources) as services_with_data;

DROP TABLE _pl00_playlists_stage;
DROP TABLE _pl00_playlist_tracks_stage;
DROP TABLE _pl00_playlist_sources_stage;

PRAGMA foreign_keys = ON;
