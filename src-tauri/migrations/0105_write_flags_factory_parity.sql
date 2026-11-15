-- SB03 (#198): fábrica = comportamiento actual para los flags write_*.
-- Las columnas existían pero nadie las leía: el escritor emitía WORK/COMPOSER,
-- LYRICS/UNSYNCEDLYRICS y el trío SYNCIFY_*_SOURCE sin condición. Al cablear
-- los flags, la semilla 0 de estos tres rompería la paridad de fábrica (AC de
-- SB03): se alinea con lo que el escritor ya escribe.
-- Solo filas vírgenes (los tres aún en semilla 0); si alguno se apartó de la
-- semilla, ese valor ahora gobierna de verdad y se respeta.
UPDATE metadata_preferences
SET write_work_composer = 1,
    write_lyrics_tags = 1,
    write_download_source = 1
WHERE id = 1
  AND write_work_composer = 0
  AND write_lyrics_tags = 0
  AND write_download_source = 0;

-- enable_lastfm: el enriquecedor existe (services/lastfm.rs) y el fallback de
-- enrich_track lo intenta hoy sin condición; la semilla 0 de 0023 rompería la
-- paridad al cablear la puerta. Un 1 almacenado (antes el toggle no hacía
-- nada, así que es indistinguible de la semilla) se conserva.
UPDATE metadata_preferences
SET enable_lastfm = 1
WHERE id = 1
  AND enable_lastfm = 0;
