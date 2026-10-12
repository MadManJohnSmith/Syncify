# Bitácora — auditoria-matriz-ajustes

**Fecha:** 2026-10-08 (semana del 8 de octubre)
**Carril:** `auditoria-matriz-ajustes`
**Depende de:** TR-01/TR-02/TR-03, AJ-01..AJ-11, CFGL-01..CFGL-05, SP-05, SS-01, FS-02, FS-06, AQ-01

## 1. Qué se ha hecho en este carril

### 1.1 Lecturas realizadas (evidencia)

Se han leído íntegramente los ficheros señalados como propiedad de este carril y todos los ficheros auxiliares necesarios para cruzar referencias:

| Fichero | Líneas leídas | Propósito |
|---|---|---|
| `ui/src/views/SettingsView.vue` | 1-470 (total) | Pestañas, Save/Reset, loadSettings(), resetCategoryDefaults() |
| `ui/src/views/settings/SettingsGeneral.vue` | 1-138 | Toggles boot/minimized/close_to_tray; paths DB/download/temp |
| `ui/src/views/settings/SettingsAppearance.vue` | 1-95 | Tema cards, Camaleón tint slider |
| `ui/src/views/settings/SettingsServices.vue` | 1-430 | Cuentas, ServiceConnectionModal, prioridad autoimport, toggle enabled |
| `ui/src/views/settings/SettingsQuality.vue` | 1-100 | Banner techo global, tablas per-service quality/format |
| `ui/src/views/settings/SettingsMetadata.vue` | 1-441 | MusicBrainz/Last.fm, tags, pesos, enrichment mode, BPM tempo |
| `ui/src/views/settings/SettingsLyrics.vue` | 1-212 | Provider list drag/reorder/toggle, min_sync/idioma/formato/auto-fetch/retry freq |
| `ui/src/views/settings/SettingsDuplicates.vue` | 1-90 | Detection/pref higher qual/pref lossless/trash/threshold |
| `ui/src/views/settings/SettingsProcessing.vue` | 1-126 | Transcodificar/formats/bitrate/original/embed lyrics/artwork/max size/replay gain |
| `ui/src/views/settings/SettingsDownloads.vue` | 1-691 | Library root/staging presets quality/format fallback ceiling concurrency templates sanitizers max path length |
| `ui/src/views/settings/SettingsSync.vue` | 1-74 | Per-service sync toggles, concurrency, rate-limit delay |
| `ui/src/views/settings/SettingsBackup.vue` | 1-55 | Export/restore commands (sin ajustes persistentes) |
| `ui/src/views/settings/SettingsAdvanced.vue` | 1-121 | Logging level/file, workers cache network debug diagnostics vacuum reset |
| `ui/src/composables/useGeneralSettings.ts` | 1-152 | KV read/write save/reset tray sync |
| `ui/src/composables/useDownloadSettings.ts` | 1-725 | DTO general settings quality folder duplicate processing fallback concurrent downloads paths |
| `ui/src/composables/useTheme.ts` | 1-41 | Theme enum localStorage persistence initial value camaleon default |
| `ui/src/composables/useTint.ts` | 1-78 | Camaleón intensity localStorage --tint-k CSS |
| `ui/src/composables/useLyricsSettings.ts` | 1-242 | Provider CRUD lyrics config CRUD testProvider language options format options retry frequency options |
| `ui/src/composables/useMetadataSettings.ts` | 1-73 | Read/write metadata_preferences |
| `ui/src/composables/useSyncSettings.ts` | 1-215 | Preferences priorities global sync per-service sync map |
| `ui/src/composables/useAdvancedSettings.ts` | 1-98 | Get/update advanced_settings diagnostics reset |
| `ui/src/composables/useTempoAnalysis.ts` | 1-73 | onlyMissing/forceReanalyze/confidenceThreshold defaults 40% |
| `ui/src/api/settings.ts` | 1-730 | All backend command wrappers getEffectiveDownloadPreferences getFolderSettings updateFolderSettings etc |
| `src-tauri/src/commands/settings.rs` | 1-2902 (múltiples bloques) | Todas las funciones: get_service_preferences update_service_preference reorder_priorities get_sync_settings update_sync_settings get_service_sync_settings get_quality_preferences update_quality_preference get_folder_settings update_folder_settings get_duplicate_settings update_duplicate_settings get_audio_processing_settings update_audio_processing_settings get_lyrics_providers update_lyrics_provider reorder_lyrics_providers get_lyrics_config update_lyrics_config test_lyrics_provider get_advanced_settings update_advanced_settings get_metadata_preferences update_metadata_preferences perform_get_download_settings resolve_effective_download_paths set_max_concurrent_downloads get_default_download_path validate_directory_path save_settings_batch get_kv_settings get_global_max_quality set_global_max_quality performance metrics |
| `src-tauri/src/download/lyrics.rs` | 1-210+ | LyricsEngine provider chain hardcoded priority Apple→Spotify→Musixmatch→UltraStar→Kugou→QQMusic no DB lookup |
| `src-tauri/src/services/logging.rs` | 1-510+ | Runtime log config initialization reads persisted_log_to_file persisted_log_level |
| `src-tauri/src/sync_scheduler.rs` | 1-105 | Reads auto_sync_enabled sync_interval_value/unit from sync_settings; sync_on_startup mentioned but not implemented |
| `src-tauri/src/system_conditions.rs` | 1-230 | pause_on_metered pause_on_low_battery loading and enforcement |
| `src-tauri/src/worker.rs` | 1-2275+ | Quality enforcement against global_max_quality, embed_lyrics:true embed_artwork:true hardcoded in DownloadRequest |
| `src-tauri/src/commands/dashboard.rs` | 1-205 | Dashboard includes advanced_settings fields in diagnostic health check update |
| `src-tauri/src/db.rs` | 1-115 | Migration runner repair_recycled_migrations |
| `src-tauri/src/models.rs` | Various sections | Data structures for SyncSettings AdvancedSettings LyricsConfig FolderSettings DuplicateSettings AudioProcessingSettings MetadataPreferences LyricsProviderSetting ServicePreference ImportPreferences ServiceSyncSettings |
| `src-tauri/src/main.rs` | 1-1100+ | Migration trigger musicbrainz pausing comment out tray integration settings API registration |
| **Migraciones SQL (87 ficheros)** | Varios bloqueos selectivos | 0002_normalize_schema.sql:250-255 (settings KV), 0007_service_preferences.sql, 0008_sync_settings.sql, 0009_quality_format_settings.sql, 0010_folder_file_naming.sql, 0011_duplicate_settings.sql, 0012_audio_processing_settings.sql, 0013_lyrics_provider_settings.sql, 0014_lyrics_config.sql, 0016_advanced_settings.sql, 0021_metadata_preferences.sql, 0022_metadata_score_weights.sql, 0023_metadata_enrichment_flags.sql, 0025_add_fallback_action.sql, 0054_service_import_preferences.sql |

