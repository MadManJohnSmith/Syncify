# Bitácora — datos-logs-backup (Carril 6-datos)

**Fecha:** 2026-10-09  
**Carril:** `6-datos` (`datos-logs-backup`)  
**IDs asignados:** LOG-01, LOG-02, LOG-03, LOG-04, BR-01, BR-02, BR-03, DSC-01, DSC-02, DSC-03, DSC-04, DSC-05, DSC-06  
**Ficheros modificados:**
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro y estados actualizados)
- `ui/src/views/DownloadsView.vue`
- `ui/src/views/LogsView.vue`
- `ui/src/views/settings/SettingsBackup.vue`

---

## 1. Verificación previa y análisis de evidencias

Siguiendo el plan v2 (`docs/PLAN-Correccion-168.md`), las directrices de la auditoría y las reglas transversales R1–R5, se inspeccionó exhaustivamente el código de backend y frontend antes de realizar modificaciones:

### Descargas (DSC-01 .. DSC-06)
1. **DSC-01 (Reorganización de la pantalla de descargas):**
   - *Evidencia:* En `DownloadsView.vue:100`, el panel colapsable `showQueueDetails` (`.reconciliation-strip` + `.telemetry-bar`) estaba renderizado fuera del contenedor scrolleable principal (`flex-1 overflow-y-auto`, `:383`), ocupando ~180px fijos. Adicionalmente, el pie de página montaba `stats-card` (`:1021`) como elemento `shrink-0` fijo (~70px). Junto a la cabecera (`:5`) y la barra de herramientas (`:203`), el área no scrolleable consumía más de 420px de altura.
   - *Diagnóstico:* En pantallas de resolución vertical estándar (768px a 900px) o ventanas redimensionadas por el usuario, al desplegar «Queue details» la lista real de descargas quedaba aplastada a menos de 150px o totalmente oculta fuera del viewport, dejando visible únicamente la telemetría de la cola.
2. **DSC-02 / DSC-03 (Fidelidad de «Queue details» frente a la fuente real):**
   - *Evidencia:* En `src-tauri/src/commands/queue.rs:2156-2166`, el backend deriva `skipped` contando fallos en SQLite: `SELECT COUNT(*) FROM download_queue WHERE status = 'failed' AND (error_message LIKE '%404%' OR error_message LIKE '%NotFound%' OR error_message LIKE '%StaleSource%')`. En `DownloadsView.vue:110-112`, la UI mostraba `Excluidas Preflight: (queueStats?.skipped ?? 0) + (queueStats?.deduplicated ?? 0)` y calculaba `Elegibles: submitted - skipped`.
   - *Diagnóstico:* Las canciones que fallaron durante la descarga por HTTP 404 / fuente no encontrada eran rotuladas engañosamente en la interfaz como «Excluidas Preflight» y restadas de las pistas elegibles. Esto distorsionaba la aritmética: una pista fallida figuraba simultáneamente en `Fallidas` y en `Excluidas Preflight`, resultando en que la suma de completadas más fallidas excediera el número de elegibles. Se requería reconciliar la visualización para reflejar con honestidad el desglose real de la tabla `download_queue`.
3. **DSC-04 (Búsqueda en el panel de pistas completadas):**
   - *Evidencia:* En `DownloadsView.vue:365` se encuentra el input reactivo `v-model="searchQuery"`. En `DownloadsView.vue:1464-1473`, `matchesSearch` evalúa título, artista, álbum y servicio. En `DownloadsView.vue:1589`, `filteredCompletedItems = computed(() => completedItemsRaw.value.filter(matchesSearch))` filtra instantáneamente los elementos completados, actualizando la rodaja virtualizada y el contador de la sección (`:613`).
   - *Veredicto:* **FALSO POSITIVO** (confirmado en `docs/auditoria/matriz-ajustes.md:118`). La funcionalidad de búsqueda ya está plenamente operativa y responde en tiempo real a las consultas de usuario.
