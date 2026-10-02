# Metadata Enrichment

**Estado de revisión:** 2026-10-02 — narrativa re-derivada del código (ítem 9.1 del plan post-auditoría, DO-4): el flujo de Spotify Audio Features se retiró del código (S68/F4-1) y este documento ya no lo describe. Gate de rutas: `scripts/tests/test_docs_consistency.py` — todas las rutas citadas existen o están declaradas como eliminadas.

**Último cambio:** ítem 9.1 del plan post-auditoría (2026-10-02) — re-derivación del flujo on-demand, del worker de fondo y de las fuentes tras la retirada de Spotify Audio Features. Antes: S199 — 2026-08-25 — archivos: `commands/metadata.rs`, `commands/tools.rs`, `main.rs`, `ui/src/views/MetadataView.vue`
**Leer antes de modificar:** `services/lastfm.rs`, `services/musicbrainz.rs`, `models.rs`
**Archivos core:**
- `src-tauri/src/commands/enrichment.rs` (325 lines)
- `src-tauri/src/enrichment_worker.rs` (297 lines) — worker de fondo
- `src-tauri/src/services/enrichment.rs` (`EnrichmentEngine`, el motor que invoca el worker)
- `src-tauri/src/services/musicbrainz.rs` (1453 lines)
- `src-tauri/src/services/lastfm.rs` (193 lines)
- `src-tauri/src/services/rate_limiter.rs` (555 lines)
- `src-tauri/src/services/tempo_analyzer.rs` — escritura de `bpm`/`musical_key`/`energy` por análisis local
- `src-tauri/src/commands/library.rs` (`enrich_metadata_musicbrainz`)
- `src-tauri/src/commands/metadata.rs` (`fetch_missing_cover_art`)
- `src-tauri/src/main.rs` (spawn del worker: `EnrichmentWorker`)
- `migrations/0018_metadata_enrichment.sql`
- `migrations/0021_metadata_preferences.sql`
- `migrations/0023_metadata_enrichment_flags.sql`
- `migrations/0047_enrichment_progress.sql` (cola del worker)

---

## Flujo Completo

### On-Demand Enrichment (UI-triggered)

```
FRONTEND → invoke("enrich_track", { track_id })
    │
    ▼
commands/enrichment.rs::enrich_track()
    │
    ├── 1. MusicBrainz: MusicBrainzClient::lookup_by_isrc(isrc) → musicbrainz_id
    │     Condition: isrc not empty AND musicbrainz_id IS NULL
    │     Writes: tracks.musicbrainz_id
    │
    └── 2. Last.fm Genre: LastFmClient::get_track_tags(artist, title) → genre, subgenre
          Condition: genre IS NULL AND artist not empty AND hay API key resuelta
          Writes: tracks.genre, tracks.subgenre
```

**No hay audio features.** S68 retiró el endpoint y F4-1 (`aa806b8`) borró el
pipeline; `enrich_track` lo deja escrito en el código como
`// _spotify_id: S68 retiró audio-features`. `bpm`, `musical_key` y `energy` ya no
llegan de Spotify: los escribe el motor (`services/enrichment.rs`) con el metadato
del servicio de origen y el análisis local de ritmo (`services/tempo_analyzer.rs`).

### Batch Enrichment (UI-triggered)

| Command | Source | Batch Size | Limit |
|---------|--------|-----------|-------|
| `enrich_metadata_musicbrainz` | MusicBrainz API | 20 ISRC por `batch_lookup_by_isrc`, con sleep de 1100 ms entre lotes | vía param (default 100000) |
| `enrich_genre_lastfm` | Last.fm API | 1 por request | 500 tracks |
| `fetch_missing_cover_art` (S199) | MusicBrainz ISRC → release-group → Cover Art Archive `front-500` (HEAD verificado antes de persistir) | 1 álbum por lookup, rate-limited por el cliente MB | vía param (default 100, clamp 1–500) |
| `start_library_enrichment` | motor local (`services/incremental_enrichment.rs`) | por lotes, con progreso por callback | vía param |
| `write_text_file` (S199) | — (persistencia de exports del frontend: letras LRC/TTML/TXT y metadata JSON) | — | rechaza ruta o contenido vacíos |

`enrich_spotify_audio_features` **no existe**: se retiró con S68 y su comando no está
registrado en el `invoke_handler` de `src-tauri/src/main.rs`.

### Background Enrichment Worker (`src-tauri/src/enrichment_worker.rs`)

El worker es de **una sola fuente: MusicBrainz**. `EnrichmentWorker::run()` (spawn en
`src-tauri/src/main.rs`) hace:

