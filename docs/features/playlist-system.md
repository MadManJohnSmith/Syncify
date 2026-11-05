# Playlist System

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-10-25 — re-revisión contra el árbol (verificación dirigida de símbolos y comportamiento): citas de línea de `commands/service.rs` y `commands/tools.rs` sustituidas por símbolos; reescrito el razonamiento de SoundCloud (`SoundcloudClient::get_playlists`/`get_playlist_tracks` existen en `services/soundcloud.rs` tras `#[allow(dead_code)]`, sin comando que los invoque) y corregido el punto de ruptura de `create_playlist` (hace `account_id.unwrap_or(1)`, nunca inserta `NULL`). Sin verificar: importaciones contra servicios reales en vivo.

**Último cambio:** re-revisión 2026-10-25 — símbolos en lugar de números de línea, razonamiento de SoundCloud corregido y punto de ruptura de `create_playlist` actualizado. Antes: ítem 9.1 del plan post-auditoría (2026-10-02) — estado real de la importación de playlists por servicio. Antes: S40 — 2026-03-30 — archivos: `migrations/0006_playlists.sql`, `commands/library.rs`, `commands/service.rs`
**Leer antes de modificar:** `commands/tools.rs` (Python bridge playlist commands), `scripts/playlist_bridge.py`
**Archivos core:**
- `migrations/0006_playlists.sql` (schema) y `migrations/0048_playlist_sync.sql` (identidad remota `playlist_sources`)
- `src-tauri/src/commands/service.rs` — `upsert_playlist_and_source`, `import_spotify_playlists`, `import_qobuz_playlists`, `import_tidal_library`, `import_deezer_library`, `import_apple_music_library` (helpers `emit_import_progress`/`emit_import_complete`)
- `src-tauri/src/commands/library.rs` (`get_local_playlist_tracks`, `get_playlists`, `add_to_playlist`, `create_playlist`)
- `src-tauri/src/commands/playlists.rs` (`recompact_playlist_positions`, `match_entry_to_track`)
- `src-tauri/src/commands/tools.rs` — `fetch_remote_playlist_tracks`, `export_playlist`, `match_playlist_to_service` (Python bridge playlist commands)
- `scripts/playlist_bridge.py` (Python bridge for cross-service ops)

---

## Flujo Completo

### Playlist Import (Spotify)

```
FRONTEND → invoke("import_spotify_playlists")
    │
    ▼
service.rs::import_spotify_playlists()
    │
    ├── 1. Load credentials, refresh token
    ├── 2. Paginate user playlists (offset/limit=50)
    ├── 3. For each playlist:
    │     a. upsert_playlist_and_source(...)
    │        → SELECT playlist_sources WHERE account_id=? AND service_playlist_id=?
    │        → fallback: SELECT playlists WHERE account_id=? AND service_playlist_id=?
    │        → si no hay service_playlist_id, busca por nombre (playlists locales)
    │        → UPDATE playlists (metadatos + last_synced) o INSERT, y registra
    │          la identidad remota en playlist_sources
    │     b. Paginate playlist tracks (limit=100) mientras page.next exista
    │     c. For each track:
    │        - get_or_create_artist → get_or_create_album_with_compilation
    │          → get_or_create_track  (varios artistas → compilación: Various Artists)
    │        - INSERT OR IGNORE track_artists (primary + featured)
    │        - INSERT OR IGNORE track_sources
    │        - INSERT OR IGNORE playlist_tracks (playlist_id, track_id, position)
    │     d. recompact_playlist_positions(playlist_id) → posiciones 1..N y track_count
    ├── 4. Emit "import-progress" per playlist
    └── 5. Emit "import-complete" { service, imported, tracks }
```

### Local Playlist Management (UI-created)

```
FRONTEND → invoke("create_playlist", { name, description })
    → library.rs: INSERT INTO playlists (account_id = account_id.unwrap_or(1),
      service_playlist_id = 'local_' || name, name, ...)

FRONTEND → invoke("add_to_playlist", { playlist_id, track_ids })
    → library.rs: INSERT OR IGNORE INTO playlist_tracks per track_id

FRONTEND → invoke("get_playlists")
    → library.rs: SELECT * FROM playlists ORDER BY name

FRONTEND → invoke("get_local_playlist_tracks", { playlist_id })
    → library.rs: SELECT tracks JOIN playlist_tracks WHERE playlist_id=?
```

