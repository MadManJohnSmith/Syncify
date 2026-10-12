-- 0093 — Caché persistente de consultas MusicBrainz por ISRC y búsqueda.
-- `payload` contiene el JSON de la primera grabación; NULL representa
-- únicamente una respuesta negativa real (HTTP 404 o lista vacía).
-- Los fallos de transporte, parseo y servidor no se guardan como negativos.
-- `fetched_at` permite expirar o purgar las entradas en una entrega posterior.

CREATE TABLE mb_query_cache (
    cache_key TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    payload TEXT,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_mb_query_cache_fetched ON mb_query_cache(fetched_at);
