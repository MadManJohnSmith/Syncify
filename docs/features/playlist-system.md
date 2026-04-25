# Playlist System

**Último cambio:** S40 — 2026-03-30 — archivos: `migrations/0006_playlists.sql`, `commands/library.rs`, `commands/service.rs`
**Leer antes de modificar:** `commands/tools.rs` (Python bridge playlist commands), `scripts/playlist_bridge.py`
**Archivos core:**
- `migrations/0006_playlists.sql` (schema)
- `src-tauri/src/commands/service.rs` L488–739 (import_spotify_playlists)
- `src-tauri/src/commands/library.rs` L623 (get_local_playlist_tracks), L1249 (get_playlists), L1277 (add_to_playlist), L1321 (create_playlist)
- `src-tauri/src/commands/tools.rs` L867–912 (list_playlists, get_playlist_tracks, export_playlist, match_playlist_to_service)
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
    │     a. INSERT OR REPLACE INTO playlists
    │        (account_id, service_playlist_id, name, description, ...)
    │     b. SELECT id FROM playlists WHERE account_id=? AND service_playlist_id=?
    │     c. Paginate playlist tracks (limit=100)
    │     d. For each track:
    │        - get_or_create_artist → get_or_create_album → get_or_create_track
    │        - INSERT OR IGNORE track_artists
    │        - INSERT OR IGNORE track_sources
    │        - INSERT OR IGNORE playlist_tracks (playlist_id, track_id, position)
    ├── 4. Emit "import-progress" per playlist
    └── 5. Emit "import-complete" { playlists, tracks }
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
FRONTEND → invoke("list_playlists", { service })
    → tools.rs → python scripts/playlist_bridge.py list <service>

FRONTEND → invoke("export_playlist", { service, playlist_id, target_service })
    → tools.rs → python scripts/playlist_bridge.py export <service> <id> <target>

FRONTEND → invoke("match_playlist_to_service", { playlist_id, target_service })
    → tools.rs → python scripts/playlist_bridge.py match <id> <target>
```

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
| `import-progress` | `{ service: "spotify_playlists", status, current, total, message }` |
| `import-complete` | `{ service: "spotify_playlists", imported, tracks }` |

---

## Sync Per-Service

Currently only Spotify playlist import is fully implemented in Rust.
Other services (Qobuz, Tidal, Deezer) rely on the Python bridge via `scripts/playlist_bridge.py`:
- `list_playlists` → fetches playlist metadata from service API
- `export_playlist` → exports tracks to target service
- `match_playlist_to_service` → matches tracks by ISRC/title to target service

### UI Bindings

| Composable/View | Commands Used |
|----------------|---------------|
| `useAccounts.ts` | `import_spotify_playlists` |
| LibraryView.vue | `get_playlists`, `get_local_playlist_tracks` |
| TrackContextMenu | `add_to_playlist`, `create_playlist` |

---

## Puntos de ruptura conocidos

1. **CASCADE on account deletion**: Deleting an account CASCADE-deletes all its playlists AND their playlist_tracks. This is intentional but destructive.
2. **No duplicate playlist detection**: Re-importing playlists uses INSERT OR REPLACE by `(account_id, service_playlist_id)`. Tracks are re-linked but positions may shift.
3. **Playlist tracks vs library entries**: Tracks in playlists are NOT automatically added to `library_entries`. A track can exist in a playlist but not appear in the main library view.
4. **Local playlists (account_id=NULL)**: The schema says `account_id NOT NULL` but `create_playlist` in library.rs may set it to NULL for locally-created playlists. This violates the constraint if not handled.
5. **Position tracking**: `position` is computed as `track_offset + index`. Re-importing doesn't clear existing tracks, so positions may become stale if the playlist was reordered on the source service.