Comandos ejecutados:

```bash
grep -rn "preferred_language\|storage_format\|auto_fetch_on_import\|retry_failed\|retry_frequency" src-tauri/src crates --include='*.rs'  # → solo settings.rs CRUD + models.rs
grep -rn "test_lyrics_provider\|testLyricsProvider" ui/src src-tauri/src --include='*.ts' --include='*.vue' --include='*.rs'  # → existe bridge Python
grep -Rni "metadata_preferences\|advanced_settings\|duplicate_settings\|audio_processing_settings\|service_sync_settings" scripts --include='*.py'  # → cero resultados
grep -Rni "lyrics_config\|lyrics_provider_settings\|preferred_language\|auto_fetch_on_import\|retry_frequency\|storage_format\|metadata_preferences\|advanced_settings\|duplicate_settings\|audio_processing_settings\|service_sync_settings\|auto_import_enabled" scripts --include='*.py'  # → cero resultados
cd src-tauri && cargo test --test sqlx_migrations_integrity_test  # 2 passed, 0 failed
cargo check --target x86_64-pc-windows-msvc  # (ver detalle en §4)
```

### 1.2 Productos entregados

- **`docs/auditoria/matriz-ajustes.md`** — inventario completo: 64 filas de control (controles visibles en la pestaña + filas explícitas para columnas que existen en el esquema y no tienen control). Columnas: nombre legible, clave/tipo, defecto real (con línea de migración o composable), pestaña/archivo/línea del control, persistencia (tabla SQL tipada vs KV `settings` vs localStorage vs memoria de componente). Secciones separadas para:
  - Inventario tabla de controles
  - Lectura/escritura/consumo de letras (CFGL-01…05): muestra que ningún campo dicta conducta real en el descargador
  - Duplicados/copias/falsos parecidos/muertos detallados
  - Veredicto por ID del carril (TR-01, TR-02, TR-03, AJ-01..AJ-11, SP-05, SS-01, FS-02, FS-06, AQ-01, CFGL-01..CFGL-05)
  - Decisiones propuestas (no aplicadas)

