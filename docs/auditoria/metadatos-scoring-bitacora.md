# Bitácora — metadatos-scoring (Ola 4)

**Fecha:** 2026-10-09  
**Carril:** `4-scoring` (`metadatos-scoring`)  
**IDs asignados:** MT-01, SC-01, SC-02, SC-03, SC-04, MD-01, MD-02, MD-03, AQ-02, MT-03, AP-01, AP-02, AP-03, AP-04  
**Ficheros modificados:**
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro y actualización de filas)
- `ui/src/views/settings/SettingsMetadata.vue`
- `ui/src/views/settings/SettingsProcessing.vue`
- `crates/syncify-metadata-domain/src/scoring.rs` (nuevo módulo de dominio puro)
- `crates/syncify-metadata-domain/src/lib.rs`
- `src-tauri/src/enrichment_worker.rs`
- `ui/src/components/settings/BaseToggle.vue`
- `ui/src/views/settings/SliderInput.vue`
- `ui/src/composables/useMetadataSettings.ts`

---

## 1. Verificación previa y análisis de evidencias por ID

### MT-01 — Verificación individual de ajustes de Metadata y Tags
- **Evidencia en código:**
  - `src-tauri/src/enrichment_worker.rs:252-270`: el worker en segundo plano consultaba directamente MusicBrainz sin verificar el ajuste `metadata_preferences.enable_musicbrainz`.
  - En `crates/syncify-flac-writer/src/lib.rs:430-845`, la función `apply_flac_tags` procesa las etiquetas Vorbis canónicas (`RELEASETYPE`, `LABEL`, `RECORDLABEL`, `ORGANIZATION`, `WORK`, `COMPOSER`, `MUSICBRAINZ_TRACKID`, `MUSICBRAINZ_RELEASEID`, `DOWNLOAD_SOURCE`, `DOWNLOAD_DATE`, `EXPLICIT`, `BPM`, `TEMPO`, `REPLAYGAIN_*`, `LYRICS`, `UNSYNCEDLYRICS`).
- **Corrección realizada:**
  - En `src-tauri/src/enrichment_worker.rs:253-288`: se conectó la lectura de `enable_musicbrainz` desde `metadata_preferences`. Si el usuario desactiva la opción, el worker marca la tarea como `skipped` ("MusicBrainz enrichment disabled in settings") y omite peticiones de red al API de MusicBrainz.
  - En `SettingsMetadata.vue`: se añadieron notas explicativas y tooltips clarificadores a cada control para que el usuario conozca con precisión qué etiquetas se escriben físicamente y cuáles operan a nivel de base de datos/biblioteca.

### SC-01 — Umbrales/pesos de scoring se mueven solos o pierden cambios al actualizar
- **Evidencia en código:**
  - En `ui/src/views/settings/SettingsMetadata.vue:130-165` y `ui/src/composables/useMetadataSettings.ts:57-73`, cada evento `@update:modelValue` de los sliders invocaba de manera inmediata e incondicional `saveSettings()`.
  - Al arrastrar el ratón o introducir cambios rápidos, se disparaban múltiples llamadas concurrentes `update_metadata_preferences` contra Tauri/SQLite. Las respuestas desordenadas provocaban sobrescritura de valores intermedios y recargas que reseteaban la posición de los sliders.
- **Corrección realizada:**
  - En `useMetadataSettings.ts:71-92`: se implementó `saveDebounced(delayMs = 300)` junto al flag reactivo `isSaving`.
  - Los sliders en `SettingsMetadata.vue` ahora utilizan `@update:modelValue="metadataSettings.saveDebounced()"`, consolidando las actualizaciones en una única llamada persistida y mostrando el indicador visual "Guardando..." / "Cambios guardados".

### SC-02 — Conexión real de los umbrales al cálculo de calificación de metadatos
- **Evidencia en código:**
  - `crates/syncify-metadata-domain` no contenía un motor unificado de cálculo de scoring.
  - En `src-tauri/src/commands/metadata.rs:360-368` existían fórmulas SQL estáticas con pesos fijos (10, 20) que no tenían en cuenta las preferencias configuradas por el usuario en `metadata_preferences`.
