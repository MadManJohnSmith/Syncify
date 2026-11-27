-- Migration 0120 (issue #403, ADR-0006 decisiones 15, 23, 27-28 y 34):
-- modo de conflicto por playlist, default por servicio y evidencia de revisión.
--
-- Una playlist remota puede estar en uno de tres modos:
--   editable       → el sync aplica los cambios del proveedor cuando no hay
--                    edición local; si ambos lados cambiaron lo mismo, el
--                    conflicto va a `playlist_conflict_reviews` y no se pisa.
--   gana_proveedor → el sync restaura el nombre y la pertenencia del origen.
--   gana_local     → el sync observa al proveedor pero no modifica la playlist.
--
-- El default inicial es «gana el proveedor» (decisión 27). `service_preferences`
-- fija el default de las playlists NUEVAS de cada servicio; cambiarlo después no
-- reescribe playlists ya existentes. El trigger solo actúa al insertar una fila
-- con el valor de fábrica.
--
-- `playlist_sources` conserva la última observación del proveedor (nombre y
-- lista ordenada de track_id locales). Esa huella es la base para distinguir una
-- edición local de un cambio del proveedor en modo editable; sin ella, el sync
-- no puede saber quién cambió cada lado y manda el conflicto a revisión en vez
-- de sobrescribir.

ALTER TABLE playlists
    ADD COLUMN conflict_mode TEXT NOT NULL DEFAULT 'gana_proveedor'
        CHECK (conflict_mode IN ('editable', 'gana_proveedor', 'gana_local'));

ALTER TABLE service_preferences
    ADD COLUMN playlist_conflict_mode TEXT NOT NULL DEFAULT 'gana_proveedor'
        CHECK (playlist_conflict_mode IN ('editable', 'gana_proveedor', 'gana_local'));

ALTER TABLE playlist_sources ADD COLUMN provider_name TEXT;

-- JSON array de track_id locales en el orden del proveedor. Se valida en Rust
-- al leerlo; un valor corrupto se trata como «sin huella», nunca como igualdad.
ALTER TABLE playlist_sources ADD COLUMN provider_track_ids TEXT;

ALTER TABLE playlist_sources ADD COLUMN provider_snapshot_at TEXT;

CREATE TRIGGER trg_playlists_conflict_mode_service_default
AFTER INSERT ON playlists
FOR EACH ROW
WHEN NEW.conflict_mode = 'gana_proveedor'
BEGIN
    UPDATE playlists
    SET conflict_mode = COALESCE((
            SELECT sp.playlist_conflict_mode
            FROM service_preferences sp
            JOIN accounts a ON a.id = NEW.account_id
            JOIN services s ON s.id = a.service_id
            WHERE sp.service_name = s.name
        ), 'gana_proveedor')
    WHERE id = NEW.id
      AND conflict_mode = 'gana_proveedor';
END;

-- Cola de revisión de conflictos de playlist. Una fila `needs_review` por
-- playlist y tipo de conflicto: un conflicto repetido refresca la evidencia en
-- vez de llenar la cola de duplicados. La resolución es de #410; aquí solo se
-- reservan los estados y la evidencia de ambos lados.
CREATE TABLE playlist_conflict_reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    conflict_kind TEXT NOT NULL CHECK (conflict_kind IN ('name', 'membership')),
    local_value TEXT,
    provider_value TEXT,
    status TEXT NOT NULL DEFAULT 'needs_review'
        CHECK (status IN ('needs_review', 'resolved_local', 'resolved_provider', 'dismissed')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT,
    CHECK (status = 'needs_review' OR resolved_at IS NOT NULL)
);

CREATE UNIQUE INDEX idx_playlist_conflict_reviews_pending
    ON playlist_conflict_reviews(playlist_id, conflict_kind)
    WHERE status = 'needs_review';

CREATE INDEX idx_playlist_conflict_reviews_status
    ON playlist_conflict_reviews(status, created_at DESC);

-- Rollback (manual, nunca se ejecuta desde una migración aplicada):
-- DROP INDEX IF EXISTS idx_playlist_conflict_reviews_status;
-- DROP INDEX IF EXISTS idx_playlist_conflict_reviews_pending;
-- DROP TABLE IF EXISTS playlist_conflict_reviews;
-- DROP TRIGGER IF EXISTS trg_playlists_conflict_mode_service_default;
-- ALTER TABLE playlist_sources DROP COLUMN provider_snapshot_at;
-- ALTER TABLE playlist_sources DROP COLUMN provider_track_ids;
-- ALTER TABLE playlist_sources DROP COLUMN provider_name;
-- ALTER TABLE service_preferences DROP COLUMN playlist_conflict_mode;
-- ALTER TABLE playlists DROP COLUMN conflict_mode;