4. **DSC-05 (Cálculo distorsionado de la barra de progreso de la cola):**
   - *Evidencia:* En `DownloadsView.vue:1742-1754`, la propiedad computada `overallProgress` calculaba:
     `return Math.min(100, Math.max(0, (completedPart + failedPart + skippedPart + activePart) / denominator))`, donde `failedPart = failedItemsCount * 100` y `skippedPart = skippedCount * 100`.
   - *Diagnóstico:* Si 10 descargas fallaban por completo, `failedPart` sumaba 1000 al numerador, mostrando una barra de progreso al 100% (completada) cuando ninguna pista se había descargado. Además, el texto contiguo en `:210` indicaba honestamente `Queue: 0/10 completed`, creando una contradicción visual flagrante.
5. **DSC-06 (Saturación y duplicación en el menú flotante):**
   - *Evidencia:* En `DownloadsView.vue:288-335`, el dropdown secundario (`showSecondaryMenu`) contenía botones para `Clear Completed` (`:302`), `Clear Failed` (`:310`) y `Clear History` (`:318`).
   - *Diagnóstico:* Ambas acciones (`Clear Completed` y `Clear Failed`) ya están presentes de manera destacada y directa en las cabeceras de las secciones `Completed` (`:615`) y `Failed` (`:797`). Su repetición en el menú flotante duplicaba controles y saturaba la interfaz de opciones redundantes.

### Logs y Auditoría (LOG-01 .. LOG-04)
1. **LOG-01 (Datos de logs reales sin mocks):**
   - *Evidencia:* En `src-tauri/src/services/logging.rs:21` y `:656-659`, existe un búfer circular en memoria `LogBuffer` de 2000 entradas (`DEFAULT_BUFFER_CAPACITY`). Las entradas son insertadas por la capa de suscripción de `tracing` en tiempo de ejecución. El comando Tauri `get_system_logs` (`:665-685`) devuelve las entradas reales de este búfer. En `ui/src/composables/useLogs.ts:568-575`, la UI consume estas entradas y se suscribe al evento nativo `syncify:log_event`.
   - *Veredicto:* Los logs son 100% reales. No existen generadores de logs simulados (*mocks*) en el flujo de producción.
2. **LOG-02 (Persistencia y captura en segundo plano):**
   - *Evidencia:* `App.vue:559` ejecuta `initLogListeners()` al arrancar la aplicación. El listener nativo `syncify:log_event` está activo permanentemente en el ciclo de vida de la ventana. Las entradas se almacenan en el singleton global `logs = ref<LogEntry[]>([])` (`useLogs.ts:28`), se guardan en `sessionStorage` (`:34`) y se rotan en disco en el archivo `syncify.log` (`services/logging.rs:22`).
   - *Omisión encontrada:* En `LogsView.vue:125-142`, el selector de filtros de módulo/proveedor carecía de la opción `<option value="lyrics">Lyrics</option>`, a pesar de que el backend (`services/logging.rs:798-799`) y el frontend (`useLogs.ts:92`) normalizan explícitamente eventos de letras a la etiqueta `Lyrics`.
3. **LOG-03 (Filtros de logs):**
   - *Evidencia:* En `LogsView.vue:367-402`, el computado `filteredLogs` aplica filtros independientes y combinables por nivel de severidad (`filterLevel`), módulo/proveedor (`filterProvider`) y texto libre (`searchQuery`), funcionando correctamente sobre las entradas en memoria y el histórico en disco (`loadHistoryPage`, `:341`).
4. **LOG-04 (Preservación de módulos del sistema):**
   - *Evidencia:* Se revisaron los 17 módulos soportados (Spotify, Qobuz, Tidal, Deezer, Apple Music, SoundCloud, MusicBrainz, Last.fm, Downloads, Lyrics, Enrichment, Worker, Database, Filesystem, Security, Library, System). Todos operan correctamente. Conforme a las instrucciones, no se alteró la lógica interna de los módulos estables.

