# Navegador de pistas — comportamiento de las listas

**Estado de revisión:** 2026-11-19 (L14, issue #109: cinco estados de
descarga y calidad efectiva en la fila; filtros de estado y calidad; cobertura
de álbum compartida con L07; acciones de encolar, re-descargar y borrar audio
sobre la cola existente. Previo: L13, issue #110: caché de páginas,
virtualización de grid y grupos, invalidación por evento).

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
- **`ui/src/composables/usePagedQueryCache.ts`** — caché de páginas por clave,
  deduplicación en vuelo y token de secuencia (L13, #110).
- **`ui/src/composables/libraryCacheInvalidation.ts`** y
  **`useLibraryCacheInvalidation.ts`** — señal síncrona de invalidación y matriz
  de eventos que la dispara.

## Fuentes declaradas

| Vista | Fuente | Paginación | Selección |
|---|---|---|---|
| `LibraryView` | `query_library` (L01, issue #98: filtros, orden, búsqueda y contadores EN SERVIDOR; módulo `commands/library_query.rs`) | offset/limit 100 (máx 200) + `has_more` + contadores en la primera página | ids |
| `LibraryAlbumsView` | `query_library_albums` (L07, issue #104: una fila por álbum vivo, cobertura, cuentas y servicios; módulo `commands/library_albums.rs`) | offset/limit 100 (máx 200) + `has_more` en servidor | sin selección |
| `LibraryArtistsView` | `query_library_artists` (L08, issue #105: una fila por artista vivo, pistas, álbumes, colaboraciones, cuentas y servicios; módulo `commands/library_artists.rs`) | offset/limit 100 (máx 200) + `has_more` en servidor | sin selección |
| `ArtistDetailView` | `get_artist` (cabecera y primera página = `top_tracks`, 5, sin contador de pistas propio: solo `tracks_total`) + `query_library_artist_tracks` (L08: todas las pistas vivas, paginadas con colaboradores y fuentes). `Download All` y `Shuffle Play` enumeran la colección completa paginando esa misma consulta (#336), no `top_tracks` | offset/limit 100 (máx 200) + `has_more` en servidor; la acción por lotes pagina a 200 | sin selección |
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

## Descargas y calidad (L14, #109)

La fila de `LibraryView` muestra cinco estados de descarga —`downloaded`,
`queued`, `downloading`, `failed`, `not_downloaded`— y la **calidad
efectiva** del fichero (`FLAC 24/96`, `MP3`). Ambos campos llegan en la misma
consulta de página (`query_library`), con subescalares sobre `downloads` y
`download_queue`: no hay una consulta por pista. `downloads.file_path` manda
sobre la cola; `cancelled`/`complete` sin ledger no se presentan como
descargados. El badge expone `data-download-status` para que la etiqueta sea
verificable; el icono y el tooltip conservan el estado.

Los filtros de la lista traducen cada estado a `download_statuses` del
contrato y los tres buckets de calidad a `quality_buckets` (los mismos del
deep-link FE-10 del dashboard). `Not Downloaded` significa ausencia de
descarga y de cola, no un cajón que mezcle los otros estados. La URL conserva
los cinco valores en `dl=`; el deep-link legacy `filter=notDownloaded` se
expande a los cuatro estados sin fichero.

La cobertura de álbum (`query_library_albums`, L07) usa la MISMA expresión
`DOWNLOAD_STATUS_SQL` de `commands/library_query.rs`: la regla de estados no
se duplica. Cada fila de álbum expone `downloadedCount`, `queuedCount`,
`downloadingCount`, `failedCount` y `notDownloadedCount`; la cobertura sigue
siendo `downloadedCount / trackCount` y es `null` cuando el total es cero (no
se inventa un porcentaje). Cola, descarga en curso y fallo no cuentan como
cobertura.

Acciones: encolar usa `add_to_queue` (la cola existente, con su guardrail de
deduplicación); re-descargar usa `force_redownload_tracks` (limpia el
Download y las filas de cola de esa pista y la vuelve a encolar por
`perform_add_to_queue`), no una segunda ruta. Borrar audio elimina el
Download, sus entradas de cola y el fichero, pero conserva Track, TrackSource,
pertenencia y evidencia de edición; es por pista y pide confirmación
(`delete_track_audio`, L04). La biblioteca no decide calidad ni fallback de
edición: solo encola, muestra estado y borra audio.

## Rutas

Las vistas leen la ruta y pocas la escriben: el deep-link de `LibraryView`
consume `?track=<id>` con `router.replace` paginando hasta 50 páginas;
`?filter=quality&quality=<bucket>` y `?filter=duplicates` (Library),
`?filter=needs_work` y `?trackId=` (Metadata), `?filter=failed` (Downloads, solo
en mount). `LibraryAlbumsView` y `LibraryArtistsView` comparten `q`, `sort` y
`dir` con la pestaña de pistas y los reescriben al cambiar texto u orden; cuenta,
servicio y cobertura (álbumes) o cuenta y servicio (artistas) son estado local de
cada lista.

## Caché de páginas e invalidación (L13, #110)

`ui/src/composables/usePagedQueryCache.ts` guarda páginas por clave de consulta
(sin `offset`, con `limit`) y deduplica peticiones en vuelo: dos cargas
simultáneas de la misma clave y página comparten una sola consulta. Cada
`invalidate()` incrementa una generación; una respuesta que empezó antes del
evento no se guarda y llega marcada con la generación vieja, para que la vista
la descarte. No hay TTL: la invalidación es explícita.

`ui/src/composables/libraryCacheInvalidation.ts` es la señal síncrona que
recorren los wrappers de la API después de confirmar una mutación (favorito,
playlist, metadatos, descarga, acción por lotes, importación de catálogo).
`ui/src/composables/useLibraryCacheInvalidation.ts` traduce los eventos de la app:
`library-updated`, `import-complete`, `sync-complete`, `sync-run-finished`,
`favorites_sync_completed` y `download_progress` SOLO cuando el estado es
terminal. La cuarentena (#72/L09) todavía no existe; cuando llegue debe emitir
`library-updated` o llamar a `invalidateLibraryPages('quarantine')`.

`useTrackBrowser` añade un token de secuencia de carga: una recarga descarta la
página de `loadMore` que estaba en vuelo, en vez de concatenarla a la consulta
nueva. En las pestañas de álbumes (L07) y artistas (L08) la misma caché envuelve
`query_library_albums` / `query_library_artists`; una invalidación invalida y
relee con debounce de 350 ms.

La cabecera de `LibraryView` conserva los contadores de la primera página: al
paginar (`offset > 0`) el servidor responde `counts: null` y la vista no los
toca. Los cambios de filtro y orden se coalescen en un microtask (una ráfaga de
clics deja una consulta) y la búsqueda mantiene el debounce de 250 ms.

## Agrupación

Agrupar por artista/álbum/género/calidad es de `LibraryView`, no vive en el
composable y no filtra ni ordena: los grupos aparecen en el orden en que el
servidor devuelve las filas y las pistas conservan ese orden (sin
`localeCompare` ni comparador de «fecha añadida»). Los grupos se aplanan en
cabeceras (64 px) y pistas (52 px) y se virtualizan con el modo variable de
`useVirtualWindow`; el grid se virtualiza por filas de baldosas y solo pinta la
ventana visible con overscan. Los grupos solo son completos cuando acabó la
paginación, pero el scroll pide la página siguiente cerca del final como en la
lista.

## Pruebas

- `ui/src/__tests__/composables/useVirtualWindow.spec.ts` — caracterización de las
  tres configs (índices, footer, throttle, reset, viewport vacío).
- `ui/src/__tests__/composables/useTrackBrowser.spec.ts` — caracterización de
  paginación, fuentes, FTS y selección.
- `ui/src/__tests__/composables/usePagedQueryCache.spec.ts` — caché por clave,
  deduplicación en vuelo y token de secuencia.
- `ui/src/__tests__/views/LibraryViewPerformance.spec.ts` — regresión del caso
  319 llamadas: un scroll de 4 minutos pide una página por límite, una ráfaga
  de filtros deja una consulta, la caché evita repetir una clave visitada, los
  contadores no se recalculan al paginar, grid/grupos renderizan solo la
  ventana y el panel consulta una vez al abrirse.
- `ui/src/__tests__/views/LibraryViewDownloads.spec.ts` — cinco estados,
  calidad efectiva, filtros de estado/calidad, encolar y re-descargar por las
  rutas existentes, borrado de audio con confirmación y una sola consulta de
  página (sin N+1).
- Los specs de vistas (`ui/src/__tests__/views/`) cubren el comportamiento de
  mount; `DownloadsViewQueue.spec.ts` fija R11 (la cola no es ref profundo).
