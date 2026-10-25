# Lyrics Resolution Matrix: 12 Configurable Sources

**Review stamp:** 2026-10-25 — rewritten against the tree: the engine no longer
runs a fixed 16-strategy cascade (the old name of this file,
`LYRICS_16_PROVIDER_MATRIX.md`, described that retired shape). The cascade is
now a catalogue of **12 user-configurable sources** with a persisted priority.
The per-provider network claims below are a **dated audit measurement**, not a
live guarantee.

The engine lives in `src-tauri/src/download/lyrics.rs`. This matrix describes
the current Rust engine only. The experimental prototype the original audit
cited alongside it is not in this repository: it lived under `workspace/`,
which `.gitignore` excludes, so no checkout and no CI job has ever contained
it. Neither has the `legacy/syncify-cli` route the early docs mentioned.

## 1. Architecture as it is today

- **Catalogue**: `LYRICS_SOURCE_IDS` in `lyrics.rs` lists exactly 12
  `provider_id`s. `fetch_source` dispatches by `provider_id` and
  `fetch_all_sources` iterates every *enabled* source in the user's saved
  order. There are no orphan fetchers and no hardcoded priority ladder.
- **User configuration**: each source's `enabled` flag and priority are
  persisted in `lyrics_provider_settings` (seeded/catalogued by
  `migrations/0013_lyrics_provider_settings.sql` and
  `migrations/0100_lyrics_provider_catalog.sql`) and edited from the UI via
  `ui/src/composables/useLyricsSettings.ts` and the lyrics settings panel.
  The catalogue migration also **retired Genius** ("retire Genius (no
  fetcher)"): it has no fetcher and no source id.
- **Shared config**: `migrations/0014_lyrics_config.sql` holds the
  language/format preferences that surround the cascade.
- A source whose id is not in `LYRICS_SOURCE_IDS` is skipped defensively
  (`sources.retain(... enabled && LYRICS_SOURCE_IDS.contains ...)`).

## 2. The 12 sources

| Source id | Fetcher (symbol) | Endpoint / mechanism | Credential | Output sync type |
| :--- | :--- | :--- | :--- | :--- |
| `apple_music_ttml` | `fetch_apple_music_ttml` | `amp-api.music.apple.com/v1/catalog/{sf}/songs/{id}/syllable-lyrics` | WebPlayKid Bearer token (auto-extracted) | `KARAOKE_WORD_SYNCED` |
| `spotify_color` | `fetch_spotify_lyrics` | `spclient.wg.spotify.com/color-lyrics/v2/track/{id}` | `SPOTIFY_SP_DC` session cookie (env var) | `KARAOKE_WORD_SYNCED` / `LINE_SYNCED` |
| `musixmatch_richsync` | `fetch_musixmatch_richsync` | `apic-desktop.musixmatch.com/ws/1.1/track.richsync.get` | Desktop usertoken (auto-fetched) | `KARAOKE_WORD_SYNCED` |
| `ultrastar` | `fetch_ultrastar_karaoke` | `usdb.animux.de/api/v1/txt` | None (public API) | `KARAOKE_WORD_SYNCED` |
| `kugou` | `fetch_kugou_karaoke` | `krcs.kugou.com/search` & `download` | None (public API) | `KARAOKE_WORD_SYNCED` |
| `qqmusic` | `fetch_qqmusic_lyrics` | `c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg` | None (public API) | `KARAOKE_WORD_SYNCED` |
| `netease_synced` | `fetch_netease_lyrics` | `music.163.com/api/song/lyric?os=pc&id={id}` — one call returns klyric **or** line-synced | None (public API) | `KARAOKE_WORD_SYNCED` / `LINE_SYNCED` |
| `lyricsplus` | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` — one call covers both grades | None (public API) | `KARAOKE_WORD_SYNCED` / `LINE_SYNCED` |
| `lrclib_synced` | `search_lyrics` | `lrclib.net/api/search?q={query}` — a single scored search (synced/title/duration/language), not separate "exact"/"simplified" tiers | None (public API) | `LINE_SYNCED` |
| `musixmatch_plain` | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | `UNSYNCED` |
| `lrclib_plain` | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (public API) | `UNSYNCED` |
| `tekstowo_plain` | `fetch_tekstowo_plain` | `tekstowo.pl/szukaj,wykonawca,{artist},tytul,{track}.html` | None (public web scraping) | `UNSYNCED` |

## 3. Dated audit snapshot (per-provider evidence)

The network/parser statuses below were measured during the lyrics audit that
produced the original 16-row matrix; they are preserved here as a snapshot of
that run, not as a live check. At that snapshot every fetcher above except
`spotify_color` was `network_tested` and reported `available`; `spotify_color`
was `fixture_tested` / `untested_network` / `requires_auth`.

### Real audio download validation case (from the same audit)

- **Track**: *Gloria Gaynor — I Will Survive*
- **Responding source**: `netease_synced` (line-synced branch)
- **Output format**: `LINE_SYNCED` (53 lines)
- **Audio file integrity**: `downloads_real_test\Gloria Gaynor\[1978] Love Tracks\05 - I Will Survive.flac` (**346.55 MB**, FLAC 192kHz/24-bit, verified with `ffprobe`)
- **Picture block**: `CoverFront` JPEG (600x600 px) embedded in FLAC stream.
- **Directory layout**: stored directly in `LibraryLayout` without artificial
  subdirectories (`raw`, `enriched`, `processed`, `final`).

> This empirical single-track validation proves end-to-end execution of one
> source within the live pipeline at the time of the audit. It does **not**
> serve as blanket proof of network availability for all 12 sources.

## 4. Enhanced LRC word-synced preservation

When `elrc_content` is present (containing word timestamps like
`<00:10.00>I <00:10.50>wish <00:11.00>you`), the engine enforces strict
quality preservation:

1. **Sidecar file (`.lrc`)**: written with the exact Enhanced LRC
   word-timestamped string.
2. **FLAC VorbisComments (`LYRICS`)**: embedded with the exact Enhanced LRC
   word-timestamped string.
3. **FLAC VorbisComments (`UNSYNCEDLYRICS`)**: embedded with clean plain text
   stripped of all `[mm:ss.xx]` and `<mm:ss.xx>` markers via
   `strip_lrc_timestamps`.
4. **Guard test (`test_enhanced_lrc_word_synced_preservation_strict`)**:
   explicitly asserts that word timestamps `<mm:ss.xx>` are not removed or
   degraded when `elrc_content` is present in the response object.