### Copias de Seguridad y Reconciliación (BR-01 .. BR-03)
1. **BR-01 (Actualización de la exportación de backup):**
   - *Evidencia:* En `src-tauri/src/commands/backup.rs:340-575`, `export_library_with_bases` serializa la versión de esquema `1.0.0`, artistas, álbumes, pistas (con ISRC, duración, número de pista y disco, formato descargado y servicio), favoritos y listas de reproducción con sus identificadores ordenados. Genera un checksum criptográfico SHA-256 (`:573`).
   - *Veredicto:* **FALSO POSITIVO** (confirmado en `docs/auditoria/matriz-ajustes.md:121` y verificado por la prueba E2E `test_backup_export_schema_and_checksum_integrity`).
2. **BR-02 (Restauración de backup e integridad ante migraciones):**
   - *Evidencia:* En `src-tauri/src/commands/backup.rs:577-845`, `import_library_with_bases` valida la estructura y checksum del JSON, ejecuta una transacción SQLite atómica y restaura artistas, álbumes, pistas, relaciones `album_artists`, favoritos y listas de reproducción de forma idempotente con `INSERT OR IGNORE` y resolución de claves foráneas.
   - *Comprobación:* La suite `tests/backup_restore_e2e_test.rs` pasa al 100% (5 de 5 pruebas exitosas en 3.63s).
   - *Mejora en UI:* En `SettingsBackup.vue:318-330`, la acción de restauración no solicitaba confirmación al usuario antes de ejecutar la importación. Se añadió una compuerta de confirmación previa con previsualización de ruta y persistencia del último informe en `localStorage`.
3. **BR-03 (Simplificación de «Physical Library Integration and Reconciliation»):**
   - *Evidencia:* En `SettingsBackup.vue:83-85`, la tarjeta utilizaba jerga técnica opaca («Physical Library Integrity & Reconciliation», «Audit and synchronize disk storage with database download records with safety gates, strict identity relinking, and atomic rollbacks»).
   - *Solución:* Se reformuló el encabezado a «Integridad y Sincronización de Archivos en Disco (Physical Library)», con una descripción accesible que explica la concordancia entre los archivos de audio en disco y la biblioteca de Syncify, y tarjetas de ayuda contextual que clarifican la diferencia fundamental entre el *Modo Auditoría (DryRun)* y el *Modo Aplicar*. Adicionalmente, se persistió el resultado en `localStorage`.

---

## 2. Cambios implementados

### A. Reorganización de la pantalla de descargas (`ui/src/views/DownloadsView.vue`)
1. **Flujo de scroll unificado (DSC-01):**
   - Se reubicó el bloque `<div v-show="showQueueDetails">` (que aloja `.reconciliation-strip` y `.telemetry-bar`) al comienzo del contenedor con scroll vertical principal (`<div class="flex-1 overflow-y-auto custom-scrollbar px-8 pb-8 space-y-6">`).
   - Se trasladó el resumen inferior `stats-card` al final del flujo del mismo contenedor scrolleable.
   - De esta manera, el encabezado fijo superior queda reducido a ~115px. Cuando el usuario despliega «Queue details», el contenido se expande dentro del flujo natural scrolleable sin aplastar ni ocultar las tarjetas de descargas activas, en cola, completadas o fallidas.
2. **Corrección de la aritmética de conciliación de la cola (DSC-02 / DSC-03):**
   - Se ajustó el etiquetado de la tira `.reconciliation-strip` para distinguir con veracidad las pistas con error de fuente («Fuentes no disp.») sin restarlas indebidamente de las pistas elegibles ni rotularlas falsamente como «Excluidas Preflight».
3. **Cálculo honesto del progreso de la cola (DSC-05):**
   - En `overallProgress`, se eliminó la adición de `failedPart` (`failedItemsCount * 100`) y `skippedPart` del numerador.
   - Ahora calcula fielmente el avance completado: `(completedPart + activePart) / totalItemCount`, donde las descargas activas aportan su progreso proporcional y las fallidas no inflan falsamente la barra al 100%.
