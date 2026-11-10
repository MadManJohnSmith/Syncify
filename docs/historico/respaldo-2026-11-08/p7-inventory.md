# Inventario P7 · TrackBrowser — árbol /tmp/syncify-p7 (rama p7/track-browser), 2026-11-07
# (Verificado contra el árbol a2bb90d; las citas del informe original de 2026-11-05 se re-verificaron: las líneas se movieron.
#  OJO: tras el reinicio del 08-11 la rama se rebasó sobre dev 74c5a03 y las líneas de views pueden desplazarse ±0: las vistas no se tocaron.)

## 1. Tamaños y mapeo
- LibraryView.vue 2963 líneas (template 1-1120, script 1122-2904). mapToTrack(item: LibraryTrack, index) en 1547-1595; interfaz Track local 1507-1540; divide services/available_services por comas (1553-1559); availabilitySummary 1576, qualityBucket 1578, filePath 1585; artGradient precomputado 1561; isFavorite con cast (item as any) 1590.
- MetadataView.vue 3212 líneas. mapToMetadataTrack 1516-1560; MetadataTrack 1334-1375 (30+ campos); availabilitySummary 1538, filePath 1544, SIN qualityBucket; campos a null fijo 1548-1558 (energy/danceability/valence/upc/copyright/bitrate/sampleRate/bitDepth/fileFormat/fileSize).
- DownloadsView.vue 2424 líneas. Mapea QueueItem→fila con 3 funciones: mapQueueItem 1478-1495, mapCompletedItem 1505-1541, mapFailedItem 1556-1595; EnrichedQueueItem 1149-1165; mapeo solo de la ventana visible (1475-1477) y del slice (1550-1552, 1604-1606).

## 2. Ventana virtual (tres copias, divergidas)
- LibraryView: ROW_HEIGHT=56 (1666), OVERSCAN=10 (1667); comentario "Ported from DownloadsView.vue:549-564, 1768-1807 (PR-02)" (1662-1665, líneas muertas); computeds 2471-2492; totalVirtualHeight añade 48px de footer (2472); scroll con throttle 120ms + lectura de layout dentro del throttle (1717-1762, SCROLL_THROTTLE_MS 1718); reset si offset fuera del total (2494-2499); template spacer 664, ventana 666-667, :style height 674, contain:strict 657-661.
- MetadataView: ROW_HEIGHT=64 (1412), OVERSCAN=5 (1413); comentarios de paridad muertos (1411, 1832); computeds 1834-1855 (sin footer); onTrackScroll SIN throttle (1857-1863); scrollToTrack 1865-1872; reset en cada cambio de búsqueda/filtro/orden (1874-1879); template spacer 143, ventana 145-148, altura 150.
- DownloadsView: ROW_HEIGHT=60 (1283), OVERSCAN=10 (1284), PROGRESS_THROTTLE_MS=250 (1285); computeds 1728-1754 (visibleQueueItems añade absoluteIndex 1747-1754); onQueueScroll sin throttle (1756-1762); jumpToPosition por input (1764-1770, jumpPositionInput 1726; paginador template 497-504); sin reset; contador "Virtual Window" template 491; spacer 531, ventana 533, altura 537.

## 3. Filtros y orden
- LibraryView: 8 pills filterPills 1332-1341 (all/downloaded/notDownloaded/favorites/flac/mp3/multiSource/duplicates) + deep-link quality 1355/2429-2437 = 9 predicados en filteredTracks 2408-2437; sortOptions 7 (1357-1365); compareTracks switch 6 casos (2381-2392); optimización sortedPrefix/pageAppendCount/mergeSorted 2372-2468.
- MetadataView: filtro texto title/artist/album (1773-1780) + switch filterType 9 casos (1783-1811: downloaded/needs_work/low-quality/missing/no-art/no-isrc/no-mb/no-genre/no-year); sort switch 3+default (1814-1826).
- DownloadsView: sin sort (orden = position del backend); viewFilter 5 pestañas (1360, filterTabs 1615-1621); matchesSearch 5 campos (1423-1432); completedLimit/failedLimit=50 (1372-1373; slices 1550-1552, 1604-1606).

## 4. Paginación
- LibraryView: PAGE_SIZE=100 (1609), currentOffset/hasMore (1605-1608), loadLibrary 1612-1635, loadMore 1638-1660, triple fuente 1617-1621/1643-1647; FTS: SEARCH_PAGE_SIZE=100 (1227), performDatabaseSearch con query prefijando * (1769-1810, searchTracks 1799), loadMoreSearchResults 1812-1827, debounce 300ms (1829-1848); la lista consume FTS o biblioteca según searchQuery (2409).
- MetadataView: getLibrary(0,500) (1586); getTracksNeedingMetadata(500) para filterType==='missing' (1580-1583); sin has_more.
- DownloadsView: getQueue(undefined,50000) (1830); recorte client-side 50/sección; jumpToPosition 1764-1770.