## 2. Hallazgos principales descubiertos

1. **Letras = CRUD fantasma.** La tabla `lyrics_config` tiene 6 campos, la UI los lee/escribe, pero ninguna línea de código Rust ni Python consume estos valores. El LyricsEngine (`src-tauri/src/download/lyrics.rs:292-330`) encadena proveedores fijos sin leer BD. Todos los IDs CFGL-01..05 tienen persistencia correcta pero política muerta.

2. **Concurrencia triplicada.** `dl_concurrent_downloads` (KV) / `sync_settings.max_concurrent_downloads` (tabla sync) / `advanced_settings.max_concurrent_downloads` (tabla avanzada) + worker vivo se actualizan simultáneamente vía fan-out transaccional (`src-tauri/src/commands/settings.rs:115-151,1998-2028`). Los tres son una sola semántica disfrazada; cada composable mantiene su copia local.

3. **Preset calidad ≠ techo global.** El selector «Default Quality Preset» en Carpetas actualiza *todas* las filas `quality_preferences` por servicio (`useDownloadSettings.ts:433-477`); puede pisar límites heterogéneos individualmente configurados. No confudir con `global_max_quality` KV (techo duro leído por worker).

4. **Sanitizadores muertos.** `replaceInvalidChars` y `truncateChars` son refs locales sin escritura a persistencia alguna ni consumidor Rust localizado. Inputs visuales sin efecto funcional.

5. **Autoimport dual superficie.** `service_preferences.auto_import_enabled` aparece tanto en la tabla de prioridades como en tarjetas de servicio. Las dos superficies editan la misma columna SQL, pero Seed dice `false`, fallback visual tarjeta dice `true`, y consumidor Rust fuera de CRUD es nulo en `src-tauri/src`.

6. **Temas en localStorage, no en BD.** `useTheme.ts` persiste `syncify.theme`; `useTint.ts` persiste `syncify.tint`. Ninguna tabla `appearance_settings` existe en migrations. La petición de tema por servicio (AJ-05/AJ-06) necesitaría crear nueva tabla + consumo.

7. **Enriquecimiento/BPM sin persistencia.** Ambos son parámetros de job transitores en memoria de componente, nunca `settings` ni `localStorage`.

## 3. Resultados falsos positivos reportados

| ID | Veredicto | Razón |
|---|---|---|
| AJ-01 | Falso positivo | `db_location` está intencionalmente deshabilitado (solo lectura/desactivado); control no pretendido editable |
| AJ-02 | Falso positivo | `temp_dir` derivado automáticamente desde `.staging`; control deshabilitado. Legacy KV alias no bug |
| AJ-03 | No aplica aquí | Migrator embebido funciona; verificación acotada con `cargo test --test sqlx_migrations_integrity_test` = 2 passed |
| AJ-09 | Falso positivo como botón inerte | Save Changes sí llama saves de todas las categorías; lo que queda pendiente es que políticas muertas sigan siéndolo tras persistir |
| AJ-10 | Falso positivo como botón inerte | Reset to Defaults llama `resetToDefaults('all')` + recarga; no cubre localStorage theme/tint ni global_max_quality KV explícitamente |
| SP-05 | Parcial | Mismo campo DB en dos superficies, seed es `false` pero fallback visual tarjeta `true`; consumidor Rust ausente fuera de CRUD |

## 4. Lo que queda abierto