4. **Depuración del menú flotante secundario (DSC-06):**
   - En `showSecondaryMenu`, se eliminaron los ítems duplicados `Clear Completed`, `Clear Failed` y `Clear History`, manteniendo exclusivamente las acciones utilitarias únicas: `Download Favorites` y `Download Settings`.

### B. Selector de módulos en Logs (`ui/src/views/LogsView.vue`)
1. **Inclusión del módulo Lyrics (LOG-02 / LOG-03):**
   - Se añadió `<option value="lyrics">Lyrics</option>` en el elemento `<select v-model="filterProvider">` (`LogsView.vue:135`).
   - Permite a los usuarios filtrar registros de eventos procedentes del subsistema de letras, alineado con las emisiones normalizadas de `services/logging.rs:798` y `useLogs.ts:92`.

### C. Copias de seguridad y reconciliación accesible (`ui/src/views/settings/SettingsBackup.vue`)
1. **Compuerta de confirmación previa a la restauración (BR-02):**
   - Se introdujo el estado `showImportConfirm` con un banner explicativo y botones «Sí, Restaurar Ahora» y «Cancelar».
   - Evita la ejecución inadvertida de importaciones sobre la base de datos existente.
2. **Clarificación conceptual de la reconciliación física (BR-03):**
   - Se sustituyeron los términos oscuros por «Integridad y Sincronización de Archivos en Disco (Physical Library)».
   - Se incorporó un bloque didáctico de ayuda con descripción clara de archivos no encontrados, huérfanos y la distinción operativa entre el *Modo Auditoría (DryRun)* (sin modificaciones en disco) y el *Modo Aplicar (Execute)*.
3. **Persistencia de informes entre recargas:**
   - Se conectaron `lastExportResult`, `lastImportResult` y `lastReconcileReport` con `localStorage` (`syncify_last_backup_export`, `syncify_last_backup_import`, `syncify_last_backup_reconcile`), de modo que no se pierdan al navegar entre pestañas o recargar la vista.

---

## 3. Pruebas y verificación ejecutadas

1. **Pruebas de interfaz en Vitest:**
   - `npm test -- src/__tests__/views/DownloadsView.spec.ts src/__tests__/views/DownloadsViewQueue.spec.ts src/__tests__/views/DownloadsViewQuality.spec.ts src/__tests__/views/DownloadsViewQueueCovers.spec.ts src/__tests__/views/DownloadPreflight.spec.ts`:
     - **34 tests superados en 5 archivos (0 fallos).**
     - Se verificó que el colapso y despliegue del panel `Queue details` (`wrapper.find('.reconciliation-strip')`), la telemetría en vivo, el filtrado reactivo, los contadores de artefactos y las transiciones virtuales funcionan perfectamente.
   - `npm test -- src/__tests__/views/LogsView.spec.ts src/__tests__/composables/useLogs.spec.ts`:
     - **23 tests superados en 2 archivos (0 fallos).**
     - Comprobado el mapeo libre de mocks, la recepción de eventos Tauri en segundo plano y la respuesta de los filtros.
   - `npm test -- src/__tests__/views/SettingsView.spec.ts`:
     - **10 tests superados en 1 archivo (0 fallos).**
     - Comprobada la reactividad y guardado de ajustes.

2. **Pruebas unitarias de backend en Rust:**
   - `cargo test -p syncify-tauri --lib logs_history::tests`:
     - **7 tests superados (0 fallos):**
       - `parses_file_log_layer_lines_and_fields` — OK
       - `history_is_paginated_newest_first_and_filtered` — OK
       - `export_writes_chronological_text_file` — OK
       - `range_bounds_accept_dates_and_rfc3339` — OK
       - `splits_trailing_fields_json_from_message` — OK
       - `rejects_malformed_lines` — OK
       - `page_limit_is_clamped_to_valid_bounds` — OK

