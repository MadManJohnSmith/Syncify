# Syncify Lyrics Engine — 16-Provider Master Audit & Classification Matrix

This document provides a provider-by-provider technical audit of all 16 providers in the Syncify lyrics engine cascade (`legacy/syncify-cli/src/download/lyrics.rs` & `src-tauri/src/download/lyrics.rs`).

## 1. Status Taxonomy Definitions

To ensure precise auditing without conflating unit test execution with live network availability, each provider is evaluated across four distinct dimensions:

- **Code Status**: `implemented` (Full source code implementation exists in Rust module).
- **Parser Status**: `parser_tested` (Deterministic offline unit test validates parsing without network calls) / `fixture_tested` (Tested against mock API payloads).
- **Network Status**: `network_tested` (Endpoint verified via live HTTP integration test) / `untested_network` (Network endpoint not exercised in isolated offline test suites).
- **Runtime State**: 
  - `available`: Publicly accessible endpoint returning valid lyrics.
  - `requires_auth`: Requires user authentication credentials (`SPOTIFY_SP_DC`).
  - `source_unavailable`: Endpoint disabled, rate-limited, or returned HTTP error.

---

## 2. 16-Provider Technical Audit Matrix

| Priority | Provider Name | Concrete Function | Endpoint / Mechanism | Required Credential | Output Sync Type | Code Status | Parser Status | Network Status | Runtime State |
| :---: | :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: | :---: |
| **1** | Apple Music TTML | `fetch_apple_music_ttml` | `amp-api.music.apple.com/v1/catalog/{sf}/songs/{id}/syllable-lyrics` | WebPlayKid Bearer token (auto-extracted) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **2** | Spotify Color Lyrics | `fetch_spotify_lyrics` | `spclient.wg.spotify.com/color-lyrics/v2/track/{id}` | `SPOTIFY_SP_DC` session cookie (env var) | `KARAOKE_WORD_SYNCED` / `LINE_SYNCED` | `implemented` | `fixture_tested` | `untested_network` | `requires_auth` |
| **3** | Musixmatch Richsync | `fetch_musixmatch_richsync` | `apic-desktop.musixmatch.com/ws/1.1/track.richsync.get` | Desktop usertoken (auto-fetched) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **4** | UltraStar USDB Karaoke | `fetch_ultrastar_karaoke` | `usdb.animux.de/api/v1/txt` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **5** | Kugou Real Karaoke | `fetch_kugou_karaoke` | `krcs.kugou.com/search` & `download` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **6** | QQ Music Karaoke | `fetch_qqmusic_lyrics` | `c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **7** | NetEase klyric Karaoke | `fetch_netease_lyrics` | `music.163.com/api/song/lyric?os=pc&id={id}` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **8** | LyricsPlus Karaoke | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **8b** | LyricsPlus Line-Synced | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **9** | LRCLIB Line-Synced (Exact) | `search_lyrics` | `lrclib.net/api/search?q={query}` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **10** | LRCLIB Line-Synced (Simplified) | `search_lyrics` | `lrclib.net/api/search?q={query_simp}` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **11** | NetEase Line-Synced | `fetch_netease_lyrics` | `music.163.com/api/song/lyric` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **12** | Musixmatch Plain (Exact) | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **13** | LRCLIB Plain (Exact) | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (Public API) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **14** | Musixmatch Plain (Simplified) | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **15** | LRCLIB Plain (Simplified) | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (Public API) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **16** | Tekstowo.pl Plain | `fetch_tekstowo_plain` | `tekstowo.pl/szukaj,wykonawca,{artist},tytul,{track}.html` | None (Public Web Scraping) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |

---

## 3. Enhanced LRC Word-Synced Preservation Strategy & Negative Testing

When `elrc_content` is present (containing word timestamps like `<00:10.00>I <00:10.50>wish <00:11.00>you`), the engine enforces strict quality preservation:

1. **Sidecar File (`.lrc`)**: Written with exact Enhanced LRC word-timestamped string.
2. **FLAC VorbisComments (`LYRICS`)**: Embedded with exact Enhanced LRC word-timestamped string.
3. **FLAC VorbisComments (`UNSYNCEDLYRICS`)**: Embedded with clean plain text stripped of all `[mm:ss.xx]` and `<mm:ss.xx>` markers via `strip_lrc_timestamps`.
4. **Negative Test Assertion (`test_enhanced_lrc_word_level_degradation_negative`)**: Explicitly asserts that word timestamps `<mm:ss.xx>` are NOT removed or degraded when `elrc_content` is present in the response object.
