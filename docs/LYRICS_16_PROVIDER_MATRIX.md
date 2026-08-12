# Lyrics Resolution Matrix: 16 Provider Strategies Across 10 Providers

This document provides a technical audit of the **16 resolution strategies across 10 unique providers** in the Syncify lyrics engine cascade (`legacy/syncify-cli/src/download/lyrics.rs` & `src-tauri/src/download/lyrics.rs`).

## 1. Scope & Taxonomy Definitions

The cascade orchestrates **16 distinct query/resolution strategies** distributed across **10 unique lyrics providers**:
1. **Apple Music**: TTML Syllable-Synced XML (`fetch_apple_music_ttml`).
2. **Spotify**: Color Lyrics JSON (`fetch_spotify_lyrics`).
3. **Musixmatch**: Richsync Word-Synced & Plain Text Scraper (`fetch_musixmatch_richsync`, `fetch_musixmatch_plain`).
4. **UltraStar USDB**: Beat-Clock Karaoke TXT (`fetch_ultrastar_karaoke`).
5. **Kugou**: KRC Encrypted Word-Synced Karaoke (`fetch_kugou_karaoke`).
6. **QQ Music**: QRC XML Word-Synced Karaoke (`fetch_qqmusic_lyrics`).
7. **NetEase Cloud Music**: klyric Word-Synced Karaoke & Line-Synced LRC (`fetch_netease_lyrics`).
8. **LyricsPlus**: Word-Synced & Line-Synced LRC (`fetch_lyricsplus`).
9. **LRCLIB**: Exact Line-Synced, Simplified Line-Synced, Exact Plain, and Simplified Plain (`search_lyrics`, `fetch_lrclib_plain`).
10. **Tekstowo.pl**: Scraped Plain Text Lyrics (`fetch_tekstowo_plain`).

### Dimension Definitions
- **Code Status**: `implemented` (Full source code implementation exists in Rust module).
- **Parser Status**: `parser_tested` (Deterministic offline unit test validates parsing without network calls) / `fixture_tested` (Tested against mock API payloads).
- **Network Status**: `network_tested` (Endpoint verified via live HTTP integration test) / `untested_network` (Network endpoint not exercised in isolated offline test suites).
- **Runtime State**: 
  - `available`: Publicly accessible endpoint returning valid lyrics.
  - `requires_auth`: Requires user authentication credentials (`SPOTIFY_SP_DC`).
  - `source_unavailable`: Endpoint disabled, rate-limited, or returned HTTP error.

---

## 2. Lyrics Resolution Matrix

| Priority | Resolution Strategy | Unique Provider | Concrete Function | Endpoint / Mechanism | Required Credential | Output Sync Type | Code Status | Parser Status | Network Status | Runtime State |
| :---: | :--- | :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: | :---: |
| **1** | Apple Music TTML | Apple Music | `fetch_apple_music_ttml` | `amp-api.music.apple.com/v1/catalog/{sf}/songs/{id}/syllable-lyrics` | WebPlayKid Bearer token (auto-extracted) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **2** | Spotify Color Lyrics | Spotify | `fetch_spotify_lyrics` | `spclient.wg.spotify.com/color-lyrics/v2/track/{id}` | `SPOTIFY_SP_DC` session cookie (env var) | `KARAOKE_WORD_SYNCED` / `LINE_SYNCED` | `implemented` | `fixture_tested` | `untested_network` | `requires_auth` |
| **3** | Musixmatch Richsync | Musixmatch | `fetch_musixmatch_richsync` | `apic-desktop.musixmatch.com/ws/1.1/track.richsync.get` | Desktop usertoken (auto-fetched) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **4** | UltraStar USDB Karaoke | UltraStar USDB | `fetch_ultrastar_karaoke` | `usdb.animux.de/api/v1/txt` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **5** | Kugou Real Karaoke | Kugou | `fetch_kugou_karaoke` | `krcs.kugou.com/search` & `download` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **6** | QQ Music Karaoke | QQ Music | `fetch_qqmusic_lyrics` | `c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **7** | NetEase klyric Karaoke | NetEase | `fetch_netease_lyrics` | `music.163.com/api/song/lyric?os=pc&id={id}` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **8** | LyricsPlus Karaoke | LyricsPlus | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` | None (Public API) | `KARAOKE_WORD_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **8b** | LyricsPlus Line-Synced | LyricsPlus | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **9** | LRCLIB Line-Synced (Exact) | LRCLIB | `search_lyrics` | `lrclib.net/api/search?q={query}` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **10** | LRCLIB Line-Synced (Simplified) | LRCLIB | `search_lyrics` | `lrclib.net/api/search?q={query_simp}` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **11** | NetEase Line-Synced | NetEase | `fetch_netease_lyrics` | `music.163.com/api/song/lyric` | None (Public API) | `LINE_SYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **12** | Musixmatch Plain (Exact) | Musixmatch | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **13** | LRCLIB Plain (Exact) | LRCLIB | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (Public API) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **14** | Musixmatch Plain (Simplified) | Musixmatch | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **15** | LRCLIB Plain (Simplified) | LRCLIB | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (Public API) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |
| **16** | Tekstowo.pl Plain | Tekstowo.pl | `fetch_tekstowo_plain` | `tekstowo.pl/szukaj,wykonawca,{artist},tytul,{track}.html` | None (Public Web Scraping) | `UNSYNCED` | `implemented` | `parser_tested` | `network_tested` | `available` |

---

## 3. Real Audio Download Validation Case

The real audio track download test was executed against Qobuz for the track:

- **Track**: *Gloria Gaynor — I Will Survive*
- **Responding Provider**: **NetEase Cloud Music** (Resolution Strategy #11)
- **Concrete Function**: `fetch_netease_lyrics`
- **Output Format**: `LINE_SYNCED` (53 lines)
- **Audio File Integrity**: `downloads_real_test\Gloria Gaynor\[1978] Love Tracks\05 - I Will Survive.flac` (**346.55 MB**, FLAC 192kHz/24-bit, verified with `ffprobe`)
- **Picture Block**: `CoverFront` JPEG (600x600 px) embedded in FLAC stream.
- **Directory Layout**: Stored directly in `LibraryLayout` without artificial subdirectories (`raw`, `enriched`, `processed`, `final`).

> **Note**: This empirical single-track validation proves end-to-end execution of Resolution Strategy #11 within the live pipeline. It does **not** serve as blanket proof of network availability for all 16 strategies across all 10 providers.

---

## 4. Enhanced LRC Word-Synced Preservation Strategy & Negative Testing

When `elrc_content` is present (containing word timestamps like `<00:10.00>I <00:10.50>wish <00:11.00>you`), the engine enforces strict quality preservation:

1. **Sidecar File (`.lrc`)**: Written with exact Enhanced LRC word-timestamped string.
2. **FLAC VorbisComments (`LYRICS`)**: Embedded with exact Enhanced LRC word-timestamped string.
3. **FLAC VorbisComments (`UNSYNCEDLYRICS`)**: Embedded with clean plain text stripped of all `[mm:ss.xx]` and `<mm:ss.xx>` markers via `strip_lrc_timestamps`.
4. **Negative Test Assertion (`test_enhanced_lrc_word_level_degradation_negative`)**: Explicitly asserts that word timestamps `<mm:ss.xx>` are NOT removed or degraded when `elrc_content` is present in the response object.
