# Bitácora — descargas-fallback (Carril 3-fallback)

**Fecha:** 2026-10-09  
**Carril:** `3-fallback` (`descargas-fallback`)  
**IDs asignados:** FS-05, AQ-03, AQ-04, AQ-05, AQ-06, FS-04  
**Ficheros modificados:**
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro y estado)
- `crates/syncify-core-domain/src/quality.rs`
- `src-tauri/src/download/orchestrator.rs`
- `src-tauri/src/worker.rs`
- `src-tauri/src/commands/settings.rs`
- `src-tauri/tests/provider_fallback_identity_test.rs`

---

## 1. Verificación previa y análisis de evidencias

Siguiendo las directrices del plan v2 (`docs/PLAN-Correccion-168.md`), la auditoría de incidencias (`docs/Errores-Pruebas-2026-10-08.md`) y las reglas R1–R5, se inspeccionó exhaustivamente el código:

1. **FS-05 — Política de fallback entre servicios y calidad:**
   - **Comportamiento previo auditado:**
     - En `crates/syncify-core-domain/src/quality.rs:94-104`, `qobuz_cascade(allow_lossy_fallback)` con `true` incluía el formato `"5"` (MP3 320 kbps) directamente en la búsqueda inicial dentro de Qobuz. Si Qobuz disponía de MP3 320 pero no de FLAC/CD, devolvía MP3 320 de inmediato; el orquestador (`src-tauri/src/download/orchestrator.rs:597-615`) daba la descarga por completada sin intentar jamás Tidal, a pesar de que Tidal pudiera disponer de la pista en calidad FLAC o Hi-Res.
     - Si Qobuz fallaba con `RejectedQuality` (cuando `allow_fallback` era false o `strict_quality` estaba activo), `orchestrator.rs:640-645` abortaba directamente con error sin evaluar Tidal.
     - El recorrido de fallback entre servicios para pistas con origen directo Qobuz o Tidal estaba severamente restringido; `orchestrator.rs:361, 419` resolvía siempre `target_service = "tidal"`, sin soporte de fallback bidireccional desde Tidal hacia Qobuz.
   - **Nueva política implementada:**
     - Al solicitar calidad sin pérdida (Lossless CD / Hi-Res), se prueba el servicio primario (e.g. Qobuz) exclusivamente para la calidad buscada sin permitir degradación interna a lossy.
     - Si el servicio primario no dispone de la calidad solicitada (`RejectedQuality` o `404`/`StaleSource`), se consulta el siguiente servicio (e.g. Tidal) buscando la misma calidad solicitada.
     - Solo si ningún servicio dispone de la calidad lossless solicitada y `allow_fallback == true` (con `strict_quality == false`), se desciende a 320 kbps (probando los servicios disponibles en 320 kbps).
     - **Suelo estricto:** NUNCA se desciende por debajo de 320 kbps (cualquier candidato con calidad "low", "medium", 96k, 128k, etc., es rechazado categóricamente con `RejectedQuality`).
     - Se dotó a `resolve_edition_identity_fallback` de resolución bidireccional entre Tidal y Qobuz.

2. **AQ-03 — Techo global de calidad:**
   - El techo global ya existe plenamente en backend: `commands/settings.rs:1486-1630` define `GLOBAL_MAX_QUALITY_KEY`, sus funciones canónicas `global_ceiling_rank`, `canonical_global_max_quality` y `clamp_quality_request`.
   - En `worker.rs:946-1040`, `resolve_effective_download_quality` ya aplica `ceiling_rank = service_rank.unwrap_or(3).min(global_rank)`.
   - La reubicación visual en la UI (mover el selector desde `SettingsDownloads.vue` a `SettingsQuality.vue`) está clasificada y programada para Ola 5 (`5-matriz`, IDs FS-02 y AQ-01), conforme a `docs/auditoria/matriz-ajustes.md:226`.

