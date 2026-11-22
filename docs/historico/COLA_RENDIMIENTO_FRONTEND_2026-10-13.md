# Cola técnica: rendimiento de la interfaz al navegar y hacer scroll

**Estado:** actualizado el 2026-11-16: **FE-PERF-01 aplicado en código** (KeepAlive acotado con medición; ver su sección), **FE-PERF-02 aplicado en código** (parcial: la clonación está corregida; ver su sección), **FE-PERF-03 aplicado en código** (división del bundle con medición de arranque; ver su sección), el resto `registrado`, sin cambio de código. Las evidencias originales se verificaron el 2026-10-13 contra el árbol de trabajo y sobre una copia de solo lectura de la base real (~390 MB, 30.527 pistas); el estado canónico por ítem está en `docs/auditoria/matriz-ajustes.md`.
**Fecha de origen:** 2026-10-13. **Última revisión:** 2026-11-16 (FE-PERF-01 aplicado). **Motivo:** el usuario observa retardos superiores a un segundo al cambiar de pantalla o pestaña, y scroll que tarda y da tirones, en toda la aplicación.
**Frontera de trabajo:** este fichero es propio y no toca `docs/historico/Errores-Pruebas-2026-10-11.md`, `docs/historico/Errores-Pruebas-2026-10-13.md` ni `docs/auditoria/matriz-ajustes.md`, reservados por el flujo de descargas concurrente. Integrar los ítems en la cola canónica cuando se libere, igual que se acordó con `docs/COLA_WEBKIT_COMPOSICION_ACELERADA.md`.

## Qué se descartó con medición (para no invertir esfuerzo donde no está el problema)

Medido sobre `/tmp/perf-hard/db-copy.db` (copia de solo lectura de la base real):

| Ruta medida | Resultado |
|---|---|
| `get_library` completo (7 `LEFT JOIN` + subconsultas) en offset 30.000 | **0,54 ms** de mediana |
| El mismo en offset 0 | **0,42 ms** |
| `SELECT COUNT(*) FROM tracks` | **0,28 ms** |
| `library_stats` (vista, 8 `COUNT`) | **2,32 ms** |
| `get_top_artists` con `GROUP BY` | **23,40 ms** |
| `get_top_genres` / `quality_distribution` | 2,80 ms / 13,51 ms |
| Recorrer y statar el directorio de descargas (328 MB, 21 ficheros) | **0,1 ms** |
| Ordenar 30.527 pistas en JS con los criterios de la app | **31,16 ms** |

**Conclusión:** ni SQLite, ni los agregados, ni el filtrado/ordenado en JS explican retardos de más de un segundo. La paginación por `OFFSET` tampoco (1,55 ms en el offset 30.500): migrar a paginación por cursor no arreglaría el retraso percibido. El coste está en la capa de vistas y en la repetición de trabajo que sí quedó registrada: en 4 minutos de navegación real el log mostró **319 llamadas a `get_library`, una cada 0,8 s de media**, mientras el usuario hacía scroll infinito.

## FE-PERF-01 · `aplicado en código (2026-11-16)` · `<KeepAlive>` acotado por `include`, con medición en la ventana real.

