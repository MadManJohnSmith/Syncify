# Plan de ejecución — 168 incidencias de pruebas manuales (v2)

Derivado de [`Errores-Pruebas-2026-10-08.md`](./Errores-Pruebas-2026-10-08.md).
**v2**: reescrito tras auditar el plan v1 contra el código con tres revisores independientes.
La v1 contenía afirmaciones falsas; están retractadas explícitamente en §2.

El orden **no es de prioridad**: es de dependencia y coste de reversión. Se ejecuta en un
único workflow con carriles de ficheros disjuntos y puerta de CI entre olas.

---

## 1. Hallazgos verificados (con evidencia)

| Hallazgo | Evidencia | Entradas |
|---|---|---|
| Sin virtualización en las vistas largas. `DownloadsView.vue:549-564` **sí** tiene uno manual funcionando; Library/Metadata/Lyrics no | `LibraryView.vue:521` renderiza `filteredTracks` enteras; `PAGE_SIZE=100` (`:1397`); `loadMore()` concatena (`:1438`) | PR-02, PR-03, PR-04 |
| `handleScroll` lee layout **antes** del corte de throttle | `LibraryView.vue:1466` lee, throttle en `:1485` | PR-01, PR-02 |
| `onUnmounted` registrado dentro de un `async` tras dos `await`: Vue lo descarta, el listener y el temporizador de 3 s sobreviven al desmontaje | `MetadataView.vue:1551` registrado tras `await` en `:1518, :1535, :1536`; temporizador en `:1545` | MET-01, MT-02, parte de PR-02 |
| El overlay `isLoading` ocupa toda la vista y se reactiva en cada refresco | `MetadataView.vue:127` (`v-if` + `flex-1`) | MET-01 |
| `get_playlists` devuelve `NULL as image_url`: la portada **no puede** mostrarse. La tabla `playlists` no tiene columna de imagen | `commands/library.rs:1532`; `migrations/0002_normalize_schema.sql:150-159` | P-04 |
| `specialBadge` se declara (`:1333`) y se pinta (`:640-642`) pero `mapToTrack` **nunca lo asigna** | `LibraryView.vue:1360-1389` | L-06 |
| Los nombres de servicio llegan crudos desde la BD y `services.name` es `apple_music`; no existe ninguna transformación guion bajo→espacio en todo el repo | `DashboardView.vue:501-502` solo capitaliza; `migrations/0002_normalize_schema.sql:267` | D-05 |
| La cascada de calidad degrada **dentro** de Qobuz antes de cambiar de servicio: 27→7→6→5, o 27→7→6 sin pérdida | `crates/syncify-core-domain/src/quality.rs:94-104`, consumido en `qobuz.rs:162-168, :421` | FS-05, AQ-06 |
| El recorrido entre servicios solo existe para pistas cuyo servicio **no** es qobuz/tidal (vía SongLink). Para Qobuz y Tidal el clasificador corta con `Err` | `orchestrator.rs:629, 635, 643`; SongLink en `:754-821` | FS-05 |
| El fallback de identidad solo devuelve Tidal: `FallbackMatch.target_service` es siempre `"tidal"` | `orchestrator.rs:361, 419` | FS-05 |
| Qobuz resuelve token con `ORDER BY a.id DESC LIMIT 1` y `.ok()`, que convierte los 4 motivos de `RequiresAuth` en `None` → el usuario ve "proxy failed" en vez de "tu cuenta está desconectada" | `qobuz.rs:279-283, :1103`; `QobuzAuthStatus` en `:272, :291, :301, :372` | DSC-07, DSC-08 |
| `unwrap_or_default()` en 4 consultas SQL de decisión convierten "BD caída" en "no hay candidatos" | `orchestrator.rs:350, 420, 457, 508`; `:543` | DSC-08 |
| `is_active` es único por servicio, así que el fallback entre cuentas no puede hacerse toggleando | `commands/accounts.rs:225, 524` | DSC-09 |
| `TidalSingleTrackRequest` no tiene campo `account_id`, y el pipeline que ya lo acepta se invoca con `None` | `download/tidal.rs:30-79`; `tidal_pipeline.rs:114-133, :490, :797` | DSC-09 |
| Los ajustes de letras existen en Rust y la UI los lee y escribe; lo que falta es que el **descargador** los aplique (matcher hardcodeado) | `download/lyrics.rs:244-268`; `commands/settings.rs:880-905`; `models.rs:134-136` | CFGL-02, CFGL-03, CFGL-05, LYM-01 |
| `strip_lrc_timestamps()` solo quita `[mm:ss.xx]`; no hay limpieza de encabezados de título/artista | `crates/syncify-lyrics-domain/src/lib.rs:294-303` | LY-02 |
| El timestamp se muestra sin condición de modo (rama sincronizada) | `LyricsView.vue:401`; la no sincronizada `:411-415` no los muestra | LY-01 |
| El tema vive en `localStorage`; no existe `appearance_settings` en el backend, hay que **crear** la superficie | `useTheme.ts:18, 34` | G-02, AJ-05, AJ-06 |
| El progreso de migración **sí** persiste en `migration_jobs`/`migration_items`; el defecto es que el frontend nunca reengancha un job en vuelo al montar | `commands/migration.rs:20-22, :1335, :1693`; `useMigration.ts:340-369`; `MigrationView.vue:1148-1157` | M-04 |
| La barra global está dentro del `<header>` de `<main>`, y el sidebar es un `<aside class="w-64">` hermano flex: por eso nunca ocupa el ancho completo | `App.vue:18, :120, :122, :342` | G-03 |
| Ninguna notificación tiene acción al hacer clic: al pulsarla solo se marca como leída | `App.vue:291` (`markAsRead`) | NT-01 |
| 14 sitios lanzan procesos sin `CREATE_NO_WINDOW` (no 10). En la ruta de descarga están los 8 de flac-writer y el remux de Tidal | `cmd_utils.rs:216, 228` (únicos que lo usan); `syncify-flac-writer/src/lib.rs:973, 1062, 2254, 2318, 2490, 2623, 2666, 2824`; `syncify-tidal-downloader/src/lib.rs:2010`; `tray.rs:203, 279` | G-01, G-06 |
| `enforce_post_download_quality_gate` devuelve lo mismo en ambas ramas: no rechaza nada | `download/audio_inspector.rs:137-153` | AQ-06 |
| El techo global **ya existe** en backend y vive en la UI dentro de Folder Structure | `commands/settings.rs:1486, 1519, 1532, 1556`; aplicado en `worker.rs:942-1040`; UI en `SettingsDownloads.vue:209-260`, montada bajo `folders` (`SettingsView.vue:74`) | AQ-03, FS-02 |
| El supuesto "duplicado" de calidad global son dos selectores **semánticamente distintos**: el techo (KV) y el preset, que además **pisa** los techos por servicio | `SettingsDownloads.vue:96-131`; `useDownloadSettings.ts:433-470` | FS-06, AQ-03 |
| No hay transcodificación en la ruta de descarga (los `-c:a copy` son remux puro). `models.rs:105 transcode_enabled` nunca se lee | `syncify-tidal-downloader/src/lib.rs:2013`; flac-writer solo usa ffmpeg para portada y PCM | AQ-06 |

