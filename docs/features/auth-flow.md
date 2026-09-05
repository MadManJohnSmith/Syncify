# Auth Flow

**Estado de revisión:** 2026-09-05 — pendiente de revalidación contra la implementación actual. Úsalo como referencia de diseño y verifica el código fuente antes de confiar en los detalles.

**Último cambio:** S43 — 2026-04-25 — archivos: `scripts/auth_bridge.py`, `scripts/services/spotify_auth.py`, `scripts/services/qobuz_auth.py`, `src-tauri/src/commands/auth.rs`
**Leer antes de modificar:** `src-tauri/src/crypto.rs`, `migrations/0031_add_credentials_invalid.sql`
**Archivos core:**
- `scripts/auth_bridge.py`
- `scripts/services/spotify_auth.py`
- `scripts/services/qobuz_auth.py`
- `scripts/services/tidal_auth.py`
- `scripts/services/deezer_auth.py`
- `scripts/services/soundcloud_auth.py`
- `scripts/services/apple_music_auth.py`
- `src-tauri/src/commands/auth.rs`
- `src-tauri/src/commands/service.rs` (L1–163: token refresh helpers)
- `src-tauri/src/crypto.rs`

---

## Flujo Completo

```
┌────────────────────────────────────────────────────────────────────────┐
│                         FRONTEND (Vue 3)                              │
│  useAccounts.ts → invoke("start_auth_and_save", { service })         │
└───────────────────────────────┬────────────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────────┐
│  RUST: commands/auth.rs → start_auth_and_save(service, state)        │
│    1. Calls start_auth(service, "login") → spawns Python subprocess  │
│    2. Parses JSON result from stdout {success, data, error}          │
│    3. If Qobuz: merges fallback from .gui_credentials_cache.json     │
│    4. Extracts display_name, email, user_id                          │
│    5. Looks up service_id via: SELECT id FROM services WHERE name=?  │
│    6. Encrypts credentials via crypto::encrypt()                     │
│    7. UPSERT into accounts (UPDATE first → INSERT if 0 rows)        │
│    8. Returns AuthResult { success, data, error }                    │
└───────────────────────────────┬────────────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────────┐
│  PYTHON: scripts/auth_bridge.py                                       │
│    Entry: main() → HANDLERS[service](action)                         │
│    Per-service handler → delegates to services/<service>_auth.py     │
│    Returns JSON to stdout: {"success": true/false, "data": {...}}    │
└───────────────────────────────┬────────────────────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
          ┌─────────────────┐    ┌──────────────────────┐
          │  BROWSER-BASED  │    │  DEVICE-CODE / ARL   │
          │  Playwright     │    │  HTTP-based flows    │
          │                 │    │                      │
          │  • Spotify      │    │  • Tidal (device     │
          │    (sp_dc)      │    │    code PKCE)        │
          │  • Qobuz        │    │  • Deezer (ARL       │
          │    (auth_token) │    │    cookie)           │
          │  • SoundCloud   │    │                      │
          │    (OAuth)      │    │                      │
          │  • Apple Music  │    │                      │
          │    (MUT)        │    │                      │
          └─────────────────┘    └──────────────────────┘
```

### Per-Service Auth Mechanisms

| Service      | Method               | Token Key(s)                          | Stored In                |
|-------------|----------------------|---------------------------------------|--------------------------|
| **Spotify** | Playwright → sp_dc   | `sp_dc`, `access_token`, `expires_at` | accounts.credentials_json (encrypted) |
| **Qobuz**   | Playwright → XHR intercept | `user_auth_token`, `username`, `password` | accounts + `.gui_credentials_cache.json` |
| **Tidal**   | Device Code (PKCE)   | `access_token`, `refresh_token`       | accounts.credentials_json |
| **Deezer**  | Playwright → ARL     | `arl` (used as access_token)          | accounts.credentials_json |
| **SoundCloud** | Playwright → OAuth | `oauth_token`, `access_token`        | accounts.credentials_json |
| **Apple Music** | Playwright → MUT | `music_user_token`, `developer_token` | accounts.credentials_json |

