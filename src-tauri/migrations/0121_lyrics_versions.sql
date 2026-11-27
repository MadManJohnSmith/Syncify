-- Migration 0121 (LY08a, issue #404; ADR-0018 decisiones 1 a 3):
-- almacén de versiones de letras, conviviendo con el almacén antiguo.
--
-- El almacén antiguo (`lyrics`) tiene una fila por (track_id, format): una
-- escritura nueva pisa la anterior y no queda rastro de la Fuente ni del
-- idioma que la produjo. La identidad objetivo de una Versión de letra es
-- (pista, Fuente de letras, idioma, formato), pero un mismo hueco puede
-- contener varias versiones observadas en momentos distintos: la versión se
-- distingue por su contenido, no por el orden de llegada. Por eso la clave
-- única incluye `content_hash` y NO la tupla de cuatro columnas.
--
-- Solo datos nuevos (ADR-0018 decisión 3): esta migración no lee, no mueve y
-- no reescribe ninguna fila de `lyrics`. Las filas antiguas siguen intactas y
-- legibles; la lectura en unión, la regla de Versión activa y los consumidores
-- son #411. La tabla antigua deja de crecer cuando #411 mueva las escrituras.
--
-- `provider_id` es el identificador canónico del catálogo
-- (`lyrics_provider_settings.provider_id`, p. ej. `lrclib_synced`), no el
-- nombre de presentación que guarda `lyrics.source` (`LRCLIB`). La identidad
-- no puede depender de una etiqueta traducible.
--
-- `language` es una etiqueta BCP 47 en minúsculas; `'und'` significa idioma
-- no determinado (el detector heurístico puede no probar ninguno). Se usa un
-- valor NOT NULL en vez de NULL porque SQLite considera distintos los NULL en
-- una restricción UNIQUE y el mismo contenido sin idioma podría duplicarse.
--
-- `sync_level` conserva el vocabulario estable del motor: `syllable`, `word`,
-- `line` y `none` (las letras planas o instrumentales no tienen timings). El
-- Nivel de sincronía es el primer criterio de la regla de #411; guardarlo aquí
-- evita inferirlo del formato, que es propiedad del contenido y no su
-- identidad.
--
-- `content_hash` es SHA-256 en hexadecimal minúscula, calculado por el
-- escritor (SQLite no trae SHA-256). Repetir la misma observación es
-- idempotente: `ON CONFLICT` refresca solo los atributos derivados y conserva
-- el `id` y el `obtained_at` de la primera vez.
--
-- Dedup ISRC (esquema/constraint): la FK a `tracks` y la clave única están
-- pensadas para el mismo patrón que 0064 usa con `lyrics`:
--
--   UPDATE OR IGNORE lyrics_versions SET track_id = :winner WHERE track_id = :loser;
--   DELETE FROM lyrics_versions WHERE track_id = :loser;
--
-- Una versión idéntica en ambas pistas choca en la clave única, `OR IGNORE`
-- la deja en la pista perdedora y el DELETE la retira sin abortar la
-- transacción; las versiones propias de la perdedora se mueven a la ganadora.
-- La orquestación del remapeo es de #411.
--
-- Pin de versión (ADR-0018 decisión 2): vive en la propia fila
-- (`pinned_at`) y no en una tabla aparte para que el remapeo ISRC lo mueva con
-- la versión. El índice único parcial garantiza como mucho un pin por pista.
-- Si las dos pistas tienen pin, el de la ganadora prevalece y el de la
-- perdedora desaparece con su fila: la misma política de conflicto que ya
-- aplica 0064 al almacén antiguo.

CREATE TABLE lyrics_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL
        CHECK (provider_id <> '' AND provider_id = trim(provider_id)
               AND provider_id = lower(provider_id)),
    language TEXT NOT NULL DEFAULT 'und'
        CHECK (language <> '' AND language = trim(language)
               AND language = lower(language)),
    format TEXT NOT NULL
        CHECK (format <> '' AND format = trim(format)
               AND format = lower(format)),
    sync_level TEXT NOT NULL DEFAULT 'none'
        CHECK (sync_level IN ('syllable', 'word', 'line', 'none')),
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL
        CHECK (length(content_hash) = 64
               AND content_hash NOT GLOB '*[^0-9a-f]*'),
    embedded_in_file INTEGER NOT NULL DEFAULT 0
        CHECK (embedded_in_file IN (0, 1)),
    obtained_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    pinned_at TEXT,
    UNIQUE (track_id, provider_id, language, format, content_hash)
);

-- Orden de lectura del historial de una pista; la clave única ya sirve las
-- búsquedas por `track_id`, pero no el ORDER BY por fecha.
CREATE INDEX idx_lyrics_versions_track_obtained
    ON lyrics_versions(track_id, obtained_at DESC);

-- Un Pin de versión por pista, como mucho. El índice es parcial: las filas
-- sin pin no participan.
CREATE UNIQUE INDEX idx_lyrics_versions_pin_per_track
    ON lyrics_versions(track_id)
    WHERE pinned_at IS NOT NULL;

-- Rollback (manual, nunca se ejecuta desde una migración aplicada):
-- DROP INDEX IF EXISTS idx_lyrics_versions_pin_per_track;
-- DROP INDEX IF EXISTS idx_lyrics_versions_track_obtained;
-- DROP TABLE IF EXISTS lyrics_versions;
