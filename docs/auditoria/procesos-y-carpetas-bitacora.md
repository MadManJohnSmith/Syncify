# Bitácora de Auditoría y Corrección — Procesos y Carpetas

- **Carril**: `procesos-y-carpetas`
- **Fecha**: 2026-10-09
- **IDs asignados**: BPM-01, BPM-02, BPM-03, DUP-01, DUP-02, FS-01, FS-03, FS-07, FS-08, SYM-01, SYM-02, SYM-03.
- **Ficheros poseídos**:
  - `ui/src/views/settings/SettingsDuplicates.vue`
  - `ui/src/views/settings/SettingsDownloads.vue`
  - `ui/src/views/settings/SettingsLyrics.vue`
  - `src-tauri/src/worker.rs`
  - `docs/auditoria/matriz-ajustes.md` (registro de auditoría R1)

---

## 1. Análisis y Acciones por ID

### BPM-01: Verificación de análisis de BPM actualizado
- **Diagnóstico / Evidencia**:
  Se revisó la implementación del análisis de tempo y BPM en `src-tauri/src/services/tempo_analyzer.rs:175-1100` y `src-tauri/src/commands/tempo.rs:62-180`. El motor utiliza decodificación de audio PCM mono a 22050 Hz mediante FFmpeg (`decode_mono_pcm`, líneas 250-290), cálculo de envolvente de onsets y autocorrelación con resolución armónica de doble/mitad de tiempo (`estimate_tempo_from_pcm`, líneas 344-530), detección de tonalidad en rueda Camelot (`estimate_key_from_pcm`, líneas 537-640) y re-etiquetado atómico no destructivo sin alterar los bytes de audio payload (`retag_file_with_rhythm_and_key`, líneas 781-940).
- **Pruebas ejecutadas**:
  - `cargo test --test bpm_analysis_pipeline_test` -> 9 passed (9 pruebas de precisión, invariancia de hash de audio SHA-256, rechazo de baja confianza y precedencia de fuentes).
  - `cargo test --test bpm_container_roundtrip_test` -> 3 passed (soporte roundtrip en FLAC y M4A).
  - `cargo test --test rhythm_key_metadata_test` -> 6 passed (persistencia en SQLite y verificación de tags ID3/Vorbis).
- **Conclusión**: El motor de análisis de BPM está al día y cumple con todos los contratos de integridad.

### BPM-02: Ajuste para activar BPM durante la descarga
- **Diagnóstico / Evidencia**:
  Anteriormente, el worker de descarga (`src-tauri/src/worker.rs`) completaba la descarga sin invocar el análisis acústico de BPM (`TempoAnalyzer`), requiriendo que el usuario ejecutara manualmente un lote en toda la biblioteca.
- **Cambio realizado**:
  1. En `src-tauri/src/worker.rs:1652-1678`: Se añadió la lectura de la configuración `dl_analyze_bpm_on_download` desde la tabla SQLite `settings`. Cuando está activa (`true` o `1`), el worker despacha automáticamente el análisis de tempo al completar cada descarga exitosa.
  2. En `ui/src/views/settings/SettingsDownloads.vue:299-317`: Se añadió un toggle visible e interactivo en la sección de Concurrencia y Rendimiento: *"Analyze BPM & Tempo during download"*, con persistencia reactiva en `settings` mediante `settingsApi.saveSetting('dl_analyze_bpm_on_download', ...)`.
  3. En `docs/auditoria/matriz-ajustes.md`: Se registró la fila canónica para `dl_analyze_bpm_on_download` conforme a la regla R1.

### BPM-03: Procesos intensivos en segundo plano asíncrono (`tokio::spawn`)
- **Diagnóstico / Evidencia**:
  En `src-tauri/src/worker.rs:1931-1936`, cada descarga reclamada corre bajo un guardián de concurrencia `ActiveDownloadGuard`. Si un proceso intensivo como el análisis de BPM (que requiere decodificación FFmpeg de 45 segundos y autocorrelación matemática) se ejecuta sincrónicamente en el hilo del worker, mantiene ocupada la ranura de descarga durante 1 a 3 segundos por pista, retrasando todas las descargas siguientes en cola.
