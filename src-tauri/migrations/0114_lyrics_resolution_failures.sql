-- Migration 0114 (LY03, issue #184): caché negativa persistente de resolución
-- de letras.
--
-- Un fallo de resolución se recuerda por (pista, Fuente de letras) con su
-- motivo y su caducidad, para que un segundo intento —manual o de un barrido—
-- no vuelva a sondear esa Fuente hasta que expire. El resultado positivo NO se
-- guarda aquí: se persiste como letra o sidecar. La Marca de borrado (0098) es
-- voluntad del usuario, no evidencia de resolución, y nunca escribe ni lee esta
-- tabla.
--
-- `reason` es la clasificación estable del fallo y la escribe el clasificador
-- del motor (la persistencia es del núcleo): 'not_found' | 'policy_rejected' |
-- 'requires_auth' | 'rate_limited' | 'source_unavailable' | 'failed'. `detail`
-- conserva el mensaje de diagnóstico del proveedor y puede ser NULL. No hay
-- CHECK sobre `reason` para no convertir cada motivo nuevo en una migración: el
-- vocabulario vive en el clasificador del crate y este comentario.
--
-- `expires_at` es una marca absoluta en UTC (`datetime('now')`); una fila está
-- vigente mientras `expires_at > datetime('now')`. Las filas caducadas no se
-- borran al leer: el lector las ignora y el índice por caducidad sirve al
-- barrido/purga que las retire. `recorded_at` permite auditar cuándo se observó
-- el fallo.

CREATE TABLE lyrics_resolution_failures (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL CHECK (length(provider_id) > 0),
    reason TEXT NOT NULL CHECK (length(reason) > 0),
    detail TEXT,
    recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    PRIMARY KEY (track_id, provider_id)
);

CREATE INDEX idx_lyrics_resolution_failures_expires
    ON lyrics_resolution_failures(expires_at);
