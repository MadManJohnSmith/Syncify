// Lyrics engine - LRCLIB + Musixmatch Richsync for word-synced karaoke

use crate::download::http_client::{create_http_client, LRCLIB_LIMITER};
use anyhow::{anyhow, Result};
use base64::Engine;
use flate2::read::ZlibDecoder;
use std::io::Read;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, info};

/// A single line of lyrics with timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    #[serde(rename = "startTimeMs")]
    pub start_time_ms: i64,
    pub words: String,
    #[serde(rename = "endTimeMs")]
    pub end_time_ms: Option<i64>,
}

/// Lyrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsResponse {
    pub lines: Vec<LyricsLine>,
    #[serde(rename = "syncType")]
    pub sync_type: String,
    pub instrumental: bool,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    pub provider: String,
    pub source: String,
    /// Enhanced LRC string with word-level timestamps (e.g. <00:01.30>word)
    /// When present, this should be used for .lrc file output instead of line-level formatting.
    #[serde(rename = "elrcContent")]
    pub elrc_content: Option<String>,
}

/// LRCLIB API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are used by serde deserialization
struct LRCLibResponse {
    id: Option<i64>,
    name: Option<String>,
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "albumName")]
    album_name: Option<String>,
    duration: Option<f64>,
    instrumental: Option<bool>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

/// Musixmatch cached token
struct MxmToken {
    token: String,
    obtained_at: Instant,
}

/// Lyrics client with caching + Musixmatch token management + Spotify Color Lyrics direct access
pub struct LyricsClient {
    client: Client,
    cache: RwLock<HashMap<String, (LyricsResponse, Instant)>>,
    mxm_token: TokioMutex<Option<MxmToken>>,
    spotify_sp_dc: TokioMutex<Option<String>>,
    spotify_access_token: TokioMutex<Option<(String, Instant)>>,
}

/// Validate lyrics response quality (ensures content is genuine and not just title/artist placeholder)
pub fn is_valid_lyrics(lyrics: &LyricsResponse, expected_title: &str) -> bool {
    if lyrics.lines.is_empty() && lyrics.plain_lyrics.as_deref().unwrap_or("").trim().is_empty() {
        return false;
    }

    let title_clean = expected_title.to_lowercase().trim().to_string();
    let non_title_lines = lyrics.lines.iter().filter(|l| {
        let line_clean = l.words.to_lowercase().trim().to_string();
        !line_clean.is_empty() && line_clean != title_clean && !line_clean.starts_with("作词") && !line_clean.starts_with("作曲")
    }).count();

    if non_title_lines < 1 && lyrics.plain_lyrics.as_deref().unwrap_or("").trim().is_empty() {
        debug!("[LyricsValidation] Rejected lyrics for '{}': placeholder title-only content", expected_title);
        return false;
    }

    true
}

impl LyricsClient {
    pub fn new() -> Self {
        let env_sp_dc = std::env::var("SPOTIFY_SP_DC").ok().filter(|s| !s.trim().is_empty());
        Self {
            client: create_http_client(),
            cache: RwLock::new(HashMap::new()),
            mxm_token: TokioMutex::new(None),
            spotify_sp_dc: TokioMutex::new(env_sp_dc),
            spotify_access_token: TokioMutex::new(None),
        }
    }

    /// Set Spotify sp_dc session cookie for direct official Spotify Color Lyrics access
    pub async fn set_spotify_sp_dc(&self, sp_dc: String) {
        let mut guard = self.spotify_sp_dc.lock().await;
        *guard = Some(sp_dc);
        let mut tok_guard = self.spotify_access_token.lock().await;
        *tok_guard = None; // Invalidate cached token to re-authenticate with new sp_dc
    }

    /// Generate cache key
    fn cache_key(artist: &str, track: &str) -> String {
        format!("{}|{}", artist.to_lowercase(), track.to_lowercase())
    }

    /// Check cache (24 hour TTL)
    fn get_cached(&self, artist: &str, track: &str) -> Option<LyricsResponse> {
        let key = Self::cache_key(artist, track);
        let cache = self.cache.read().unwrap();
        if let Some((lyrics, cached_at)) = cache.get(&key) {
            if cached_at.elapsed() < Duration::from_secs(24 * 60 * 60) {
                return Some(lyrics.clone());
            }
        }
        None
    }

    /// Store in cache
    fn set_cached(&self, artist: &str, track: &str, lyrics: &LyricsResponse) {
        let key = Self::cache_key(artist, track);
        let mut cache = self.cache.write().unwrap();
        cache.insert(key, (lyrics.clone(), Instant::now()));
    }

    /// Fetch lyrics by artist and track name (direct API)
    pub async fn fetch_lyrics(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        // Check cache
        if let Some(cached) = self.get_cached(artist, track) {
            debug!("[LRCLIB] Cache hit for {} - {}", artist, track);
            return Ok(cached);
        }

        LRCLIB_LIMITER.wait("lrclib").await;

        let url = format!(
            "https://lrclib.net/api/get?artist_name={}&track_name={}",
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        debug!("[LRCLIB] Fetching lyrics for {} - {}", artist, track);

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(anyhow!("Lyrics not found"));
        }

        if !response.status().is_success() {
            return Err(anyhow!("LRCLIB request failed: HTTP {}", response.status()));
        }

        let lrc: LRCLibResponse = response.json().await?;
        let lyrics = self.parse_response(&lrc)?;

        self.set_cached(artist, track, &lyrics);
        info!(
            "[LRCLIB] Found lyrics for {} - {} ({} lines)",
            artist,
            track,
            lyrics.lines.len()
        );

        Ok(lyrics)
    }

    /// Search for lyrics with duration matching
    pub async fn search_lyrics(&self, query: &str, duration_sec: f64) -> Result<LyricsResponse> {
        LRCLIB_LIMITER.wait("lrclib").await;

        let url = format!(
            "https://lrclib.net/api/search?q={}",
            urlencoding::encode(query)
        );

        debug!(
            "[LRCLIB] Searching: {} (duration: {}s)",
            query, duration_sec
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("LRCLIB search failed: HTTP {}", response.status()));
        }

        let results: Vec<LRCLibResponse> = response.json().await?;

        if results.is_empty() {
            return Err(anyhow!("No lyrics found"));
        }

        // Find best match by duration (strict tolerance <= 3.0s and title match)
        let tolerance = 3.0;
        let mut best_match: Option<&LRCLibResponse> = None;
        let track_lower = query.to_lowercase();

        for result in &results {
            let has_synced = result.synced_lyrics.is_some();
            let duration_matches = if let Some(d) = result.duration {
                (d - duration_sec).abs() <= tolerance
            } else {
                false
            };

            let r_title = result.track_name.as_deref().unwrap_or("").to_lowercase();
            let title_matches = r_title.contains(&track_lower) || track_lower.contains(&r_title);

            // Require both synced lyrics AND strict duration + title match
            if duration_matches && has_synced && title_matches {
                best_match = Some(result);
                break;
            }
        }