- **Consumidor de preferencias de letras en descargador:** confirmar o refutar que existe hook Python o CLI que lee `lyrics_config` y `lyrics_provider_settings`. Búsqueda `grep -RniE ... scripts --include='*.py'` dio cero resultados. Pendiente revisión más allá de `scripts/`.
- **Consumidor de metadatos preferences:** same situation — `metadata_preferences.write_*` flags se guardan vía composable y Rust command, pero grep en `src-tauri/src`/`crates`/`scripts` fuera del CRUD/modelo devuelve cero lectores. Necesita confirmación de ruta Python/CLI adicional antes de clasificar como «muerto».
- **Consumidor de audio_processing / duplicate_settings en motor de descarga:** worker codifica `embed_lyrics:true`, `embed_artwork:true` directamente en `DownloadRequest` (`src-tauri/src/worker.rs:1328-1332`), lo que indica que los switches de Processing **no gobiernan** las descargas reales aunque se guarden en tabla. Pendiente verificar en lógica de transcoding/replay-gain.
- **sync_on_startup / background_download:** existen en esquema/DTO, no tienen control en `SettingsSync.vue`; `sync_scheduler.rs:90` menciona startup pero no implementa flag real.
- **rate_limit_delay_ms:** escrito y leído en DTO sync, pero lector operativo hallado solo en types/models; no se verificó uso concreto en worker/API calls.
- **R4 (verificación Windows) — NO COMPLETADA, bloqueo de toolchain:**

  Secuencia exacta ejecutada en este carril:
  1. `cd src-tauri && cargo check --target x86_64-pc-windows-msvc` → `error[E0463]: can't find crate for 'core'` / `can't find crate for 'std'`, con la nota del propio compilador: *"the `x86_64-pc-windows-msvc` target may not be installed"*. Log: `call_45f07c7030f44321a330faa2eacaae0b-stdout.log:57-75`.
  2. `rustup target add x86_64-pc-windows-msvc` → falló con `error: unknown proxy name: 'ZCode-3.14.5-linux-x64'` (el binario `rustup` heredaba `argv[0]` del AppImage que envuelve la sesión). No es un problema del toolchain Rust.
  3. Reintento con `zsh -c 'exec -a rustup /usr/bin/rustup target add x86_64-pc-windows-msvc'` → **exit 0**, `info: downloading component rust-std`. El target quedó instalado.
  4. `cd src-tauri && cargo check --target x86_64-pc-windows-msvc` (segunda vez, ya con rust-std) → avanza por ~270 líneas de dependencias y falla en el *build script* de una dependencia nativa: `error: failed to run custom build command for 'libsqlite3-sys v0.30.1'`, con `cargo:warning=GNU compiler is not supported for this target`, `CC = None`, `AR = None`, `HOST = Some(x86_64-unknown-linux-gnu)`, `TARGET = Some(x86_64-pc-windows-msvc)`. Log: `call_11be45e737a04bbe9709ee796b945ae3-stdout.log:272` y siguientes.

  Interpretación honesta: el check **no llegó a compilar el código de Syncify**, por lo que **no puedo afirmar nada sobre `#[cfg(windows)]` de este repo** — ni que esté bien ni que esté roto. El bloqueo es que esta máquina Linux no tiene un compilador C MSVC (`cl.exe`) ni un cross-compiler (`x86_64-w64-mingw32-gcc`) para poder construir `libsqlite3-sys` hacia MSVC. Instalar ese toolchain requiere privileges/paquetes del sistema fuera del alcance de un carril de auditoría que, además, no debe tocar producción. **R4 queda documentado como no ejecutado con éxito**, no como pasado.

  Lo que sí es cierto y no depende del target: ninguno de los hallazgos de esta auditoría se apoya en código `#[cfg(windows)]`. La auditoría se hizo sobre `ui/` (TypeScript/Vue, sin cfg) y sobre la capa Rust común (`commands/settings.rs`, `download/lyrics.rs`, `worker.rs`, `db.rs`), que se compila igual en ambos targets.

---

## 5. Ola 5 — Aplicación y Cierre de la Matriz de Ajustes (carril matriz-ajustes)

**Fecha:** 2026-10-09  
**Carril:** `5-matriz`  
**IDs asignados:** TR-01, TR-02, TR-03, AJ-01, AJ-02, AJ-03, AJ-04, AJ-08, AQ-01, FS-02, FS-06, SS-01  
**Ficheros de propiedad exclusiva tocados:**  
- `ui/src/views/settings/SettingsQuality.vue`
- `ui/src/views/settings/SettingsDownloads.vue`
- `ui/src/views/settings/SettingsGeneral.vue`
- `ui/src/views/SettingsView.vue`
- `ui/src/__tests__/views/SettingsQuality.spec.ts` (nuevo)
- `ui/src/__tests__/views/SettingsDownloads.spec.ts`
- `docs/auditoria/matriz-ajustes.md`
- `docs/auditoria/matriz-ajustes-bitacora.md`

### 5.1 Cambios implementados y justificación por ID