---

## 2. Retractaciones (afirmaciones de la v1 que eran falsas)

No están "reinterpretadas": **se retiran**. Ninguna tarea debe basarse en ellas.

1. **L-07 "los nombres de campo no coinciden"** — falsa. No existen `quality_label` ni
   `quality_formats`; el backend devuelve `quality` y `quality_bucket`
   (`library.rs:192, :201`; `api/types.ts:47`). Los filtros reales están en
   `LibraryView.vue:2098-2103` (`t.quality.includes('/')`, `includes('kbps')`) y
   `:2110-2112` (`t.services.length >= 2`). **L-07 sigue siendo un fallo real, con otra
   causa**: hay que determinar por qué esos filtros no encuentran nada con datos válidos.
2. **"`handle_qobuz` traga el error con `let _ =`"** — falsa. No existe esa función ni ningún
   `let _ =`. El clasificador de `orchestrator.rs:616-741` **propaga** el error, y no hacer
   fallback ante 401 está codificado a propósito (`"Automatic fallback aborted."` en `:629`).
   El tragado real está en `qobuz.rs:1103` y en los `unwrap_or_default()` de `orchestrator.rs`.
3. **"`queue.rs:460` fija un único servicio, no hay lista de alternativas"** — falsa tal como
   se citó. `queue.rs:460` inicia la consulta de candidatos (`:474-488`) y el ranking
   (`:498-620`). Lo cierto es el residuo: sobrevive un `chosen_candidate` por encolado.