3. **AQ-04 — Heredar techo global o fijar calidad propia por servicio:**
   - En `commands/settings.rs:1532`, `service_max_quality_rank` solo reconocía vocabulario de calidad explícito ("high", "lossless", "hires", etc.). Si un servicio configuraba "inherit", `worker.rs:986` emitía un warning de vocabulario desconocido: `Unknown quality_preferences.max_quality vocabulary; ignoring service cap`.
   - Se añadió `is_service_quality_inherited` y soporte para "inherit", "inherit_global", "global" y "default" en `commands/settings.rs` y `worker.rs:980-994`, registrando informativamente que el servicio hereda el techo global y eliminando advertencias erróneas.

4. **AQ-05 — Opciones no disponibles no deben ser seleccionables:**
   - En la base de datos, la tabla `services` registra la capacidad física real reportada por cada servicio en `services.max_quality` (ej. Spotify y SoundCloud están limitados a 'lossy').
   - Se implementó `perform_get_available_qualities_for_service` en `commands/settings.rs:433-463` para filtrar dinámicamente las opciones válidas basadas estrictamente en `services.max_quality`, condicionando las opciones a lo que el servicio realmente reporta en lugar de listas estáticas inventadas.

5. **AQ-06 — Verificación de sobre-muestreo / recodificación artificial (Falso Positivo):**
   - En `download/audio_inspector.rs:137-153`, `enforce_post_download_quality_gate` evalúa las propiedades físicas reales del fichero en disco.
   - En toda la ruta de descarga (`tidal.rs`, `qobuz.rs`, `flac-writer`), las operaciones de remux usan `-c:a copy` (copia pura de stream sin recodificación).
   - No existe transcodificación ni inflación artificial de archivos de audio en la app. La inquietud de que se estuviera "subiendo artificialmente de MP3 a FLAC" es un **falso positivo** de auditoría manual; la aplicación preserva el contenedor y stream exactos provistos por la API o proxy.

6. **FS-04 — Verificación del techo de descarga global:**
   - Verificado en `worker.rs:946-1040` y `commands/settings.rs:1486-1630`: el techo global (`global_max_quality`) actúa como límite superior inviolable combinándose con el techo del servicio mediante `min(global_ceiling, service_cap)`.
   - Respaldado por la suite de pruebas unitarias en `commands/settings.rs:3036-3150`.

---

## 2. Ajustes realizados

### 2.1 `crates/syncify-core-domain/src/quality.rs`
- Añadido `QualityPolicy::is_lossless_or_hires(&str) -> bool` para identificar peticiones de calidad Hi-Res o Lossless CD (16/44.1).
- Añadido `QualityPolicy::is_below_minimum_floor(quality, format, bitrate) -> bool` para implementar la regla: "bajar a 320 — pero NUNCA por debajo."
- Actualizado `evaluate_stream_resolution` (`quality.rs:650-680`): ante un downgrade de calidad, si la calidad resultante es inferior al suelo de 320 kbps (ej. "low", "medium", 96k, 128k, etc.), se rechaza inmediatamente con `RejectedQuality`, incluso si `allow_fallback` está activado.
- Actualizado `is_quality_inferior` (`quality.rs:485`) para reconocer formatos de archivo "FLAC", "CD", etc., en la comparación de candidatos.
- Añadidas pruebas unitarias en `test_quality_policy_evaluate_stream_resolution_matrix` que validan el rechazo bajo el suelo de 320 kbps y la aceptación en el suelo de 320 kbps.

### 2.2 `src-tauri/src/commands/settings.rs`
- Añadida función canónica `is_service_quality_inherited(Option<&str>) -> bool`.
- Actualizado `service_max_quality_rank` para reconocer `"inherit"`, `"inherit_global"`, `"global"` y `"default"`.
- Añadido `perform_get_available_qualities_for_service` y comando IPC Tauri `get_available_qualities_for_service` para obtener las opciones válidas por servicio condicionadas a `services.max_quality`.

### 2.3 `src-tauri/src/worker.rs`
- En `resolve_effective_download_quality` (`worker.rs:980-995`), integrado `is_service_quality_inherited`: cuando un servicio está configurado para heredar, se registra con nivel `info` y se delega honestamente en `global_rank` sin emitir falsos warnings de vocabulario desconocido.

