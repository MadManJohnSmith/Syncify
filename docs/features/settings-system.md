# Settings System

**Estado de revisión:** 2026-09-05 — pendiente de revalidación contra la implementación actual. Úsalo como referencia de diseño y verifica el código fuente antes de confiar en los detalles.

**Último cambio:** S40 — 2026-03-30 — archivos: `commands/settings.rs`, composables
**Leer antes de modificar:** `models.rs` (struct definitions), migrations 0007–0016
**Archivos core:**
- `src-tauri/src/commands/settings.rs` (1083 lines)
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
- Backend: `get_kv_settings(keys)` → `HashMap<String, String>`
- Backend: `save_settings_batch(settings)` → transactional upsert
- Frontend: each composable defines its key prefixes (e.g. `dl_`, `gen_`, `adv_`)

**Pattern 2: Typed Tables (domain-specific)** — dedicated tables with typed columns
- `service_preferences` → `get_service_preferences()` / `update_service_preference()`
- `sync_settings` → `get_sync_settings()` / `update_sync_settings()`
- `service_sync_settings` → `get_service_sync_settings()` / `update_service_sync_settings()`
- `quality_preferences` → `get_quality_preferences()` / `update_quality_preference()`
- `folder_settings` → `get_folder_settings()` / `update_folder_settings()`
- `duplicate_settings` → `get_duplicate_settings()` / `update_duplicate_settings()`
- `audio_processing_settings` → `get_audio_processing_settings()` / `update_audio_processing_settings()`
- `lyrics_provider_settings` → `get_lyrics_providers()` / `update_lyrics_provider()`
- `lyrics_config` → `get_lyrics_config()` / `update_lyrics_config()`
- `metadata_preferences` → `get_metadata_preferences()` / `update_metadata_preferences()`

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
1. Detects if `dl_download_path` was requested and is missing/empty
2. Computes default via `default_download_path()` → `dirs::audio_dir()/Syncify`
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
| `useLibrary.ts` | Library data loading, search | Commands |
| `useQueue.ts` | Download queue state | Commands |
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
| `settings` (KV) | No (multi-row) | 0001_init |
| `service_preferences` | No (per-service) | 0007 |
| `sync_settings` | Yes (id=1) | 0008 |
| `service_sync_settings` | No (per-service) | 0008 |
| `quality_preferences` | No (per-service) | 0009 |
| `folder_settings` | Yes (id=1) | 0010 |
| `duplicate_settings` | Yes (id=1) | 0011 |
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
5. **Default download path**: `default_download_path()` depends on `dirs::audio_dir()` which may return `None` on some systems, falling back to just `"Syncify"` (relative path).