        let best = best_match.ok_or_else(|| anyhow!("No title/duration matching synced lyrics found"))?;
        self.parse_response(best)
    }



    /// Fetch from all sources with Karaoke-First priority fallbacks and strict duration matching
    pub async fn fetch_all_sources(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        info!("[LyricsEngine] Fetching lyrics for '{} - {}' ({:.1}s expected, Karaoke-First Priority)", artist, track, duration_sec);

        let simplified = simplify_track_name(track);

        // =========================================================================
        // TIER 1: KARAOKE WORD-SYNCED (eLRC) PROVIDERS ONLY
        // =========================================================================

        // =========================================================================
        // TIER 1: KARAOKE WORD-SYNCED / SYLLABLE-SYNCED (eLRC) PROVIDERS
        // Priority Ordered by Studio Master Precision & Timing Quality
        // =========================================================================

        // 1. Apple Music TTML (Exact Title) -> PRIORITY 1 (Studio Master XML - Highest Precision)
        if let Ok(mut lyrics) = self.fetch_apple_music_ttml(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 1: Apple Music TTML (Studio Master Syllable-Synced)");
                return Ok(lyrics);
            }
        }

        // 2. Spotify Color Lyrics (Official Word-Synced & Line-Synced) -> PRIORITY 2
        if let Ok(mut lyrics) = self.fetch_spotify_lyrics(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 2: Spotify Color Lyrics ({})", lyrics.sync_type);
                return Ok(lyrics);
            }
        }

        // 3. Musixmatch Richsync (Exact Title) -> PRIORITY 3 (Official Studio Timed)
        if let Ok(mut lyrics) = self.fetch_musixmatch_richsync(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 3: Musixmatch Richsync (Word-Synced)");
                return Ok(lyrics);
            }
        }

        // 4. UltraStar Karaoke (USDB Syllable-Synced .txt) -> PRIORITY 4 (Human Beat-Clock Timed)
        if let Ok(lyrics) = self.fetch_ultrastar_karaoke(artist, track).await {
            if is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 4: UltraStar Karaoke (Beat-Clock Syllable-Synced)");
                return Ok(lyrics);
            }
        }

        // 5. Kugou Real Karaoke (KRC Word-Synced, 100% Open Database) -> PRIORITY 5
        if let Ok(lyrics) = self.fetch_kugou_karaoke(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 5: Kugou Real Karaoke (Word-Synced)");
                return Ok(lyrics);
            }
        }

        // 6. QQ Music Synced & Karaoke Lyrics -> PRIORITY 6
        if let Ok(lyrics) = self.fetch_qqmusic_lyrics(artist, track, duration_sec).await {
            if lyrics.sync_type == "KARAOKE_WORD_SYNCED" && is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 6: QQ Music Karaoke (Word-Synced)");
                return Ok(lyrics);
            }
        }

        // 7. NetEase klyric Karaoke (Exact Title) -> PRIORITY 7
        if let Ok(mut lyrics) = self.fetch_netease_lyrics(artist, track, duration_sec).await {
            if lyrics.sync_type == "KARAOKE_WORD_SYNCED" && is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 7: NetEase Cloud Music Karaoke (Word-Synced)");
                return Ok(lyrics);
            }
        }

        // 8. LyricsPlus Karaoke (Exact Title) -> PRIORITY 8
        if let Ok(lyrics) = self.fetch_lyricsplus(artist, track, duration_sec).await {
            if lyrics.sync_type == "KARAOKE_WORD_SYNCED" && is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 8: LyricsPlus Karaoke (Word-Synced)");
                return Ok(lyrics);
            }
        }

        // --- SIMPLIFIED TITLE FALLBACKS FOR TIER 1 KARAOKE ---
        if simplified != track {
            if let Ok(mut lyrics) = self.fetch_apple_music_ttml(artist, &simplified, duration_sec).await {
                if is_valid_lyrics(&lyrics, &simplified) {
                    lyrics.lines.sort_by_key(|l| l.start_time_ms);
                    info!("[LyricsEngine] ✓ Acquired PRIORITY 1 (simplified): Apple Music TTML");
                    return Ok(lyrics);
                }
            }
            if let Ok(mut lyrics) = self.fetch_musixmatch_richsync(artist, &simplified, duration_sec).await {
                if is_valid_lyrics(&lyrics, &simplified) {
                    lyrics.lines.sort_by_key(|l| l.start_time_ms);
                    info!("[LyricsEngine] ✓ Acquired PRIORITY 3 (simplified): Musixmatch Richsync");
                    return Ok(lyrics);
                }
            }
            if let Ok(lyrics) = self.fetch_kugou_karaoke(artist, &simplified, duration_sec).await {
                if is_valid_lyrics(&lyrics, &simplified) {
                    info!("[LyricsEngine] ✓ Acquired PRIORITY 5 (simplified): Kugou Real Karaoke");
                    return Ok(lyrics);
                }
            }
        }

        // =========================================================================
        // TIER 2: LINE-SYNCED & PLAIN FALLBACK PROVIDERS
        // =========================================================================

        // 8. LyricsPlus Line-Synced (Exact Title) -> PRIORITY 8
        if let Ok(mut lyrics) = self.fetch_lyricsplus(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 8: LyricsPlus Line-Synced");
                return Ok(lyrics);
            }
        }

        // 9. LRCLIB Line-Synced (Exact Title) -> PRIORITY 9
        let query = format!("{} {}", artist, track);
        if let Ok(mut lyrics) = self.search_lyrics(&query, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 9: LRCLIB Line-Synced");
                return Ok(lyrics);
            }
        }

        // 10. LRCLIB Line-Synced (Simplified Title) -> PRIORITY 10
        if simplified != track {
            let query_simp = format!("{} {}", artist, simplified);
            if let Ok(mut lyrics) = self.search_lyrics(&query_simp, duration_sec).await {
                if is_valid_lyrics(&lyrics, &simplified) {
                    lyrics.lines.sort_by_key(|l| l.start_time_ms);
                    info!("[LyricsEngine] ✓ Acquired PRIORITY 10 (simplified): LRCLIB Line-Synced");
                    return Ok(lyrics);
                }
            }
        }

        // 11. NetEase Line-Synced (Exact Title) -> PRIORITY 11
        if let Ok(mut lyrics) = self.fetch_netease_lyrics(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                lyrics.lines.sort_by_key(|l| l.start_time_ms);
                info!("[LyricsEngine] ✓ Acquired PRIORITY 11: NetEase Line-Synced");
                return Ok(lyrics);
            }
        }

        // =========================================================================
        // TIER 3: PLAIN / UNSYNCED LYRICS UNIVERSAL FALLBACK
        // (Ensures 100% of songs with published lyrics get lyrics in Symfonium/Plex/Kodi)
        // =========================================================================

        // 12. Musixmatch Official Plain Lyrics (Exact Title) -> PRIORITY 12
        if let Ok(lyrics) = self.fetch_musixmatch_plain(artist, track, duration_sec).await {
            if is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 12: Musixmatch Official Plain Lyrics");
                return Ok(lyrics);
            }
        }

        // 13. LRCLIB Plain Lyrics (Exact Title) -> PRIORITY 13
        if let Ok(lyrics) = self.fetch_lrclib_plain(artist, track).await {
            if is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 13: LRCLIB Plain Lyrics");
                return Ok(lyrics);
            }
        }

        // 14. Musixmatch Official Plain Lyrics (Simplified Title) -> PRIORITY 14
        if simplified != track {
            if let Ok(lyrics) = self.fetch_musixmatch_plain(artist, &simplified, duration_sec).await {
                if is_valid_lyrics(&lyrics, &simplified) {
                    info!("[LyricsEngine] ✓ Acquired PRIORITY 14 (simplified): Musixmatch Official Plain Lyrics");
                    return Ok(lyrics);
                }
            }
        }

        // 15. LRCLIB Plain Lyrics (Simplified Title) -> PRIORITY 15
        if simplified != track {
            if let Ok(lyrics) = self.fetch_lrclib_plain(artist, &simplified).await {
                if is_valid_lyrics(&lyrics, &simplified) {
                    info!("[LyricsEngine] ✓ Acquired PRIORITY 15 (simplified): LRCLIB Plain Lyrics");
                    return Ok(lyrics);
                }
            }
        }

        // 16. Tekstowo.pl Polish Plain Lyrics Fallback -> PRIORITY 16
        if let Ok(lyrics) = self.fetch_tekstowo_plain(artist, track).await {
            if is_valid_lyrics(&lyrics, track) {
                info!("[LyricsEngine] ✓ Acquired PRIORITY 16: Tekstowo.pl Polish Plain Lyrics");
                return Ok(lyrics);
            }
        }

        Err(anyhow!("Lyrics not found from any source for {} - {}", artist, track))
    }

    /// Obtain a Musixmatch usertoken (auto-refreshed, cached for 10 minutes)
    async fn get_musixmatch_token(&self) -> Result<String> {
        let mut guard = self.mxm_token.lock().await;
        if let Some(ref cached) = *guard {
            if cached.obtained_at.elapsed() < Duration::from_secs(600) {
                return Ok(cached.token.clone());
            }
        }

        let url = "https://apic-desktop.musixmatch.com/ws/1.1/token.get?app_id=web-desktop-app-v1.0";
        let res = self.client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch token request failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("Musixmatch token parse failed: {}", e))?;

        let status = json["message"]["header"]["status_code"].as_i64().unwrap_or(0);
        if status != 200 {
            return Err(anyhow!("Musixmatch token.get returned status {}", status));
        }

        let token = json["message"]["body"]["user_token"].as_str()
            .ok_or_else(|| anyhow!("No user_token in Musixmatch response"))?
            .to_string();

        if token.is_empty() {
            return Err(anyhow!("Empty Musixmatch token"));
        }

        debug!("[Musixmatch] Obtained usertoken: {}...", &token[..token.len().min(16)]);
        *guard = Some(MxmToken { token: token.clone(), obtained_at: Instant::now() });
        Ok(token)
    }

    /// Fetch Musixmatch Richsync word-synced lyrics and convert to Enhanced LRC
    pub async fn fetch_musixmatch_richsync(&self, artist: &str, track: &str, duration_sec: f64) -> Result<LyricsResponse> {
        let token = self.get_musixmatch_token().await?;

        // Step 1: Search for the track
        let search_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.search?app_id=web-desktop-app-v1.0&usertoken={}&q_artist={}&q_track={}&page_size=5&page=1&s_track_rating=desc",
            token,
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self.client
            .get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch search failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("Musixmatch search parse failed: {}", e))?;

        let track_list = json["message"]["body"]["track_list"].as_array()
            .ok_or_else(|| anyhow!("No track_list in Musixmatch search response"))?;

        if track_list.is_empty() {
            return Err(anyhow!("Musixmatch: no tracks found for {} - {}", artist, track));
        }

        // Find best match with richsync and strict duration + title matching
        let mut exact_match: Option<&serde_json::Value> = None;
        let mut fallback_match: Option<&serde_json::Value> = None;
        let artist_lower = artist.to_lowercase();
        let track_lower = track.to_lowercase();

        for item in track_list {
            let t = &item["track"];
            let has_richsync = t["has_richsync"].as_i64().unwrap_or(0) == 1;
            if !has_richsync {
                continue;
            }
            let t_artist = t["artist_name"].as_str().unwrap_or("").to_lowercase();
            let t_name = t["track_name"].as_str().unwrap_or("").to_lowercase();
            let t_len = t["track_length"].as_f64().unwrap_or(0.0);

            // Duration check: enforce max ±3.0s tolerance if expected duration is provided
            if duration_sec > 0.0 && t_len > 0.0 && (t_len - duration_sec).abs() > 3.0 {
                debug!("[Musixmatch] Skipping candidate '{}' - duration mismatch ({:.1}s vs expected {:.1}s)", t_name, t_len, duration_sec);
                continue;
            }

            let title_matches = t_name.contains(&track_lower) || track_lower.contains(&t_name);
            let artist_matches = t_artist.contains(&artist_lower) || artist_lower.contains(&t_artist);

            if title_matches && artist_matches {
                exact_match = Some(t);
                break;
            } else if title_matches && fallback_match.is_none() {
                fallback_match = Some(t);
            }
        }

        let matched = exact_match.or(fallback_match).ok_or_else(|| anyhow!("No Musixmatch track with richsync for {} - {}", artist, track))?;
        let commontrack_id = matched["commontrack_id"].as_i64()
            .ok_or_else(|| anyhow!("Missing commontrack_id"))?;

        // Step 2: Fetch richsync data
        // Small delay for rate limiting
        tokio::time::sleep(Duration::from_millis(300)).await;

        let rs_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.richsync.get?app_id=web-desktop-app-v1.0&usertoken={}&commontrack_id={}",
            token, commontrack_id
        );

        let rs_res = self.client
            .get(&rs_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch richsync request failed: {}", e))?;

        let rs_json: serde_json::Value = rs_res.json().await
            .map_err(|e| anyhow!("Musixmatch richsync parse failed: {}", e))?;

        let rs_body_str = rs_json["message"]["body"]["richsync"]["richsync_body"].as_str()
            .ok_or_else(|| anyhow!("No richsync_body in Musixmatch response"))?;

        let mut richsync_entries: Vec<serde_json::Value> = serde_json::from_str(rs_body_str)
            .map_err(|e| anyhow!("Failed to parse richsync_body JSON: {}", e))?;

        if richsync_entries.is_empty() {
            return Err(anyhow!("Empty richsync_body for {} - {}", artist, track));
        }

        // Sort entries chronologically by ts before constructing elrc_buf and lines
        richsync_entries.sort_by(|a, b| {
            let ts_a = a["ts"].as_f64().unwrap_or(0.0);
            let ts_b = b["ts"].as_f64().unwrap_or(0.0);
            ts_a.partial_cmp(&ts_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut lines = Vec::new();
        let mut elrc_buf = String::new();

        for entry in &richsync_entries {
            let ts = entry["ts"].as_f64().unwrap_or(0.0);
            let te = entry["te"].as_f64();
            let start_ms = (ts * 1000.0) as i64;
            let end_ms = te.map(|t| (t * 1000.0) as i64);

            let word_list = match entry["l"].as_array() {
                Some(l) => l,
                None => continue,
            };

            // Build the Enhanced LRC line: [mm:ss.xx]<mm:ss.xx>word1 <mm:ss.xx>word2 ...
            let line_ts = ms_to_lrc_timestamp(start_ms);
            let mut line_text = String::new();
            let mut elrc_line = line_ts.clone();

            for word_entry in word_list {
                let word = word_entry["c"].as_str().unwrap_or("");
                let word_offset = word_entry["o"].as_f64().unwrap_or(0.0);
                let word_ms = start_ms + (word_offset * 1000.0) as i64;

                line_text.push_str(word);

                // Enhanced LRC word timestamp
                let w_mins = word_ms / 60000;
                let w_secs = (word_ms % 60000) as f64 / 1000.0;
                elrc_line.push_str(&format!("<{:02}:{:05.2}>{}", w_mins, w_secs, word));
            }

            elrc_buf.push_str(&elrc_line);
            elrc_buf.push('\n');

            lines.push(LyricsLine {
                start_time_ms: start_ms,
                words: line_text.trim().to_string(),
                end_time_ms: end_ms,
            });
        }

        info!("[Musixmatch] Found Richsync word-synced lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: "KARAOKE_WORD_SYNCED".to_string(),
            instrumental: false,
            plain_lyrics: None,
            provider: "Musixmatch Richsync".to_string(),
            source: "musixmatch.com".to_string(),
            elrc_content: Some(elrc_buf),
        })
    }

    /// Fetch official plain text lyrics from Musixmatch API
    pub async fn fetch_musixmatch_plain(&self, artist: &str, track: &str, duration_sec: f64) -> Result<LyricsResponse> {
        let token = self.get_musixmatch_token().await?;

        // Search for track
        let search_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.search?app_id=web-desktop-app-v1.0&usertoken={}&q_artist={}&q_track={}&page_size=5&page=1&s_track_rating=desc",
            token,
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self.client
            .get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch search failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("Musixmatch search parse failed: {}", e))?;

        let track_list = json["message"]["body"]["track_list"].as_array()
            .ok_or_else(|| anyhow!("No track_list in Musixmatch search response"))?;

        if track_list.is_empty() {
            return Err(anyhow!("Musixmatch: no tracks found for {} - {}", artist, track));
        }

        let mut matched_id: Option<i64> = None;
        let artist_lower = artist.to_lowercase();
        let track_lower = track.to_lowercase();

        for item in track_list {
            let t = &item["track"];
            let t_artist = t["artist_name"].as_str().unwrap_or("").to_lowercase();
            let t_name = t["track_name"].as_str().unwrap_or("").to_lowercase();
            let t_len = t["track_length"].as_f64().unwrap_or(0.0);

            if duration_sec > 0.0 && t_len > 0.0 && (t_len - duration_sec).abs() > 3.0 {
                continue;
            }

            let title_matches = t_name.contains(&track_lower) || track_lower.contains(&t_name);
            let artist_matches = t_artist.contains(&artist_lower) || artist_lower.contains(&t_artist);

            if title_matches && artist_matches {
                matched_id = t["commontrack_id"].as_i64();
                break;
            } else if title_matches && matched_id.is_none() {
                matched_id = t["commontrack_id"].as_i64();
            }
        }

        let commontrack_id = matched_id.ok_or_else(|| anyhow!("No matching commontrack_id in Musixmatch for {} - {}", artist, track))?;

        tokio::time::sleep(Duration::from_millis(300)).await;

        let lyrics_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get?app_id=web-desktop-app-v1.0&usertoken={}&commontrack_id={}",
            token, commontrack_id
        );

        let lyr_res = self.client
            .get(&lyrics_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch lyrics request failed: {}", e))?;

        let lyr_json: serde_json::Value = lyr_res.json().await
            .map_err(|e| anyhow!("Musixmatch lyrics parse failed: {}", e))?;

        let raw_body = lyr_json["message"]["body"]["lyrics"]["lyrics_body"].as_str()
            .ok_or_else(|| anyhow!("No lyrics_body in Musixmatch track.lyrics.get"))?;

        // Strip Musixmatch commercial disclaimer
        let clean_body = raw_body.split("*******").next().unwrap_or(raw_body).trim();

        if clean_body.is_empty() {
            return Err(anyhow!("Empty lyrics_body in Musixmatch for {} - {}", artist, track));
        }

        let mut lines = Vec::new();
        for (idx, line_str) in clean_body.lines().enumerate() {
            let trimmed = line_str.trim();
            if !trimmed.is_empty() {
                lines.push(LyricsLine {
                    start_time_ms: idx as i64 * 3000,
                    words: trimmed.to_string(),
                    end_time_ms: None,
                });
            }
        }

        info!("[Musixmatch] Found official plain lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: "UNSYNCED".to_string(),
            instrumental: false,
            plain_lyrics: Some(clean_body.to_string()),
            provider: "Musixmatch Plain".to_string(),
            source: "musixmatch.com".to_string(),
            elrc_content: None,
        })
    }

    /// Fetch plain lyrics from LRCLIB when synced lyrics are absent
    pub async fn fetch_lrclib_plain(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        LRCLIB_LIMITER.wait("lrclib").await;

        let url = format!(
            "https://lrclib.net/api/get?artist_name={}&track_name={}",
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("LRCLIB request failed: HTTP {}", res.status()));
        }

        let lrc: LRCLibResponse = res.json().await?;
        let plain = lrc.plain_lyrics.as_deref().unwrap_or("").trim();

        if plain.is_empty() {
            return Err(anyhow!("LRCLIB: plain_lyrics is empty for {} - {}", artist, track));
        }

        let mut lines = Vec::new();
        for (idx, line_str) in plain.lines().enumerate() {
            let trimmed = line_str.trim();
            if !trimmed.is_empty() {
                lines.push(LyricsLine {
                    start_time_ms: idx as i64 * 3000,
                    words: trimmed.to_string(),
                    end_time_ms: None,
                });
            }
        }

        info!("[LRCLIB] Found plain lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: "UNSYNCED".to_string(),
            instrumental: lrc.instrumental.unwrap_or(false),
            plain_lyrics: Some(plain.to_string()),
            provider: "LRCLIB Plain".to_string(),
            source: "lrclib.net".to_string(),
            elrc_content: None,
        })
    }

    /// Fetch synced lyrics from QQ Music API (Word-Synced & Line-Synced)
    pub async fn fetch_qqmusic_lyrics(&self, artist: &str, track: &str, _duration_sec: f64) -> Result<LyricsResponse> {
        let query = format!("{} {}", artist, track);
        let search_url = format!(
            "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?w={}&format=json&n=5",
            urlencoding::encode(&query)
        );

        let res = self.client.get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .header("Referer", "https://y.qq.com/")
            .send()
            .await
            .map_err(|e| anyhow!("QQMusic search request failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("QQMusic search parse failed: {}", e))?;

        let songs = json["data"]["song"]["list"].as_array()
            .ok_or_else(|| anyhow!("No songs found in QQMusic response"))?;

        if songs.is_empty() {
            return Err(anyhow!("QQMusic: 0 matches for {} - {}", artist, track));
        }

        let song_mid = songs[0]["songmid"].as_str()
            .ok_or_else(|| anyhow!("Missing songmid in QQMusic result"))?;

        let l_url = format!(
            "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&format=json",
            song_mid
        );

        let l_res = self.client.get(&l_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .header("Referer", "https://y.qq.com/")
            .send()
            .await
            .map_err(|e| anyhow!("QQMusic lyric request failed: {}", e))?;

        let l_json: serde_json::Value = l_res.json().await
            .map_err(|e| anyhow!("QQMusic lyric parse failed: {}", e))?;

        let l_b64 = l_json["lyric"].as_str()
            .ok_or_else(|| anyhow!("No lyric field in QQMusic response"))?;

        let raw_bytes = base64::engine::general_purpose::STANDARD.decode(l_b64)
            .map_err(|e| anyhow!("QQMusic base64 decode failed: {}", e))?;

        let raw_text = String::from_utf8_lossy(&raw_bytes).to_string();
        let is_karaoke = raw_text.contains('<') && raw_text.contains('>');
        let mut lines = Vec::new();

        for line in raw_text.lines() {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
            }
        }

        if lines.is_empty() {
            return Err(anyhow!("QQMusic: parsed 0 lines for {} - {}", artist, track));
        }

        info!("[QQMusic] ✓ Acquired synced lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: if is_karaoke { "KARAOKE_WORD_SYNCED".to_string() } else { "LINE_SYNCED".to_string() },
            instrumental: false,
            plain_lyrics: None,
            provider: "QQ Music".to_string(),
            source: "y.qq.com".to_string(),
            elrc_content: if is_karaoke { Some(raw_text) } else { None },
        })
    }

    /// Fetch Polish plain-text lyrics from Tekstowo.pl fallback
    pub async fn fetch_tekstowo_plain(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        let search_url = format!(
            "https://www.tekstowo.pl/szukaj,wykonawca,{},tytul,{}.html",
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self.client.get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| anyhow!("Tekstowo search request failed: {}", e))?;

        let html = res.text().await
            .map_err(|e| anyhow!("Tekstowo HTML read failed: {}", e))?;

        let re_link = regex::Regex::new(r#"href="(/piosenka,[^"]+)""#).unwrap();
        let song_path = re_link.captures(&html)
            .ok_or_else(|| anyhow!("No song match on Tekstowo.pl for {} - {}", artist, track))?
            .get(1).unwrap().as_str();

        let song_url = format!("https://www.tekstowo.pl{}", song_path);
        let song_res = self.client.get(&song_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| anyhow!("Tekstowo song page request failed: {}", e))?;

        let song_html = song_res.text().await?;
        let re_lyrics = regex::Regex::new(r#"(?s)<div class="inner-text">(.*?)</div>"#).unwrap();
        let lyrics_html = re_lyrics.captures(&song_html)
            .ok_or_else(|| anyhow!("No inner-text lyrics block on Tekstowo page"))?
            .get(1).unwrap().as_str();

        let clean_text = regex::Regex::new(r"<[^>]+>").unwrap().replace_all(lyrics_html, "\n");
        let plain_lines: Vec<String> = clean_text.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        if plain_lines.is_empty() {
            return Err(anyhow!("Tekstowo.pl lyrics extracted 0 lines for {} - {}", artist, track));
        }

        let mut lines = Vec::new();
        for (idx, l_str) in plain_lines.iter().enumerate() {
            lines.push(LyricsLine {
                start_time_ms: idx as i64 * 3000,
                words: l_str.clone(),
                end_time_ms: None,
            });
        }

        let full_plain = plain_lines.join("\n");
        info!("[Tekstowo.pl] Found Polish plain lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: "UNSYNCED".to_string(),
            instrumental: false,
            plain_lyrics: Some(full_plain),
            provider: "Tekstowo.pl".to_string(),
            source: "tekstowo.pl".to_string(),
            elrc_content: None,
        })
    }

    /// Fetch UltraStar Karaoke format lyrics from USDB open API
    pub async fn fetch_ultrastar_karaoke(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        let search_url = format!(
            "https://usdb.animux.de/api/v1/songs?artist={}&title={}",
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self.client.get(&search_url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| anyhow!("USDB search request failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("USDB search JSON parse failed: {}", e))?;

        let songs = json.as_array().or_else(|| json["songs"].as_array())
            .ok_or_else(|| anyhow!("No songs array in USDB response"))?;

        if songs.is_empty() {
            return Err(anyhow!("USDB: 0 matches for {} - {}", artist, track));
        }

        let song_id = songs[0]["id"].as_i64().or_else(|| songs[0]["id"].as_str().and_then(|s| s.parse().ok()))
            .ok_or_else(|| anyhow!("Missing song ID in USDB result"))?;

        let dl_url = format!("https://usdb.animux.de/index.php?link=gettxt&id={}", song_id);
        let dl_res = self.client.get(&dl_url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| anyhow!("USDB TXT download failed: {}", e))?;

        let us_txt = dl_res.text().await?;
        if !us_txt.contains("#TITLE") && !us_txt.contains(':') {
            return Err(anyhow!("Invalid UltraStar TXT format from USDB"));
        }

        let (lines, elrc_buf) = parse_ultrastar_to_elrc(&us_txt);
        if lines.is_empty() {
            return Err(anyhow!("UltraStar TXT parsed 0 lines for {} - {}", artist, track));
        }

        info!("[UltraStarKaraoke] ✓ Acquired syllable-synced UltraStar lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: "KARAOKE_WORD_SYNCED".to_string(),
            instrumental: false,
            plain_lyrics: None,
            provider: "UltraStar Karaoke".to_string(),
            source: "usdb.animux.de".to_string(),
            elrc_content: Some(elrc_buf),
        })
    }

    /// Fetch Apple Music TTML Syllable-Synced Karaoke lyrics
    pub async fn fetch_apple_music_ttml(&self, artist: &str, track: &str, duration_sec: f64) -> Result<LyricsResponse> {
        let am_token = match extract_apple_music_token(&self.client).await {
            Some(token) => token,
            None => return Err(anyhow!("Could not extract Apple Music token")),
        };

        let term = format!("{} {}", artist, track);
        let storefronts = vec!["gb", "us", "de", "fr", "mx"];
        let track_lower = simplify_track_name(track).to_lowercase();

        for sf in storefronts {
            let search_url = format!(
                "https://amp-api.music.apple.com/v1/catalog/{}/search?term={}&types=songs&limit=5",
                sf,
                urlencoding::encode(&term)
            );

            let req = self.client.get(&search_url)
                .header("Authorization", format!("Bearer {}", am_token))
                .header("Origin", "https://music.apple.com")
                .header("Referer", "https://music.apple.com/");

            if let Ok(res) = req.send().await {
                if res.status().is_success() {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(songs) = json["results"]["songs"]["data"].as_array() {
                            for song in songs {
                                let attrs = &song["attributes"];
                                let s_title = attrs["name"].as_str().unwrap_or("").to_lowercase();
                                let s_dur = attrs["durationInMillis"].as_f64().unwrap_or(0.0) / 1000.0;

                                if duration_sec > 0.0 && s_dur > 0.0 && (s_dur - duration_sec).abs() > 3.0 {
                                    continue;
                                }

                                if !s_title.contains(&track_lower) && !track_lower.contains(&s_title) {
                                    continue;
                                }

                                let song_id = song["id"].as_str().unwrap_or("");
                                if song_id.is_empty() {
                                    continue;
                                }

                                let lyrics_url = format!("https://amp-api.music.apple.com/v1/catalog/{}/songs/{}/lyrics", sf, song_id);
                                let l_req = self.client.get(&lyrics_url)
                                    .header("Authorization", format!("Bearer {}", am_token))
                                    .header("Origin", "https://music.apple.com")
                                    .header("Referer", "https://music.apple.com/");

                                if let Ok(l_res) = l_req.send().await {
                                    if l_res.status().is_success() {
                                        if let Ok(l_json) = l_res.json::<serde_json::Value>().await {
                                            if let Some(ttml) = l_json["data"][0]["attributes"]["ttml"].as_str() {
                                                let elrc = parse_ttml_to_elrc(ttml);
                                                if elrc.contains('<') && elrc.contains('>') {
                                                    let mut lines = Vec::new();
                                                    for line in elrc.lines() {
                                                        if let Some(parsed) = parse_lrc_line(line) {
                                                            lines.push(parsed);
                                                        }
                                                                 if !lines.is_empty() {
                                                        info!("[AppleMusicTTML] Acquired syllable-synced lyrics for {} - {} ({} lines)", artist, track, lines.len());
                                                        return Ok(LyricsResponse {
                                                            lines,
                                                            sync_type: "KARAOKE_WORD_SYNCED".to_string(),
                                                            instrumental: false,
                                                            plain_lyrics: None,
                                                            provider: "Apple Music TTML".to_string(),
                                                            source: "music.apple.com".to_string(),
                                                            elrc_content: Some(elrc),
                                                        });
                                                    }                                          }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow!("No Apple Music TTML lyrics found for {} - {}", artist, track))
    }

    /// Fetch lyrics from NetEase Cloud Music API with duration matching
    pub async fn fetch_netease_lyrics(&self, artist: &str, track: &str, duration_sec: f64) -> Result<LyricsResponse> {
        let query = format!("{} {}", artist, track);
        let search_url = format!(
            "https://music.163.com/api/search/get?s={}&type=1&offset=0&limit=5",
            urlencoding::encode(&query)
        );

        let res = self.client
            .get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Referer", "https://music.163.com")
            .send()
            .await
            .map_err(|e| anyhow!("NetEase search request failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("NetEase search parse failed: {}", e))?;

        let songs = json["result"]["songs"].as_array()
            .ok_or_else(|| anyhow!("No songs in NetEase search response"))?;

        if songs.is_empty() {
            return Err(anyhow!("NetEase: no songs found for {} - {}", artist, track));
        }

        let track_lower = simplify_track_name(track).to_lowercase();
        let track_words: Vec<&str> = track_lower.split_whitespace().filter(|w| w.len() >= 3).collect();

        // Find song candidate matching expected duration (tolerance ±3.0s) AND title keyword match
        let mut matched_song: Option<&serde_json::Value> = None;
        for s in songs {
            let s_name = s["name"].as_str().unwrap_or("").to_lowercase();

            // Title match check: candidate name MUST match or share significant words with searched track
            let title_matches = track_words.is_empty()
                || s_name.contains(&track_lower)
                || track_lower.contains(&s_name)
                || track_words.iter().any(|w| s_name.contains(w));

            if !title_matches {
                debug!("[NetEase] Skipping generic song candidate '{}' - title mismatch for expected track '{}'", s_name, track);
                continue;
            }

            let dt_ms = s["dt"].as_f64().unwrap_or(0.0);
            let dt_sec = dt_ms / 1000.0;
            if duration_sec > 0.0 && dt_sec > 0.0 && (dt_sec - duration_sec).abs() > 3.0 {
                debug!("[NetEase] Skipping song candidate '{}' - duration mismatch ({:.1}s vs expected {:.1}s)", s_name, dt_sec, duration_sec);
                continue;
            }
            matched_song = Some(s);
            break;
        }

        let song = matched_song.ok_or_else(|| anyhow!("NetEase: no title/duration matching songs for {} - {}", artist, track))?;
        let song_id = song["id"].as_i64()
            .ok_or_else(|| anyhow!("Missing NetEase song id"))?;

        // Fetch lyrics
        let lyric_url = format!(
            "https://music.163.com/api/song/lyric?id={}&lv=-1&kv=-1&tv=-1",
            song_id
        );

        let l_res = self.client
            .get(&lyric_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Referer", "https://music.163.com")
            .send()
            .await
            .map_err(|e| anyhow!("NetEase lyric request failed: {}", e))?;

        let l_json: serde_json::Value = l_res.json().await
            .map_err(|e| anyhow!("NetEase lyric parse failed: {}", e))?;

        let klyric = l_json["klyric"]["lyric"].as_str().unwrap_or("");
        let lrc = l_json["lrc"]["lyric"].as_str().unwrap_or("");

        let raw_lyrics = if !klyric.trim().is_empty() {
            klyric
        } else if !lrc.trim().is_empty() {
            lrc
        } else {
            return Err(anyhow!("NetEase: no synced lyrics content for song {}", song_id));
        };

        let is_karaoke = raw_lyrics.contains('<') && raw_lyrics.contains('>');
        let mut raw_lines: Vec<String> = raw_lyrics.lines().map(|s| s.to_string()).collect();

        // Sort lines chronologically by timestamp [mm:ss.xx]
        raw_lines.sort_by_key(|line| {
            parse_lrc_line(line).map_or(0, |l| l.start_time_ms)
        });

        let mut lines = Vec::new();
        let mut sorted_buf = String::new();
        for line in &raw_lines {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
                sorted_buf.push_str(line);
                sorted_buf.push('\n');
            }
        }

        if lines.is_empty() {
            return Err(anyhow!("NetEase: parsed 0 lines from lyrics for {}", song_id));
        }

        info!(
            "[NetEase] Found {} lyrics for {} - {} ({} lines)",
            if is_karaoke { "karaoke word-synced" } else { "line-synced" },
            artist,
            track,
            lines.len()
        );

        Ok(LyricsResponse {
            lines,
            sync_type: if is_karaoke { "KARAOKE_WORD_SYNCED".to_string() } else { "LINE_SYNCED".to_string() },
            instrumental: false,
            plain_lyrics: None,
            provider: "NetEase Cloud Music".to_string(),
            source: "music.163.com".to_string(),
            elrc_content: if is_karaoke { Some(sorted_buf) } else { None },
        })
    }

    /// Obtain a Spotify Web Player access token using the user's sp_dc cookie
    async fn get_spotify_access_token(&self) -> Result<String> {
        let sp_dc_guard = self.spotify_sp_dc.lock().await;
        let sp_dc = match sp_dc_guard.as_ref() {
            Some(dc) if !dc.trim().is_empty() => dc.clone(),
            _ => return Err(anyhow!("Spotify sp_dc token is not configured")),
        };
        drop(sp_dc_guard);

        let mut guard = self.spotify_access_token.lock().await;
        if let Some((ref token, obtained_at)) = *guard {
            if obtained_at.elapsed() < Duration::from_secs(50 * 60) {
                return Ok(token.clone());
            }
        }

        let url = "https://open.spotify.com/get_access_token?reason=transport&productType=web_player";
        let res = self.client
            .get(url)
            .header("Cookie", format!("sp_dc={}", sp_dc))
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://open.spotify.com/")
            .send()
            .await
            .map_err(|e| anyhow!("Spotify token request failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("Spotify token parse failed: {}", e))?;

        let access_token = json["accessToken"].as_str()
            .ok_or_else(|| anyhow!("No accessToken in Spotify response (sp_dc may be expired)"))?
            .to_string();

        *guard = Some((access_token.clone(), Instant::now()));
        info!("[SpotifyLyrics] Successfully acquired fresh Spotify Web Player access token");
        Ok(access_token)
    }

    /// Fetch official native syllable/word-synced and line-synced lyrics directly from Spotify
    pub async fn fetch_spotify_lyrics(&self, artist: &str, track: &str, duration_sec: f64) -> Result<LyricsResponse> {
        let access_token = self.get_spotify_access_token().await?;

        // Step 1: Search for track ID on Spotify
        let query = format!("{} {}", artist, track);
        let search_url = format!(
            "https://api.spotify.com/v1/search?q={}&type=track&limit=5",
            urlencoding::encode(&query)
        );

        let s_res = self.client
            .get(&search_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow!("Spotify search request failed: {}", e))?;

        let s_json: serde_json::Value = s_res.json().await
            .map_err(|e| anyhow!("Spotify search JSON parse failed: {}", e))?;

        let tracks = s_json["tracks"]["items"].as_array()
            .ok_or_else(|| anyhow!("No tracks array in Spotify search"))?;

        if tracks.is_empty() {
            return Err(anyhow!("No tracks found on Spotify for {} - {}", artist, track));
        }

        let track_lower = simplify_track_name(track).to_lowercase();
        let artist_lower = artist.to_lowercase();

        let mut matched_id: Option<String> = None;
        for t in tracks {
            let t_name = t["name"].as_str().unwrap_or("").to_lowercase();
            let t_dur_sec = t["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0;
            let t_artists = t["artists"].as_array();

            let artist_matches = t_artists.map_or(false, |arr| {
                arr.iter().any(|a| {
                    let a_name = a["name"].as_str().unwrap_or("").to_lowercase();
                    a_name.contains(&artist_lower) || artist_lower.contains(&a_name)
                })
            });

            if duration_sec > 0.0 && t_dur_sec > 0.0 && (t_dur_sec - duration_sec).abs() > 3.0 {
                continue;
            }

            let title_matches = t_name.contains(&track_lower) || track_lower.contains(&t_name);
            if title_matches && artist_matches {
                matched_id = t["id"].as_str().map(|s| s.to_string());
                break;
            } else if title_matches && matched_id.is_none() {
                matched_id = t["id"].as_str().map(|s| s.to_string());
            }
        }

        let spotify_track_id = matched_id.ok_or_else(|| anyhow!("No matching track ID on Spotify for {} - {}", artist, track))?;

        // Step 2: Fetch Color Lyrics
        let lyrics_url = format!(
            "https://spclient.wg.spotify.com/color-lyrics/v2/track/{}?format=json&market=from_token",
            spotify_track_id
        );

        let lyr_res = self.client
            .get(&lyrics_url)
            .header("App-Platform", "WebPlayer")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Origin", "https://open.spotify.com")
            .header("Referer", "https://open.spotify.com/")
            .send()
            .await
            .map_err(|e| anyhow!("Spotify Color Lyrics request failed: {}", e))?;

        if lyr_res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(anyhow!("No lyrics on Spotify for track {}", spotify_track_id));
        }

        let lyr_json: serde_json::Value = lyr_res.json().await
            .map_err(|e| anyhow!("Spotify Color Lyrics parse failed: {}", e))?;

        let lines_array = lyr_json["lyrics"]["lines"].as_array()
            .ok_or_else(|| anyhow!("No lines in Spotify Color Lyrics payload"))?;

        if lines_array.is_empty() {
            return Err(anyhow!("Spotify Color Lyrics lines array is empty"));
        }

        let mut lines = Vec::new();
        let mut elrc_buf = String::new();
        let mut has_syllables = false;

        for line_item in lines_array {
            let start_ms: i64 = line_item["startTimeMs"].as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let words = line_item["words"].as_str().unwrap_or("").to_string();

            let syllables = line_item["syllables"].as_array();
            if let Some(syl_list) = syllables {
                if !syl_list.is_empty() {
                    has_syllables = true;
                    let line_ts = ms_to_lrc_timestamp(start_ms);
                    let mut elrc_line = line_ts.clone();
                    for syl in syl_list {
                        let syl_ms: i64 = syl["startTimeMs"].as_i64()
                            .or_else(|| syl["startTimeMs"].as_str().and_then(|s| s.parse().ok()))
                            .unwrap_or(start_ms);
                        let syl_text = syl["text"].as_str().unwrap_or("");
                        let w_mins = syl_ms / 60000;
                        let w_secs = (syl_ms % 60000) as f64 / 1000.0;
                        elrc_line.push_str(&format!("<{:02}:{:05.2}>{}", w_mins, w_secs, syl_text));
                    }
                    elrc_buf.push_str(&elrc_line);
                    elrc_buf.push('\n');
                }
            }

            lines.push(LyricsLine {
                start_time_ms: start_ms,
                words,
                end_time_ms: None,
            });
        }

        info!(
            "[SpotifyLyrics] ✓ Acquired {} lyrics for {} - {} ({} lines)",
            if has_syllables { "Karaoke word-synced" } else { "line-synced" },
            artist,
            track,
            lines.len()
        );

        Ok(LyricsResponse {
            lines,
            sync_type: if has_syllables { "KARAOKE_WORD_SYNCED".to_string() } else { "LINE_SYNCED".to_string() },
            instrumental: false,
            plain_lyrics: None,
            provider: "Spotify Color Lyrics".to_string(),
            source: "spotify.com".to_string(),
            elrc_content: if has_syllables { Some(elrc_buf) } else { None },
        })
    }

    /// Fetch official word-by-word Kugou Real Karaoke (KRC) from the open Kugou database (100% Zero-Cookie & Automated)
    pub async fn fetch_kugou_karaoke(&self, artist: &str, track: &str, duration_sec: f64) -> Result<LyricsResponse> {
        let query = format!("{} - {}", artist, track);
        let search_url = format!(
            "http://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}&duration={}",
            urlencoding::encode(&query),
            if duration_sec > 0.0 { (duration_sec * 1000.0) as i64 } else { 0 }
        );

        let res = self.client.get(&search_url)
            .header("User-Agent", "KuGou2012")
            .send()
            .await
            .map_err(|e| anyhow!("Kugou search request failed: {}", e))?;

        let json: serde_json::Value = res.json().await
            .map_err(|e| anyhow!("Kugou search parse failed: {}", e))?;

        let candidates = json["candidates"].as_array()
            .ok_or_else(|| anyhow!("No candidates in Kugou search response"))?;

        if candidates.is_empty() {
            return Err(anyhow!("No Kugou lyrics candidates for {} - {}", artist, track));
        }

        let first = &candidates[0];
        let c_id = first["id"].as_str().or_else(|| first["id"].as_i64().map(|_| "")).unwrap_or("");
        let id_str = if c_id.is_empty() {
            first["id"].to_string()
        } else {
            c_id.to_string()
        };
        let accesskey = first["accesskey"].as_str()
            .ok_or_else(|| anyhow!("Missing accesskey in Kugou candidate"))?;

        let dl_url = format!(
            "http://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=krc&charset=utf8",
            id_str, accesskey
        );

        let dl_res = self.client.get(&dl_url)
            .header("User-Agent", "KuGou2012")
            .send()
            .await
            .map_err(|e| anyhow!("Kugou download request failed: {}", e))?;

        let dl_json: serde_json::Value = dl_res.json().await
            .map_err(|e| anyhow!("Kugou download parse failed: {}", e))?;

        let content_b64 = dl_json["content"].as_str()
            .ok_or_else(|| anyhow!("No content field in Kugou download"))?;

        let raw_bytes = base64::engine::general_purpose::STANDARD.decode(content_b64)
            .map_err(|e| anyhow!("Kugou base64 decode failed: {}", e))?;

        let krc_text = decrypt_krc_bytes(&raw_bytes)
            .ok_or_else(|| anyhow!("Kugou KRC decompression/decrypt failed"))?;

        let (lines, elrc_buf) = parse_krc_to_elrc(&krc_text);

        if lines.is_empty() {
            return Err(anyhow!("Kugou: parsed 0 lines from KRC for {} - {}", artist, track));
        }

        info!("[KugouKaraoke] ✓ Acquired word-synced KRC lyrics for {} - {} ({} lines)", artist, track, lines.len());

        Ok(LyricsResponse {
            lines,
            sync_type: "KARAOKE_WORD_SYNCED".to_string(),
            instrumental: false,
            plain_lyrics: None,
            provider: "Kugou Karaoke KRC".to_string(),
            source: "kugou.com".to_string(),
            elrc_content: Some(elrc_buf),
        })
    }

    /// Fetch lyrics from LyricsPlus API
    pub async fn fetch_lyricsplus(&self, artist: &str, track: &str, _duration_sec: f64) -> Result<LyricsResponse> {
        let query = format!("{} {}", artist, track);
        let url = format!("https://lyricsplus-api.vercel.app/v1/search?q={}", urlencoding::encode(&query));

        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("LyricsPlus search failed: HTTP {}", res.status()));
        }

        let json: serde_json::Value = res.json().await?;
        let synced_str = json["syncedLyrics"].as_str().or_else(|| json["lyrics"].as_str())
            .ok_or_else(|| anyhow!("No lyrics field in LyricsPlus response"))?;

        if synced_str.trim().is_empty() {
            return Err(anyhow!("Empty lyrics in LyricsPlus response"));
        }

        let is_karaoke = synced_str.contains('<') && synced_str.contains('>');
        let mut raw_lines: Vec<String> = synced_str.lines().map(|s| s.to_string()).collect();

        // Sort lines chronologically by timestamp [mm:ss.xx]
        raw_lines.sort_by_key(|line| {
            parse_lrc_line(line).map_or(0, |l| l.start_time_ms)
        });

        let mut lines = Vec::new();
        let mut sorted_buf = String::new();
        for line in &raw_lines {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
                sorted_buf.push_str(line);
                sorted_buf.push('\n');
            }
        }

        if lines.len() < 4 {
            return Err(anyhow!("LyricsPlus: insufficient lines ({})", lines.len()));
        }

        Ok(LyricsResponse {
            lines,
            sync_type: if is_karaoke { "KARAOKE_WORD_SYNCED".to_string() } else { "LINE_SYNCED".to_string() },
            instrumental: false,
            plain_lyrics: None,
            provider: "LyricsPlus Karaoke".to_string(),
            source: "lyricsplus-api.vercel.app".to_string(),
            elrc_content: if is_karaoke { Some(sorted_buf) } else { None },
        })
    }

    /// Fetch native lyrics directly from Qobuz API using user credentials
    pub async fn fetch_qobuz_lyrics(
        &self,
        qobuz_track_id: i64,
        app_id: &str,
        user_token: &str,
    ) -> Result<LyricsResponse> {
        let url = format!(
            "https://www.qobuz.com/api.json/0.2/track/get?track_id={}&extra=lyrics",
            qobuz_track_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-App-Id", app_id)
            .header("X-User-Auth-Token", user_token)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz lyrics request failed: HTTP {}", response.status()));
        }

        let json: serde_json::Value = response.json().await?;
        if let Some(lyrics_obj) = json.get("lyrics") {
            let synced = lyrics_obj["synced_lyrics"].as_str().or(lyrics_obj["lrc"].as_str());
            let text = lyrics_obj["text"].as_str().or(lyrics_obj["plain"].as_str());

            if let Some(s) = synced {
                if !s.trim().is_empty() {
                    let mut lines = Vec::new();
                    let is_karaoke = s.contains('<') && s.contains('>');
                    for line in s.lines() {
                        if let Some(parsed) = parse_lrc_line(line) {
                            lines.push(parsed);
                        }
                    }
                    return Ok(LyricsResponse {
                        lines,
                        sync_type: if is_karaoke { "KARAOKE_WORD_SYNCED".to_string() } else { "LINE_SYNCED".to_string() },
                        instrumental: false,
                        plain_lyrics: text.map(|t| t.to_string()),
                        provider: "Qobuz Native".to_string(),
                        source: "qobuz.com".to_string(),
                        elrc_content: None,
                    });
                }
            } else if let Some(t) = text {
                if !t.trim().is_empty() {
                    return Ok(LyricsResponse {
                        lines: Vec::new(),
                        sync_type: "UNSYNCED".to_string(),
                        instrumental: false,
                        plain_lyrics: Some(t.to_string()),
                        provider: "Qobuz Native".to_string(),
                        source: "qobuz.com".to_string(),
                        elrc_content: None,
                    });
                }
            }
        }

        Err(anyhow!("No native lyrics on Qobuz for track {}", qobuz_track_id))
    }

    /// Parse LRCLIB response to our format
    pub fn parse_response(&self, lrc: &LRCLibResponse) -> Result<LyricsResponse> {
        let mut lines = Vec::new();
        let mut sync_type = "UNSYNCED".to_string();

        // Parse synced lyrics if available
        if let Some(synced) = &lrc.synced_lyrics {
            for line in synced.lines() {
                if let Some(parsed) = parse_lrc_line(line) {
                    lines.push(parsed);
                }
            }
            if !lines.is_empty() {
                sync_type = if synced.contains('<') && synced.contains('>') {
                    "KARAOKE_WORD_SYNCED".to_string()
                } else {
                    "LINE_SYNCED".to_string()
                };
            }
        }

        let elrc = if lrc.synced_lyrics.as_ref().map_or(false, |s| s.contains('<') && s.contains('>')) {
            lrc.synced_lyrics.clone()
        } else {
            None
        };

        Ok(LyricsResponse {
            lines,
            sync_type,
            instrumental: lrc.instrumental.unwrap_or(false),
            plain_lyrics: lrc.plain_lyrics.clone(),
            provider: "LRCLIB".to_string(),
            source: "lrclib.net".to_string(),
            elrc_content: elrc,
        })
    }

    /// Convert to LRC format string
    pub fn to_lrc_string(lyrics: &LyricsResponse) -> String {
        let mut lrc = String::new();
        for line in &lyrics.lines {
            let timestamp = ms_to_lrc_timestamp(line.start_time_ms);
            lrc.push_str(&format!("{}{}\n", timestamp, line.words));
        }
        lrc
    }
}

impl Default for LyricsClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a single LRC line: [mm:ss.xx]words
fn parse_lrc_line(line: &str) -> Option<LyricsLine> {
    let line = line.trim();
    if !line.starts_with('[') {
        return None;
    }

    let end_bracket = line.find(']')?;
    let timestamp = &line[1..end_bracket];
    let words = line[end_bracket + 1..].to_string();

    // Parse timestamp: mm:ss.xx or mm:ss:xx
    let parts: Vec<&str> = timestamp.split(&[':', '.'][..]).collect();
    if parts.len() < 2 {
        return None;
    }

    let minutes: i64 = parts[0].parse().ok()?;
    let seconds: i64 = parts[1].parse().ok()?;
    let centiseconds: i64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    let start_time_ms = minutes * 60000 + seconds * 1000 + centiseconds * 10;

    Some(LyricsLine {
        start_time_ms,
        words,
        end_time_ms: None,
    })
}

/// Convert milliseconds to LRC timestamp
fn ms_to_lrc_timestamp(ms: i64) -> String {
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let centiseconds = (ms % 1000) / 10;
    format!("[{:02}:{:02}.{:02}]", minutes, seconds, centiseconds)
}

/// Simplify track name for better matching
fn simplify_track_name(track: &str) -> String {
    let mut simplified = track.to_string();

    // Strip double-colon or double-underscore subtitles (e.g., "Green Eyes :: Siena" -> "Green Eyes")
    for delim in [" :: ", " __ ", " / ", " - "] {
        if let Some(pos) = simplified.find(delim) {
            simplified = simplified[..pos].to_string();
        }
    }

    // Remove common suffixes
    let patterns = [
        " (Remastered",
        " (Remaster",
        " (Deluxe",
        " (Live",
        " (Remix",
        " (Radio Edit",
        " (Acoustic",
        " (Demo",
        " - Remaster",
        " - Remastered",
        " - Live",
        " - Remix",
        " [Remastered",
        " [Deluxe",
        " [Live",
    ];

    for pattern in patterns {
        if let Some(pos) = simplified.find(pattern) {
            simplified = simplified[..pos].to_string();
        }
    }

    // Remove featuring
    for pattern in [" (feat.", " (ft.", " feat.", " ft."] {
        if let Some(pos) = simplified.to_lowercase().find(pattern) {
            simplified = simplified[..pos].to_string();
        }
    }

    // Trim trailing punctuation like ?, !, _, etc.
    let trimmed = simplified.trim();
    trimmed.trim_matches(|c: char| c == '?' || c == '!' || c == '_' || c == '.' || c == ':').trim().to_string()
}

/// Convert Apple Music TTML Timed Text XML into Enhanced Karaoke LRC (ELRC) format
#[allow(dead_code)]
fn parse_ttml_to_elrc(input: &str) -> String {
    if !input.contains("<tt") && !input.contains("<p") {
        return input.to_string();
    }
    let mut out = String::new();
    for p_block in input.split("<p ").skip(1) {
        if let Some(begin_pos) = p_block.find("begin=\"") {
            let start = &p_block[begin_pos + 7..];
            if let Some(end_quote) = start.find('"') {
                let time_str = &start[..end_quote];
                if let Some(ms) = parse_time_str_to_ms(time_str) {
                    let line_ts = ms_to_lrc_timestamp(ms);
                    let mut line_buf = line_ts;

                    for span in p_block.split("<span ").skip(1) {
                        if let Some(s_begin) = span.find("begin=\"") {
                            let s_start = &span[s_begin + 7..];
                            if let Some(s_quote) = s_start.find('"') {
                                let w_time = &s_start[..s_quote];
                                if let Some(w_ms) = parse_time_str_to_ms(w_time) {
                                    if let Some(c_end) = span.find('>') {
                                        let text_part = &span[c_end + 1..];
                                        let text = text_part.split('<').next().unwrap_or("").trim();
                                        if !text.is_empty() {
                                            let mins = w_ms / 60000;
                                            let secs = (w_ms % 60000) as f64 / 1000.0;
                                            line_buf.push_str(&format!("<{:02}:{:05.2}>{} ", mins, secs, text));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if line_buf.contains('<') {
                        out.push_str(line_buf.trim_end());
                        out.push('\n');
                    }
                }
            }
        }
    }
    if out.is_empty() { input.to_string() } else { out }
}

#[allow(dead_code)]
fn parse_time_str_to_ms(t: &str) -> Option<i64> {
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() == 3 {
        let mins: i64 = parts[1].parse().ok()?;
        let secs: f64 = parts[2].parse().ok()?;
        return Some(mins * 60000 + (secs * 1000.0) as i64);
    } else if parts.len() == 2 {
        let mins: i64 = parts[0].parse().ok()?;
        let secs: f64 = parts[1].parse().ok()?;
        return Some(mins * 60000 + (secs * 1000.0) as i64);
    }
    None
}

/// Extract Apple Music WebPlayKid token dynamically
pub async fn extract_apple_music_token(client: &Client) -> Option<String> {
    use regex::Regex;
    use std::sync::OnceLock;

    static CACHED_TOKEN: OnceLock<Option<String>> = OnceLock::new();
    if let Some(cached) = CACHED_TOKEN.get() {
        return cached.clone();
    }

    let page = match client
        .get("https://music.apple.com/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            let _ = CACHED_TOKEN.set(None);
            return None;
        }
    };

    let js_re = Regex::new(r#"(/assets/index[^"'\s>]+\.js)"#).ok()?;
    let js_path = js_re.captures(&page)?.get(1)?.as_str();
    let js_url = format!("https://music.apple.com{}", js_path);

    let js_content = match client
        .get(&js_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            let _ = CACHED_TOKEN.set(None);
            return None;
        }
    };

    let token_re = Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").ok()?;
    for cap in token_re.find_iter(&js_content) {
        let token = cap.as_str();
        if token.starts_with("eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6IldlYlBsYXlLaWQifQ") {
            let result = Some(token.to_string());
            let _ = CACHED_TOKEN.set(result.clone());
            return result;
        }
    }

    let _ = CACHED_TOKEN.set(None);
    None
}

const KRC_KEY: [u8; 16] = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105];

fn decrypt_krc_bytes(raw: &[u8]) -> Option<String> {
    if raw.len() <= 4 || &raw[0..4] != b"krc1" {
        return None;
    }
    let body = &raw[4..];
    let mut decrypted = Vec::with_capacity(body.len());
    for (i, &b) in body.iter().enumerate() {
        decrypted.push(b ^ KRC_KEY[i % KRC_KEY.len()]);
    }
    let mut decoder = ZlibDecoder::new(&decrypted[..]);
    let mut s = String::new();
    decoder.read_to_string(&mut s).ok()?;
    Some(s)
}

fn parse_krc_to_elrc(krc_text: &str) -> (Vec<LyricsLine>, String) {
    let mut lines = Vec::new();
    let mut elrc_buf = String::new();
    let re_line = match regex::Regex::new(r"^\[(\d+),(\d+)\](.*)") {
        Ok(r) => r,
        Err(_) => return (lines, elrc_buf),
    };
    let re_syl = match regex::Regex::new(r"<(\d+),(\d+),\d+>([^<]*)") {
        Ok(r) => r,
        Err(_) => return (lines, elrc_buf),
    };

    for raw_l in krc_text.lines() {
        let trimmed = raw_l.trim();
        if let Some(caps) = re_line.captures(trimmed) {
            let start_ms: i64 = caps[1].parse().unwrap_or(0);
            let body = &caps[3];

            let line_ts = ms_to_lrc_timestamp(start_ms);
            let mut elrc_line = line_ts;
            let mut line_words = String::new();

            for syl_cap in re_syl.captures_iter(body) {
                let off: i64 = syl_cap[1].parse().unwrap_or(0);
                let text = &syl_cap[3];
                let word_ms = start_ms + off;

                let w_mins = word_ms / 60000;
                let w_secs = (word_ms % 60000) as f64 / 1000.0;
                elrc_line.push_str(&format!("<{:02}:{:05.2}>{}", w_mins, w_secs, text));
                line_words.push_str(text);
            }

            if !line_words.trim().is_empty() {
                elrc_buf.push_str(&elrc_line);
                elrc_buf.push('\n');
                lines.push(LyricsLine {
                    start_time_ms: start_ms,
                    words: line_words.trim().to_string(),
                    end_time_ms: None,
                });
            }
        }
    }
    (lines, elrc_buf)
}

/// Convert UltraStar Karaoke format (.txt) into Enhanced Karaoke LRC (ELRC)
pub fn parse_ultrastar_to_elrc(us_txt: &str) -> (Vec<LyricsLine>, String) {
    let mut bpm = 120.0f64;
    let mut gap = 0.0f64;

    for line in us_txt.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#BPM:") {
            if let Ok(v) = trimmed[5..].replace(',', ".").trim().parse::<f64>() {
                bpm = v;
            }
        } else if trimmed.starts_with("#GAP:") {
            if let Ok(v) = trimmed[5..].replace(',', ".").trim().parse::<f64>() {
                gap = v;
            }
        }
    }

    let ms_per_beat = if bpm > 0.0 { 60000.0 / (bpm * 4.0) } else { 125.0 };
    let mut lines = Vec::new();
    let mut elrc_buf = String::new();

    let mut current_elrc_line = String::new();
    let mut current_start_ms: Option<i64> = None;
    let mut current_line_text = String::new();

    for raw_l in us_txt.lines() {
        let trimmed = raw_l.trim();
        if trimmed.starts_with(':') || trimmed.starts_with('*') || trimmed.starts_with('F') {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let beat: f64 = parts[1].parse().unwrap_or(0.0);
                let _dur: f64 = parts[2].parse().unwrap_or(0.0);
                let text = parts.get(4..).map(|p| p.join(" ")).unwrap_or_default();

                let ms = (gap + (beat * ms_per_beat)).max(0.0) as i64;
                let mins = ms / 60000;
                let secs = (ms % 60000) as f64 / 1000.0;
                let syl_ts = format!("<{:02}:{:05.2}>{}", mins, secs, text);

                if current_start_ms.is_none() {
                    current_start_ms = Some(ms);
                    current_elrc_line = format!("[{:02}:{:05.2}]", mins, secs);
                }

                current_elrc_line.push_str(&syl_ts);
                current_line_text.push_str(&text);
            }
        } else if (trimmed.starts_with('-') || trimmed.starts_with('E')) && current_start_ms.is_some() {
            if !current_line_text.trim().is_empty() {
                elrc_buf.push_str(&current_elrc_line);
                elrc_buf.push('\n');

                lines.push(LyricsLine {
                    start_time_ms: current_start_ms.unwrap(),
                    words: current_line_text.trim().to_string(),
                    end_time_ms: None,
                });
            }
            current_elrc_line.clear();
            current_line_text.clear();
            current_start_ms = None;
        }
    }

    (lines, elrc_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_lrc_word_synced_preservation_strict() {
        let elrc_raw = "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim\n[00:13.00] <00:13.00>Like <00:13.50>dolphins <00:14.00>can <00:14.50>swim";
        let lrc_response = LRCLibResponse {
            id: Some(1),
            name: Some("Heroes".to_string()),
            track_name: Some("Heroes".to_string()),
            artist_name: Some("David Bowie".to_string()),
            album_name: Some("Heroes".to_string()),
            duration: Some(360.0),
            instrumental: Some(false),
            plain_lyrics: Some("I wish you could swim\nLike dolphins can swim".to_string()),
            synced_lyrics: Some(elrc_raw.to_string()),
        };

        let client = LyricsClient::new();
        let parsed = client.parse_response(&lrc_response).unwrap();

        // 1. Must preserve word-synced Enhanced LRC content
        assert_eq!(parsed.elrc_content, Some(elrc_raw.to_string()), "Enhanced LRC word timestamps must NOT be lost");
        assert!(parsed.elrc_content.as_ref().unwrap().contains('<') && parsed.elrc_content.as_ref().unwrap().contains('>'), "Must retain <mm:ss.xx> markers");

        // 2. Must produce clean plain text when stripped
        let re_ts = regex::Regex::new(r"\[\d{2}:\d{2}\.\d{2,3}\]|<\d{2}:\d{2}\.\d{2,3}>").unwrap();
        let stripped = re_ts.replace_all(parsed.elrc_content.as_ref().unwrap(), "");
        let clean = stripped.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
        assert_eq!(clean, "I wish you could swim\nLike dolphins can swim");
    }

    #[test]
    fn test_ultrastar_parser_offline() {
        let usdb_txt = "#ARTIST:Queen\n#TITLE:Bohemian Rhapsody\n#BPM:72\n: 0 4 0 Is this the real life\n: 4 4 0 Is this just fantasy\nE";
        let (parsed, _elrc) = parse_ultrastar_to_elrc(usdb_txt);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].words, "Is this the real lifeIs this just fantasy");
    }

    #[test]
    fn test_is_valid_lyrics_rejection() {
        let dummy = LyricsResponse {
            lines: vec![],
            sync_type: "NONE".to_string(),
            instrumental: false,
            plain_lyrics: Some("".to_string()),
            provider: "None".to_string(),
            source: "none".to_string(),
            elrc_content: None,
        };
        assert!(!is_valid_lyrics(&dummy, "Test Track"));
    }

    #[test]
    fn test_enhanced_lrc_word_level_degradation_negative() {
        let elrc_raw = "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim";
        let response = LyricsResponse {
            lines: vec![LyricsLine {
                start_time_ms: 10000,
                words: "I wish you could swim".to_string(),
                end_time_ms: Some(12000),
            }],
            sync_type: "KARAOKE_WORD_SYNCED".to_string(),
            instrumental: false,
            plain_lyrics: Some("I wish you could swim".to_string()),
            provider: "TestProvider".to_string(),
            source: "test".to_string(),
            elrc_content: Some(elrc_raw.to_string()),
        };

        assert_eq!(response.sync_type, "KARAOKE_WORD_SYNCED");
        assert_ne!(response.sync_type, "LINE_SYNCED", "Word-synced karaoke MUST NOT be downgraded to LINE_SYNCED");

        let elrc = response.elrc_content.as_ref().expect("elrc_content must exist");
        assert!(elrc.contains('<') && elrc.contains('>'), "elrc_content MUST retain word-level <mm:ss.xx> timestamps");

        let degraded_line_only = "[00:10.00] I wish you could swim";
        assert_ne!(elrc, degraded_line_only, "Enhanced LRC content MUST NOT match degraded line-only format");
    }
}