## 5. Ruta (solo lectura de query)
- LibraryView: ?filter=quality&quality=<bucket> y ?filter=duplicates (FE-10, 2758-2762; applyRouteFilters 2763-2777); ?track=<id> consume con router.replace (2797-2799) paginando hasta 50 páginas (TRACK_DEEP_LINK_MAX_PAGES 2789; focusTrackFromQuery 2791-2835); watch de route.query 2878-2880.
- MetadataView: ?filter=needs_work en mount (1731-1734); ?trackId= en mount (1736-1744, selectTrack+scrollToTrack); watch route.query.filter 2469-2475.
- DownloadsView: ?filter=failed solo en mount (2276-2281). Sin store ni pinia (no existe ui/src/stores/); estado en refs locales del SFC.

## 6. Agrupación/reorden/selección
- Agrupación solo LibraryView: groupBy 'none'|'artist'|'album'|'year'|'genre'|'quality' (1308); dropdown sin Year (template 202); computeds 2520-2632; ramas template 622/827/894/944/1000/1052; expandedGroups hardcodeado con demos ['Kavinsky','M83','OutRun',...] (2518); grupos NO virtualizados (v-for completo 913-938, 975-994...).
- Reorden: LibraryView reordena COLUMNAS (DEFAULT_COLUMNS 1384-1393, handlers 1399-1448, localStorage syncify_library_columns_order 1461-1498, saneado L-06 1482-1491). DownloadsView reordena FILAS (draggedIndex 1795, onDragStart 1797-1799, onDrop 1805-1822 → invokeCommand('reorder_queue',{queueIds}) 1816; template draggable 538-543). MetadataView sin reorden.
- Selección: LibraryView flag en fila track.isSelected (1536) + selectedCount incremental (1881-1884, L-05 1879-1880); toggleSelectAll 2366-2370; clearSelection 2088-2091; acciones masivas por flag (2022, 2258). MetadataView selectedTracks: number[] (1436); selectTrack reemplaza (2488-2494); toggleTrackSelection (2496-2503); selectAll sobre filteredTracks (2509-2511); preserva ids vivos entre recargas (1596-1597). DownloadsView sin selección (solo Sets de detalle 1249-1267).

## 7. Diferencias entre vistas
- Fuente: Library = biblioteca + FTS paginados; Metadata = biblioteca página fija 500 (o get_tracks_needing_metadata); Downloads = cola completa sin FTS ni paginación.
- Campos: Library servicio/quality/downloadStatus/lyricsType/bpm/editionItemId (1507-1540); Metadata isrc/mbid/score/issues/musicalKey... y deriva issues en frontend (calculateIssues 1477-1479, missingMetadataFields 1483-1500); Downloads progreso (percent/bytes/phase/timeline/phase_timings, 1149-1165) mutando in-place por eventos (1931-2001).
- Acciones: Library play/enqueue/descargas/favorito/links/metadata-nav/show-in-folder/borrar/playlist; Metadata edición/tags de fichero/read-write_track_tags/reparación país/auto-fix batches; Downloads pausa/reanuda worker/cancelar/reintentar/limpiar/reordenar.
- Toast: Library y Downloads usan useToast (1130, 1116); Metadata tiene showToast LOCAL con timers (2819-2830, activeToastTimers 1397, fix MT-02 unmount 1691-1713) y no importa composables de @/composables/.
- Stats/worker: LibraryView REPLICA el panel de DownloadsView (comentario 1673-1676, impl 1677-1709, poll 4000ms 2852, setConcurrency vía settingsApi.setMaxConcurrentDownloads 1685-1696); DownloadsView usa queueApi.setMaxConcurrent con clamp 1..10 (1401-1414) y se actualiza por DOWNLOAD_PROGRESS (2285), sin poll.

## 8. Composables existentes
- No hay useTrack* ni paginación ni virtual scroll. Barrel ui/src/composables/index.ts:1-10 exporta useEventBus/useAsyncState/useToast/useKeyboardShortcuts/useLogs; el resto por ruta directa.
- usePlayer usado por LibraryView (1886); useKeyboardShortcuts solo por KeyboardShortcuts.vue (LibraryView NO lo usa y define handleKeydown propio 2708-2756); useEventBus (1131, 1122); useDownloadSettings (DownloadsView 1124).