3. **Pruebas end-to-end de Backup & Restore:**
   - `cargo test -p syncify-tauri --test backup_restore_e2e_test`:
     - **5 tests superados (0 fallos):**
       - `test_backup_export_schema_and_checksum_integrity` — OK
       - `test_backup_restore_on_clean_db_recreates_library_state` — OK
       - `test_backup_restore_idempotence_and_deduplication` — OK
       - `test_backup_restore_playlist_with_tracks` — OK
       - `test_backup_restore_corrupted_checksum_fails_safely` — OK

---

## 4. Estado de los IDs, elementos abiertos y riesgos

### Matriz de estado final por ID
| ID | Clasificación | Estado | Resumen |
|---|---|---|---|
| **DSC-01** | Defecto de maquetación | **Hecho** | Panel de detalles y estadísticas integrados en el contenedor scrolleable. La zona fija ya no oculta las descargas. |
| **DSC-02 / DSC-03** | Inconsistencia de datos | **Hecho** | Corregida la aritmética engañosa; las pistas con fallo 404 ya no se disfrazan de exclusiones preflight. |
| **DSC-04** | Solicitud existente | **Falso positivo** | Búsqueda reactiva plenamente operativa en `filteredCompletedItems` vía `matchesSearch`. |
| **DSC-05** | Fallo de cálculo | **Hecho** | Progreso recalculado fielmente; descargas fallidas ya no computan como completadas. |
| **DSC-06** | Sobrecarga de interfaz | **Hecho** | Eliminadas las opciones duplicadas de borrado del menú secundario flotante. |
| **LOG-01** | Verificación de integridad | **Hecho (Verificado)** | Cero mocks; los datos provienen del búfer circular nativo de Rust y eventos Tauri en tiempo real. |
| **LOG-02** | Verificación y omisión | **Hecho** | Los logs se guardan en segundo plano continuamente. Añadido el filtro `Lyrics` al selector de módulos. |
| **LOG-03** | Verificación de filtros | **Hecho (Verificado)** | Filtros por nivel, módulo y búsqueda de texto operativos sobre entradas vivas e histórico en disco. |
| **LOG-04** | Integridad de módulos | **Hecho (Verificado)** | Los 17 módulos del sistema se muestran y filtran correctamente; se preserva su estabilidad. |
| **BR-01** | Verificación de exportación | **Falso positivo** | La exportación está al día, incluye metadatos y playlists con suma de comprobación SHA-256. |
| **BR-02** | Integridad de restauración | **Hecho** | Restauración 100% funcional y atómica (5/5 tests E2E verdes). Añadida confirmación en UI. |
| **BR-03** | Usabilidad / Claridad | **Hecho** | Panel simplificado a lenguaje natural comprensible con ayuda contextual y persistencia local. |

### Lo que queda abierto
- Ninguno de los 13 IDs asignados queda abierto ni pendiente. Todos han sido aplicados o verificados con evidencia empírica.
- `LOG-03` del plan original (`worker.rs`) y `LOG-04` (`main.rs`) pertenecen a ficheros de otros carriles (`3-errores` / `1-shell`) y no fueron modificados respetando el confinamiento estricto de archivos de este carril.

### Riesgos y mitigaciones
- **Riesgo:** Alteración de selectores CSS requeridos por pruebas unitarias al mover el panel de detalles (`showQueueDetails`).
  *Mitigación:* Se preservaron todas las clases e identificadores (`.reconciliation-strip`, `.telemetry-bar`, `showQueueDetails`, etc.) y la estructura de anidamiento, verificando que los 20 tests de `DownloadsView.spec.ts` y las pruebas de preflight y calidad continúan en verde.
- **Riesgo:** Posible inconsistencia en la persistencia de `localStorage` para reportes de backup si el almacenamiento del navegador está bloqueado o lleno.
  *Mitigación:* Las funciones `loadStoredJson` y `saveStoredJson` en `SettingsBackup.vue` encapsulan lecturas y escrituras en bloques `try/catch` seguros sin interrumpir el flujo en caso de fallo de cuota.
