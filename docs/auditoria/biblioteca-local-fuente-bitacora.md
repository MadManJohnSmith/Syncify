# Bitácora de Auditoría y Ajustes — Carril biblioteca-local-fuente

- **Carril**: `biblioteca-local-fuente` (Ola 7 / `7-local`)
- **Fecha**: 2026-10-09
- **IDs asignados**: F-01, F-02
- **Ficheros intervenidos**:
  - `crates/syncify-flac-writer/src/lib.rs`
  - `src-tauri/src/commands/library.rs`
  - `ui/src/api/library.ts`
  - `ui/src/components/LocalLibrarySyncModal.vue`
  - `ui/src/__tests__/api/library.spec.ts`
  - `ui/src/__tests__/components/LocalLibrarySyncModal.spec.ts`
  - `docs/auditoria/matriz-ajustes.md`

---

## 1. Qué se cambió (con rutas y líneas exactas)

### F-01: Escaneo real de archivos del disco, extracción de metadatos, deduplicación e importación en base de datos
- **Problema previo**: La app original surgió como una herramienta para escanear bibliotecas locales y contrastarlas con servicios de streaming. Sin embargo, los comandos `scan_local_library` en `src-tauri/src/commands/tools.rs:116` y `ui/src/views/AccountsView.vue:1550-1600` se limitaban a invocar un script auxiliar y almacenar un objeto volátil en la memoria de la interfaz (`libraryPaths.value.push(newPath)`), sin registrar pistas, artistas ni álbumes en SQLite, constituyendo un simulacro sin persistencia ni reconciliación real.
- **Solución aplicada**:
  1. **Lectura de metadatos FLAC en `crates/syncify-flac-writer/src/lib.rs:2955-3080`**: Se implementó la función pública `read_flac_metadata(file_path: &Path) -> Result<FlacMetadata, String>`, que parsea el bloque `STREAMINFO` (frecuencia de muestreo, profundidad de bits, muestras totales) y comentarios Vorbis (`TITLE`, `ARTIST`, `ALBUM`, `ALBUMARTIST`, `TRACKNUMBER`, `DISCNUMBER`, `DATE`/`YEAR`, `GENRE`, `ISRC`, `MUSICBRAINZ_TRACKID`, `MUSICBRAINZ_ARTISTID`, `BPM`, `EXPLICIT`, `COMPILATION`).
  2. **Escáner e inspector local en `src-tauri/src/commands/library.rs:5356-5530`**: Se implementó `scan_local_audio_files(dir_path: &Path, recursive: bool) -> Result<Vec<LocalScannedTrack>, String>`, compatible con `.flac`, `.mp3`, `.m4a`, `.wav`, `.ogg`, `.opus`, `.aac`, `.alac`. Calcula hash criptográfico SHA-256 para identificación unívoca por contenido y evalúa la calidad con el motor compartido de `syncify-metadata-domain::scoring::calculate_metadata_score` bajo las reglas de paridad estricta **SC-03** (`TrackMetadataScoreInput::new_local`, `is_local: true`).
  3. **Sincronización y deduplicación real en `src-tauri/src/commands/library.rs:5532-5845`**: Se implementó `sync_local_library_inner(db: &SqlitePool, directory: &str, recursive: bool, auto_resolve: bool) -> Result<LocalSyncReport, String>` y `sync_local_library(...)`:
     - **Detección de archivos existentes**: Comprueba duplicados en la tabla `downloads` por ruta absoluta o hash SHA-256 para evitar inserciones redundantes.
     - **Deduplicación contra catálogo de streaming existente**:
       - Búsqueda exacta por código ISRC (`tracks.isrc`).
       - Búsqueda exacta por título normalizado + artista principal normalizado (`LOWER(TRIM(t.title))` y `LOWER(TRIM(a.name))`).
       - En caso de coincidencia, la pista física local se enlaza a la pista existente mediante `downloads` (`effective_service = 'local'`), `track_sources` (`service_id = local_service_id`, `quality_score = score`) y `library_entries`, actualizando el estado a descargado/disponible sin duplicar la canción en la biblioteca.
     - **Inserción de canciones nuevas**: Inserta o recupera artista en `artists` (`sanitize_artist_name`), álbum en `albums` (`sanitize_album_title`), enlaces `album_artists` y `track_artists`, registro en `tracks`, registro en `downloads`, registro en `track_sources` y entrada en `library_entries` bajo la cuenta local.
     - **Mantenimiento de contadores y clusters**: Actualiza `albums.local_track_count` y ejecuta la resolución automática de duplicados (`auto_resolve_duplicates_inner`).