4. **"`main.ts:33` monta con `skipInitialLoad:true`"** — falsa; no existe en el repo.
   **"El tema no está disponible al arrancar"** — falsa: `ui/index.html:8-17` lo aplica antes
   del primer paint. **"`appearance_settings` no se llama"** — correcta, pero el comando no
   existe: hay que crearlo.
5. **M-01 "Templates no existe, falso positivo"** — falsa. `Saved Templates` funciona:
   `MigrationView.vue:858-905`, `useMigration.ts:49, 105`, API get/save/delete/use,
   tabla `migration_templates` (`commands/migration.rs:22`). **M-01 sigue vivo**: hay que
   decidir con el usuario si se explica o se retira.
6. **D-05 "el separador está bien aplicado"** — falsa. **No existe** ninguna
   transformación guion bajo→espacio. El síntoma reportado tiene causa real: el nombre crudo
   `apple_music` se renderiza tal cual. Es un fallo de datos + falta de formateo.
7. **`lyrics/download.rs:273-330`** — ruta inexistente; el fichero real es
   `src-tauri/src/download/lyrics.rs`. Y "no los lee el consumidor" era demasiado fuerte: la
   UI sí los lee y escribe.
8. **"La persistencia del job de migración no existe"** — falsa. Existe en tabla; el defecto
   es de frontend (no reengancha el job en vuelo).
9. **NT-01 "`App.vue:124-134` llama a `window.__zcode_notify`"** — evidencia fabricada; no
   existen ni `handleNotify` ni esa variable. La conclusión sí se sostiene (`App.vue:291`).
10. **"168 entradas = 31 causas raíz"** — insostenido como afirmación global. Se sustituye
    por la matriz de trazabilidad de §3.
11. **Colapso de la primera fila** — tres IDs mal atribuidos: D-07 (widgets desbordados) y
    L-13 (`menuHeight = 360` hardcodeado en `LibraryView.vue:1795`) no tienen relación con el
    virtualizador; L-11 es el `absolute top-full` de `LibraryView.vue:181`.
12. **G-06** — la lista de 14 sitios es la buena; `system_conditions.rs:94, 116` son
    `busctl`/`gdbus` (solo Linux) y no causan el bug reportado.

### Contratos que no se rompen

- **`src-tauri/tests/provider_fallback_identity_test.rs`** es el contrato ejecutable de
  `orchestrator.rs:616-741`, con 9 casos que asertan el comportamiento actual. Cambiar
  FS-05 lo rompe **a propósito**: se actualiza, no se "arregla".
- **`cli_tauri_behavioral_parity_test.rs:562`** implica que hay un CLI espejo: todo cambio
  de política de calidad/fallback se replica allí o el test falla.

---

## 3. Trazabilidad: los 168 IDs con dueño

Ola · carril · ID. Ningún ID queda sin asignar.

**Ola 0 — auditorías (solo lectura)**
- `0-matriz-ajustes`: TR-01, TR-02, TR-03, AJ-01..AJ-11, SP-05, SS-01, FS-02, FS-06, AQ-01, CFGL-01..CFGL-05
- `0-notificaciones`: NT-01..NT-04
- `0-triaje`: **los 28 IDs huérfanos** de la v1 → L-01, L-02, DSC-01..DSC-06, Q-01, PR-03, PR-04, MET-02, MET-03, LY-04, LY-06, LY-07, LY-09, LY-11, LY-12, LOG-01..LOG-04, AQ-02, MT-03, BR-01..BR-03

**Ola 1 — cimientos**
- `1-shell`: G-03, G-04, G-05, BP-01..BP-04, PL-01, PL-02, PL-03, PL-06, PL-07
- `1-procesos-windows`: G-01, G-06
- `2-letras-dominio`: LY-02 (base del dominio, para que las letras que llegan limpias no se re-ensucien)

**Ola 2 — render** (depende de `1-shell`: las vistas se montan en el shell nuevo)
- `2-biblioteca`: PR-01, PR-02, L-11, L-12
- `2-metadata`: MT-02, MET-01, MET-02
- `2-letras`: PR-03
- `2-dashboard`: D-01..D-07
- `2-playlists`: P-01..P-06

