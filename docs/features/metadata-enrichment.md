# Metadata Enrichment

**Estado de revisión:** 2026-09-05 — pendiente de revalidación contra la implementación actual. Úsalo como referencia de diseño y verifica el código fuente antes de confiar en los detalles.

**Último cambio:** S199 — 2026-08-25 — archivos: `commands/metadata.rs`, `commands/tools.rs`, `main.rs`, `ui/src/views/MetadataView.vue`
**Leer antes de modificar:** `services/lastfm.rs`, `services/spotify.rs` (audio features), `models.rs`
**Archivos core:**
- `src-tauri/src/commands/enrichment.rs` (453 lines)
- `src-tauri/src/services/musicbrainz.rs` (388 lines)
- `src-tauri/src/services/lastfm.rs` (6120 bytes)
- `src-tauri/src/services/rate_limiter.rs` (7313 bytes)
- `src-tauri/src/main.rs` L391–686 (background enrichment worker)
- `migrations/0018_metadata_enrichment.sql`
- `migrations/0021_metadata_preferences.sql`
- `migrations/0023_metadata_enrichment_flags.sql`

---

## Flujo Completo

### On-Demand Enrichment (UI-triggered)

```
FRONTEND → invoke("enrich_track", { track_id })
    │
    ▼
enrichment.rs::enrich_track()
    │
    ├── 1. MusicBrainz: lookup_by_isrc(isrc) → musicbrainz_id
    │     Condition: isrc exists AND musicbrainz_id IS NULL
    │
    ├── 2. Spotify Audio Features: get_audio_features_batch([spotify_id])
    │     Condition: Spotify account connected AND bpm IS NULL
    │     Writes: bpm, musical_key, energy, danceability, valence, acousticness, instrumentalness
    │
    └── 3. Last.fm Genre: get_track_tags(artist, title) → genre, subgenre
          Condition: genre IS NULL AND artist not empty
```

### Batch Enrichment (UI-triggered)

| Command | Source | Batch Size | Limit |
|---------|--------|-----------|-------|
| `enrich_spotify_audio_features` | Spotify API | 100 per batch | 1000 tracks |
| `enrich_genre_lastfm` | Last.fm API | 1 per request | 500 tracks |
| `enrich_metadata_musicbrainz` | MusicBrainz API | 1 per request | via param |
| `fetch_missing_cover_art` (S199) | MusicBrainz ISRC → release-group → Cover Art Archive `front-500` (HEAD verificado antes de persistir) | 1 álbum por lookup, rate-limited por el cliente MB | via param (default 100) |
| `write_text_file` (S199) | — (persistencia de exports del frontend: letras LRC/TTML/TXT y metadata JSON) | — | rechaza ruta o contenido vacíos |

### Background Enrichment Worker (main.rs L391–686)

```
Startup → sleep 30s → enrichment loop:
    │
    ├── Load enrichment_flags from metadata_preferences
    │
    ├── MusicBrainz (if enabled):
    │     SELECT tracks WHERE isrc NOT NULL AND musicbrainz_id IS NULL LIMIT 100
    │     → client.enrich_tracks(db, 100)
    │     → UPDATE tracks SET musicbrainz_id = ? (or 'NOT_FOUND')
    │
    ├── Spotify Audio Features (if account exists):
    │     SELECT tracks JOIN track_sources WHERE bpm IS NULL LIMIT 100
    │     → spotify_client.get_audio_features_batch(spotify_ids)
    │     → UPDATE tracks SET bpm, musical_key, energy, ...
    │
    ├── Last.fm Genre (if enabled + API key):
    │     SELECT tracks WHERE genre IS NULL LIMIT 50
    │     → lastfm_client.get_track_tags(artist, title)
    │     → UPDATE tracks SET genre, subgenre
    │
    └── Sleep 300s (5 minutes) → repeat
```

---

## Fuentes de Metadata

| Source | Data Provided | Rate Limit | Env Var Required |
|--------|--------------|------------|------------------|
| **MusicBrainz** | MBID, artist credits, releases, release groups | 1 req/1.1s (enforced) | None |
| **Spotify** | BPM, key, energy, danceability, valence, acousticness, instrumentalness | Standard Spotify limits | Spotify account connected |
| **Last.fm** | Genre tags, subgenre tags | Standard Last.fm limits | API key: settings KV `lastfm_api_key` (UI: Metadata → Auto-Fix → Last.fm, S200) o `LASTFM_API_KEY` env como fallback |
| **AcoustID** | Audio fingerprint matching | Standard AcoustID limits | `ACOUSTID_API_KEY` (flag exists, not implemented in background) |

