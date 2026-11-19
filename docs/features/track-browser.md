# Navegador de pistas — comportamiento de las listas

**Estado de revisión:** 2026-11-07 (ADR-0025, rama `p7/track-browser`).

Una sola maquinaria sirve las listas de pistas de la UI. Este documento dice cómo
se comporta una lista; las vistas solo declaran su fuente.

## Piezas

- **`ui/src/composables/useVirtualWindow.ts`** — kernel de ventana virtual usado
  por las tres vistas grandes. Configuración explícita por vista; no hay copias.
- **`ui/src/composables/useTrackBrowser.ts`** — composable de las listas basadas
  en biblioteca (`LibraryView`, `MetadataView`): fuente declarada, mapeo inyectado,
  paginación offset/limit + `has_more`, búsqueda FTS con paginador propio y
  **selección por ids**.
- **`DownloadsView`** usa el kernel directamente: su cola muta filas en vivo con
  eventos de progreso y reordena por drag, y no comparte tipo de fila con las otras
  dos, así que no entra al contrato del composable.

## Fuentes declaradas

| Vista | Fuente | Paginación | Selección |
|---|---|---|---|
| `LibraryView` | `query_library` (L01, issue #98: filtros, orden, búsqueda y contadores EN SERVIDOR; módulo `commands/library_query.rs`) | offset/limit 100 (máx 200) + `has_more` + contadores en la primera página | ids |
| `LibraryAlbumsView` | `query_library_albums` (L07, issue #104: una fila por álbum vivo, cobertura, cuentas y servicios; módulo `commands/library_albums.rs`) | offset/limit 100 (máx 200) + `has_more` en servidor | sin selección |
| `MetadataView` | `get_library(0,500)` o `get_tracks_needing_metadata(500)` según `filterType` | página única, sin `has_more` | ids |
| `DownloadsView` | `get_queue` completa | sin paginación server-side; recorte client-side a 50 por sección y salto por posición | sin selección |

La optimización de prefijo ordenado de `LibraryView` (`sortedPrefix`/`mergeSorted`)
se retiró con L01: el orden lo fija el servidor y el renderer solo concatena
páginas. `search_tracks` sigue existiendo para la búsqueda global (CommandPalette,
SearchView); ya no sustituye la lista de la biblioteca.

## Selección por ids

Los ids seleccionados viven en el composable, nunca en las filas. `toggle` muta in
situ, `selectOnly` reemplaza si no está, `selectAll` reemplaza con las filas
visibles, y `preserve` mantiene solo ids vivos tras cada recarga. El contador es
derivado: no hay contadores incrementales paralelos.

**L05 (#102, 2026-11-18):** `LibraryView` añade un segundo modo, «todo el filtro»
+ exclusiones. La casilla de cabecera selecciona el total del servidor (no la
página cargada) y el contador se deriva de ese total menos las exclusiones; los
ids explícitos siguen siendo el otro modo. El modo filtro vive en la vista porque
`useTrackBrowser` no conoce filtros de biblioteca y `selectAll` conserva su
semántica de ids para `MetadataView`. Las acciones por lotes pasan por
`preview_library_action` + `run_library_action` (bloques de 500, eventos
agregados).

## Paridad congelada (deuda de homogenización)

La extracción preservó el comportamiento exacto de cada clon. Las divergencias son
configuración visible, no bugs nuevos: OVERSCAN 10/5/10, footer de 48 px solo en
`LibraryView` cuando no hay más páginas, throttle de scroll de 120 ms solo en
`LibraryView`, reset de viewport vacío distinto por vista, y `scrollToOffset` sin
recorte. Homogenizarlas es un cambio de una línea por vista y una decisión propia.

## Backend de las filas

Las tres fuentes de biblioteca computan `quality_bucket` en SQL con la misma
expresión CASE (test de paridad contra los literales del dashboard por fuente). El
filtro de calidad de `LibraryView` compara el bucket en las tres. Deuda: el
`metadata_score` de `search_tracks` no excluye `NOT_FOUND`/`MISMATCH` ni exige
`release_year > 0` — diverge del de `get_library`.

## Rutas

Las vistas leen la ruta y pocas la escriben: el deep-link de `LibraryView`
consume `?track=<id>` con `router.replace` paginando hasta 50 páginas;
`?filter=quality&quality=<bucket>` y `?filter=duplicates` (Library),
`?filter=needs_work` y `?trackId=` (Metadata), `?filter=failed` (Downloads, solo
en mount). `LibraryAlbumsView` comparte `q`, `sort` y `dir` con la pestaña de
pistas y los reescribe al cambiar texto u orden; cuenta, servicio y cobertura son
estado local de la lista de álbumes.

## Agrupación

Agrupar por artista/álbum/género/calidad es de `LibraryView`, no está virtualizada
y no vive en el composable. Si algún día se virtualiza, es un diseño propio (los
grupos solo son completos cuando acabó la paginación).

## Pruebas

- `ui/src/__tests__/composables/useVirtualWindow.spec.ts` — caracterización de las
  tres configs (índices, footer, throttle, reset, viewport vacío).
- `ui/src/__tests__/composables/useTrackBrowser.spec.ts` — caracterización de
  paginación, fuentes, FTS y selección.
- Los specs de vistas (`ui/src/__tests__/views/`) cubren el comportamiento de
  mount; `DownloadsViewQueue.spec.ts` fija R11 (la cola no es ref profundo).