### F-02: Biblioteca local como servicio de primera clase en base de datos, streaming local y metadatos
- **Problema previo**: La biblioteca local no figuraba como servicio registrado en `services` ni poseía cuenta de usuario en `accounts`. Las canciones locales no participaban plenamente en la resolución de streaming del protocolo `syncify-media://` ni en las consultas agregadas de la biblioteca.
- **Solución aplicada**:
  1. **Registro de servicio y cuenta en `src-tauri/src/commands/library.rs:5270-5320`**: Se implementó `ensure_local_service_registered(db: &SqlitePool) -> Result<(i64, i64), String>`, asegurando la existencia del servicio `local` (`supports_download = 0`, `max_quality = 'hires'`) en `services` y de la cuenta `Local Library` (`local@syncify.internal`) en `accounts`.
  2. **Resolución dinámica de IDs de servicio en reconciliación `src-tauri/src/commands/library.rs:4550-4560`**: Se reemplazó la asignación fija (`3` para Tidal, `1` para Spotify, `2` para Qobuz) por una consulta dinámica con `SELECT id FROM services WHERE LOWER(name) = LOWER(?)`, permitiendo que el servicio `local` enlace archivos huérfanos con su identificador real.
  3. **Streaming y reproducción garantizada**: Dado que las canciones locales se insertan en `downloads` con `file_path` y `file_format`, el comando existente `resolve_playback_source` (`src-tauri/src/commands/playback.rs:38-75`) las reconoce, valida y autoriza inmediatamente para streaming mediante el protocolo nativo `syncify-media://`.
  4. **Consulta y edición de metadatos**: `get_track_metadata` en `src-tauri/src/commands/library.rs:50-98` devuelve `downloaded_from: 'local'`, `imported_from: 'local'` y `file_path`. Para pistas FLAC locales, `write_track_tags` (`commands/tags.rs:240-300`) y `read_track_tags` interactúan directamente con el archivo físico mediante `syncify-flac-writer`.
  5. **API de cliente en `ui/src/api/library.ts:20-33, 243-285, 1391`**: Se agregaron los tipos `LocalSyncReport`, `LocalSyncOptions` y la función `syncLocalLibrary`, integrada y exportada en el objeto `libraryApi`.
  6. **Componente de interfaz `ui/src/components/LocalLibrarySyncModal.vue`**: Nuevo componente modal para seleccionar directorio de música, configurar búsqueda recursiva y resolución de duplicados, y ejecutar la sincronización mostrando el desglose real (archivos escaneados, pistas importadas, duplicados emparejados con streaming, pistas omitidas).

---

## 2. Evidencia y Pruebas Ejecutadas

1. **Pruebas unitarias de lectura/escritura de metadatos FLAC en `crates/syncify-flac-writer`**:
   ```bash
   cargo test -p syncify-flac-writer
   ```
   *Salida*: 13 pruebas pasadas en `src/lib.rs` (incluyendo `test_read_flac_metadata_roundtrip`) y 27 pruebas totales en el crate. Cero fallos.

2. **Pruebas unitarias de sincronización y deduplicación en `src-tauri/src/commands/library.rs`**:
   ```bash
   cargo test -p syncify-tauri --lib local_library_tests
   ```
   *Salida*: 6 pruebas pasadas:
   - `test_ensure_local_service_registered`: OK
   - `test_sync_local_library_new_tracks_persisted_and_scored`: OK (verifica inserción completa y scoring SC-03)
   - `test_sync_local_library_deduplicates_by_isrc`: OK (verifica deduplicación e interconexión con pistas existentes de streaming)
   - `test_sync_local_library_deduplicates_by_title_artist`: OK (verifica deduplicación por nombre de artista y título)
   - `test_sync_local_library_skips_already_imported_file`: OK (verifica idempotencia de escaneo)
   - `test_local_track_ready_for_playback`: OK (verifica que la fila en `downloads` satisface el contrato de `resolve_playback_source`)

3. **Verificación de invariantes en compuertas de seguridad previas**:
   ```bash
   cargo test --test library_reconciliation_safety_gate_test
   ```
   *Salida*: 10 pruebas pasadas en 0.04s. Todas las garantías de no-mutación en dry run y protección contra relinkeo ambiguo se mantienen intactas.

4. **Verificación cruzada de compilación para Windows (Regla R4)**:
   ```bash
   cargo check --target x86_64-pc-windows-msvc -p syncify-flac-writer
   ```
   *Salida*: Finalizado sin errores en 0.43s.

5. **Pruebas unitarias de TypeScript/Vitest para la API de biblioteca**:
   ```bash
   cd ui && npx vitest run src/__tests__/api/library.spec.ts
   ```
   *Salida*: 21 pruebas pasadas (incluyendo las suites para `syncLocalLibrary` y manejo de errores).

6. **Pruebas unitarias del componente de interfaz `LocalLibrarySyncModal.vue`**:
   ```bash
   cd ui && npx vitest run src/__tests__/components/LocalLibrarySyncModal.spec.ts
   ```
   *Salida*: 4 pruebas pasadas (renderizado, visibilidad, cancelación y flujo completo de sincronización con reporte).

7. **Comprobación de tipos en frontend con `vue-tsc`**:
   ```bash
   cd ui && npx vue-tsc --noEmit
   ```
   *Salida*: Cero errores de tipado.

---

## 3. Qué queda abierto y por qué

1. **Vigilancia de cambios en tiempo real vía watcher (inotify / ReadDirectoryChangesW)**: Aunque la interfaz dispone de la firma `stopLibraryWatcher`, el demonio de observación activa continua del sistema de archivos no se activa por defecto durante el escaneo para prevenir sobrecarga de descriptores de archivo en directorios con decenas de miles de canciones.
2. **Escritura física de etiquetas en contenedores MP3/ID3v2**: La lectura de formatos diversos está soportada y la escritura nativa de VorbisComments en FLAC está plenamente integrada con `syncify-flac-writer`. Para archivos MP3, la actualización de metadatos se persiste en la base de datos SQLite y la edición binaria de tags ID3 permanece delegada a los puentes de mutagen cuando sea requerida.

---

## 4. Riesgos evaluados

1. **Riesgo de colisión en canciones con títulos idénticos de distintos artistas**:
   - *Mitigación*: La deduplicación local exige coincidencia estricta de código ISRC o coincidencia simultánea de título y artista principal (`ta.role = 'primary'`). Si dos artistas distintos tienen una canción con el mismo nombre, no se produce fusión errónea.
2. **Riesgo de degradación de rendimiento por I/O sincrónico en escaneos masivos**:
   - *Mitigación*: Las lecturas de metadatos y recorrido de carpetas operan por lotes sin mantener bloqueos prolongados sobre SQLite. Las transacciones se aplican de forma atómica y las operaciones de deduplicación reutilizan índices existentes en `tracks(isrc)` y `downloads(file_hash)`.