- **Corrección realizada:**
  - En `crates/syncify-metadata-domain/src/scoring.rs`: se creó el módulo de dominio puro `scoring` con las estructuras `ScoringWeights`, `TrackMetadataScoreInput` y la función determinista `calculate_metadata_score(weights, track)`.
  - Regla de ponderación:
    - Base fija indispensable: Título (10%) y Artista (10%) suman hasta 20%.
    - Pool configurable (80% restante): Distribuido proporcionalmente según los pesos `weight_album`, `weight_isrc`, `weight_mb_id`, `weight_cover`, `weight_year`, `weight_genre`. Si no hay pesos configurados, la puntuación se escala en base a la presencia de título y artista.
  - En `SettingsMetadata.vue`: se añadió un simulador reactivo en vivo (`simulatedScore`) que recalcula al instante la calificación estimada en tiempo real según el movimiento de los sliders.

### SC-03 — Paridad de scoring entre biblioteca local y pistas de streaming
- **Evidencia en código y verificación:**
  - `TrackMetadataScoreInput` admite campos opcionales `file_path: Option<String>` y `is_local: bool` para pistas escaneadas localmente, sin exigir identificadores de servicios de streaming (`spotify_id`, `tidal_id`, etc.).
  - Se añadió la prueba unitaria `test_local_track_parity_with_downloaded_track_sc_03` en `crates/syncify-metadata-domain/src/scoring.rs:222-261`, verificando que una pista local y una descargada con idénticos metadatos reciben exactamente la misma puntuación de calidad (0-100%).

### SC-04 y AP-04 — Botón '?' con tooltip en cada ajuste individual
- **Evidencia y diseño accesible:**
  - Los componentes `ui/src/components/settings/BaseToggle.vue` y `ui/src/views/settings/SliderInput.vue` se enriquecieron con la prop opcional `tooltip`.
  - Se renderiza un badge circular `?` con hover accesible y `role="tooltip"`, mostrando la explicación contextual de cada opción tanto en `SettingsMetadata.vue` como en `SettingsProcessing.vue`.

### MD-01 / MD-02 / MD-03 — Explicación de botones de metadatos, Merge con estadísticas e Historial inverso
- **Evidencia en código:**
  - En `SettingsMetadata.vue:200-480`, los procesos de enriquecimiento y reparación disponen de:
    - "Start Incremental Enrichment" ("Merge con estadísticas"): ejecuta `startLibraryEnrichment` / `previewLibraryEnrichment` y muestra estadísticas reales y al día en `EnrichmentJobSummary` (`totalEligible`, `totalComplete`, `totalSkippedPrecedence`, `modifiedTracks`).
    - "Review Repair Plan" ("Review title / Prepared dry run"): abre `TidalRepairReviewModal.vue` (`get_tidal_repair_dry_run`), ejecutando una simulación segura no mutante.
    - "Review Renames": abre `DisambiguationRepairReviewModal.vue` (`plan_disambiguation_repair`), preparando renombres con verificación SHA-256 y rollback automático.
    - "Repair History" ("Applied / reverse history"): abre `RepairHistoryModal.vue` (`get_repair_history`), que consume `fetch_repair_history` en `src-tauri/src/services/repair_history.rs:102-130`. La consulta SQL ejecuta expresamente `ORDER BY timestamp DESC, id DESC`, entregando el registro de auditoría en orden cronológico inverso (el más reciente primero).
- **Corrección realizada:**
  - Se añadieron etiquetas explicativas, badges descriptivos y textos informativos en `SettingsMetadata.vue` aclarando el funcionamiento y garantías de seguridad de cada acción.

### AQ-02 — Reestructuración balanceada de Ajustes
- Se organizaron las vistas `SettingsMetadata.vue` y `SettingsProcessing.vue` con secciones temáticas limpias (Fuentes, Comportamiento de Etiquetado, Ponderación de Calidad, Reparación Segura, Transcodificación, Incrustación y ReplayGain). No se eliminó ninguna opción funcional ni se introdujeron opciones redundantes o decorativas.

### MT-03 — Contraste de toggles activos en el tema Tinta
- **Evidencia:**
  - En `ui/src/styles/main.css:58, 154`, el tema `tinta` define `--accent: #f4f4f5` (casi blanco) y `--color-primary: var(--accent)`.
  - En `BaseToggle.vue:29`, el knob activo tenía `bg-white`. Al activarse, la pista se pintaba casi blanca (`bg-primary`) y el knob blanco (`bg-white`), desapareciendo visualmente por falta de contraste.
