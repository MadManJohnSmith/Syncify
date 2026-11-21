# Brief — Inventario P7 · TrackBrowser

Inventario de HECHOS sobre `/tmp/syncify-p7` (rama `p7/track-browser`, base
`a2bb90d` = dev con P6 mergeado). Solo evidencia con `file:line`, sin propuestas ni
diseño. Las citas de líneas del informe original (2026-11-05, sobre `main`) hay que
RE-VERIFICARLAS contra este árbol: pueden haberse movido.

## Contexto de la parte (ficha 5 del informe)

Tres vistas grandes clonan el mismo comportamiento de lista: mapear el modelo,
filtrar, ordenar, paginar, virtualizar y sincronizar la ruta. La parte propone un
composable único (`useTrackBrowser(source)`) y borrar los clones. Los hallazgos
originales citaban: mapeo duplicado (`qualityBucket`, `availabilitySummary`,
`filePath`), ventanas virtuales con alturas distintas (56/64/60 px), 8 predicados de
filtro, switch de orden de 10 casos, paginación triplicada + paginador FTS,
filas agrupadas, reordenación y selección por ids.

## Qué inventariar (con file:line actualizados)

1. **Las tres vistas** (`ui/src/views/LibraryView.vue`, `MetadataView.vue`,
   `DownloadsView.vue`): tamaño real hoy (líneas), y para cada una:
   - el mapeo de modelo→fila (dónde y qué campos; si sigue duplicado entre vistas);
   - la ventana virtual (altura de fila, cómo se calcula, comentario de paridad);
   - predicados de filtro y switch de orden (cuántos, dónde);
   - paginación (offset/limit, hasMore, paginador FTS aparte si existe);
   - sincronización de ruta/URL (query params? estado en composable? store?);
   - filas agrupadas (por álbum/artista?), reordenación y selección por ids:
     mecánica exacta y dónde vive.
2. **Diferencias entre vistas**: qué hace cada una que las otras no (fuente de
   datos: biblioteca vs FTS vs descargas; campos de fila distintos; acciones de
   fila distintas). Esto define si un composable único es viable o si las fuentes
   divergen demasiado.
3. **Composables existentes** en `ui/src/composables/` que ya hagan parte de este
   trabajo (useTrack*? paginación? virtual scroll?) y su forma.
4. **API/IPC que consumen las vistas** (`ui/src/api/*`): wrappers usados por cada
   vista, payloads, y si queda algún `invoke` crudo en estas vistas.
5. **Tests UI** que cubren estas vistas/composables hoy (ficheros y qué cubren),
   y qué gates existen para UI (`npm run test:run` specs relevantes).
6. **Backend de las listas**: comandos que sirven filas (library/queue/downloads),
   forma de respuesta, paginación server-side si existe. Solo lo que las vistas
   consumen.
7. **Rarezas**: TODOs/FIXMEs/comentarios de paridad, hacks conocidos, puntos donde
   las tres copias ya divergen de forma visible.

## Restricciones

- Solo lectura. No compiles, no lances tests, no modifiques nada.
- No toques el worktree principal ni otros worktrees.
- El informe del inventario se entrega como texto: cada hecho con `file:line` del
  árbol de `/tmp/syncify-p7` (rama p7/track-browser).
