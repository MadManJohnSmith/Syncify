# Auth Flow

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-11-14 — re-derivación contra el árbol (verificación dirigida de símbolos y comportamiento). Re-deriva y absorbe la re-revisión del 2026-10-25 (refresh de Spotify con un único camino PKCE, sin discriminación por `token_type`; `refresh_from_sp_dc` no existe en el árbol; el fallback Qobuz lee la BD cifrada canónica, no el JSON del puente; upsert de cuentas con reconciliación de identidades en `upsert_service_account`; siembra de `services` en la migration 0002; fallback a Chrome del sistema compartido por 5 puentes de auth). Novedades del árbol desde entonces: el contrato del puente es único — `scripts/services/bridge_protocol.py` define el envelope `{"success": …}`/`BridgeFailure` con exit 1 y `scripts/auth_bridge.py` cierra en `run(_handle, parse=_parse)` — y `HANDLERS` incluye `apple_music` (6 servicios). El refresh de Spotify resuelve su configuración con `resolve_spotify_config` (`src-tauri/src/commands/settings.rs`, no `service.rs`). La sonda solo-debug de Deezer (`debug_deezer_cookie_probe`) se documenta en `authentication.md`. Sigue sin verificar en escritorio real el flujo completo contra proveedores vivos: los tests que necesitan red están `#[ignore]`.