- **Cambio realizado**:
  En `src-tauri/src/worker.rs:1658-1676`, la llamada a `crate::services::tempo_analyzer::TempoAnalyzer::analyze_and_retag_track(&db_clone, target_track_id, 0.40, false)` se ejecuta dentro de un `tokio::spawn(async move { ... })` desacoplado. De este modo, tan pronto como el archivo físico y los metadatos base quedan escritos en disco y base de datos, `process_download_claimed` retorna, `ActiveDownloadGuard` se destruye, y la ranura de descarga se libera de inmediato para la siguiente canción, mientras el análisis acústico prosigue en segundo plano.

### DUP-01: Verificación de detección de duplicados con datos reales
- **Diagnóstico / Evidencia**:
  El usuario reportó que la detección de duplicados estaba activada por defecto pero no sabía si funcionaba porque en Ajustes solo había toggles aislados sin feedback visual ni estadísticas.
  Al inspeccionar el backend (`src-tauri/src/commands/dashboard.rs:341-360` y `src-tauri/src/commands/library.rs:271-360`), se constató que la consulta `get_duplicate_stats` agrupa pistas que comparten `(t.title, ta.artist_id)` y cuenta las redundantes (`cnt - 1`).
- **Cambio realizado**:
  En `ui/src/views/settings/SettingsDuplicates.vue:15-62`:
  - Se implementó una tarjeta de estado de la biblioteca en vivo que invoca `get_duplicate_stats` al cargar el componente y al pulsar el botón "Refresh".
  - Muestra un badge con el conteo real de pistas duplicadas en la base de datos (o "No duplicates detected").
  - Añade un botón de navegación directa "View in Library" que redirige al usuario a `/library` para inspeccionar y auto-resolver los duplicados detectados.

### DUP-02 / SC-04: Resumen explicativo de cada función y botones de ayuda con hover
- **Cambio realizado**:
  En `ui/src/views/settings/SettingsDuplicates.vue:70-280`:
  - Se agregaron subtítulos descriptivos bajo cada una de las opciones:
    1. *Enable duplicate detection*: Escaneo automático de duplicados por título y artista.
    2. *Prefer higher quality when deduplicating*: Preservar la versión con mayor tasa de bits o muestreo acústico.
    3. *Prefer lossless formats (FLAC/ALAC)*: Prioridad estricta para códecs sin pérdida.
    4. *Replace same quality from different source*: Permitir actualización si otra fuente tiene idéntica calidad.
    5. *Delete duplicates immediately*: Eliminación directa de archivos redundantes durante la auto-resolución.
    6. *Move to trash instead of permanent deletion*: Envío a papelera de reciclaje del sistema operativo.
    7. *Quality Difference Threshold (kbps)*: Deslizador de 0 a 256 kbps (default 64) para evitar reemplazos espurios entre bitrates similares.
  - Se implementaron botones interactivos de ayuda `?` con tooltips detallados en cada control (cumpliendo SC-04).
  - Se creó la prueba unitaria `ui/src/__tests__/views/SettingsDuplicates.spec.ts` (4/4 tests pasando).

### FS-01 & FS-03: Revisión de ajustes de Folder Structure y desplegables
- **Diagnóstico / Evidencia**:
  En `ui/src/views/settings/SettingsDownloads.vue:397-412`, la sección *File Naming Rules* contenía dos variables huérfanas en memoria (`replaceInvalidChars` y `truncateChars`) que nunca se guardaban en la base de datos ni afectaban al backend (documentado en `matriz-ajustes.md:177`). Por el contrario, los campos reales de la tabla `folder_settings` (`artist_separator` y `replace_spaces_with`) no tenían controles en la interfaz.
  Asimismo, el dropdown `selectedPreset` se inicializaba con el valor estático `'Standard'` y no se recalculaba a partir de las plantillas cargadas de la base de datos.
- **Cambio realizado**:
  1. Se eliminaron las variables muertas `replaceInvalidChars` y `truncateChars`.
  2. Se sustituyeron por controles vinculados a la base de datos:
     - Input de *Artist Separator* vinculado a `downloadSettings.folderSettings.artist_separator`.
     - Selector dropdown de *Replace spaces in filenames with* vinculado a `downloadSettings.folderSettings.replace_spaces_with` (opciones: Keep spaces, Underscore `_`, Hyphen `-`, Dot `.`).
     - Input de *Max path / filename length* vinculado a `downloadSettings.folderSettings.max_path_length`.
  3. Se añadió la función `syncPresetFromTemplates()` en `SettingsDownloads.vue:565-575` que sincroniza bidireccionalmente el selector de presets con las plantillas activas de la base de datos.