**Ola 3 — motor de descargas** (un solo dueño de `orchestrator.rs` y `worker.rs`)
- `3-errores`: DSC-07, DSC-08
- `3-cuentas`: DSC-09
- `3-fallback`: FS-05, AQ-03, AQ-04, AQ-05, AQ-06, FS-04

**Ola 4 — la app deja de mentir** (tras olas 2 y 3, para no volver a tocar `LibraryView.vue`)
- `4-biblioteca`: L-01..L-10
- `4-scoring`: MT-01, SC-01..SC-04, MD-01, MD-02, MD-03
- `4-procesamiento`: BPM-01..BPM-03, DUP-01, DUP-02, AP-01..AP-04
- `4-carpetas`: FS-01, FS-03, FS-07, FS-08, SYM-01..SYM-03

**Ola 5 — ajustes** (requiere `0-matriz-ajustes`)
- `5-matriz`: TR-03, AJ-02..AJ-08, AQ-01, FS-02, FS-06, SP-05, SS-01
- `5-tema`: G-02, AJ-05, AJ-06

**Ola 6 — cuentas, migración, datos**
- `6-cuentas`: C-01..C-10, Q-01, SP-01..SP-05
- `6-migracion`: M-01..M-07
- `6-datos`: LOG-01..LOG-04, BR-01..BR-03

**Ola 7 — reproductor y biblioteca local**
- `7-reproductor`: PL-04, PL-05, PL-08, PL-09, PL-10
- `7-local`: F-01, F-02

---

## 4. Topología ejecutable

| Ola | Carriles | Regla |
|---|---|---|
| 0 | 3 | solo lectura, escriben en `docs/auditoria/`. Puerta: informes presentes |
| 1 | 3 | disjuntos. Puerta A: CI |
| 2 | 5 | disjuntos. Puerta B: CI + medición de Library |
| 3 | 3 **secuenciales** | comparten `orchestrator.rs`/`worker.rs`. Puerta C: test focalizado de fallback |
| 4 | 4 | disjuntos. Puerta D: CI |
| 5 | 2 | disjuntos. Puerta E: CI |
| 6 | 3 | disjuntos. Puerta F: CI |
| 7 | 2 | disjuntos. Puerta G: CI completo |

**Ficheros con dueño único** (nadie más los toca en la misma ola): `App.vue`,
`StatusBar.vue`, `LibraryView.vue`, `MetadataView.vue`, `LyricsView.vue`,
`DashboardView.vue`, `PlaylistView.vue`, `MigrationView.vue`, `useTheme.ts`,
`ui/index.html`, `download/orchestrator.rs`, `worker.rs`, `commands/queue.rs`,
`download/qobuz.rs`, `crates/syncify-core-domain/src/quality.rs`, `download/lyrics.rs`,
`commands/settings.rs`, `syncify-flac-writer`, `syncify-tidal-downloader`.

**Reglas transversales**
1. Ningún ajuste nuevo sin fila en la matriz de `0-matriz-ajustes`.
2. Ninguna prueba borrada sin que el comportamiento haya cambiado a propósito, y dicho por qué.
3. `provider_fallback_identity_test.rs` y `cli_tauri_behavioral_parity_test.rs` se actualizan
   en el mismo cambio que altera la política, nunca para silenciar un fallo.
4. Un `#[cfg(windows)]` mal puesto no falla en Linux: verificar con
   `cargo check --target x86_64-pc-windows-msvc`.
5. Si una tarea no puede completarse, se documenta el bloqueo; no se marca hecha.

---

## 5. Decisiones que corresponden al usuario

Se acumulan durante el workflow y se listan al final, sin bloquear:

- **M-01**: ¿"Saved Templates" se explica mejor o se elimina?
- **L-06**: ¿qué debe significar realmente la columna "Special"?
- **LY-12**: qué hace exactamente Delete (¿desaparece la canción de la biblioteca, del disco
  o solo de la vista?). El comando solo borra filas de `lyrics`
  (`commands/lyrics.rs:753-777`); hay que reproducirlo para confirmar qué vio el usuario.
- **AQ-05**: si Spotify admite o no descarga en alta resolución. El plan lo trata como
  hipótesis, no como hecho.
- **LY-02**: criterio de limpieza de encabezados. Se necesitan muestras reales para no
  borrar versos legítimos.
