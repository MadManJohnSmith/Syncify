-- Migration 0063 (S198): add qobuz_id to albums.
--
-- Owner's live audit (docs/s197_auditoria_importaciones.md, 2026-08-24) proved
-- only the Tidal arm persists favorite albums (is_favorite=1, via tidal_id).
-- The Qobuz sync downloads favorite-album catalogs for expansion but cannot
-- mark them: the albums table had no qobuz_id column at all. Mirrors
-- migration 0036 (albums.spotify_id) and 0037 (albums.tidal_id).

ALTER TABLE albums ADD COLUMN qobuz_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_albums_qobuz_id ON albums(qobuz_id) WHERE qobuz_id IS NOT NULL;
