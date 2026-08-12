# Syncify Lyrics Engine — 16-Provider Master Audit & Classification Matrix

This document provides a provider-by-provider technical audit of all 16 providers in the Syncify lyrics engine cascade (`legacy/syncify-cli/src/download/lyrics.rs` & `src-tauri/src/download/lyrics.rs`).

## 1. Quality Hierarchy & Multi-Tier Cascade Architecture

The lyrics engine prioritizes lyrics by timing quality in 3 distinct tiers:

```
TIER 1: KARAOKE / SYLLABLE & WORD-SYNCED (eLRC) — Priorities 1 to 8
  ↳ Highest timing precision (<mm:ss.xx>Word or TTML syllable timestamps)
  ↳ Preserved in sidecar .lrc and VorbisComment LYRICS without timestamp degradation

TIER 2: LINE-SYNCED (LRC) — Priorities 8 to 11
  ↳ Line-level timing ([mm:ss.xx]Line)
  ↳ Used when no word-synced karaoke lyrics exist for the track

TIER 3: PLAIN / UNSYNCED TEXT FALLBACK — Priorities 12 to 16
  ↳ Plain text lyrics without timestamps
  ↳ Guarantees 100% of songs with published lyrics get lyrics in Symfonium/Plex/Kodi
  ↳ Stored cleanly in VorbisComment UNSYNCEDLYRICS
```

---

## 2. 16-Provider Technical Audit Matrix