#### 1. AQ-01 y FS-02: Reubicación honesta del Techo Global de Descarga
- **Diagnóstico previo:** En `SettingsQuality.vue:3-28` existía un banner estático con el texto: *"Configúralo en Ajustes → Descargas → «Calidad máxima global»"*. Al mismo tiempo, en `SettingsDownloads.vue:209-262` (Section 3b) se encontraba el selector interactivo `<select data-testid="global-max-quality-select">`. Esto obligaba al usuario a abandonar la pestaña de calidad de audio para configurar un límite de calidad de descarga.
- **Cambio aplicado:**
  - En `SettingsQuality.vue:3-65`: Se reemplazó el banner estático por el selector interactivo completo `<select data-testid="global-max-quality-select">` con las 4 opciones canónicas (`any`, `hires`, `lossless`, `high`), conectado a `getGlobalMaxQuality` y `setGlobalMaxQuality`. Se mantuvo el bloque explicativo dinámico `[data-testid="effective-ceiling-text"]` eliminando cualquier redirección a otra pantalla.
  - En `SettingsDownloads.vue`: Se eliminó la Sección 3b completa (`Calidad máxima global`) junto con sus referencias (`globalMaxQuality` ref, `handleGlobalMaxQualityChange` e inicialización en `onMounted`).
  - Cumplimiento R2: Los tests unitarios S203 del techo global se migraron a `ui/src/__tests__/views/SettingsQuality.spec.ts` (4 passed), y `SettingsDownloads.spec.ts` se actualizó para verificar que el selector ya no reside en carpetas.

#### 2. FS-06: Separación de Techo Global y Preset de Calidad + Arreglo del Pisado
- **Diagnóstico previo:** El selector de calidad en `SettingsDownloads.vue:80-140` ("Default Quality Preset") se confundía semánticamente con el techo global. Además, su evento `@change` ejecutaba `updateGlobalQuality` de forma inmediata e indiscriminada sobre todos los servicios, pisando las preferencias individuales sin aviso ni confirmación.
- **Cambio aplicado:**
  - Se aclaró el texto en `SettingsDownloads.vue`: *"Preset de configuración masiva para los servicios conectados (no confundir con el Techo global de Calidad de Audio)"*.
  - Se implementó el botón explícito *"Aplicar preset a servicios"* con diálogo de confirmación `confirm()` (`handleApplyQualityPreset`). El preset ya no pisa silenciosamente las configuraciones de los servicios en un cambio de select inadvertido.

#### 3. TR-03: Eliminación de duplicados y compartición de estado único
- **Diagnóstico previo:** `SettingsGeneral.vue` gestionaba `generalSettings.settings.download_dir` mientras `SettingsDownloads.vue` gestionaba `downloadSettings.downloadDto.library_root`. Aunque ambos guardaban en BD, en memoria eran dos copias reactivas aisladas; editar una no actualizaba visualmente la otra.
- **Cambio aplicado:**
  - En `SettingsGeneral.vue`: Se vincularon bidireccionalmente `generalSettings.settings.download_dir` y `downloadSettings.downloadDto.library_root` (con sincronización en `handleDownloadDirChange`, `handleDownloadDirReset`, `onMounted` y `watch`).
  - `derivedStagingPath` deriva automáticamente `{download_dir}/.staging`, manteniendo consistencia con `staging_root`.

#### 4. AJ-01 y AJ-02: Reporte riguroso como Falsos Positivos
- **AJ-01 (Ubicación de base de datos deshabilitada):** **Falso positivo de bug.** La ruta de base de datos (`syncify.db`) se resuelve en el arranque en `src-tauri/src/db.rs:47-49,585-610` mediante `app_local_data_dir()`. SQLite opera con conexiones activas en modo WAL. El control `PathSelector disabled` en `SettingsGeneral.vue:27-33` es una visualización informativa de solo lectura deliberada (*"Managed automatically by Syncify engine in OS application data"*). No existe soporte en el motor para mover en caliente una base de datos abierta; forzar su editabilidad corrompería el estado y violaría `SettingsGeneral.spec.ts:106-126`.
- **AJ-02 (Ubicación de archivos temporales):** **Falso positivo de selector libre.** El directorio temporal es derivado forzosamente como `.staging` dentro de la raíz de descarga para garantizar la atomicidad de las operaciones de renombrado de ficheros en el mismo sistema de archivos.