### 2.4 `src-tauri/src/download/orchestrator.rs`
- **Fallback de identidad bidireccional:** `resolve_edition_identity_fallback` (`orchestrator.rs:320-610`) determina `target_service` dinámicamente (`tidal` cuando el origen es `qobuz`, `qobuz` cuando el origen es `tidal`) parametrizando las consultas SQLite a `track_sources` y las llamadas a las APIs de Qobuz y Tidal.
- **Inversión de política en `download_track_cancellable` (`orchestrator.rs:633-850`):**
  - Si la petición es lossless/Hi-Res, se ejecuta el intento primario en Qobuz con `allow_fallback = false` para impedir que Qobuz degrade prematuramente a MP3 320.
  - Si Qobuz falla por no tener la calidad buscada (`RejectedQuality`) o por 404 (`StaleSource`), se intenta Tidal en búsqueda de la calidad solicitada.
  - Si Tidal dispone de la calidad solicitada, se completa la descarga vía Tidal preservando la procedencia original y efectiva.
  - Solo si ningún servicio dispone de la calidad solicitada y `allow_fallback == true` (sin `strict_quality`), se ejecuta el fallback al suelo de 320 kbps (primero en Qobuz si la pista existía, luego en Tidal comprobando que no descienda de 320 kbps).
  - Idéntica política simétrica aplicada cuando el servicio primario es Tidal (probando Tidal lossless -> Qobuz lossless -> 320 kbps).
  - En SongLink (`orchestrator.rs:880-980`), implementada la búsqueda en dos fases: Fase 1 (todos los candidatos buscando lossless) y Fase 2 (fallback a 320 kbps con filtro de suelo).

### 2.5 `src-tauri/tests/provider_fallback_identity_test.rs`
- Preservados los 9 tests de integración existentes.
- Añadido `test_10_cross_service_quality_first_policy_hierarchy`: valida que el fallback priorice la calidad entre servicios antes de degradar a lossy.
- Añadido `test_11_quality_floor_enforcement_never_below_320`: valida el suelo estricto de 320 kbps.
- Añadido `test_12_bidirectional_tidal_to_qobuz_fallback`: valida la resolución bidireccional de Tidal hacia Qobuz.

---

## 3. Pruebas ejecutadas y evidencias

1. **Suite de integración `provider_fallback_identity_test`:**
   ```bash
   cargo test --test provider_fallback_identity_test
   ```
   **Resultado:** 12 passed; 0 failed; 0 warnings.
2. **Suite de paridad CLI vs Tauri:**
   ```bash
   cargo test --test cli_tauri_behavioral_parity_test
   ```
   **Resultado:** 2 passed; 0 failed.
3. **Suite de persistencia de fallback de calidad:**
   ```bash
   cargo test --test quality_fallback_persistence_test
   ```
   **Resultado:** 1 passed; 0 failed.
4. **Dominio de calidad (`syncify-core-domain`):**
   ```bash
   cargo test -p syncify-core-domain
   ```
   **Resultado:** 66 passed; 0 failed.
5. **Ajustes de descargas y reconciliación:**
   ```bash
   cargo test --test download_settings_canonical_test
   cargo test --test hires_quality_reconciliation_test
   cargo test --test download_settings_commands_test
   cargo test --test settings_runtime_validation_s148a_test
   ```
   **Resultado:** Todos pasados limpiamente.
6. **Verificación de destino Windows (R4):**
   ```bash
   cargo check --target x86_64-pc-windows-msvc -p syncify-core-domain
   ```
   **Resultado:** Compilación limpia sin errores.

---

## 4. Qué queda abierto

1. **Reubicación visual en la UI de Ajustes (AQ-03 / FS-02):**
   El control del selector del techo global (`global_max_quality`) se encuentra actualmente en la vista de carpetas/descargas (`SettingsDownloads.vue:209-260`) en lugar de junto a los techos por servicio en `SettingsQuality.vue`. Su reubicación física en el frontend Vue 3 está asignada a la Ola 5 (`5-matriz`, IDs FS-02 / AQ-01), conforme a las reglas de partición de ficheros del plan v2.