**Issue**: [#49](https://github.com/MadManJohnSmith/Syncify/issues/49) (spec [#34](https://github.com/MadManJohnSmith/Syncify/issues/34)).

**Aplicado:** `ui/src/keepAliveViews.ts` define el conjunto acotado (`include` fijo de 4 vistas: Dashboard, Library, Downloads, Settings, con la justificación de cada una y las exclusiones argumentadas) y `ui/src/App.vue` envuelve el router-view con `<KeepAlive :include="KEEP_ALIVE_INCLUDE" :max="KEEP_ALIVE_MAX">` — nunca global, con techo duro de retención (LRU). Las vistas incluidas declaran su `name` con `defineOptions` para que el `include` sea consistente. **Decisión de transición:** `mode="out-in"` se mantiene también en las vistas cacheadas (coste fijo ~2x150 ms a cambio de no solapar dos vistas de altura completa en el mismo contenedor flex durante el modo concurrente). Comportamiento asociado al cacheo: LibraryView detiene su sondeo de 4 s mientras está desactivada (`onActivated`/`onDeactivated`) y Dashboard se relee una vez por ráfaga de cambios de biblioteca/cola en vez de quedarse congelado cacheado. El análisis original que motivó el arreglo queda abajo como contexto.

`ui/src/App.vue` envuelve el `<router-view>` en `<transition name="fade" mode="out-in">` y no existía ningún `<KeepAlive>` (verificado por búsqueda de la cadena). Consecuencia concreta: al entrar y salir de una vista se vuelve a montar el componente completo y a ejecutar su `onMounted`. El `Dashboard` lanza en `onMounted` una llamada a `createLibrarySnapshot()` más 11 llamadas de API en `Promise.all`, de modo que entrar y salir de Dashboard repetía las 12.

**Medición en la ventana real** (Tauri dev / WebKitGTK, sonda dev-only `ui/src/perf/navigationProbe.ts` + harness `tools/nav-perf.sh`, perfil de datos desechable vía `XDG_DATA_HOME`; definición: confirmación de navegación → primer frame con la vista destino pintada, transición incluida; ~10 muestras por par tras 2 vueltas de calentamiento; BD desechable vacía, así que el coste de montaje medido es el suelo, no el de una biblioteca real de 30k pistas; sin Chromium headless):

| Par | antes p50/p95 (ms) | después p50/p95 (ms) |
|---|---|---|
| Dashboard→Library | 279 / 299 | 285 / 303 |
| Library→Downloads | 328 / 345 | 313 / 353 |
| Downloads→Settings | 288 / 353 | 286 / 359 |
| Settings→Dashboard | 287 / 302 | 286 / 301 |

**Lectura honesta:** en esta máquina la BD real está vacía (0 pistas), así que el coste de montaje que el cacheo elimina (render + 13 IPC del Dashboard) es pequeño y el p95 queda dominado por la transición `out-in` conservada (~300 ms fijos): el tiempo pared no baja de forma perceptible con esta fixture. Lo que la medición sí verifica por mecanismo es que las 39 muestras «después» salieron todas con `kind=cached` (KeepAlive reutiliza los mismos nodos DOM; ninguna visita remonta) frente a las 47 «antes» todas `kind=mount`, y que las llamadas del Dashboard dejan de repetirse por visita (verificado además por tests de comportamiento). Con biblioteca real crecida, el coste que desaparece es el de montaje + IPC de cada revisita; el sobrecoste de la transición conservada queda documentado como decisión (user story 6 de #34).

## FE-PERF-02 · `aplicado en código (2026-10-25, parcial)` · `loadMore` clona el array entero y `filteredTracks` reordena todo en cada invalidación.

**Actualización 2026-10-25:** la clonación está corregida en `ui/src/views/LibraryView.vue` — `loadMore` acumula con `tracks.value.push(...newTracks)` y `filteredTracks` ya no recopia/reordena el conjunto entero en cada invalidación; `docs/auditoria/matriz-ajustes.md` la registra como «parcial (clonación corregida)». El análisis original que motivó el arreglo queda abajo como contexto.

`ui/src/views/LibraryView.vue:1613` hace `tracks.value = [...tracks.value, ...newTracks]` al acumular una página, y `:2358` hace `trackList = [...trackList].sort(...)` dentro del `computed` `filteredTracks` (`:2326`). Con `PAGE_SIZE = 100` (`:1572`) y 30.527 pistas, recorrer la biblioteca son ~306 páginas, y **cada** una invalida el computado que recopia y reordena todo el conjunto. La medida de referencia es **31,16 ms** por reordenada de 30.527 elementos.
**Diseño obligatorio del arreglo:** acumular sin clonar (mutación controlada del array o `concat` sobre referencia no reactiva), y que el computado dependa de las claves que realmente cambian (filtros y `sortBy`) para no reordenar cuando solo crece la lista. No debe cambiar el resultado observable: mismas filas, mismo orden.
**Nota:** `handleScroll` actualiza `libraryScrollTop.value` en **cada** evento de scroll sin throttle (`:1685`); el throttle de 120 ms (`:1678`) solo cubre las mediciones de layout (`:1709-1720`). Actualizar el índice de la ventana virtual por evento es el comportamiento deseado para que el scroll no vaya a saltos, así que **no** conviene limitarlo sin medir el coste del `slice` (~0,067 ms con 100 filas).

## FE-PERF-03 · `aplicado en código (2026-11-16)` · Coste de parseo repetido por cambio de pantalla.

**Issue**: [#50](https://github.com/MadManJohnSmith/Syncify/issues/50) (spec [#34](https://github.com/MadManJohnSmith/Syncify/issues/34)).

**Aplicado:** `ui/src/main.ts` deja de importar estáticamente `DashboardView`, `AlbumDetailView` y `ArtistDetailView`: TODAS las vistas llegan por `import()` dinámico en el router. El chunk principal baja de 452.50 kB / 139.58 kB gz a **399.00 kB / 126.92 kB gz** (build real, `ui/dist`) y `DashboardView` (33.3 kB), `AlbumDetailView` (9.5 kB) y `ArtistDetailView` (9.3 kB) pasan a chunks propios. La política de reparto queda documentada en `ui/vite.config.ts` (sección `build`): **sin `manualChunks` a propósito** — el reparto lo hacen los límites de `import()`, y no se aplica vendor split porque el harness mide en `vite dev` (donde `manualChunks` no aplica) y separar vendor no reduce bytes ni parseo en disco local.

**Medición de arranque en la ventana real** (misma infraestructura que FE-PERF-01: `tools/nav-perf.sh` + sonda dev-only `ui/src/perf/navigationProbe.ts`, perfil desechable vía `XDG_DATA_HOME`, sin headless; definición: `performance.timeOrigin` → `first-view-ready`, es decir página empieza a cargar → primera vista pintada con la splash fuera; 3 pasadas por variante, BD desechable vacía; artefactos en `~/.local/state/syncify-fperf03/`):

| Métrica | antes (3 pasadas) | después (3 pasadas) | Δ mediana |
|---|---|---|---|
| Arranque total | 1683 / 1594 / 1656 ms | 1480 / 1440 / 1507 ms | **−176 ms (−10,6%)** |
| Carga+parseo del módulo principal (page origin → sonda cargada) | 1144 / 1061 / 1116 ms | 916 / 922 / 930 ms | **−194 ms (−17,4%)** |
| Init backend + montaje (sonda cargada → primera vista) | 539 / 533 / 540 ms | 564 / 518 / 577 ms | sin cambio (el fetch del chunk de Dashboard solapa con el init) |

Las 3 pasadas «después» quedan por debajo de las 3 «antes» y la fase que cambia es exactamente la que toca el reparto (carga+parseo del grafo de módulos de arranque); la navegación medida por FE-PERF-01 no se mueve (p50 ~280-312 ms en ambas). La sonda se extendió sin tocar sus canales de FE-PERF-01: publica `pageTimeOrigin` (epoch de `performance.timeOrigin`) para derivar el arranque. **Lectura honesta:** la medición corre sobre el dev server (el harness existente), así que mide el grafo de módulos de arranque en condiciones de desarrollo; el efecto en el build de producción se verifica por el mecanismo análogo (el chunk principal pierde el código de las 3 vistas, confirmado en el manifest del build), no por parseo de chunks medido en la app empaquetada.

**Sin chunks huérfanos ni carga duplicada:** verificado sobre el build con sourcemaps (`TAURI_DEBUG=1`): 24 chunks JS, todos alcanzables desde el entry por el grafo de referencias `import` estáticas y dinámicas (ningún huérfano), y las 115 fuentes de los sourcemaps aparecen cada una en EXACTAMENTE un chunk (cero duplicación; Rollup no duplica módulos y no hay `manualChunks` que pueda hacerlo).

El análisis original que motivó el arreglo queda abajo como contexto.

Medido sobre el build real (`ui/dist`): `index-*.js` = 429 kB raw / 133 kB gz, `index-*.css` = 177 kB / 21 kB gz, y por vistas `SettingsView` 224 kB / 54 kB gz, `MetadataView` 121 kB / 29 kB, `LyricsView` 98 kB / 24 kB, `MigrationView` 94 kB / 21 kB, `AccountsView` 85 kB / 21 kB, `LibraryView` 80 kB / 20 kB, `DownloadsView` 75 kB / 18 kB, `PlaylistView` 45 kB / 12 kB. Total JS ≈ **1,32 MB raw / 347 kB comprimido**.
`ui/src/main.ts:12-13` importa estáticamente `DashboardView`, `AlbumDetailView` y `ArtistDetailView`; el resto son lazy (`:14`, `:20`, `:22`, `:23`, `:24`).
**Qué significa:** las vistas lazy se descargan del disco local y se parsean en cada visita mientras no haya `KeepAlive`. Para `LibraryView` (20 kB comprimidos) ese coste es pequeño; el caso caro es el arranque inicial y las vistas grandes. **No se midió** el tiempo de parseo real dentro del webview (los intentos con Chromium headless y `performance.getEntries` no dieron medida fiable del chunk), así que este ítem queda como **sospecha de coste, no causa comprobada**.
**Diseño obligatorio del arreglo:** dividir el chunk principal de 429 kB y confirmar el `manualChunks` de Vite, con medición antes/después del arranque. No aplicar por el tamaño en sí.

## FE-PERF-04 · `registrado` · Repetición de agregados equivalentes en el Dashboard.

**Issue**: [#51](https://github.com/MadManJohnSmith/Syncify/issues/51) (spec [#34](https://github.com/MadManJohnSmith/Syncify/issues/34)).

`src-tauri/src/commands/dashboard.rs:43-95` (`create_library_snapshot`) ejecuta **6 sentencias de recuento separadas** sobre `tracks`, `albums`, `artists`, `downloads`, `SUM(file_size_bytes)` sobre `downloads`, y `COUNT(DISTINCT track_id)` sobre `lyrics`. La vista `library_stats` hace otras 8 cuentas. `fetchData()` vuelve a pedir `getLibraryStats`, `getMetadataStats`, `getLyricsStats` y `getLibrarySnapshots` (`ui/src/views/DashboardView.vue:630-641`), que ya cubren lo mismo.
**Medido:** cada recuento suelto son 3-7 ms, y la vista 2,32 ms; no es un cuello de botella por hoy. Conviene unificar cuando se unifique el snapshot, y **mientras tanto no merece optimizarse por velocidad**.
**Qué no se ha demostrado:** que estos recuentos aporten algo perceptible. Este ítem se registra para no duplicar trabajo cuando se toque el Dashboard por otros motivos.

## Orden recomendado y verificación

1. ~~**FE-PERF-02** (sin clonar, sin reordenar en falso)~~ **aplicado (parcial)**: atacaba directamente los tirones de scroll y era acotado a `LibraryView.vue`.
2. ~~**FE-PERF-01** (`KeepAlive` acotado)~~ **aplicado**: ataca el retraso al cambiar de pantalla y evita repetir las llamadas del Dashboard.
3. ~~**FE-PERF-03** (división del bundle) solo con medición de arranque~~ **aplicado**: imports dinámicos de todas las vistas; el arranque medido mejoró (1656 → 1480 ms mediana) y el vendor split se descartó por falta de mecanismo y datos.
4. **FE-PERF-04** al tocar el Dashboard por otra razón.

**Verificación obligatoria antes y después:** medir en la app real el tiempo desde el clic hasta el primer frame con contenido, scroll sostenido con cuenta de fotogramas perdidos, y llamadas a `get_library` por minuto durante el uso normal. Tests automatizados puros para `loadMore` (mismas filas, mismo orden, sin clonar) y para el listado con `sortBy`. Mantener la ruta compatible de WebKit intacta: nada de este fichero toca `src-tauri/src/main.rs` ni las variables de composición.

**Relacionado:** `docs/COLA_WEBKIT_COMPOSICION_ACELERADA.md` (composición y compatibilidad Intel Iris Xe) y `docs/historico/perf-run-2026-10-12.md` (revisión de backend).

## Cierre (nota del 2026-11-22)

Cola terminada; movida de `docs/` a `docs/historico/` el 2026-11-22 sin
reescribir lo anterior (la nota va al final para no desplazar las líneas que
cita `docs/auditoria/matriz-ajustes.md`). Tracker el 2026-11-22: FE-PERF-01
(#49), FE-PERF-03 (#50) y FE-PERF-04 (#51) cerrados el 2026-11-16, y la spec
#34 también. FE-PERF-04 no quedó solo `registrado` como dicen su sección y el
orden recomendado: se aplicó con el PR #301 (squash `0684c43`, lectura
unificada de los agregados del Dashboard). Evidencia, no estado actual.