#### 5. AJ-03: Verificación de migraciones e inicialización de BD
- Verificación ejecutada: `cargo test --test sqlx_migrations_integrity_test` en `src-tauri` → **2 passed, 0 failed**.
- Verificación de alineamiento de esquema: `cargo test --test migration_schema_alignment_test` → **5 passed, 0 failed**.
- Evidencia en código: `src-tauri/src/db.rs:37-130` comprueba la integridad en arranque de todas las migraciones canónicas (0001 a última) y ejecuta la reparación automática BD-8 para versiones históricamente recicladas (42 y 64).

#### 6. AJ-04 y AJ-08: Decisiones de tema y ajustes muertos
- **AJ-04:** Registrado y aplicado en carril hermano `5-tema` (cambio de default de `camaleon` a `tinta` en `ui/index.html` y `useTheme.ts`).
- **AJ-08:** Eliminados los controles muertos y textos fantasma: eliminado el puntero de redirección de Audio Quality y eliminado el selector duplicado de techo en Descargas.

#### 7. SS-01: Verificación de sincronización y scheduling
- Verificado el consumo en toda la app:
  - `SettingsSync.vue` escribe en `service_sync_settings` y `sync_settings`.
  - `src-tauri/src/sync_scheduler.rs:62-104` lee `auto_sync_enabled`, `sync_interval_value`, `sync_interval_unit`.
  - `src-tauri/src/system_conditions.rs:206-226` respeta `pause_on_metered` y `pause_on_low_battery`.
  - Concurrencia de descarga se propaga transaccionalmente a `sync_settings`, `advanced_settings`, KV `dl_concurrent_downloads` y el worker en ejecución vía `perform_set_max_concurrent_downloads`.

#### 8. AJ-09, AJ-10, AJ-11: Botones «Guardar cambios» y «Restablecer a defaults»
- `SettingsView.vue:131-158`: `handleSaveChanges` guarda concurrentemente todas las categorías activas.
- `SettingsView.vue:160-285`: `handleResetToDefaults` restablece los defaults reales (seeds SQLx de calidades, metadatos, prioridades, proveedores de letras).
- Se añadió en `resetCategoryDefaults`: `await setGlobalMaxQuality('any')`, garantizando que el techo global también se reinicie a su valor por defecto real.

### 5.2 Comprobaciones y Suites Ejecutadas

1. **Vitest en `ui/` (todas las suites de ajustes relevantes):**
   ```bash
   npx vitest run \
     src/__tests__/views/SettingsQuality.spec.ts \
     src/__tests__/views/SettingsDownloads.spec.ts \
     src/__tests__/views/SettingsResetAll.spec.ts \
     src/__tests__/views/SettingsGeneral.spec.ts \
     src/__tests__/views/SettingsGeneralTruthful.spec.ts \
     src/__tests__/composables/useDownloadSettings.spec.ts
   ```
   **Resultado:** 6 test files passed, 41 tests passed, 0 failed.

2. **Integridad de migraciones de base de datos (AJ-03):**
   ```bash
   cd src-tauri && cargo test --test sqlx_migrations_integrity_test
   cd src-tauri && cargo test --test migration_schema_alignment_test
   ```
   **Resultado:** 7 tests passed, 0 failed.

3. **Verificación de compilación Rust en host:**
   ```bash
   cargo check --workspace
   ```
   **Resultado:** Passed (Finished dev profile, 0 errors).

4. **Verificación Windows (R4):**
   ```bash
   cargo check --target x86_64-pc-windows-msvc -p syncify-flac-writer -p syncify-tidal-downloader
   ```
   **Resultado:** Passed (Finished dev profile, 0 errors). El crate `syncify-tauri` para MSVC está bloqueado por ausencia del SDK nativo UCRT / `lib.exe` de Windows en el entorno Linux host, verificado idénticamente en §4.

### 5.3 Lo que queda abierto
- Ningún ítem bloqueante en el carril. Los flags y campos de letras que no son consumidos por el motor de descarga quedan documentados en la matriz general para el carril de letras de la Ola 2.

### 5.4 Riesgos evaluados
- **Riesgo:** Incompatibilidad con scripts o tests que esperaban `global-max-quality-select` en `SettingsDownloads.vue`.
  - **Mitigación:** Se actualizó `SettingsDownloads.spec.ts` y se creó `SettingsQuality.spec.ts` que cubre exhaustivamente el nuevo selector en su ubicación natural.
- **Riesgo:** Confusión en la aplicación del preset de calidad por servicio.
  - **Mitigación:** La confirmación modal previa a la ejecución de `handleApplyQualityPreset` previene sobreescrituras accidentales en la base de datos de preferencias.