### Prioridad de Fuentes

1. **MusicBrainz** runs first (ISRC → MBID lookup)
2. **Spotify Audio Features** runs second (requires Spotify track linkage)
3. **Last.fm Genre** runs third (artist+title lookup)
4. **AcoustID** flag exists in DB but not implemented in background worker

### MusicBrainz Client Details (`musicbrainz.rs`)

- **Rate limiter**: `std::sync::Mutex<Instant>` → enforces 1.1s between requests
- `lookup_by_isrc(isrc)` → recording query → returns first match
- `batch_lookup_by_isrc(isrcs)` → OR query → returns HashMap<id, Recording>
- `search_recordings(title, artist, album, limit)` → Lucene query with escaping
- `get_recording_details(mbid)` → genres + ISRCs via inc=genres+isrcs
- `enrich_tracks(db, limit)` → batch SELECT + per-track lookup + UPDATE
- NOT_FOUND marking: `musicbrainz_id = 'NOT_FOUND'` to avoid re-checking

### Enrichment Flags (metadata_preferences table)

| Flag | Column | Default | Controls |
|------|--------|---------|----------|
| MusicBrainz | `enable_musicbrainz` | 1 (true) | Background MB enrichment |
| Last.fm | `enable_lastfm` | 0 (false) | Background genre enrichment |
| AcoustID | `enable_acoustid` | 0 (false) | Not implemented yet |

Loaded by `load_enrichment_flags(db)` in main.rs on each enrichment cycle.

---

## Campos críticos en DB

| Table | Column | Written By | Notes |
|-------|--------|-----------|-------|
| `tracks` | `musicbrainz_id` | MusicBrainz enrichment | UUID or 'NOT_FOUND' |
| `tracks` | `bpm` | Spotify audio features | float |
| `tracks` | `musical_key` | Spotify audio features | e.g. "C Major" |
| `tracks` | `energy`, `danceability`, `valence` | Spotify | 0.0–1.0 float |
| `tracks` | `acousticness`, `instrumentalness` | Spotify | 0.0–1.0 float |
| `tracks` | `genre`, `subgenre` | Last.fm | Text tags |
| `tracks` | `enrichment_status` | enrichment | 'spotify_done' |
| `tracks` | `enriched_at` | enrichment | CURRENT_TIMESTAMP |
| `metadata_preferences` | `enable_musicbrainz/lastfm/acoustid` | UI settings | boolean flags |

---

## Eventos Tauri emitidos

| Event | Emitted By | Payload |
|-------|-----------|---------|
| `enrichment-progress` | enrichment.rs (batch) | `{ type, status, current, total, message }` |
| `background-enrichment-status` | main.rs worker | `{ type, status, pending/enriched, message }` |

type values: `"spotify_audio_features"`, `"lastfm_genre"`, `"musicbrainz"`, `"spotify"`, `"lastfm"`, `"idle"`
status values: `"started"`, `"progress"`, `"completed"`, `"running"`, `"error"`, `"skipped"`, `"waiting"`

---

## Puntos de ruptura conocidos

1. **MusicBrainz rate limiter is per-client**: Background worker and on-demand enrichment create separate clients — no shared rate limit. Concurrent use may hit 503.
2. **NOT_FOUND sentinel**: Setting `musicbrainz_id = 'NOT_FOUND'` prevents re-lookup but is fragile. If MB adds the recording later, it won't be retried.
3. **Spotify token refresh in enrichment**: Background worker uses the stored token without refreshing. If expired, the entire batch fails silently.
4. ~~**Last.fm requires env var**: If `LASTFM_API_KEY` is not set, genre enrichment silently skips. No user-visible error.~~ **RESUELTO S200**: la key se configura desde la UI (settings KV, con input y estado en la tarjeta Last.fm de la tab Metadata); el resolver es BD→env y devuelve error accionable si no hay ninguna.
5. **enrich_before_download is a no-op**: The function at L417 fetches track info but does NOT actually enrich — it just counts tracks. Misleading name.
