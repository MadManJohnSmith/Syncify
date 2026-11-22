# Playlist System

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-11-21 — PL02 (#155) sobre el modelo de ocurrencias de PL00 (#153): añadir una pista ya presente crea otra ocurrencia, quitar quita una, reordenar mapea cada entrada a su ocurrencia y persiste posiciones contiguas 1..N, `track_count` cuenta ocurrencias y el import de archivo conserva repeticiones. Antes: PL01 (#154) — CRUD local, identidad estable y borrado con tombstone. Sin verificar: importaciones contra servicios reales en vivo.

**Último cambio:** 2026-11-21 — ocurrencias y posiciones de PL02 (#155). Antes: 2026-11-18 — CRUD local y borrado de PL01 (#154). Antes: re-revisión 2026-10-25 — símbolos en lugar de números de línea y razonamiento de SoundCloud corregido. Antes: ítem 9.1 del plan post-auditoría (2026-10-02) — estado real de la importación de playlists por servicio. Antes: S40 — 2026-03-30 — archivos: `migrations/0006_playlists.sql`, `commands/library.rs`, `commands/service.rs`
**Leer antes de modificar:** `commands/tools.rs` (Python bridge playlist commands), `scripts/playlist_bridge.py`
**Archivos core:**
- `migrations/0006_playlists.sql` (schema), `migrations/0048_playlist_sync.sql` (identidad remota `playlist_sources`) y `migrations/0110_pl00_local_playlist_model.sql` (modelo local: `local_uuid`, tombstone `deleted_at`, unicidad por posición)
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
    │        → si la fila encontrada tiene tombstone (deleted_at), devuelve None:
    │          el usuario la borró y el sync no la recrea (PL01 #154)
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
    → library.rs: INSERT INTO playlists (account_id NULL, local_uuid UUID,
      service_playlist_id = 'local_' || name, name, ...)
      (el nombre no es único: la cuenta NULL lo exime del índice UNIQUE; la
      identidad estable es local_uuid — PL00 #153, PL01 #154)

FRONTEND → invoke("update_playlist", { id, name?, description? })
    → playlists.rs: UPDATE solo name/description (+ updated_at) y solo si
      deleted_at IS NULL; local_uuid, service_playlist_id y account_id no se
      tocan (renombrar no cambia la identidad — PL01 #154)

FRONTEND → invoke("delete_playlist", { id })
    → playlists.rs: una local (sin cuenta o smart) se DELETE de verdad; una
      importada deja tombstone (deleted_at), borra sus playlist_tracks y
      conserva playlist_sources (la identidad remota que el sync consulta y
      con la que no la recrea — PL01 #154)

FRONTEND → invoke("add_to_playlist", { playlist_id, track_ids })
    → library.rs: una ocurrencia por track_id al final de la lista (repetir la
      pista la vuelve a añadir; posiciones MAX+1 y track_count de ocurrencias
      en la misma transacción — PL02 #155)

FRONTEND → invoke("get_playlists")
    → library.rs: SELECT ... WHERE deleted_at IS NULL ORDER BY name

FRONTEND → invoke("remove_from_playlist", { playlist_id, track_ids })
    → playlists.rs: cada entrada quita UNA ocurrencia (la de menor posición de
      esa pista) y recompacta posiciones 1..N + track_count en la misma
      transacción (PL02 #155)

FRONTEND → invoke("reorder_playlist_tracks", { playlist_id, positions })
    → playlists.rs: el payload es el orden completo; la k-ésima entrada de una
      pista mueve su k-ésima ocurrencia y se persiste 1..N (PL02 #155)

FRONTEND → invoke("get_local_playlist_tracks", { playlist_id })
    → library.rs: SELECT tracks JOIN playlist_tracks WHERE playlist_id=?
      (una fila por ocurrencia, con su position)
```

Borrar una cuenta elimina sus playlists importadas (`DELETE FROM playlists
WHERE account_id = ?` en `perform_delete_account`, con sus fuentes) y nunca
las locales: estas no tienen cuenta (`account_id IS NULL`) y sobreviven al
borrado. La confirmación del borrado es el diálogo in-app R13 de la UI.

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

## Schema (0006 + 0110)

### playlists

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | INTEGER | PK AUTOINCREMENT | |
| `account_id` | INTEGER | NULL FK → accounts(id) ON DELETE CASCADE | Las locales no tienen cuenta (`NULL`, PL00 #153); las importadas sí |
| `service_playlist_id` | TEXT | NULL | Identidad remota; en las locales es la etiqueta histórica `local_<nombre>` |
| `name` | TEXT | NOT NULL | No es identidad y puede repetirse (PL01 #154) |
| `description` | TEXT | | Optional description |
| `owner_name` | TEXT | | Playlist owner |
| `is_public` | INTEGER | DEFAULT 1 | Boolean flag |
| `is_collaborative` | INTEGER | DEFAULT 0 | Boolean flag |
| `image_url` | TEXT | | Cover art URL |
| `track_count` | INTEGER | DEFAULT 0 | From service metadata |
| `last_synced` | TEXT | | Last sync timestamp |
| `local_uuid` | TEXT | UNIQUE parcial (no NULL) | Identidad estable de las locales; renombrar no la toca (0110) |
| `deleted_at` | TEXT | NULL | Tombstone de las importadas borradas (0110/PL01) |
| `created_at` | TEXT | DEFAULT CURRENT_TIMESTAMP | |
| `updated_at` | TEXT | DEFAULT CURRENT_TIMESTAMP | |

UNIQUE constraint: `(account_id, service_playlist_id)` — solo gobierna filas
con cuenta: SQLite trata los NULL como distintos, por eso dos locales pueden
compartir nombre. La identidad de las locales es `local_uuid` (índice único
parcial).

### playlist_tracks

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | INTEGER | PK AUTOINCREMENT | |
| `playlist_id` | INTEGER | NOT NULL FK → playlists(id) ON DELETE CASCADE | |
| `track_id` | INTEGER | NOT NULL FK → tracks(id) ON DELETE CASCADE | |
| `position` | INTEGER | NOT NULL DEFAULT 0 | Track order in playlist |
| `added_at` | TEXT | | When track was added |

UNIQUE constraint: `(playlist_id, position)` (0110: posiciones canónicas
1..N; la misma pista puede repetirse en la playlist)

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
| `playlist_tracks` | `playlist_id`, `track_id` | import / add_to_playlist | no es único: la misma pista se repite; lo único es `(playlist_id, position)` (PL02 #155) |

---

## Ocurrencias de la misma pista (PL02 #155)

Una playlist es una secuencia de ocurrencias, no un conjunto de pistas: la
misma pista puede aparecer varias veces y cada fila de `playlist_tracks` es una
ocurrencia con su propia `position`.

| Acción | Semántica |
|---|---|
| Añadir (`add_to_playlist`) | crea siempre una ocurrencia nueva al final; repetir la pista la repite |
| Quitar (`remove_from_playlist`) | quita UNA ocurrencia por entrada del payload (la de menor posición de esa pista); con la pista repetida en el payload, cada repetición consume la siguiente |
| Reordenar (`reorder_playlist_tracks`) | el payload es el orden completo; cada entrada mueve su ocurrencia y las posiciones quedan contiguas 1..N |
| `track_count` | número de ocurrencias (`COUNT(*)` de `playlist_tracks`), no de pistas distintas |
| Import M3U/CSV/TXT | conserva las repeticiones del archivo tal cual, en orden |

Posiciones canónicas: **1..N** contiguas, como fija 0110 y recompacta
`recompact_playlist_tracks_in_conn` (el único sitio que asigna posiciones). La
UI numera las filas desde 1 al mostrarlas; el payload del reorden usa índices
1..N que el backend reescribe como secuencia contigua.

La acción por lotes de biblioteca (`library_actions::apply_add_to_playlist`)
sigue siendo idempotente por pista y reporta `already_in_playlist` en su
preview: es una superficie distinta (selección masiva con preview) y no crea
ocurrencias. Alinear esa acción con la semántica de ocurrencias queda fuera de
PL02.

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
5. **Position tracking**: solo las rutas que recomprimen al final lo garantizan. `import_spotify_playlists` llama a `recompact_playlist_positions(playlist_id)` tras cada playlist y deja las posiciones en 1..N; las que no (p. ej. `AppleMusicClient::import_playlists`, que escribe `position` desde el índice de la relación) dejan la posición como vino del servicio, así que un reordenado en el origen se propaga como desalineación local. Las operaciones del usuario (alta, quitar, reordenar) sí dejan 1..N y `track_count` al día (PL02 #155).