```
loop (mientras no esté detenido):
    wait_if_paused()                          ← pausado por la UI (pause_enrichment_worker)
    process_next_track()                      ← 1 pista por iteración
        │
        ├── SELECT la pista pendiente (ep.status IS NULL | 'pending' | ('failed' AND retry_count < 3))
        │     sobre enrichment_progress con service = 'all', ORDER BY t.id ASC LIMIT 1
        ├── INSERT/UPDATE enrichment_progress → status='in_progress'
        ├── emite "syncify:enrichment_event" + "background-enrichment-status" (type "musicbrainz")
        ├── rate_limiter.acquire("musicbrainz")        ← permiso compartido, perfil MB
        ├── EnrichmentEngine::resolve_track_metadata(artist, album, title, isrc)
        ├── UPDATE tracks SET musicbrainz_id = ?, enrichment_status='enriched', enriched_at=…
        └── UPDATE enrichment_progress → status='completed'
    sleep 5s si no había trabajo · sleep 2s si la iteración falló
```

Lo que el worker **no** hace, frente a lo que decía esta narrativa antes:

- **No lee `metadata_preferences`.** No hay `enable_musicbrainz` ni `enable_lastfm`
  en su código: los flags se guardan y se editan en Settings → Metadata, pero ningún
  camino de enriquecimiento los consulta (grep de `metadata_preferences` en
  `src-tauri/src`: solo `commands/settings.rs` y el arranque comentado de `main.rs`).
- **No llama a Spotify ni a Last.fm.** Ni audio features ni género: la única escritura
  de metadatos del bucle es `tracks.musicbrainz_id` (+ `enrichment_status`/`enriched_at`).
- **No marca `'NOT_FOUND'`.** Ese centinela lo escribe el comando por lotes
  `enrich_metadata_musicbrainz` (`commands/library.rs`), no el worker.

---

## Fuentes de Metadata

| Source | Data Provided | Rate Limit | Env Var Required |
|--------|--------------|------------|------------------|
| **MusicBrainz** | MBID, artist credits, releases, release groups | 1 req/1.1s por cliente (`Mutex<Instant>` en `MusicBrainzClient::rate_limit`) + `RateLimiter.acquire("musicbrainz")` en el worker | None |
| **Last.fm** | Genre tags, subgenre tags | Standard Last.fm limits | API key: settings KV `lastfm_api_key` (UI: Metadata → Auto-Fix → Last.fm, S200) o `LASTFM_API_KEY` env como fallback |

**Retirado**: Spotify Audio Features (S68/F4-1). Ya no es una fuente, ni en el worker
ni en los comandos.

### Prioridad de Fuentes

1. **MusicBrainz** runs first (ISRC → MBID lookup)
2. **Last.fm Genre** runs second (artist+title lookup), solo en los caminos bajo demanda
   (`enrich_track`, `enrich_genre_lastfm`) — el worker de fondo no la ejecuta


### MusicBrainz Client Details (`services/musicbrainz.rs`)

- **Rate limiter**: `std::sync::Mutex<Instant>` → enforces 1.1s between requests
- `lookup_by_isrc(isrc)` → recording query → returns first match
- `batch_lookup_by_isrc(isrcs)` → OR query → returns HashMap<id, Recording>
- `search_recordings(title, artist, album, limit)` → Lucene query with escaping
- `get_recording_details(mbid)` → genres + ISRCs via inc=genres+isrcs
- `search_artist` / `get_artist_external_ids` / `search_release_by_barcode` /
  `search_release_by_title_and_artist` / `get_release_with_tracks` — los usa el motor
  de enriquecimiento y el de sincronización, no los comandos de este documento
- NOT_FOUND marking: `musicbrainz_id = 'NOT_FOUND'` (y `'MISMATCH'`) los escribe el
  comando por lotes `enrich_metadata_musicbrainz`; el worker de fondo no los marca.
  `enrich_tracks(db, limit)` ya no existe en el cliente.

### Enrichment Flags (metadata_preferences table)

| Flag | Column | Default | Estado real |
|------|--------|---------|-------------|
| MusicBrainz | `enable_musicbrainz` | 1 (true) | Se persiste y se edita en Settings → Metadata; **ningún camino de enriquecimiento lo lee** |
| Last.fm | `enable_lastfm` | 0 (false) | Ídem: se persiste, no condiciona la ejecución |
| AcoustID | `enable_acoustid` | 0 (false) | Ídem |

Los flags viven en `metadata_preferences` y los leen/escriben solo
`get_metadata_preferences` / `update_metadata_preferences`
(`src-tauri/src/commands/settings.rs`). El worker de fondo no los consulta, así que
**no sirven para desactivar ni para limitar nada**: apagarlos no cambia lo que se
enriquece. El control efectivo del worker es `pause_enrichment_worker` /
`resume_enrichment_worker` (estado en memoria, no persistente).

