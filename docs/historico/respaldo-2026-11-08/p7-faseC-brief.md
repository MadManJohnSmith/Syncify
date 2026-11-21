# Brief — P7 Fase C: composable de biblioteca, selección por ids y limpiezas

Contrato: **ADR-0025** decisiones 2, 3 y 4. Hechos: **/tmp/p7-inventory.md** (§1,
§3, §4, §5, §6, §7, §12). Base: `33f0955` en `/tmp/syncify-p7` (rama
`p7/track-browser`; Fases A y B ya mergeadas en la rama). Fase UI pura.

## Alcance exacto (Q1: mapeo + paginación + selección; filtros/orden siguen en las vistas)

### Entregable: `ui/src/composables/useTrackBrowser.ts`

Para las listas basadas en biblioteca (LibraryView, MetadataView). Contrato:

- **Fuente declarada por la vista** (el composable no conoce los comandos):
  - Fuente paginada de biblioteca (LibraryView): recibe qué cargar por página
    (la triple fuente `get_library`/`get_favorite_tracks`/`get_duplicate_tracks`
    según filtros) y devuelve página + `total` + `has_more`. PAGE_SIZE 100 y el
    cambio de fuente en caliente se comportan HOY igual (inventario §4).
  - Fuente de búsqueda FTS (LibraryView): `searchTracks` con query prefijando `*`
    por palabra, paginador propio de 100 y `loadMore` — hoy vive en
    `performDatabaseSearch`/`loadMoreSearchResults` (1769-1827); el debounce de
    300 ms del input puede quedar en la vista o moverse (paridad: el ritmo no
    cambia).
  - Fuente de página única (MetadataView): `getLibrary(0,500)` o
    `getTracksNeedingMetadata(500)` según `filterType`, sin `has_more`.
- **Mapeo inyectado**: cada vista pasa su mapper (Library `mapToTrack`,
  Metadata `mapToMetadataTrack`) — los tipos de fila siguen siendo de la vista.
  El cast `(item as any).is_favorite` (1590) muere en la migración si toca.
- **Selección por ids** (decisión 3): `selectedIds: number[]` en el composable con
  `toggle(id)`, `selectOnly(id)`, `selectAll(currentRows)`, `clear()`,
  `preserve(ids vivos)` entre recargas (semántica exacta de MetadataView
  2488-2511 y 1596-1597) y un `selectedCount` derivado. LibraryView migra:
  - Muere `track.isSelected` (1536) y el contador incremental (1881-1884): el
    contador pasa a derivarse de ids (el comportamiento externo de L-05 no cambia:
    mismos números en la UI).
  - Las acciones masivas que filtran por flag (2022, 2258) pasan a filtrar por ids.
  - `toggleSelectAll` (2366-2370) usa `selectAll(currentRows)`.
- **Paginación con paridad**: `currentOffset/hasMore/loadMore` con la optimización
  de prefijo ordenado (`pageAppendCount/sortedPrefix/mergeSorted`, 2372-2468)
  conservada — puede vivir en la vista o en el composable, pero NO se reescribe a
  sort completo (paridad de rendimiento y de comportamiento).
- El deep-link `?track=<id>` (2791-2835, auto-paginación hasta 50 páginas,
  `router.replace`, readback del scroll) debe seguir funcionando igual
  (`LibraryViewTrackDeepLink.spec.ts` es la red).

### Migraciones

- **MetadataView**: fuente única de página fija, mapeo inyectado, selección ya por
  ids (pasa a usar el composable, conservando `selectTrack`/`toggleTrackSelection`/
  `selectAll` externos si el template los consume). El watch de ruta (2469-2475) y
  el reset del kernel (Fase B) no cambian.
- **LibraryView**: fuente paginada + FTS, mapeo, selección por ids. NO tocar
  filtros (2408-2437), sort (2381-2392), agrupación, columnas ni ruta.
- **Limpiezas (decisión 4)**: muere el tipo `'year'` de `groupBy` (1308) y las
  demos de `expandedGroups` (2518) (estado inicial: todo colapsado); muere el
  comentario S194 obsoleto (1888-1892).

### NO tocar

- DownloadsView (Fase B ya le puso el kernel; su lógica de cola no entra aquí).
- Filtros/orden/predicados de las vistas (siguen donde están).
- Agrupación más allá de las dos limpiezas.
- El toast local de MetadataView (deuda del ADR).
- Rust (Fase A ya alineó `quality_bucket`).

## Caracterización (obligatoria)

`ui/src/__tests__/composables/useTrackBrowser.spec.ts`: captura paginación (offset,
has_more, cambio de fuente, FTS con query prefijado y loadMore), selección
(toggle/selectOnly/selectAll/clear/preserve-ids-vivos), mapeo aplicado a las filas.
Si un test exige algo que la vista real no hace, gana la vista.

## Puertas

- `npm --prefix ui run build`
- `npm --prefix ui run test:run` — 128+ ficheros, 1108 tests mínimo. Los specs de
  LibraryView pueden necesitar ajustes SOLO donde asertaban el flag
  `isSelected`/el contador (documéntalo); el resto debe pasar tal cual.

## Commits

Uno o dos coherentes con puertas entre ellos; sugerido:
1. `refactor(ui): useTrackBrowser con paginación y selección por ids (ADR-0025, P7)`
2. `refactor(ui): Library y Metadata sobre el navegador y limpiezas (ADR-0025, P7)`
Mensajes en español, estilo del repo.

## Restricciones

- Solo `/tmp/syncify-p7`, rama `p7/track-browser`. No pushees. Fase UI pura.
- No toques el worktree principal, otros worktrees ni el target de Rust.
- No des-ignores tests; no relajes aserciones; sin `#[allow(dead_code)]`.

## Informe

1. API final del composable y qué quedó en cada vista.
2. Qué specs cambiaron y por qué (esperado: los del flag de Library).
3. Puertas (cifras), hashes.
4. Rarezas/divergencias descubiertas que la paridad haya obligado a conservar.