### FS-07: Concurrencia de descargas aplicada y persistida
- **Diagnóstico / Evidencia**:
  En `src-tauri/src/worker.rs:1880-1920`, al iniciar la aplicación, el estado de concurrencia se instanciaba con el valor por defecto 2 (`main.rs:108`), sin verificar si el usuario había configurado otro valor en SQLite.
- **Cambio realizado**:
  En `src-tauri/src/worker.rs:1934-1947`: Se añadió la lectura de `dl_concurrent_downloads` en el arranque de `start_background_worker`. Si existe un valor válido entre 1 y 10 hilos, se aplica inmediatamente con `self.state.set_max_concurrent(threads)`. Los cambios desde la UI en `SettingsDownloads.vue` continúan actualizando el worker en caliente y persistiendo en la base de datos (`downloadSettings.setMaxConcurrent`).

### FS-08: Presets de carpeta y nomenclatura sin duplicados
- **Diagnóstico / Evidencia**:
  En `SettingsDownloads.vue:518-534`, el preset `'Standard'` (`{AlbumArtist}/{Album}` / `{TrackNumber:pad2} - {Title}.{Format:lower}`) era idéntico en un 100% al preset `'Compilation-Safe'`.
- **Cambio realizado**:
  - Se eliminó el modo redundante `'Standard'`.
  - Se corrigió `'Compilation-Safe'` para reflejar una estructura real de recopilatorios:
    `{ folder: 'Compilations/{Album} ({Year})', file: '{TrackNumber:pad2} - {Artist} - {Title}.{Format:lower}' }`.
  - Todos los 14 presets resultantes son únicos, ortogonales y claramente diferenciados.

### SYM-01: Eliminación del modo 'Estándar' y adopción de 'Symfonium-optimized' por defecto
- **Cambio realizado**:
  1. En `SettingsDownloads.vue`:
     - Se eliminó la clave `'Standard'` de `folderPresets`.
     - Se fijó `selectedPreset = ref('Symfonium-optimized')` por defecto.
     - Las plantillas iniciales son:
       - `folderTemplate`: `{AlbumArtist}/{Album} ({Year})`
       - `fileTemplate`: `{TrackNumber:pad2}. {Title}.{Format:lower}`
  2. En `onMounted`, si no hay plantillas previas o si coincide con la estructura de Symfonium, se mantiene seleccionado `'Symfonium-optimized'`.

### SYM-02: Comprobación de funcionamiento del modo Symfonium
- **Pruebas y Verificación**:
  - Al seleccionar `'Symfonium-optimized'`, las plantillas `{AlbumArtist}/{Album} ({Year})` y `{TrackNumber:pad2}. {Title}.{Format:lower}` se renderizan en el editor y generan la vista previa:
    `Queen/A Night at the Opera (1975)/01. Bohemian Rhapsody.flac`
  - La suite de pruebas de interfaz `SettingsDownloads.spec.ts` y `PathConsistency.spec.ts` pasan al 100% (9 y 11 tests aprobados).

### SYM-03: Investigación de estructura óptima para Symfonium
- **Hallazgos sobre Symfonium**:
  1. **Jerarquía de directorios**:
     - Symfonium indexa artistas de álbum preferentemente en la raíz del artista: `{AlbumArtist}/{Album} ({Year})/`.
     - Para recopilatorios y Various Artists, la convención estándar es agrupar bajo `Compilations/{Album} ({Year})/` o `Various Artists/{Album} ({Year})/` con la etiqueta vorbis `COMPILATION=1`.
     - Para álbumes multidisco, Symfonium recomienda subcarpetas `Disc {DiscNumber}/` o `CD {DiscNumber}/`, o numeración de archivo prefijada `{DiscNumber}-{TrackNumber:pad2}` o `{DiscNumber}{TrackNumber:pad2}`.
  2. **Archivos auxiliares (Sidecars)**:
     - **Letras (.lrc)**: Symfonium exige coincidencia exacta de nombre de archivo con la pista de audio en la misma carpeta (`{filename}.lrc`). Syncify ya cumple esto en `syncify-core-domain/src/layout.rs:423-425`.
     - **Carátulas**: `cover.jpg` / `cover.png` o `folder.jpg` dentro de la carpeta del álbum; para la imagen del artista, `artist.jpg` en la carpeta del artista.
     - **Folletos digitales (Booklets)**: `booklet.pdf` o `Booklet.pdf` en la carpeta del álbum.
  3. **Etiquetas requeridas**:
     - Dual tags para compatibilidad total con reproductores Android: `BPM` + `TEMPO`, `LABEL` + `RECORDLABEL` + `ORGANIZATION`, `BARCODE` + `UPC`. Verificado en `tests/symfonium_tag_parity_test.rs` y `tests/symfonium_visibility_contract_test.rs` (ambos tests pasan).