### Spotify Token Refresh (Rust-side)

The function `get_or_refresh_spotify_token()` in `service.rs` L77–163 handles token lifecycle:

1. Checks `expires_at` with 300s buffer
2. Discriminates by `token_type` field:
   - `"sp_dc"` → calls `services::spotify::refresh_from_sp_dc(sp_dc)` → new bearer token
   - `"oauth"` → calls `SpotifyConfig::refresh_access_token(refresh_token)` → standard OAuth
3. Re-encrypts updated credentials and saves to `accounts` table

### Qobuz Fallback Chain (auth.rs L165–217)

1. Try `user_auth_token` / `auth_token` from Python bridge response
2. Validate with `is_viable_qobuz_token_auth()` (≥16 chars, no JSON blobs, no whitespace)
3. Fallback: read `scripts/.gui_credentials_cache.json` → `qobuz_session.auth_token`
4. Fallback: read `qobuz.username` + `qobuz.password` from same cache
5. Final: check `QOBUZ_USERNAME` / `QOBUZ_EMAIL` + `QOBUZ_PASSWORD` env vars
6. If all fail → return error asking user to log in manually

---

## Campos críticos en DB

| Table       | Column              | Type    | Written By        | Notes                                          |
|------------|---------------------|---------|-------------------|-------------------------------------------------|
| `services`  | `id`, `name`        | INT/TXT | Migration 0001    | Static seed: spotify, qobuz, tidal, deezer, soundcloud, apple_music |
| `accounts`  | `service_id`        | INT FK  | start_auth_and_save | References services.id                         |
| `accounts`  | `credentials_json`  | TEXT    | start_auth_and_save | AES-256-GCM encrypted (via crypto.rs)          |
| `accounts`  | `credentials_invalid` | INT  | Startup purge     | 1 = credentials irrecoverable, needs re-auth    |
| `accounts`  | `is_active`         | INT    | start_auth_and_save | 1 = connected, 0 = deactivated                 |
| `accounts`  | `display_name`      | TEXT   | start_auth_and_save | User-facing name from service profile           |
| `accounts`  | `email`             | TEXT   | start_auth_and_save | Optional, from service profile                  |
| `accounts`  | `last_synced`       | TEXT   | start_auth_and_save | CURRENT_TIMESTAMP on connect                    |

---

## Eventos Tauri emitidos

| Event Name                   | Emitted By           | Payload Shape                                    |
|------------------------------|----------------------|-------------------------------------------------|
| `python_deps_missing`        | main.rs setup        | `{ message: string }`                           |
| `credential_migration_partial` | main.rs setup      | `{ failed_count, failed_ids, message }`         |
| `stale_credentials_purged`   | main.rs setup        | `{ purged_count, services[], message }`         |

---

## Puntos de ruptura conocidos

1. **crypto.rs key rotation**: Si la máquina cambia (o el keychain se reinicia), `decrypt()` falla para credenciales existentes. El startup purge en main.rs marca `credentials_invalid = 1` y emite `stale_credentials_purged`.
2. **Qobuz token viability**: El filtro `is_viable_qobuz_token_auth()` rechaza tokens JSON serialized (`{"v":29}`), tokens cortos (<16 chars), y placeholders. Si la lógica de filtrado cambia, la cadena de fallback completa puede quebrarse.
3. **Python subprocess stdout pollution**: `auth_bridge.py` usa `suppress_stdout()` y busca `{"success"` como marcador JSON. Si algún servicio imprime a stdout antes del JSON result, el parser en auth.rs falla.
4. **UPSERT vs INSERT OR REPLACE**: `start_auth_and_save` usa UPDATE-then-INSERT para evitar CASCADE DELETE en `library_entries` y `playlists`. Cambiar a `INSERT OR REPLACE` borraría la biblioteca importada.
5. **Playwright dependency**: Todas las autenticaciones browser-based requieren `playwright` instalado con Chromium. El script fallback a Chrome del sistema si Chromium no está disponible (solo Qobuz).
