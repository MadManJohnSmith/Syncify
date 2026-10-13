# Cola técnica: rendimiento de la interfaz al navegar y hacer scroll

**Estado:** registrado, sin cambio de código. Todas las evidencias se verificaron el 2026-10-13 contra el árbol de trabajo y sobre una copia de solo lectura de la base real (~390 MB, 30.527 pistas).
**Fecha:** 2026-10-13. **Motivo:** el usuario observa retardos superiores a un segundo al cambiar de pantalla o pestaña, y scroll que tarda y da tirones, en toda la aplicación.
**Frontera de trabajo:** este fichero es propio y no toca `docs/Errores-Pruebas-2026-10-11.md`, `docs/Errores-Pruebas-2026-10-13.md` ni `docs/auditoria/matriz-ajustes.md`, reservados por el flujo de descargas concurrente. Integrar los ítems en la cola canónica cuando se libere, igual que se acordó con `docs/COLA_WEBKIT_COMPOSICION_ACELERADA.md`.

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

## FE-PERF-01 · `registrado` · Sin `<KeepAlive>`, cada cambio de pantalla destruye y reconstruye la vista.

`ui/src/App.vue:370-373` envuelve el `<router-view>` en `<transition name="fade" mode="out-in">` y **no existe ningún `<KeepAlive>` en `ui/src/App.vue` ni en `ui/src/main.ts`** (verificado por búsqueda de la cadena). Consecuencia concreta: al entrar y salir de una vista se vuelve a montar el componente completo y a ejecutar su `onMounted`. El `Dashboard` lanza en `onMounted` una llamada a `createLibrarySnapshot()` más 11 llamadas de API en `Promise.all` (`ui/src/views/DashboardView.vue:830-831` y `:628-636`), de modo que entrar y salir de Dashboard repite las 12.
**Por qué importa:** `mode="out-in"` obliga además a que la vista saliente termine su animación antes de empezar a montar la entrante, así que el retraso percibido se suma.
**Diseño obligatorio del arreglo:** `<KeepAlive>` con `include`/`exclude` acotado, no global, para no retener todas las vistas en memoria (el caso contrario también degrada). Decidir explícitamente si `mode="out-in"` debe mantenerse en las vistas que se cachean. Nombres de componente consistentes: `include` filtra por el `name` del componente, y conviene confirmar que las vistas lo declaran.
**Qué no se ha demostrado:** que esto sea la causa única del retraso; se verificó el mecanismo y el coste repetido, no la latencia de navegación medida en la ventana real.

## FE-PERF-02 · `registrado` · `loadMore` clona el array entero y `filteredTracks` reordena todo en cada invalidación.

`ui/src/views/LibraryView.vue:1613` hace `tracks.value = [...tracks.value, ...newTracks]` al acumular una página, y `:2358` hace `trackList = [...trackList].sort(...)` dentro del `computed` `filteredTracks` (`:2326`). Con `PAGE_SIZE = 100` (`:1572`) y 30.527 pistas, recorrer la biblioteca son ~306 páginas, y **cada** una invalida el computado que recopia y reordena todo el conjunto. La medida de referencia es **31,16 ms** por reordenada de 30.527 elementos.
**Diseño obligatorio del arreglo:** acumular sin clonar (mutación controlada del array o `concat` sobre referencia no reactiva), y que el computado dependa de las claves que realmente cambian (filtros y `sortBy`) para no reordenar cuando solo crece la lista. No debe cambiar el resultado observable: mismas filas, mismo orden.
**Nota:** `handleScroll` actualiza `libraryScrollTop.value` en **cada** evento de scroll sin throttle (`:1685`); el throttle de 120 ms (`:1678`) solo cubre las mediciones de layout (`:1709-1720`). Actualizar el índice de la ventana virtual por evento es el comportamiento deseado para que el scroll no vaya a saltos, así que **no** conviene limitarlo sin medir el coste del `slice` (~0,067 ms con 100 filas).

