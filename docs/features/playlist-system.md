# Playlist System

**Estado de revisión:** 2026-10-02 — narrativa re-derivada del código (ítem 9.1 del plan post-auditoría, DO-5): la sección "Sync Per-Service" ya no afirma que solo Spotify importe playlists en Rust; el motor unificado las importa para Qobuz, Tidal, Spotify, Deezer y Apple Music. Gate de rutas: `scripts/tests/test_docs_consistency.py` — todas las rutas citadas existen o están declaradas como eliminadas.

**Último cambio:** ítem 9.1 del plan post-auditoría (2026-10-02) — estado real de la importación de playlists por servicio. Antes: S40 — 2026-03-30 — archivos: `migrations/0006_playlists.sql`, `commands/library.rs`, `commands/service.rs`
**Leer antes de modificar:** `commands/tools.rs` (Python bridge playlist commands), `scripts/playlist_bridge.py`
**Archivos core:**
- `migrations/0006_playlists.sql` (schema)
- `src-tauri/src/commands/service.rs` L816 (`import_spotify_playlists`), L1352 (`import_qobuz_playlists`), L1384 (`import_tidal_library`), L1470 (`import_deezer_library`), L2165 (`import_apple_music_library`), L43 (`upsert_playlist_and_source`)
- `src-tauri/src/commands/library.rs` (`get_local_playlist_tracks`, `get_playlists`, `add_to_playlist`, `create_playlist`)
- `src-tauri/src/commands/tools.rs` L857 (`fetch_remote_playlist_tracks`), L866 (`export_playlist`), L885 (`match_playlist_to_service`)
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
    → library.rs: INSERT INTO playlists (account_id=NULL, name, ...)

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
| `account_id` | INTEGER | NOT NULL FK → accounts(id) ON DELETE CASCADE | NULL for local playlists |
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
(expansión paginada cuando la relación viene truncada). SoundCloud no entra: su API
pública no expone playlists de usuario.

| Servicio | Ruta de importación en Rust | Comando |
|---|---|---|
| Spotify | motor + `import_spotify_playlists` | `import_spotify_playlists` |
| Qobuz | motor + `import_qobuz_playlists` | `import_qobuz_playlists` |
| Tidal | motor (rama `tidal`) | `import_tidal_library` |
| Deezer | motor (rama `deezer`) | `import_deezer_library` |
| Apple Music | `AppleMusicClient::import_playlists` desde el brazo `apple_music` | `import_apple_music_library` |
| SoundCloud | — (capacidad no expuesta por la API pública) | — |

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

---

## Puntos de ruptura conocidos

1. **CASCADE on account deletion**: Deleting an account CASCADE-deletes all its playlists AND their playlist_tracks. This is intentional but destructive.
2. **Re-importar no duplica playlists**: `upsert_playlist_and_source()` busca primero en `playlist_sources` por `(account_id, service_playlist_id)` y, si no encuentra, actualiza o inserta en `playlists`; las rutas que no pasan por él (p. ej. `AppleMusicClient::import_playlists`) hacen su propio `ON CONFLICT` sobre la misma clave. Lo que sí cambia en una reimportación son `playlist_tracks`: las pistas se re-vinculan y el orden se rehace.
3. **Playlist tracks vs library entries**: una pista puede estar en `playlist_tracks` sin fila en `library_entries`, y entonces la vista de biblioteca no la muestra. El motor unificado (`EnrichmentEngine`) y el importador de Apple Music crean la entrada (`library_entries` con `is_liked=1`); las rutas que solo escriben `playlist_tracks` no.
4. **Local playlists (account_id=NULL)**: The schema says `account_id NOT NULL` but `create_playlist` in library.rs may set it to NULL for locally-created playlists. This violates the constraint if not handled.
5. **Position tracking**: solo las rutas que recomprimen al final lo garantizan. `import_spotify_playlists` llama a `recompact_playlist_positions(playlist_id)` tras cada playlist y deja las posiciones en 1..N; las que no (p. ej. `AppleMusicClient::import_playlists`, que escribe `position` desde el índice de la relación) dejan la posición como vino del servicio, así que un reordenado en el origen se propaga como desalineación local.
