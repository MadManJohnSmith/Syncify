# Library Import

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-10-25 — re-revisión contra el árbol (verificación dirigida de símbolos y comportamiento): números de línea de `commands/service.rs` sustituidos por símbolos (el fichero ronda las 9.400 líneas), `import_qobuz_library` ya no pagina ×500 — delega en el motor unificado `perform_sync_service_with_emitter` (S128B/TASK-108) —, page sizes corregidos (Deezer 50, SoundCloud 100 vía `next_href`, Tidal 10 en favoritos) y caché de álbumes actualizada a `get_or_create_album_with_compilation` (Various Artists canónico + `is_compilation`). Sigue sin verificar en escritorio real la ejecución de imports contra servicios vivos (GUI, red): véase README §"Verification and limitations".

**Último cambio:** 2026-10-25 — re-revisión de comportamiento contra el árbol — archivos: `commands/service.rs`, `import_cache.rs`, `services/import_pagination.rs`, `services/soundcloud.rs`, `services/tidal.rs`. Previo: S42 — 2026-04-01 — archivos: `service.rs`, `import_cache.rs`, `services/qobuz.rs`
**Leer antes de modificar:** `commands/auth.rs`, `crypto.rs`
**Archivos core:**
- `src-tauri/src/commands/service.rs` (comandos de import + motor unificado)
- `src-tauri/src/import_cache.rs`
- `src-tauri/src/services/import_pagination.rs` (paginación y normalización de `added_at`)
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
    │ Qobuz: delega en el motor unificado perform_sync_service_with_emitter
    │ Tidal: delega en TidalClient (favoritos, playlists, álbumes, artistas)
    │ Deezer: sequential × 50 · SoundCloud: × 100 (next_href) · Apple Music: × 100
    ▼
PHASE 4: Per-track processing via ImportCache
    │ 1. Skip if no album / empty data / duration=0
    │ 2. cache.get_or_create_artist(name) → artist_id
    │ 3. cache.get_or_create_album_with_compilation(key, name, ...) → album_id
    │ 4. get_or_create_track(track, isrc, album_id) → track_id
    │ 5. INSERT OR IGNORE track_artists (primary + featured)
    │ 6. INSERT INTO library_entries (account_id, track_id, is_liked=1,
    │    added_at normalizado)
    │ 7. INSERT OR REPLACE track_sources
    │ 8. Emit progress every 50 tracks
    ▼
PHASE 5: Emit "import-complete" → return ImportResult { imported, skipped }
```

### Import Functions

Todas viven en `commands/service.rs`. Se citan por símbolo (el fichero ronda las
9.400 líneas y los números de línea rotan con cada commit).

| Function                      | Concurrency      | Page Size | Notas |
|------------------------------|------------------|-----------|-------|
| `import_spotify_library`     | 4 páginas paralelas (`futures::future::join_all`) | 50 | |
| `import_spotify_playlists`   | Sequential        | 50 (playlists) / 100 (tracks) | |
| `import_qobuz_library`      | Delega            | N/A | Motor unificado `perform_sync_service_with_emitter` (S128B/TASK-108); activa `prefs.purchases` |
| `import_qobuz_purchases`     | Delega            | N/A | Compras con `is_purchased = 1` (TASK-108) |
| `import_tidal_library`       | Delega en `TidalClient` | 10 en favoritos (S78) | Fases «Warp Speed»: `import_favorites` + `import_playlists` + `import_favorite_albums` + `import_favorite_artists` |
| `import_deezer_library`      | Sequential        | 50 | |
| `import_soundcloud_library`  | Delega en `sync_soundcloud_likes_with_engine` (vía `run_soundcloud_likes_import`) | 100 (likes, paginado por `next_href`) | Deduplicación canónica del motor |
| `import_apple_music_library` | Sequential (núcleo compartido `run_apple_music_library_import` con el brazo del motor unificado) | 100 | |
| `import_service` (dispatcher)| Delegates         | N/A | |

### Motor unificado y enriquecimiento

- **Vía principal:** `perform_sync_service_with_emitter` (`commands/service.rs`)
  es el camino de varios servicios. `import_qobuz_library` delega directamente en
  él (S128B/TASK-108); los comandos legacy de SoundCloud y Apple Music comparten
  núcleo con los brazos correspondientes del motor
  (`sync_soundcloud_likes_with_engine`, `run_apple_music_library_import`).
- **Enriquecimiento en línea:** durante el import se enriquece y persiste con
  `enrich_persist_with_locked_retry` (`commands/service.rs`), con reintento bajo
  lock para BD ocupada.
- **`added_at` normalizado:** `normalize_added_at`
  (`services/import_pagination.rs`) rellena valores ausentes/vacíos y sustituye
  epochs 1970 por una fecha válida antes de escribir `library_entries.added_at`.

---

## ImportCache API (`import_cache.rs`)

```rust
struct ImportCache {
    artists: HashMap<String, i64>,     // name → artist_id
    albums: HashMap<String, i64>,      // "artist_id:album_name" → album_id
    service_ids: HashMap<String, i64>, // service_name → service_id
    various_artists_id: Option<i64>,   // canonical Various Artists resuelto una vez
}
```

| Method | Strategy |
|--------|----------|
| `get_or_create_artist(db, name)` | Cache → SELECT LOWER → INSERT OR IGNORE → SELECT |
| `get_or_create_album_with_compilation(db, key, ...)` | Cache → SELECT by title+artist → INSERT OR IGNORE → link album_artists; canonical Various Artists (id 30698), marca `albums.is_compilation = 1` y deduplica álbumes compartidos bajo Various Artists |
| `get_or_create_various_artists(db)` | Resuelve y cachea el ID canónico de Various Artists |
| `get_service_id(db, name)` | Cache → SELECT FROM services |
| `stats()` | Returns (artists_cached, albums_cached) |

Fully lock-free design using SQLite ACID constraints.

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