---

## Campos críticos en DB

| Table | Column | Written By | Notes |
|-------|--------|-----------|-------|
| `tracks` | `musicbrainz_id` | MusicBrainz enrichment | UUID, o `'NOT_FOUND'` / `'MISMATCH'` como centinelas |
| `tracks` | `bpm` | `EnrichmentEngine::apply_to_database` (metadato del servicio de origen), `services/tempo_analyzer.rs` (análisis local) e `services/incremental_enrichment.rs` (provenance `local_analysis`) | float |
| `tracks` | `musical_key` | los mismos tres caminos | key inicial (p. ej. "C Major") |
| `tracks` | `energy` | los mismos tres caminos | 0.0–1.0 float |
| `tracks` | `danceability`, `valence`, `acousticness`, `instrumentalness` | **nadie**: las crea `migrations/0018_metadata_enrichment.sql` y el motor las maneja en memoria, pero ninguna sentencia las persiste | columnas heredadas de Spotify Audio Features, retiradas con S68 |
| `tracks` | `genre`, `subgenre` | Last.fm | Text tags |
| `tracks` | `enrichment_status` | enriquecimiento | `'enriched'` (worker), `'complete'` (motor), `'partial'`, `'error'`, `'manual'` |
| `tracks` | `enriched_at` | enriquecimiento | CURRENT_TIMESTAMP |
| `enrichment_progress` | `status`, `retry_count`, `last_error` | worker de fondo | cola real del bucle: `'pending'`, `'in_progress'`, `'completed'`, `'failed'` |
| `metadata_preferences` | `enable_musicbrainz/lastfm/acoustid` | UI settings | boolean flags persistidos, sin efecto de ejecución |

---

## Eventos Tauri emitidos

| Event | Emitted By | Payload |
|-------|-----------|---------|
| `enrichment-progress` / `enrichment_progress` | `commands/enrichment.rs` (`enrich_genre_lastfm`, enriquecimiento incremental) | `{ type, status, current, total, message }` |
| `enrichment-progress` | `commands/library.rs` (`enrich_metadata_musicbrainz`) | `{ status, total, current, enriched, failed, currentTrack }` |
| `syncify:enrichment_event` | worker de fondo | `{ track_id, title, artist, service, status, message, timestamp }` |
| `background-enrichment-status` | worker de fondo | `{ type: "musicbrainz", status, message, track_id, title, artist, enriched, processed }` |

type values emitidos: `"lastfm_genre"`, `"musicbrainz"`, `"idle"`
status values emitidos: `"started"`, `"progress"`, `"processing"`, `"completed"`,
`"running"`, `"in_progress"`, `"error"`, `"partial"`, `"skipped"`, `"waiting"`

---

## Puntos de ruptura conocidos

1. **MusicBrainz rate limiter is per-client**: cada `MusicBrainzClient` lleva su propio
   `Mutex<Instant>`; el worker además pide permiso al `RateLimiter` compartido, pero los
   comandos por lotes construyen el suyo. El uso concurrente puede llevar a 503.
2. **NOT_FOUND sentinel**: `musicbrainz_id = 'NOT_FOUND'` evita la reconsulta pero es
   frágil: si MusicBrainz añade el recording después, no se reintenta.
3. **Los flags de `metadata_preferences` no gobiernan nada**: se pueden apagar en la UI
   y el enriquecimiento sigue igual (ver la tabla de arriba).
4. **El worker solo escribe `musicbrainz_id`**: `enrichment_status` pasa a `'enriched'`
   aunque el resto del metadato (género, país, sello, bpm) siga sin resolver; quien
   resuelve el resto es `EnrichmentEngine` por otras rutas.
5. **La rama `'failed' AND retry_count < 3` de la consulta del worker es inalcanzable**:
   nada escribe `status='failed'` en `enrichment_progress` ni incrementa `retry_count`
   (los únicos `retry_count + 1` del árbol son de `download_queue`). El efecto real es el
   contrario: si `process_next_track()` devuelve error, la fila se queda en
   `'in_progress'`, el `SELECT` deja de devolver esa pista y el bucle sigue con la
   siguiente — la pista se abandona en silencio, sin `last_error` ni reintento.
6. ~~**Last.fm requires env var**: If `LASTFM_API_KEY` is not set, genre enrichment silently skips. No user-visible error.~~ **RESUELTO S200**: la key se configura desde la UI (settings KV, con input y estado en la tarjeta Last.fm de la tab Metadata); el resolver es BD→env y devuelve error accionable si no hay ninguna.

