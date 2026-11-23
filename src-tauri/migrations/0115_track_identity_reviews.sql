-- Migration 0115 (issue #73, ADR-0006 decisión 16): cola de revisión de
-- identidad de pista.
--
-- La decisión de identidad del sync es automática cuando la evidencia es
-- inequívoca: ISRC idéntico enlaza; sin ISRC, artista + título + duración
-- dentro de la tolerancia enlazan. Cuando la huella encuentra más de un Track
-- candidato, el sync NO fusiona: importa el ítem como Track propio y deja aquí
-- la evidencia de ambos lados para que una persona resuelva. El título por sí
-- solo nunca enlaza, así que no genera fila.
--
-- Esta migración no toca las columnas de `tracks` ni `track_sources` (0002,
-- 0097): la identidad se calcula desde lo que ya existe y la cola es un
-- registro aparte. Sí añade el índice aditivo `idx_tracks_duration_identity`
-- (abajo), que cambia el plan de la búsqueda por duración; el test de
-- duplicados lo tiene en cuenta.
--
-- `status` es la máquina de estados de la revisión:
--   pending           → el sync la dejó abierta, nadie ha decidido
--   resolved_merged   → una persona confirmó que los Tracks son el mismo
--   resolved_separate → una persona confirmó que son grabaciones distintas
--   dismissed         → la evidencia dejó de aplicar (p. ej. el ítem ya no está)
-- `resolved_track_id` y `resolved_at` los escribe la resolución (T08b); esta
-- migración los reserva para no volver a tocar la tabla.
--
-- `reason` es vocabulario del núcleo, no del proveedor: hoy
-- 'fingerprint_multiple_candidates' es el único motivo que llega aquí. Sin
-- CHECK para que un motivo nuevo sea un cambio de código y su comentario, no
-- otra migración (mismo criterio que 0114).
--
-- Idempotencia: hay como mucho una revisión `pending` por (servicio, ítem,
-- motivo). Reintentar el mismo ítem en otro sync refresca la evidencia en esa
-- fila; las decisiones ya tomadas no se reabren solas.

CREATE TABLE track_identity_reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL REFERENCES services(id),
    service_track_id TEXT NOT NULL CHECK (length(trim(service_track_id)) > 0),
    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved_merged', 'resolved_separate', 'dismissed')),
    incoming_title TEXT,
    incoming_artist TEXT,
    incoming_album TEXT,
    incoming_duration_ms INTEGER,
    incoming_isrc TEXT,
    resolved_track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT,
    CHECK (status = 'pending' OR resolved_at IS NOT NULL)
);

CREATE UNIQUE INDEX idx_track_identity_reviews_pending
    ON track_identity_reviews(service_id, service_track_id, reason)
    WHERE status = 'pending';

CREATE INDEX idx_track_identity_reviews_status
    ON track_identity_reviews(status, created_at DESC);

-- Evidencia del lado local, congelada en el momento de la detección: si el
-- Track cambia después, la revisión sigue mostrando lo que se comparó. El
-- delta de duración es la evidencia de la tolerancia (±2000 ms); NULL cuando
-- alguno de los dos lados no publica duración.
CREATE TABLE track_identity_review_candidates (
    review_id INTEGER NOT NULL REFERENCES track_identity_reviews(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    title TEXT,
    artist TEXT,
    album TEXT,
    duration_ms INTEGER,
    isrc TEXT,
    duration_delta_ms INTEGER,
    PRIMARY KEY (review_id, track_id)
);

CREATE INDEX idx_track_identity_review_candidates_track
    ON track_identity_review_candidates(track_id);

-- La huella acota la búsqueda por duración ±2 s: sin este índice el sync
-- recorrería `tracks` entera por cada ítem. Es aditivo y no cambia ninguna
-- consulta existente.
CREATE INDEX idx_tracks_duration_identity
    ON tracks(duration_ms) WHERE duration_ms IS NOT NULL;