### Cross-Service Playlist Operations (Python Bridge)

```
FRONTEND → invoke("fetch_remote_playlist_tracks", { service, playlist_id })
    → tools.rs → python scripts/playlist_bridge.py get <service> <playlist_id>

FRONTEND → invoke("export_playlist", { service, playlist_id, format })
    → tools.rs → python scripts/playlist_bridge.py export <service> <id> --format json|m3u

FRONTEND → invoke("match_playlist_to_service", { playlist_file, target_service })
    → tools.rs → python scripts/playlist_bridge.py match <playlist_file> <target>
    (clasifica las entradas por disponibilidad de ISRC; NO consulta el catálogo del destino)
```

No hay comando Tauri `list_playlists`: el listado remoto vive en el subcomando
`playlist_bridge.py list <service>` (con sus `get_*_playlists()` por servicio), pero
ningún command de Rust lo invoca; la UI lista las playlists locales con
`get_playlists` y las del servicio con `fetch_remote_playlist_tracks`.

---

## Schema (0006_playlists.sql)

### playlists

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | INTEGER | PK AUTOINCREMENT | |
| `account_id` | INTEGER | NOT NULL FK → accounts(id) ON DELETE CASCADE | `create_playlist` usa la cuenta 1 para playlists locales (nunca `NULL`) |
| `service_playlist_id` | TEXT | NOT NULL | Service-specific ID |
| `name` | TEXT | NOT NULL | Playlist name |
| `description` | TEXT | | Optional description |
| `owner_name` | TEXT | | Playlist owner |
| `is_public` | INTEGER | DEFAULT 1 | Boolean flag |
| `is_collaborative` | INTEGER | DEFAULT 0 | Boolean flag |
| `image_url` | TEXT | | Cover art URL |
| `track_count` | INTEGER | DEFAULT 0 | From service metadata |
| `last_synced` | TEXT | | Last sync timestamp |
| `created_at` | TEXT | DEFAULT CURRENT_TIMESTAMP | |
| `updated_at` | TEXT | DEFAULT CURRENT_TIMESTAMP | |

UNIQUE constraint: `(account_id, service_playlist_id)`

### playlist_tracks

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | INTEGER | PK AUTOINCREMENT | |
| `playlist_id` | INTEGER | NOT NULL FK → playlists(id) ON DELETE CASCADE | |
| `track_id` | INTEGER | NOT NULL FK → tracks(id) ON DELETE CASCADE | |
| `position` | INTEGER | NOT NULL DEFAULT 0 | Track order in playlist |
| `added_at` | TEXT | | When track was added |

UNIQUE constraint: `(playlist_id, track_id)`

### Indexes

- `idx_playlists_account` ON playlists(account_id)
- `idx_playlist_tracks_playlist` ON playlist_tracks(playlist_id)
- `idx_playlist_tracks_track` ON playlist_tracks(track_id)

---

## Campos críticos en DB

| Table | Column | Written By | Notes |
|-------|--------|-----------|-------|
| `playlists` | `account_id` | import / create_playlist | FK with CASCADE |
| `playlists` | `service_playlist_id` | import | Spotify/Qobuz playlist ID |
| `playlist_tracks` | `position` | import | Preserves original order |
| `playlist_tracks` | `playlist_id`, `track_id` | import / add_to_playlist | UNIQUE pair |

---

## Eventos Tauri emitidos

| Event | Payload |
|-------|---------|
| `import-progress` | `{ service, status, current, total, message }` (`emit_import_progress`) |
| `import-complete` | dos formas: `import_spotify_playlists` emite `{ service, imported, tracks }`; el helper `emit_import_complete` (resto de servicios) emite `{ service, imported, skipped, message }` |

---

## Sync Per-Service

**La importación de playlists ocurre en Rust para 5 servicios, no solo para Spotify.**
El motor unificado `perform_sync_service_with_emitter` llama a
`upsert_playlist_and_source()` en las ramas `qobuz`, `tidal`, `spotify` y `deezer`, y
la rama `apple_music` importa las suyas vía `AppleMusicClient::import_playlists`
(expansión paginada cuando la relación viene truncada). SoundCloud no entra en la
importación, pero no por falta de capacidad: `SoundcloudClient::get_playlists` y
`SoundcloudClient::get_playlist_tracks` existen en `src-tauri/src/services/soundcloud.rs`
(detrás de `#[allow(dead_code)]`, cubiertos por `tests/soundcloud_playlists_client_test.rs`)
y el puente Python sí tiene `get_user_playlists` para sus servicios. Lo que falta es el
cableado: ningún comando Rust invoca a esos métodos del cliente para importar, y la UI
rechaza el servicio en `importPlaylists` con `Unsupported playlist service`.

