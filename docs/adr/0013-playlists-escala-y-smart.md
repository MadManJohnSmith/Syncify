# ADR-0013 — Playlists: paginación, smart playlists y portadas

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo PLAYLISTS.
**Spec:** #152 · **Tickets:** #153–#163.

## Contexto

`get_playlist_tracks` recorta `limit` a 500 (`src-tauri/src/commands/library.rs`) y
`ui/src/views/PlaylistView.vue` carga solo la primera página: una playlist de más de
500 pistas se muestra truncada.

Las smart playlists existen desde la migración 0084
(`src-tauri/migrations/0084_smart_playlists_rules.sql`) con reglas AND sobre género,
calidad, artista, año, servicio, letras, fecha y título; no tienen OR, orden ni límite.

Las playlists locales no tienen portada; las importadas usan `image_url` del proveedor.

## Decisión

1. **Paginación real y virtualización.** `get_playlist_tracks` mantiene páginas, pero
   la vista carga bajo demanda y virtualiza; no hay tope de 500 visible. El export
   recorre todas las páginas.

2. **Smart playlists v2.** Campos nuevos: álbum, duración, play_count, último play,
   favorito y estado de descarga, además de los actuales. Reglas con grupos AND/OR
   anidados, orden configurable y límite de 1 a 10.000.

3. **Evaluación en SQL, segura y con índice.** El query builder parametriza cada
   valor; preview de recuento y primera página; prueba de rendimiento con 30k.

4. **Portadas.** Imagen del proveedor si existe; si no, mosaico determinista de hasta
   cuatro portadas de pistas. Las smart usan el mosaico de su resultado actual.

5. **Lista y recuento.** Las playlists se agrupan por locales, servicio y smart; el
   `track_count` se reconcilia desde las filas y cuenta ocurrencias, no pistas únicas.

6. **Import/export conserva orden y repeticiones.** M3U/CSV/TXT al importar y M3U al
   exportar; nada se deduplica en silencio.

## Consecuencias

- Las reglas v2 se guardan en `rules_json` con versión; las v1 se leen como AND.
- El builder con grupos necesita tests de inyección y de precedencia.
- La virtualización reutiliza el patrón de biblioteca/descargas.
- El mosaico se cachea y se invalida al cambiar la pertenencia.

## Alternativas descartadas

- **Subir el tope de 500.** Sigue sin escalar y no arregla la vista.
- **Dejar reglas AND-only.** No cubre «últimas 100 escuchadas de este género».
- **Sin portadas.** La playlist local quedaría como una fila de texto.