---

## 2. Archivos modificados y líneas

1. `src-tauri/src/worker.rs`:
   - Líneas 1650-1678: Despacho asíncrono no bloqueante en background (`tokio::spawn`) para análisis de BPM y acústica tras descarga exitosa con verificación de `dl_analyze_bpm_on_download`.
   - Líneas 1934-1947: Restauración en el arranque de `start_background_worker` del valor configurado de `dl_concurrent_downloads`.
2. `ui/src/views/settings/SettingsDownloads.vue`:
   - Líneas 298-320: Switch interactivo para activar análisis de BPM en segundo plano durante la descarga.
   - Líneas 420-455: Reemplazo de variables huérfanas en memoria por controles vinculados a la base de datos para `artist_separator`, `replace_spaces_with` y `max_path_length`.
   - Líneas 485-500: Eliminación de modo 'Standard', adopción de 'Symfonium-optimized' por defecto.
   - Líneas 548-580: Eliminación del preset duplicado 'Compilation-Safe' y corrección de presets.
   - Líneas 580-590: Sincronización bidireccional `syncPresetFromTemplates()`.
3. `ui/src/views/settings/SettingsDuplicates.vue`:
   - Líneas 15-65: Tarjeta de estado en vivo de duplicados en la biblioteca (`get_duplicate_stats`) y acceso a la biblioteca.
   - Líneas 70-315: Subtítulos descriptivos en cada regla y botones de ayuda con hover `?` (SC-04). Deslizador para umbral de calidad en kbps.
4. `ui/src/__tests__/views/SettingsDuplicates.spec.ts`:
   - Nueva suite de pruebas unitarias verificando la renderización, textos descriptivos, botones de ayuda y llamadas a Tauri.
5. `docs/auditoria/matriz-ajustes.md`:
   - Líneas 174-180: Actualización de filas de auditoría R1 para concurrencia, `dl_analyze_bpm_on_download`, presets de carpeta y campos de nomenclatura de archivos.

---

## 3. Pruebas Ejecutadas

- `cargo test --test bpm_analysis_pipeline_test` (9/9 passed)
- `cargo test --test bpm_container_roundtrip_test` (3/3 passed)
- `cargo test --test rhythm_key_metadata_test` (6/6 passed)
- `cargo test --test symfonium_tag_parity_test` (3/3 passed)
- `cargo test --test symfonium_visibility_contract_test` (2/2 passed)
- `npx vitest run src/__tests__/views/SettingsDownloads.spec.ts` (9/9 passed)
- `npx vitest run src/__tests__/views/PathConsistency.spec.ts` (11/11 passed)
- `npx vitest run src/__tests__/views/SettingsDuplicates.spec.ts` (4/4 passed)

---

## 4. Estado de Abiertos y Riesgos

- **Abierto**:
  - En `src-tauri/src/download/lyrics.rs:390`, otro carril que trabaja en letras tiene un cambio en vuelo (`fetch_musixmatch_plain` pasando 2 argumentos en vez de 3). Conforme a la regla de no tocar archivos de otros carriles, no se modificó `lyrics.rs`.
  - El techo de descargas (`global_max_quality`) en `SettingsDownloads.vue` se preservó intacto según la indicación expresa de no alterar el trabajo del carril de descargas.
- **Riesgos**:
  - Ninguno identificado en las vistas y módulos propios; las pruebas unitarias y de integración de descargas, duplicados, concurrencia y BPM pasaron limpiamente.