- **Corrección realizada:**
  - En `BaseToggle.vue:29`: se modificó el knob para usar `bg-on-accent` en estado activo (`checked ? 'right-0 bg-on-accent border-primary' : 'left-0 bg-white border-gray-300 dark:border-gray-700'`).
  - `--color-on-accent` en el tema tinta vale `#0a0a0b` (`main.css:61, 147`), proporcionando contraste óptimo (negro profundo sobre fondo blanco brillante).

### AP-01, AP-02, AP-03, AP-04 — Audio Processing, incrustación de letras, carátula y ReplayGain
- **AP-01:** Audio Processing desactivado por defecto: se añadió un banner explicativo en `SettingsProcessing.vue` indicando que Syncify prioriza descargas nativas bit-perfect directas de las plataformas, recomendando activar el procesamiento solo para equipos o reproductores antiguos que requieran MP3/AAC.
- **AP-02:** Incrustación de letras: en `crates/syncify-flac-writer/src/lib.rs:780-790`, el escritor nativo de FLAC incrusta automáticamente `LYRICS` (Enhanced LRC) y `UNSYNCEDLYRICS` (texto plano limpio). El ajuste `embed_lyrics` en `SettingsProcessing.vue` controla el comportamiento durante procesos de transcodificación y tareas por lotes (`batch_embed_lyrics`). Se aclaró en la interfaz con un tooltip explícito.
- **AP-03:** Tamaño máximo de carátula y ReplayGain:
  - En `syncify-flac-writer/src/lib.rs:845-875`, la carátula se optimiza y valida con un techo físico de 800 KB (`MAX_EMBEDDED_PICTURE_BYTES`) para máxima compatibilidad con reproductores móviles como Symfonium.
  - Las etiquetas de ReplayGain (`REPLAYGAIN_TRACK_GAIN`, `REPLAYGAIN_TRACK_PEAK`, etc.) se escriben en bloques Vorbis en `syncify-flac-writer/src/lib.rs:706-735`.
  - Todo ello se documentó con tooltips detallados en `SettingsProcessing.vue`.

---

## 2. Pruebas ejecutadas y comprobaciones

1. **Pruebas de dominio Rust:**
   ```bash
   cargo test -p syncify-metadata-domain
   ```
   **Resultado:** 45 pruebas pasadas (incluyendo las nuevas de scoring y paridad local).

2. **Verificación de compilación cruzada Windows (R4):**
   ```bash
   cargo check --target x86_64-pc-windows-msvc -p syncify-metadata-domain
   ```
   **Resultado:** Compilación limpia sin errores.

3. **Pruebas de accesibilidad y componentes UI:**
   ```bash
   npm --prefix ui test src/__tests__/components/SettingsTogglesAccessibility.spec.ts -- --run
   ```
   **Resultado:** 6 pruebas pasadas.

4. **Pruebas de vistas de Ajustes:**
   ```bash
   npm --prefix ui test src/__tests__/views/SettingsResetAll.spec.ts -- --run
   ```
   **Resultado:** Pasado correctamente.

5. **Pruebas de modales de reparación:**
   ```bash
   npm --prefix ui test src/__tests__/components/RepairHistoryModal.spec.ts src/__tests__/components/TidalRepairReviewModal.spec.ts src/__tests__/components/DisambiguationRepairReviewModal.spec.ts -- --run
   ```
   **Resultado:** 17 pruebas pasadas en los 3 suites de modales.

---

## 3. Elementos abiertos

- El recálculo masivo del campo `metadata_score` en la tabla `tracks` de SQLite para toda la base de datos histórica persistida está previsto para ejecutarse durante la migración/sincronización de base de datos de la Ola 6 / Ola 8. El motor de cálculo (`syncify-metadata-domain::scoring`) y los endpoints reactivos de UI quedan plenamente disponibles para dicho pase.

---

## 4. Riesgos evaluados

- **Impacto en temas claros (Riso):** En el tema Riso, `--on-accent` es blanco y `--accent` es azul/rosa, manteniendo el contraste del switch de manera legible.
- **Rendimiento de guardado debounced:** Al introducir 300 ms de debounce en los sliders de scoring, el usuario percibe una interfaz fluida sin sobrecargar la persistencia en disco ni provocar saltos de foco.