## 9. API/IPC
- LibraryView importa libraryApi/searchTracks/enqueueTracks/reconcileQueue (@/api/library), addToQueue/addBatchToQueue/enqueueEligibleBatch/queueApi (@/api/queue), playlistsApi, settingsApi (1125-1128). 1 invoke crudo: enrich_metadata_musicbrainz (1291-1292).
- MetadataView importa libraryApi/lyricsApi/metadataApi/settingsApi/toolsApi/TauriEvents (1313-1319). 6 invokes crudos dinámicos: read_track_tags (1958, 3104), write_track_tags (1997), enrich_genre_lastfm (2157), enrich_metadata (2640), enrich_metadata_musicbrainz (2650). Ninguno con wrapper.
- DownloadsView: queueApi+classifyFailureReason (@/api/queue), auditDownloadQueue (@/api/library), settingsApi, invokeCommand (@/api/tauri) (1119-1128). Bypasses: invokeCommand('reorder_queue') (1816) existiendo reorderQueue en library.ts:493-495; invokeCommand('show_in_folder') (2106) existiendo libraryApi.showInFolder (library.ts:317-319).
- Wrappers relevantes: normalizeLibraryPage (library.ts:49-58); getLibrary/getFavoriteTracks/getDuplicateTracks 63-93; searchTracks 116-122; enqueueTracks 980-1046 (normalizeQualityPreference 959-975); getQueue preserva identidad del array (queue.ts:281-287); getMetadataStats acepta camelCase y snake_case (metadata.ts:159-171); getTracksNeedingMetadata 176-179; setMaxConcurrent (queue.ts:449); getWorkerStatus (queue.ts:417).

## 10. Tests UI y gates
- Gate: npm --prefix ui run test:run = vitest run. 127 ficheros bajo ui/src.
- Specs de vistas en ui/src/__tests__/views/: LibraryView.spec.ts (1266 líneas, 47 tests), LibraryViewStates.spec.ts (205, 5), LibraryViewTrackDeepLink.spec.ts (148, 4), LibraryViewPlayErrors.spec.ts (157, 4), MetadataView.spec.ts (695, 17), MetadataViewAnimatedCover.spec.ts, DownloadsView.spec.ts (1143, 31), DownloadsViewQueue.spec.ts (150, 5, R11), DownloadsViewQuality.spec.ts (322, 4), DownloadsViewQueueCovers.spec.ts (86, 2), DownloadPreflight.spec.ts (186, 3).
- No hay tests aislados de la ventana virtual ni de los predicados: solo vía mount.
- Gates transversales: tauri_events_symmetry.spec.ts y dead_commands_hygiene_test.rs; specs de API: library.spec.ts (23), queue.spec.ts (17), metadata.spec.ts (9).

## 11. Backend de las listas
- get_library (library.rs:379-385, inner 387-514): offset/limit con limit.unwrap_or(100).min(500) (393); página por subconsulta ORDER BY title,id LIMIT ? OFFSET ? (486) + ORDER BY t.title ASC (495); total COUNT (398-401); has_more (505); LibraryPage {tracks,total,offset,limit,has_more} (types.rs:346-352). SELECT computa services/imported_from/downloaded_from/available_services/availability_summary por GROUP_CONCAT (421-435), quality (436), quality_bucket CASE (440-445), download_status (446-450), metadata_score 100 pts (452-461), lyrics_type (463-468); verify_page_files (504).
- get_duplicate_tracks (library.rs:518+): mismo patrón paginado; duplicados por Title + primary artist (533-547); mismo SELECT SIN quality_bucket.
- get_favorite_tracks (2111-2116, query_favorite_tracks 2118+): CTE FAVORITE_ROWS_CTE (2065); pagina dentro del CTE ORDER BY favorite_at DESC, sort_title ASC, display_album ASC, edition_item_id ASC (2196-2199); orden final (2216); una fila POR RELEASE favoritado (edition_item_id, GROUP BY t.id + release, 2215).
- search_tracks (1648-1790): FTS5 library_fts MATCH + fallback LIKE (1666, 1758); su metadata_score NO excluye NOT_FOUND/MISMATCH ni exige release_year>0 (1720-1723 vs 457-460); SIN quality_bucket; ORDER BY t.title ASC (1760); SearchResult (types.rs:356).
- get_queue (queue.rs:2262+): limit por defecto 50000 (2267-2270); status_filter opcional; Vec<QueueItem> con position/progress_percent/bytes_downloaded/cover_art_url resuelto del disco (2274-2282+); reorder_queue aplica el reorden.
- Registro en main.rs: get_library 957, get_duplicate_tracks 963, search_tracks 965, get_favorite_tracks 986.

## 12. Rarezas
- Comentarios de paridad con líneas muertas: LibraryView 1664, 1675; MetadataView 1411, 1832.
- Comentario S194 obsoleto (LibraryView 1888-1892): menciona protocolo syncify-media retirado (C-19); el código real llama usePlayer().play (1893-1901).
- groupBy 'year' tipo muerto (1308): sin dropdown ni computed ni template.
- expandedGroups con demos hardcodeadas (2518).
- Cast (item as any).is_favorite (1590) aunque LibraryTrack lo tipa.
- quality_bucket con test de paridad backend (library.rs:2586 test_get_library_quality_bucket_matches_dashboard) solo en get_library → filtro quality degrada en FTS/favoritos (fallback documentado 2430-2435).
- Toast local en MetadataView (ver §7).
- Bypasses IPC en DownloadsView (ver §9).