| Servicio | Ruta de importación en Rust | Comando |
|---|---|---|
| Spotify | motor + `import_spotify_playlists` | `import_spotify_playlists` |
| Qobuz | motor + `import_qobuz_playlists` | `import_qobuz_playlists` |
| Tidal | motor (rama `tidal`) | `import_tidal_library` |
| Deezer | motor (rama `deezer`) | `import_deezer_library` |
| Apple Music | `AppleMusicClient::import_playlists` desde el brazo `apple_music` | `import_apple_music_library` |
| SoundCloud | — (cliente capaz en `services/soundcloud.rs`, sin comando que lo invoque) | — |

La UI enruta por servicio en `ui/src/api/playlists.ts::importPlaylists`
(spotify/qobuz/tidal/deezer) y rechaza el resto con `Unsupported playlist service`.

Lo que **sí** va por el puente Python `scripts/playlist_bridge.py` son las operaciones
de archivo, no la importación:
- `fetch_remote_playlist_tracks` (`playlist_bridge.py get <service> <id>`) → pistas de
  una playlist remota sin importar
- `export_playlist` (`playlist_bridge.py export <service> <id> --format json|m3u`) →
  exporta a archivo
- `match_playlist_to_service` (`playlist_bridge.py match <playlist_file> <target>`) →
  clasifica las entradas del archivo de playlist por disponibilidad de ISRC (no consulta
  el catálogo del destino; el matching real por ISRC contra la biblioteca local es
  `commands::playlists::match_entry_to_track`)

### UI Bindings

| Composable/View | Commands Used |
|----------------|---------------|
| `ui/src/api/playlists.ts`, `ui/src/api/accounts.ts` | `import_spotify_playlists`, `import_qobuz_playlists`, `import_tidal_library`, `import_deezer_library` |
| LibraryView.vue | `get_playlists`, `get_local_playlist_tracks` |
| PlaylistView.vue | `importPlaylists` (el wrapper que enruta por servicio) |
| TrackContextMenu | `add_to_playlist`, `create_playlist` |

Los wrappers de estos comandos viven en `ui/src/api/` (`libraryApi.getPlaylists` y el
objeto `playlistsApi`, entre otros); las vistas no invocan `invoke` directamente.

---

## Puntos de ruptura conocidos

1. **CASCADE on account deletion**: Deleting an account CASCADE-deletes all its playlists AND their playlist_tracks. This is intentional but destructive.
2. **Re-importar no duplica playlists**: `upsert_playlist_and_source()` busca primero en `playlist_sources` por `(account_id, service_playlist_id)` y, si no encuentra, actualiza o inserta en `playlists`; las rutas que no pasan por él (p. ej. `AppleMusicClient::import_playlists`) hacen su propio `ON CONFLICT` sobre la misma clave. Lo que sí cambia en una reimportación son `playlist_tracks`: las pistas se re-vinculan y el orden se rehace.
3. **Playlist tracks vs library entries**: una pista puede estar en `playlist_tracks` sin fila en `library_entries`, y entonces la vista de biblioteca no la muestra. El motor unificado (`EnrichmentEngine`) y el importador de Apple Music crean la entrada (`library_entries` con `is_liked=1`); las rutas que solo escriben `playlist_tracks` no.
4. **Local playlists cuelgan de la cuenta 1**: la columna `account_id` es `NOT NULL` con FK a `accounts`, y `create_playlist` (`commands/library.rs`) hace `account_id.unwrap_or(1)`, así que nunca inserta `NULL` — las playlists creadas desde la UI quedan asociadas a la cuenta 1, con `service_playlist_id = 'local_' || name` y `track_count = 0`.
5. **Position tracking**: solo las rutas que recomprimen al final lo garantizan. `import_spotify_playlists` llama a `recompact_playlist_positions(playlist_id)` tras cada playlist y deja las posiciones en 1..N; las que no (p. ej. `AppleMusicClient::import_playlists`, que escribe `position` desde el índice de la relación) dejan la posición como vino del servicio, así que un reordenado en el origen se propaga como desalineación local.
