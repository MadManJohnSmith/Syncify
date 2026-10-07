-- 0091 — Fase 1 del player: estadísticas de reproducción local.
--
-- `tracks.play_count` / `tracks.last_played` cuentan escuchas de pistas
-- locales (fase 1: archivos ya descargados; sin streaming). El comando
-- `record_local_play` incrementa el contador UNA sola vez por sesión de
-- escucha (`playback_listens.listen_session_id`, UUID v4 canónico generado
-- por la UI por reproducción lógica); los reintentos de IPC con el mismo
-- token caen en ON CONFLICT DO NOTHING y no tocan `tracks`.
--
-- DECISIÓN DOCUMENTADA (fase 1): `playback_listens` crece una fila por
-- escucha SIN purga. Trabajo futuro: purga por antigüedad de
-- `counted_at` (p. ej. mantener un horizonte fijo) si el volumen de filas
-- lo llega a requerir; el recuento agregado vive en `tracks` y no depende
-- de la tabla de sesiones.
--
-- DECISIÓN DOCUMENTADA (fase 1): el backup portátil NO exporta
-- `play_count`/`last_played` (commands/backup.rs), igual que no exporta
-- otros datos derivados del uso; se pierden al restaurar en otra máquina.
--
-- Las pistas con duración < 30 s no cuentan reproducción jamás: lo decide
-- la UI (umbral de 30 s de reproducción real acreditada) antes de invocar
-- `record_local_play`; el backend garantiza atomicidad e idempotencia, no
-- puede certificar por sí solo el tiempo audible.

ALTER TABLE tracks ADD COLUMN play_count INTEGER NOT NULL DEFAULT 0 CHECK(play_count >= 0);
ALTER TABLE tracks ADD COLUMN last_played TEXT;

CREATE TABLE playback_listens (
    listen_session_id TEXT PRIMARY KEY,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    counted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_playback_listens_track ON playback_listens(track_id);
