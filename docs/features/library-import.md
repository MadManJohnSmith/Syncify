# Library Import

**Último cambio:** S42 — 2026-04-01 — archivos: `service.rs`, `import_cache.rs`, `services/qobuz.rs`
**Leer antes de modificar:** `commands/auth.rs`, `crypto.rs`
**Archivos core:**
- `src-tauri/src/commands/service.rs` (L238–1800)
- `src-tauri/src/import_cache.rs`
- `src-tauri/src/services/{spotify,qobuz,tidal,deezer,soundcloud,apple_music}.rs`

---

## Flujo Completo

```
FRONTEND → invoke("import_spotify_library") / invoke("import_service", {service})
    │
    ▼
PHASE 1: load_service_credentials(db, name) → decrypt → JSON creds
    │ [Spotify: get_or_refresh_spotify_token() → fresh bearer]
    ▼
PHASE 2: Fetch first page (limit=1) → get total → emit "import-progress" started
    │
    ▼
PHASE 3: Parallel/sequential page fetching
    │ Spotify: 4 pages concurrent × 50 tracks
    │ Qobuz: sequential × 500
    │ Others: sequential × 50–100
    ▼
PHASE 4: Per-track processing via ImportCache
    │ 1. Skip if no album / empty data / duration=0
    │ 2. cache.get_or_create_artist(name) → artist_id
    │ 3. cache.get_or_create_album(key, name, ...) → album_id
    │ 4. get_or_create_track(track, isrc, album_id) → track_id
    │ 5. INSERT OR IGNORE track_artists (primary + featured)
    │ 6. INSERT OR IGNORE library_entries (account_id, track_id)
    │ 7. INSERT OR REPLACE track_sources
    │ 8. Emit progress every 50 tracks
    ▼
PHASE 5: Emit "import-complete" → return ImportResult { imported, skipped }
```

### Import Functions

| Function                      | Line  | Concurrency      | Page Size |
|------------------------------|-------|-------------------|-----------|
| `import_spotify_library`     | L238  | 4 pages parallel  | 50        |
| `import_spotify_playlists`   | L488  | Sequential        | 50/100    |
| `import_qobuz_library`      | L860  | Sequential        | 500       |
| `import_tidal_library`       | L1048 | Sequential        | 100       |
| `import_deezer_library`      | L1211 | Sequential        | 100       |
| `import_soundcloud_library`  | L1367 | Sequential        | 50        |
| `import_apple_music_library` | L1515 | Sequential        | 100       |
| `import_service` (dispatcher)| L1705 | Delegates         | N/A       |

---

## ImportCache API (`import_cache.rs`, 185 lines)

```rust
struct ImportCache {
    artists: HashMap<String, i64>,     // name → artist_id
    albums: HashMap<String, i64>,      // "artist_id:album_name" → album_id
    service_ids: HashMap<String, i64>, // service_name → service_id
}
```

| Method | Strategy |
|--------|----------|
| `get_or_create_artist(db, name)` | Cache → SELECT LOWER → INSERT OR IGNORE → SELECT |
| `get_or_create_album(db, lock, key, ...)` | Cache → SELECT by title+artist → INSERT OR IGNORE → link album_artists |
| `get_service_id(db, name)` | Cache → SELECT FROM services |
| `stats()` | Returns (artists_cached, albums_cached) |

Lock parameter `_album_lock` kept for API compat but unused (lock-free design).

---

## Campos críticos en DB

| Table | Key Columns | Written By |
|-------|-------------|------------|
| `tracks` | id, title, isrc, duration_ms, track_number | import |
| `artists` | id, name | ImportCache (case-insensitive dedup) |
| `albums` | id, title, release_date, cover_art_url | ImportCache |
| `album_artists` | album_id, artist_id, is_primary | ImportCache |
| `track_artists` | track_id, artist_id, role | import ("primary"/"featured") |
| `library_entries` | account_id, track_id, added_at, is_liked | import (UNIQUE constraint) |
| `track_sources` | track_id, service_id, service_track_id | import (cross-service link) |
| `playlists` | account_id, service_playlist_id, name | playlist import |
| `playlist_tracks` | playlist_id, track_id, position | playlist import |

---

## Eventos Tauri emitidos

| Event | Payload |
|-------|---------|
| `import-progress` | `{ service, status: "started"\|"progress", current, total, message }` |
| `import-complete` | `{ service, imported, skipped, message }` |

---

## Puntos de ruptura conocidos

1. **ImportCache is per-call**: Not shared across services. Partial failure + reimport may create duplicate albums.
2. **Parallel page fetching**: Spotify uses `futures::join_all` × 4 pages. Rate-limit failures are logged but skipped.
3. **ISRC dedup**: Tracks without ISRC may appear as duplicates across services.
4. **Playlist tracks ≠ library**: `import_spotify_playlists` does NOT add tracks to `library_entries`. Playlist-only tracks won't show in main library.
5. **DB retry**: `track_artists` INSERT retries up to 3× with 100ms×retry backoff for busy DB.
