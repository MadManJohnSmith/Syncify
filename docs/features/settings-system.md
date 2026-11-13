# Settings System

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-10-25 — re-revisión contra el árbol (verificación dirigida de símbolos y comportamiento): corregida la cifra de `commands/settings.rs` (se citan símbolos, no números de línea), eliminada la fila inexistente `useLibrary.ts` (la carga de biblioteca vive en `ui/src/api/library.ts`), corregido el origen de la tabla `settings` (migration 0002, no 0001) y reescrito el `default_download_path` (candidatos validados con fichero probe; nunca una ruta relativa). Rellenados vacíos: bloque SPRINT 120 de descargas unificadas, comandos Last.fm y las composables que faltaban en el listado. Sigue sin verificar en escritorio real el comportamiento GUI; contrasta con el código antes de confiar en los detalles.

**Último cambio:** 2026-10-25 — re-revisión de comportamiento; archivos: `src-tauri/src/commands/settings.rs`, composables
**Leer antes de modificar:** `models.rs` (struct definitions), migrations 0007–0016
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
- Used by: download settings, general settings, advanced settings
- Backend: `get_kv_settings(keys)` → `HashMap<String, String>` (enmascara claves sensibles antes de devolverlas)
- Backend: `save_settings_batch(settings)` → transactional upsert (`BEGIN IMMEDIATE`); sincroniza `dl_download_path` con `folder_settings.base_folder` y guarda las claves `spotify_*` cifradas
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
- `metadata_preferences` → `get_metadata_preferences()` / `update_metadata_preferences()`

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

### Self-Healing Behavior

`get_kv_settings()` auto-creates `dl_download_path` if missing or blank:
1. Detects if a download-path key (`dl_download_path`, `download_dir`, `download_path`) was requested and is missing/empty
2. Computes the default via `resolve_effective_download_paths(db)` → `library_root`, con `default_download_path()` como fallback
3. Persists default to DB immediately
4. Returns the default in the response

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
| `useDownloadSettings.ts` | Download path, quality, format, concurrency | KV |
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
| `metadata_preferences` | Yes (id=1) | 0021 |

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
