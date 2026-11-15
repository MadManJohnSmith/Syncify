# Settings System

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-11-14 — re-derivación contra el árbol (verificación dirigida de símbolos y comportamiento; D-04d, issue #61). Re-deriva y absorbe la re-revisión del 2026-10-25 (cifra de `commands/settings.rs` sustituida por símbolos; origen de la tabla `settings` en la migration 0002; `default_download_path` con candidatos validados por fichero probe; bloque SPRINT 120, comandos Last.fm y listado de composables). Ajustado ahora al árbol:

- **SB01 (#199): cada dato en una sola superficie.** La ruta de descarga canónica es `folder_settings.base_folder`; las claves KV `dl_download_path`/`download_dir`/`download_path` son legado: `resolve_effective_download_paths` las lee solo como fallback y `get_kv_settings` las sirve por lectura directa (read-through), sin auto-sembrar espejos; `save_settings_batch` escribe la ruta a través de esas claves hacia `folder_settings.base_folder`. El límite concurrente canónico vive en `sync_settings.max_concurrent_downloads` (la KV `dl_concurrent_downloads` queda como lectura tolerante de restos legacy, p. ej. en el arranque del worker).
- **SB03 (#198): ajustes huérfanos cableados.** Los 10 flags `write_*` de `metadata_preferences` condicionan la escritura de tags vía `apply_tag_policy` (`services/tag_writer.rs`) en los puntos de escritura de FLAC; `preserve_custom_tags` gobierna el strip de comentarios Vorbis no gestionados (`TagWriteOptions`); `enable_lastfm` puerta `enrich_genre_lastfm` y el fallback de `enrich_track` (patrón `enable_musicbrainz`); `overwrite_on_reimport`, `multi_value_separator` y `sync_include_appearances` también condicionan comportamiento. Paridad de fábrica con `migrations/0105_write_flags_factory_parity.sql`.
- Corregido el self-heal (ya no persiste), añadida la guardia de placeholders enmascarados en `save_settings_batch` (TASK-100) y actualizado el listado de composables (`useEnrichmentProgress.ts`, `useTrackBrowser.ts`, `useVirtualWindow.ts`).

Sigue sin verificar en escritorio real el comportamiento GUI; contrasta con el código antes de confiar en los detalles.

**Último cambio:** 2026-11-14 — re-derivación completa contra el árbol (issue #61) — archivos: `src-tauri/src/commands/settings.rs`, `src-tauri/src/services/tag_writer.rs`, `src-tauri/migrations/0105_write_flags_factory_parity.sql`, composables. Previo: 2026-10-25 — re-revisión de comportamiento; archivos: `src-tauri/src/commands/settings.rs`, composables
**Leer antes de modificar:** `models.rs` (struct definitions), migrations 0007–0016, `src-tauri/src/settings_defaults.rs`
**Archivos core:**
- `src-tauri/src/commands/settings.rs` (KV store, tablas tipadas, SPRINT 120 de descargas unificadas)
- `src-tauri/src/models.rs` (struct definitions for all settings types)
- `ui/src/composables/useGeneralSettings.ts`
- `ui/src/composables/useDownloadSettings.ts`
- `ui/src/composables/useSyncSettings.ts`
- `ui/src/composables/useLyricsSettings.ts`
- `ui/src/composables/useMetadataSettings.ts`
- `ui/src/composables/useAdvancedSettings.ts`

---

## Flujo Completo

```
FRONTEND                              BACKEND
─────────                             ───────
composable.load()                     
  → invoke("get_kv_settings", keys)   → settings.rs::get_kv_settings()
  ← HashMap<String, String>              SELECT key, value FROM settings
                                          WHERE key IN (...)
                                          [self-heal dl_download_path default]

composable.save()
  → invoke("save_settings_batch",     → settings.rs::save_settings_batch()
       { settings: HashMap })             BEGIN TRANSACTION
                                          for (key, value):
                                            INSERT OR REPLACE INTO settings
                                          COMMIT
```

### Two Settings Storage Patterns

**Pattern 1: KV Store (generic)** — `settings` table
- Used by: download settings (legacy keys), general settings, advanced settings
- Backend: `get_kv_settings(keys)` → `HashMap<String, String>` (enmascara claves sensibles antes de devolverlas)
- Backend: `save_settings_batch(settings)` → transactional upsert (`BEGIN IMMEDIATE`); las claves de ruta (`dl_download_path`/`download_dir`/`download_path`) se escriben a través hacia `folder_settings.base_folder` (SB01), las claves `spotify_*` se guardan cifradas y las claves sensibles con placeholder enmascarado (`••••`/`****`) se ignoran para no pisar el secreto (TASK-100)
- Frontend: each composable defines its key prefixes (e.g. `dl_`, `gen_`, `adv_`)

**Pattern 2: Typed Tables (domain-specific)** — dedicated tables with typed columns
- `service_preferences` → `get_service_preferences()` / `update_service_preference()`
- `sync_settings` → `get_sync_settings()` / `update_sync_settings()`
- `service_sync_settings` → `get_service_sync_settings()` / `update_service_sync_settings()`
- `quality_preferences` → `get_quality_preferences()` / `update_quality_preference()`
- `folder_settings` → `get_folder_settings()` / `update_folder_settings()`
- `audio_processing_settings` → `get_audio_processing_settings()` / `update_audio_processing_settings()`
- `duplicate_settings` → **sin comandos desde #197 (SB02)**: sus ajustes carecían de lector, la pestaña se ocultó y los comandos se retiraron (reapertura en `docs/auditoria/matriz-ajustes.md`); la tabla queda para el reset
- `lyrics_provider_settings` → `get_lyrics_providers()` / `update_lyrics_provider()`
- `lyrics_config` → `get_lyrics_config()` / `update_lyrics_config()`
- `metadata_preferences` → `get_metadata_preferences()` / `update_metadata_preferences()`. Desde SB03 (#198) sus flags condicionan comportamiento real: `apply_tag_policy` (`services/tag_writer.rs`) filtra el DTO de `FlacMetadata` en los puntos de escritura según los `write_*`; `enable_lastfm` puerta el enriquecimiento de género Last.fm (`resolve_lastfm_api_key` en `commands/enrichment.rs`); `overwrite_on_reimport`, `multi_value_separator` y `sync_include_appearances` están cableados al reimportado y al motor. Fábrica alineada con `migrations/0105_write_flags_factory_parity.sql`.

**Pattern 3: Unified Download Settings (SPRINT 120)** — `DownloadSettingsDto` en `src-tauri/src/commands/settings.rs`
- DTO único para descargas, plantillas de carpeta/fichero, concurrencia y sidecars (`generate_lyrics_lrc`, `generate_cover_art`, `generate_animated_cover`, `generate_booklet`, `generate_artist_sidecars`, ...)
- Comandos: `get_download_settings` / `save_download_settings` / `get_default_download_path` (registrados en `src-tauri/src/main.rs`)
- Paths efectivos calculados por `resolve_effective_download_paths(db)`: `library_root` (canónico, de `folder_settings`) y `staging_root` (descarga temporal); expone además `path_status` y `free_space_bytes`

**Last.fm API key** — comandos `get_lastfm_api_key_status` / `set_lastfm_api_key` en `src-tauri/src/commands/settings.rs` (registrados en `src-tauri/src/main.rs`); el enriquecimiento la resuelve con `resolve_lastfm_api_key` en `src-tauri/src/commands/enrichment.rs`.

---

## KV Store Schema

Table: `settings`

| Column | Type | Notes |
|--------|------|-------|
| `key` | TEXT PRIMARY KEY | Unique setting identifier |
| `value` | TEXT | String value (all types stored as text) |
| `updated_at` | TEXT | Last modification timestamp |

### Known Key Prefixes

| Prefix | Composable | Example Keys |
|--------|-----------|--------------|
| `dl_` | useDownloadSettings | `dl_download_path`, `dl_quality`, `dl_format` |
| `gen_` | useGeneralSettings | `gen_theme`, `gen_language` |
| `adv_` | useAdvancedSettings | `adv_debug_mode`, `adv_log_level` |

### Self-Healing Behavior (SB01: read-through, sin auto-siembra)

`get_kv_settings()` resuelve `dl_download_path` ausente o vacío pero **ya no persiste** el valor en la KV:
1. Detects if a download-path key (`dl_download_path`, `download_dir`, `download_path`) was requested and is missing/empty
2. Computes the effective value via `resolve_effective_download_paths(db)` → `library_root` (canónico: `folder_settings.base_folder`; fallback legado: las propias claves KV; último recurso `default_download_path()`)
3. Returns the value in the response, solo en memoria (el comentario del código lo dice explícito: «solo lectura a través»)

---

## Composable Pattern (Frontend)

All settings composables follow the same structure:

```typescript
export function useXxxSettings() {
  const settings = ref<Record<string, string>>({});
  const loading = ref(false);
  const saving = ref(false);

  async function load() {
    loading.value = true;
    settings.value = await invoke("get_kv_settings", { keys: [...] });
    loading.value = false;
  }

  async function save() {
    saving.value = true;
    await invoke("save_settings_batch", { settings: settings.value });
    saving.value = false;
  }

  return { settings, loading, saving, load, save };
}
```

### Full Composable List

| File | Purpose | Pattern |
|------|---------|---------|
| `useGeneralSettings.ts` | Theme, language, startup | KV |
| `useDownloadSettings.ts` | Ruta y concurrencia de descarga: carga primero `get_effective_download_preferences` (canónico), con fallback legacy KV → unificado → default; la concurrencia canónica vive en `sync_settings` (SB01) | DTO + KV |
| `useSyncSettings.ts` | Auto-sync, intervals, per-service sync | Typed tables |
| `useLyricsSettings.ts` | Provider priority, config, sync level | Typed tables |
| `useMetadataSettings.ts` | MB/LastFM/AcoustID toggles, tag preferences | Typed table |
| `useAdvancedSettings.ts` | Debug mode, log level, cache, diagnostics | KV + typed |
| `useAccounts.ts` | Account connection state, import triggers | Commands |
| `useAccountsStatus.ts` | Service connection status polling | Commands |
| `useGlobalProgressBar.ts` | Barra de progreso global ponderada (agrega todas las tareas activas) | Events |
| `useIncrementalEnrichment.ts` | Enriquecimiento incremental: modo, preview, progreso del job | Commands + Events |
| `useLogs.ts` | Logs de sistema y auditoría compartidos (S170) | Events |
| `useNotificationListener.ts` | Eventos de ciclo de vida del supervisor de descargas | Events |
| `usePlayer.ts` | Reproductor local: cola con identidad, modos persistentes, crédito de escucha, MediaSession | Commands |
| `useTempoAnalysis.ts` | Análisis de BPM por lotes con progreso | Commands + Events |
| `useEnrichmentProgress.ts` | Progreso del enriquecimiento (contrato tipado, S170) | Commands + Events |
| `useTrackBrowser.ts` | Kernel genérico de ventana virtual de listas de pistas (ADR-0025) | Utility |
| `useVirtualWindow.ts` | Ventana virtual sobre la lista visible | Utility |
| `useTheme.ts` | Temas estándar/de servicio, tema dinámico por portada | LocalStorage |
| `useTint.ts` | Tinte camaleón desde la portada (`--tint-h` / `--tint-k`) | Vue |
| ~~`useQueue.ts`~~ | ELIMINADO sin consumidores (`417a0eb`); la API de cola vive en `ui/src/api/queue.ts` y su configuración en `useDownloadSettings.ts` | Commands |
| `useGlobalTasks.ts` | Background task coordination, toasts | Events |
| `useEventBus.ts` | Cross-component event communication | Vue |
| `useToast.ts` | Toast notification management | Vue |
| `useKeyboardShortcuts.ts` | Global keyboard shortcuts | Vue |
| `useMigration.ts` | Service-to-service migration | Commands |
| `useAsyncState.ts` | Generic async state wrapper | Utility |

---

## Campos críticos en DB

| Table | Singleton? | Migration |
|-------|-----------|-----------|
| `settings` (KV) | No (multi-row) | 0002_normalize_schema |
| `service_preferences` | No (per-service) | 0007 |
| `sync_settings` | Yes (id=1) | 0008 |
| `service_sync_settings` | No (per-service) | 0008 |
| `quality_preferences` | No (per-service) | 0009 |
| `folder_settings` | Yes (id=1) | 0010 |
| `duplicate_settings` | Yes (id=1) | 0011 | <!-- sin superficie IPC desde #197; solo reset -->
| `audio_processing_settings` | Yes (id=1) | 0012 |
| `lyrics_provider_settings` | No (per-provider) | 0013 |
| `lyrics_config` | Yes (id=1) | 0014 |
| `metadata_preferences` | Yes (id=1) | 0021 (paridad de fábrica: 0105) |

---

## Eventos Tauri emitidos

No events emitted by settings commands (synchronous request/response only).

---

## Puntos de ruptura conocidos

1. **KV vs typed inconsistency**: Two parallel storage patterns. Some composables use KV (`get_kv_settings`), others use typed commands. Mixing them causes confusion.
2. **No validation on KV values**: `save_settings_batch` writes raw strings. Invalid values (e.g. negative numbers for max_path_length) pass through.
3. **Singleton tables assume id=1**: All singleton settings (sync, folder, etc.) hardcode `WHERE id = 1`. If the row is deleted, all reads fail.
4. **`save_settings_batch` transaction**: Uses `db.begin()` transaction. If any key fails, entire batch rolls back (all-or-nothing).
5. **Default download path**: `default_download_path()` (`src-tauri/src/commands/settings.rs`) construye candidatos — `dirs::audio_dir()/Syncify`, `$HOME/Music/Syncify` y, como último recurso, `std::env::temp_dir()/Syncify` — y valida cada uno con `ensure_writable_dir` (crea el directorio y comprueba escritura con un fichero probe). Nunca devuelve una ruta relativa.