## FE-PERF-03 · `registrado` · Coste de parseo repetido por cambio de pantalla.

Medido sobre el build real (`ui/dist`): `index-*.js` = 429 kB raw / 133 kB gz, `index-*.css` = 177 kB / 21 kB gz, y por vistas `SettingsView` 224 kB / 54 kB gz, `MetadataView` 121 kB / 29 kB, `LyricsView` 98 kB / 24 kB, `MigrationView` 94 kB / 21 kB, `AccountsView` 85 kB / 21 kB, `LibraryView` 80 kB / 20 kB, `DownloadsView` 75 kB / 18 kB, `PlaylistView` 45 kB / 12 kB. Total JS ≈ **1,32 MB raw / 347 kB comprimido**.
`ui/src/main.ts:12-13` importa estáticamente `DashboardView`, `AlbumDetailView` y `ArtistDetailView`; el resto son lazy (`:14`, `:20`, `:22`, `:23`, `:24`).
**Qué significa:** las vistas lazy se descargan del disco local y se parsean en cada visita mientras no haya `KeepAlive`. Para `LibraryView` (20 kB comprimidos) ese coste es pequeño; el caso caro es el arranque inicial y las vistas grandes. **No se midió** el tiempo de parseo real dentro del webview (los intentos con Chromium headless y `performance.getEntries` no dieron medida fiable del chunk), así que este ítem queda como **sospecha de coste, no causa comprobada**.
**Diseño obligatorio del arreglo:** dividir el chunk principal de 429 kB y confirmar el `manualChunks` de Vite, con medición antes/después del arranque. No aplicar por el tamaño en sí.

## FE-PERF-04 · `registrado` · Repetición de agregados equivalentes en el Dashboard.

`src-tauri/src/commands/dashboard.rs:43-95` (`create_library_snapshot`) ejecuta **6 sentencias de recuento separadas** sobre `tracks`, `albums`, `artists`, `downloads`, `SUM(file_size_bytes)` sobre `downloads`, y `COUNT(DISTINCT track_id)` sobre `lyrics`. La vista `library_stats` hace otras 8 cuentas. `fetchData()` vuelve a pedir `getLibraryStats`, `getMetadataStats`, `getLyricsStats` y `getLibrarySnapshots` (`ui/src/views/DashboardView.vue:630-641`), que ya cubren lo mismo.
**Medido:** cada recuento suelto son 3-7 ms, y la vista 2,32 ms; no es un cuello de botella por hoy. Conviene unificar cuando se unifique el snapshot, y **mientras tanto no merece optimizarse por velocidad**.
**Qué no se ha demostrado:** que estos recuentos aporten algo perceptible. Este ítem se registra para no duplicar trabajo cuando se toque el Dashboard por otros motivos.

## Orden recomendado y verificación

1. **FE-PERF-02** (sin clonar, sin reordenar en falso): ataca directamente los tirones de scroll y es acotado a `LibraryView.vue`.
2. **FE-PERF-01** (`KeepAlive` acotado): ataca el retraso al cambiar de pantalla y evita repetir las 12 llamadas del Dashboard.
3. **FE-PERF-03** (división del bundle) solo con medición de arranque.
4. **FE-PERF-04** al tocar el Dashboard por otra razón.

**Verificación obligatoria antes y después:** medir en la app real el tiempo desde el clic hasta el primer frame con contenido, scroll sostenido con cuenta de fotogramas perdidos, y llamadas a `get_library` por minuto durante el uso normal. Tests automatizados puros para `loadMore` (mismas filas, mismo orden, sin clonar) y para el listado con `sortBy`. Mantener la ruta compatible de WebKit intacta: nada de este fichero toca `src-tauri/src/main.rs` ni las variables de composición.

**Relacionado:** `docs/COLA_WEBKIT_COMPOSICION_ACELERADA.md` (composición y compatibilidad Intel Iris Xe) y `docs/perf-run-2026-10-12.md` (revisión de backend).