| Priority | Provider Name | Concrete Function | Endpoint / Mechanism | Required Credential | Input / Output Format | Behavior w/o Credential | HTTP 401/403/404 Handling | Unit Test Status | Integration Test Status | Empirical Status |
| :---: | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: |
| **1** | Apple Music TTML | `fetch_apple_music_ttml` | `amp-api.music.apple.com/v1/catalog/{sf}/songs/{id}/syllable-lyrics` | WebPlayKid Bearer token (auto-extracted from web player JS) | Song search $\rightarrow$ TTML XML $\rightarrow$ `KARAOKE_WORD_SYNCED` | Extractor returns `None`, skips to Priority 2 | Skips storefront / skips to Priority 2 | PASS (Deterministic TTML XML Parser) | PASS (Auto-token extraction) | **Active / Available** |
| **2** | Spotify Color Lyrics | `fetch_spotify_lyrics` | `spclient.wg.spotify.com/color-lyrics/v2/track/{id}` | `SPOTIFY_SP_DC` session cookie (env var) | Track ID $\rightarrow$ Color Lyrics JSON $\rightarrow$ `KARAOKE_WORD_SYNCED` or `LINE_SYNCED` | Returns `source_unavailable`, skips to Priority 3 | Catches auth error, skips to Priority 3 | PASS (Deterministic Color Lyrics Parser) | Requires `SPOTIFY_SP_DC` | **Requires Auth (Optional)** |
| **3** | Musixmatch Richsync | `fetch_musixmatch_richsync` | `apic-desktop.musixmatch.com/ws/1.1/track.richsync.get` | Desktop usertoken (auto-fetched via `token.get`) | `q_artist` + `q_track` $\rightarrow$ `commontrack_id` $\rightarrow$ Richsync JSON $\rightarrow$ `KARAOKE_WORD_SYNCED` | Auto-fetches desktop token seamlessly | Catches error / 401 token refresh, skips to Priority 4 | PASS (Deterministic Richsync JSON Parser) | PASS (Desktop API) | **Active / Available** |
| **4** | UltraStar USDB Karaoke | `fetch_ultrastar_karaoke` | `usdb.animux.de/api/v1/txt` | None (Public API) | Artist + Title $\rightarrow$ UltraStar `.txt` beat-clock $\rightarrow$ `KARAOKE_WORD_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Priority 5 | PASS (Deterministic USDB TXT Parser) | PASS (Public USDB) | **Active / Available** |
| **5** | Kugou Real Karaoke | `fetch_kugou_karaoke` | `krcs.kugou.com/search` & `download` | None (Public API) | Track search $\rightarrow$ KRC encrypted blob $\rightarrow$ XOR key decrypt $\rightarrow$ `KARAOKE_WORD_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Priority 6 | PASS (Deterministic KRC Decrypt Parser) | PASS (Kugou API) | **Active / Available** |
| **6** | QQ Music Karaoke | `fetch_qqmusic_lyrics` | `c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg` | None (Public API) | Track search $\rightarrow$ QRC XML / Base64 $\rightarrow$ `KARAOKE_WORD_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Priority 7 | PASS (Deterministic QRC XML Parser) | PASS (QQ Music API) | **Active / Available** |
| **7** | NetEase klyric Karaoke | `fetch_netease_lyrics` | `music.163.com/api/song/lyric?os=pc&id={id}&lv=-1&kv=-1` | None (Public API) | Song search $\rightarrow$ Lyric JSON (`klyric`) $\rightarrow$ `KARAOKE_WORD_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Priority 8 | PASS (Deterministic klyric JSON Parser) | PASS (NetEase API) | **Active / Available** |
| **8** | LyricsPlus Karaoke | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` | None (Public API) | Search query $\rightarrow$ `syncedLyrics` (`<mm:ss.xx>`) $\rightarrow$ `KARAOKE_WORD_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Line-Synced | PASS (Deterministic LyricsPlus Parser) | PASS (Vercel API) | **Active / Available** |
| **8b** | LyricsPlus Line-Synced | `fetch_lyricsplus` | `lyricsplus-api.vercel.app/v1/search?q={term}` | None (Public API) | Search query $\rightarrow$ `syncedLyrics` (`[mm:ss.xx]`) $\rightarrow$ `LINE_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Priority 9 | PASS (Deterministic Line Parser) | PASS (Vercel API) | **Active / Available** |
| **9** | LRCLIB Line-Synced (Exact) | `search_lyrics` | `lrclib.net/api/search?q={query}` | None (Public API) | `artist` + `track` $\rightarrow$ `syncedLyrics` $\rightarrow$ `LINE_SYNCED` | N/A (Public) | Rate-limited by `LRCLIB_LIMITER`, skips to Priority 10 | PASS (Deterministic LRCLIB Parser) | PASS (LRCLIB API) | **Active / Available** |
| **10** | LRCLIB Line-Synced (Simplified) | `search_lyrics` | `lrclib.net/api/search?q={query_simp}` | None (Public API) | Stripped title *(Remastered/Deluxe/Live)* $\rightarrow$ `LINE_SYNCED` | N/A (Public) | Rate-limited by `LRCLIB_LIMITER`, skips to Priority 11 | PASS (Deterministic Title Simplifier) | PASS (LRCLIB API) | **Active / Available** |
| **11** | NetEase Line-Synced | `fetch_netease_lyrics` | `music.163.com/api/song/lyric` | None (Public API) | Song ID $\rightarrow$ `lrc` JSON field $\rightarrow$ `LINE_SYNCED` | N/A (Public) | Catches HTTP error / 404, skips to Priority 12 | PASS (Deterministic NetEase Parser) | PASS (NetEase API) | **Active / Available** |
| **12** | Musixmatch Plain (Exact) | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | `commontrack_id` $\rightarrow$ `lyrics_body` $\rightarrow$ `UNSYNCED` | Auto-fetches desktop token | Strips disclaimer, skips to Priority 13 | PASS (Deterministic Disclaimer Stripper) | PASS (Musixmatch API) | **Active / Available** |
| **13** | LRCLIB Plain (Exact) | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (Public API) | Artist + Track $\rightarrow$ `plainLyrics` $\rightarrow$ `UNSYNCED` | N/A (Public) | Rate-limited by `LRCLIB_LIMITER`, skips to Priority 14 | PASS (Deterministic Plain Parser) | PASS (LRCLIB API) | **Active / Available** |
| **14** | Musixmatch Plain (Simplified) | `fetch_musixmatch_plain` | `apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get` | Desktop usertoken (auto-fetched) | Stripped title $\rightarrow$ `lyrics_body` $\rightarrow$ `UNSYNCED` | Auto-fetches desktop token | Strips disclaimer, skips to Priority 15 | PASS (Deterministic Plain Parser) | PASS (Musixmatch API) | **Active / Available** |
| **15** | LRCLIB Plain (Simplified) | `fetch_lrclib_plain` | `lrclib.net/api/get` | None (Public API) | Stripped title $\rightarrow$ `plainLyrics` $\rightarrow$ `UNSYNCED` | N/A (Public) | Rate-limited by `LRCLIB_LIMITER`, skips to Priority 16 | PASS (Deterministic Plain Parser) | PASS (LRCLIB API) | **Active / Available** |
| **16** | Tekstowo.pl Plain | `fetch_tekstowo_plain` | `tekstowo.pl/szukaj,wykonawca,{artist},tytul,{track}.html` | None (Public Web Scraping) | HTML Search $\rightarrow$ Scrape `<div class="inner-text">` $\rightarrow$ `UNSYNCED` | N/A (Public) | Catches HTTP error / 404, returns `Err("Lyrics not found")` | PASS (Deterministic HTML Scraper) | PASS (Tekstowo Scraper) | **Active / Available** |

---

## 3. Enhanced LRC Word-Synced Preservation Strategy

When `elrc_content` is present (containing word timestamps like `<00:10.00>I <00:10.50>wish <00:11.00>you`), the engine enforces strict quality preservation:

1. **Sidecar File (`.lrc`)**: Written with exact Enhanced LRC word-timestamped string.
2. **FLAC VorbisComments (`LYRICS`)**: Embedded with exact Enhanced LRC word-timestamped string.
3. **FLAC VorbisComments (`UNSYNCEDLYRICS`)**: Embedded with clean plain text stripped of all `[mm:ss.xx]` and `<mm:ss.xx>` markers via `strip_lrc_timestamps`.
4. **Zero Quality Degradation**: The engine never strips `<mm:ss.xx>` word markers down to `[mm:ss.xx]` line markers when `elrc_content` is returned by Tier 1 providers.
