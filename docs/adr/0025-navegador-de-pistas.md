# ADR-0025 — Navegador de pistas: kernel de ventana virtual y un composable de biblioteca

**Estado:** aceptado · **Fecha:** 2026-11-07
**Origen:** parte P7 del informe de arquitectura (ficha 5), cerrada en grilling con inventario verificado contra `dev` (`a2bb90d`).

## Contexto

Tres vistas clonan el comportamiento de lista con tres fuentes que ya divergieron:

- **LibraryView** (~3.0k líneas): paginación server-side (100/página) con triple
  fuente (`get_library`/`get_favorite_tracks`/`get_duplicate_tracks`), búsqueda FTS
  con paginador propio y debounce, 9 predicados de filtro, orden de 6 casos con
  optimización de prefijo ordenado, agrupación **no virtualizada**, reorden de
  columnas con persistencia, selección por flag en fila + contador.
- **MetadataView** (~3.2k líneas): una página fija de 500 sin paginar, filtros de
  9 casos client-side, orden de 3 casos, selección por array de ids.
- **DownloadsView** (~2.4k líneas): cola completa (`limit` 50000) sin orden ni
  selección, drag de filas con `reorder_queue`, recorte client-side a 50 por
  sección, mutación de filas **en vivo** con eventos de progreso.

La ventana virtual está triplicada y **ya divergió**: OVERSCAN 10/5/10, +48 px de
footer al spacer solo en Library, throttle de scroll (120 ms) solo en Library,
políticas de reset distintas. Los comentarios de paridad citan líneas muertas y el
de LibraryView (S194) aún menciona el protocolo `syncify-media` retirado (C-19).
Hay tres convenciones de selección sin helper común. El filtro de calidad de
Library degrada a comparación de string exacto en FTS y favoritos porque solo
`get_library` computa `quality_bucket` en SQL.

## Decisión

1. **Kernel de ventana virtual compartido**, usado por las tres vistas (muere el
   triple clon), con configuración explícita por vista (`rowHeight`, overscan,
   footer, throttle, reset): **paridad estricta** — cero cambios visuales; las
   divergencias quedan congeladas como configuración y su homogenización es deuda.
2. **Un composable de navegador de pistas** para las listas basadas en biblioteca
   (LibraryView, MetadataView): mapeo de fila, paginación offset/limit + `hasMore`
   y fuente de búsqueda, más **selección por ids** (Library migra su flag en fila;
   Metadata ya está ahí).
3. **La cola mantiene su lógica** sobre el kernel: mutación en vivo, drag de filas
   y secciones no entran al contrato del composable.
4. **Agrupación fuera** del composable (queda en LibraryView como hoy, incluida su
   no virtualización), con limpiezas puntuales de LibraryView: el tipo `'year'`
   muerto, los nombres de demo en `expandedGroups` y el comentario S194 obsoleto.
5. **Alineación mínima del backend**: `quality_bucket` entra en el SELECT de
   `search_tracks` y `get_favorite_tracks` con la expresión CASE ya existente en
   `get_library`, con test de paridad al estilo del existente; el filtro de calidad
   deja de degradar. La divergencia del `metadata_score` de FTS (sin exclusiones)
   queda documentada como deuda y no se toca.
6. **Higiene IPC incluida**: wrappers en `ui/src/api/` para `read_track_tags`,
   `write_track_tags`, `enrich_genre_lastfm`, `enrich_metadata` y
   `enrich_metadata_musicbrainz`; migración de los 9 puntos crudos (6 en
   MetadataView, 1 en LibraryView) y de los 2 bypasses de DownloadsView
   (`reorder_queue`, `show_in_folder` existiendo wrapper).
7. **Puerta de aceptación**: suite UI completa + tests de caracterización del
   kernel y del composable (incluidas las divergencias congeladas) + **pasada
   visual humana** de las tres vistas por el propietario antes del merge.

## Consecuencias

- Un fichero explica cómo se comporta una lista; desaparecen el triple clon de
  ventana virtual y los dos mapeadores de biblioteca. La complejidad se concentra,
  no se muda.
- La paridad estricta congela divergencias hoy invisibles como configuración
  explícita: homogenizarlas después es un cambio de una línea por vista.
- La selección queda unificada por ids en toda la app; las acciones masivas de
  Library dejan de filtrar por flag de fila.
- El backend queda alineado en `quality_bucket` para las tres fuentes de
  biblioteca; el contrato `LibraryPage`/`SearchResult` no cambia de forma.
- Deuda declarada: `metadata_score` de FTS divergente, agrupación no virtualizada
  (problema de diseño propio si algún día se virtualiza), toast local de
  MetadataView no unificado con `useToast` (lleva el fix de ciclo de vida MT-02),
  homogenización de OVERSCAN/footer/throttle/reset.