**Migración declarada:** [#62](https://github.com/MadManJohnSmith/Syncify/issues/62) — inicio de sesión dentro del webview de la app; la sonda de Deezer es [#63](https://github.com/MadManJohnSmith/Syncify/issues/63).

**Último cambio:** 2026-11-19 — módulo de refresco de credenciales (`src-tauri/src/commands/account_refresh.rs`, issue #95). Previo: 2026-11-14 — re-derivación completa contra el árbol (D-04a, issue #58); archivos: `src-tauri/src/commands/auth.rs`, `scripts/auth_bridge.py`, `scripts/services/bridge_protocol.py`, `src-tauri/src/commands/settings.rs`. Previo: 2026-10-25 — re-revisión de comportamiento; archivos: `src-tauri/src/commands/auth.rs`, `src-tauri/src/commands/service.rs`, `src-tauri/src/services/spotify.rs`
**Leer antes de modificar:** `src-tauri/src/crypto.rs`, `migrations/0031_add_credentials_invalid.sql`
**Archivos core:**
- `scripts/auth_bridge.py`
- `src-tauri/src/services/spotify.rs` (OAuth de Spotify; ya no existe `scripts/services/spotify_auth.py`, Fusionado en el núcleo Rust)
- `scripts/services/qobuz_auth.py`
- `scripts/services/tidal_auth.py`
- `scripts/services/deezer_auth.py`
- `scripts/services/soundcloud_auth.py`
- `scripts/services/apple_music_auth.py`
- `src-tauri/src/commands/auth.rs`
- `src-tauri/src/commands/service.rs` (helpers de refresh de token; `get_or_refresh_spotify_token`)
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
│    3. If Qobuz: merges fallback from the encrypted accounts DB       │
│    4. Extracts display_name, email, user_id                          │
│    5. Looks up service_id via: SELECT id FROM services WHERE name=?  │
│    6. Encrypts credentials via crypto::encrypt()                     │
│    7. Reconciles account identity (email / external_user_id)         │
│    8. Returns AuthResult { success, data, error }                    │
└───────────────────────────────┬────────────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────────┐
│  PYTHON: scripts/auth_bridge.py                                       │
│    Entry: run(_handle, parse=_parse) (services/bridge_protocol.py)   │
│    _handle → HANDLERS[service](action)                               │
│    Per-service handler → delegates to services/<service>_auth.py     │
│    Returns JSON to stdout: {"success": true/false, "data": {...}}    │
│    (BridgeFailure → {"success": false, "error": ...}, exit 1)        │
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
| **Qobuz**   | Playwright → XHR intercept | `user_auth_token`, `username`, `password` | accounts.credentials_json (encrypted) |
| **Tidal**   | Device Code (PKCE)   | `access_token`, `refresh_token`       | accounts.credentials_json |
| **Deezer**  | Playwright → ARL     | `arl` (used as access_token)          | accounts.credentials_json |
| **SoundCloud** | Playwright → OAuth | `oauth_token`, `access_token`        | accounts.credentials_json |
| **Apple Music** | Playwright → MUT | `music_user_token`, `developer_token` | accounts.credentials_json |

### Spotify Token Refresh (Rust-side)

The function `get_or_refresh_spotify_token()` in `src-tauri/src/commands/service.rs` handles token lifecycle with a single PKCE path:

1. Checks `expires_at` with 300s buffer
2. Refreshes via `grant_type=refresh_token` against `accounts.spotify.com`; the client config is resolved by `resolve_spotify_config(db)` en `src-tauri/src/commands/settings.rs` (settings en BD primero, env como fallback en dev)
3. Re-encrypts updated credentials (`access_token`, `refresh_token`, `expires_at`) and saves to `accounts` table

No discrimina por `token_type`: no hay camino separado para sesiones sp_dc (`refresh_from_sp_dc` no existe en el árbol). `SpotifyConfig::refresh_access_token` existe en `src-tauri/src/services/spotify.rs` pero solo se usa internamente. La reparación de sesión por sp_dc es un comando aparte: `refresh_spotify_session(sp_dc)` en `src-tauri/src/commands/auth.rs`.

### Qobuz Fallback Chain (`commands/auth.rs`: `start_auth_and_save` → `load_qobuz_db_fallback_auth`)

1. Try `user_auth_token` / `auth_token` / `access_token` from Python bridge response
2. Validate with `is_viable_qobuz_token_auth()` (≥16 chars, no JSON blobs, no JWT-looking `eyJ` prefixes, no whitespace)
3. Fallback: `load_qobuz_db_fallback_auth()` reads the canonical encrypted DB (AES-256-GCM) — solo si el servicio tiene exactamente una cuenta activa (`service_has_exactly_one_account`) y solo si el `username` cacheado coincide con el login del puente (`qobuz_cache_matches_login`); devuelve token, username y password
4. Final: check `QOBUZ_USERNAME` / `QOBUZ_EMAIL` + `QOBUZ_PASSWORD` env vars
5. If all fail → return error asking user to log in manually

Histórico: el fallback ya no lee `scripts/.gui_credentials_cache.json`. Ese JSON hoy solo lo usan puentes Python (`scripts/services/soundcloud_auth.py`, `scripts/services/tidal_service.py`), no el fallback Qobuz, y el gate lo tiene en EXPECTED_ABSENT.

### Tidal Token Expiry Injection (auth.rs)

Antes de guardar credenciales de Tidal, `start_auth_and_save` garantiza que el payload lleve expiración (`inject_tidal_expiry`): conserva la que ya traiga el payload; si no trae, usa la expiry cacheada en BD (`load_tidal_db_cached_token_expiry`) y, si no hay ninguna, un piso conservador de +4h. Sin expiración, `TidalGuiCredentials::is_expired()` trataría cada login recién hecho como caducado y forzaría un refresh OAuth en el primer uso.

---

### Refresco de credenciales por cuenta (ADR-0007 decisión 14)

El módulo `src-tauri/src/commands/account_refresh.rs` unifica la política de
refresco sobre las credenciales cifradas de `accounts`:

- **Proactivo**: si el payload publica la expiración (`expires_at`, o el alias
  `token_expiry` de Tidal), se refresca ANTES de usar la credencial cuando
  `now >= expires_at - 300` (mismo buffer que `get_or_refresh_spotify_token` y
  el `is_expired` de `TidalGuiCredentials`).
- **Reactivo**: sin vencimiento publicado, el primer 401/403 dispara UN
  intento de refresco (`handle_credential_rejection`); con vencimiento
  publicado, el 401/403 marca directamente (el proactivo ya actuó antes del
  uso).
- **Sin bucle**: una cuenta con `credentials_invalid = 1` no recibe intentos
  (`RefreshOutcome::NeedsReauth` / `RejectionOutcome::AlreadyMarked`); solo
  reautenticar la desbloquea.
- **Sin envenenar por caída**: un fallo TEMPORAL de transporte (red, 5xx,
  respuesta ilegible) NO marca la cuenta (`RetryLater`), la misma distinción
  S185 que aplica `resolve_and_refresh_gui_credentials_for_account` a Tidal.
  Solo el rechazo del grant (400/401/403 al refresco) o un refresco imposible
  marcan reautenticación, vía `mark_account_credentials_invalid` (que sanea
  el motivo y emite `auth-state-updated`).
- **Artefacto seguro**: la persistencia (`apply_refreshed_credentials`) es un
  `UPDATE ... WHERE id` — nunca duplica cuentas — y solo escribe tras un
  transporte exitoso; el payload de Tidal se construye por fusión, así que
  campos ajenos al struct (`session_id`) sobreviven al refresco.

Los puntos de entrada (`refresh_account_credential`, `handle_credential_rejection`)
reciben el reloj inyectado y un transporte (`CredentialRefreshTransport`) para
probarse con reloj y tokens falsos, sin red
(`src-tauri/tests/account_credential_refresh_test.rs`). La adopción en el
pipeline de descarga y en el sync queda para sus carriles: Spotify y Tidal ya
refrescan proactivamente antes de cada uso en sus caminos actuales, y el sync
y las descargas ya marcan al 401/403.

## Campos críticos en DB

| Table       | Column              | Type    | Written By        | Notes                                          |
|------------|---------------------|---------|-------------------|-------------------------------------------------|
| `services`  | `id`, `name`        | INT/TXT | Migration 0002 (STEP 8) | Static seed: spotify, qobuz, tidal, deezer, soundcloud, apple_music |
| `accounts`  | `service_id`        | INT FK  | start_auth_and_save | References services.id                         |
| `accounts`  | `credentials_json`  | TEXT    | start_auth_and_save | AES-256-GCM encrypted (via crypto.rs)          |
| `accounts`  | `credentials_invalid` | INT  | Startup purge     | 1 = credentials irrecoverable, needs re-auth    |
| `accounts`  | `is_active`         | INT    | start_auth_and_save | 1 = connected, 0 = deactivated                 |
| `accounts`  | `display_name`      | TEXT   | start_auth_and_save | User-facing name from service profile           |
| `accounts`  | `email`             | TEXT   | start_auth_and_save | Optional, from service profile                  |
| `accounts`  | `last_synced`       | TEXT   | start_auth_and_save | CURRENT_TIMESTAMP on connect                    |
| `accounts`  | `external_user_id`  | TEXT   | upsert_service_account | Identidad estable del proveedor (migration 0089); clave de deduplicación |
| `accounts`  | `invalid_reason`    | TEXT   | Startup purge     | Por qué las credenciales se marcaron inválidas (migration 0051) |
| `accounts`  | `streaming_entitled` | INT   | start_auth_and_save | Entitlement Qobuz: 1 = sí, 0 = no, -1 = desconocido (migration 0095) |
| `accounts`  | `display_name_custom` | INT  | Rename en UI (`commands/accounts.rs`) | 1 = el usuario renombró la cuenta; un re-login no la pisa (migration 0101) |

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
4. **UPSERT vs INSERT OR REPLACE**: `start_auth_and_save` delega en `upsert_service_account`, que resuelve la fila destino por identidad (email normalizado y/o `external_user_id`; si email y user_id apuntan a filas distintas gana el user_id; INSERT solo si nada casa) y conserva TODAS las filas para que la biblioteca CASCADE de cada cuenta sobreviva al re-login. Cambiar a `INSERT OR REPLACE` borraría la biblioteca importada.
5. **Playwright dependency**: Todas las autenticaciones browser-based requieren `playwright` instalado con Chromium. El script fallback a Chrome del sistema si Chromium no está disponible; `scripts/services/browser_launcher.py` se comparte con 5 puentes de auth (deezer, apple_music, soundcloud, tidal, qobuz).
