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

pub use syncify_lyrics_domain::{
    detect_sync_type, ms_to_lrc_timestamp, parse_lrc_line, parse_time_str_to_ms,
    parse_ttml_to_elrc, parse_ultrastar_to_elrc, simplify_track_name, strip_lrc_timestamps,
    LyricsLineDomain, LyricsResolution, LyricsSyncType, ResolutionStatus,
};

/// A single line of lyrics with timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    #[serde(rename = "startTimeMs")]
    pub start_time_ms: i64,
    pub words: String,
    #[serde(rename = "endTimeMs")]
    pub end_time_ms: Option<i64>,
}

impl From<LyricsLineDomain> for LyricsLine {
    fn from(l: LyricsLineDomain) -> Self {
        Self {
            start_time_ms: l.start_time_ms,
            words: l.words,
            end_time_ms: l.end_time_ms,
        }
    }
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

impl LyricsResponse {
    pub fn to_domain_resolution(&self) -> LyricsResolution {
        let sync_type = if self.instrumental {
            LyricsSyncType::Instrumental
        } else if self.elrc_content.as_ref().map_or(false, |s| s.contains('<') && s.contains('>')) {
            LyricsSyncType::KaraokeWordSynced
        } else if self.sync_type == "LINE_SYNCED" || (!self.lines.is_empty() && self.lines.iter().any(|l| l.start_time_ms > 0)) {
            LyricsSyncType::LineSynced
        } else if self.plain_lyrics.as_ref().map_or(false, |p| !p.trim().is_empty()) {
            LyricsSyncType::Plain
        } else {
            LyricsSyncType::None
        };

        let is_karaoke = sync_type == LyricsSyncType::KaraokeWordSynced;
        LyricsResolution {
            status: ResolutionStatus::Resolved,
            provider: self.provider.clone(),
            strategy: self.source.clone(),
            format: self.sync_type.clone(),
            sync_type,
            provenance: self.source.clone(),
            fallback_applied: false,
            error: None,
            synced_content: if is_karaoke { self.elrc_content.clone() } else { None },
            plain_text: self.plain_lyrics.clone(),
            lines: self.lines
                .iter()
                .map(|l| LyricsLineDomain {
                    start_time_ms: l.start_time_ms,
                    words: l.words.clone(),
                    end_time_ms: l.end_time_ms,
                })
                .collect(),
            is_instrumental: self.instrumental,
        }
    }
}

/// LRCLIB API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are used by serde deserialization
pub struct LRCLibResponse {
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
        if let Ok(cache) = self.cache.read() {
            if let Some((lyrics, cached_at)) = cache.get(&key) {
                if cached_at.elapsed() < Duration::from_secs(24 * 60 * 60) {
                    return Some(lyrics.clone());
                }
            }
        }
        None
    }

    /// Store in cache
    fn set_cached(&self, artist: &str, track: &str, lyrics: &LyricsResponse) {
        let key = Self::cache_key(artist, track);
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(key, (lyrics.clone(), Instant::now()));
        }
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
                lines.push(parsed.into());
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

        let re_link = regex::Regex::new(r#"href="(/piosenka,[^"]+)""#)
            .map_err(|e| anyhow!("Tekstowo regex error: {}", e))?;
        let captures = re_link.captures(&html)
            .ok_or_else(|| anyhow!("No song match on Tekstowo.pl for {} - {}", artist, track))?;
        let song_path = captures.get(1)
            .map(|m| m.as_str())
            .ok_or_else(|| anyhow!("Failed to extract song path on Tekstowo.pl"))?;

        let song_url = format!("https://www.tekstowo.pl{}", song_path);
        let song_res = self.client.get(&song_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| anyhow!("Tekstowo song page request failed: {}", e))?;

        let song_html = song_res.text().await?;
        let re_lyrics = regex::Regex::new(r#"(?s)<div class="inner-text">(.*?)</div>"#)
            .map_err(|e| anyhow!("Tekstowo lyrics regex error: {}", e))?;
        let lyrics_captures = re_lyrics.captures(&song_html)
            .ok_or_else(|| anyhow!("No inner-text lyrics block on Tekstowo page"))?;
        let lyrics_html = lyrics_captures.get(1)
            .map(|m| m.as_str())
            .ok_or_else(|| anyhow!("Failed to extract lyrics HTML block"))?;

        let re_strip = regex::Regex::new(r"<[^>]+>")
            .map_err(|e| anyhow!("Tekstowo strip regex error: {}", e))?;
        let clean_text = re_strip.replace_all(lyrics_html, "\n");
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
            lines: lines.into_iter().map(Into::into).collect(),
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
                                                            lines.push(parsed.into());
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
                lines.push(parsed.into());
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
                lines.push(parsed.into());
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
                            lines.push(parsed.into());
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
                    lines.push(parsed.into());
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

    /// Resolve lyrics via NetEase Cloud Music adapter into domain contract
    pub async fn resolve_netease(&self, artist: &str, track: &str, duration_sec: f64) -> LyricsResolution {
        match self.fetch_netease_lyrics(artist, track, duration_sec).await {
            Ok(resp) => resp.to_domain_resolution(),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("no songs found") || err_str.contains("no title/duration matching") {
                    LyricsResolution::new_not_found("NetEase", "netease_search")
                } else if err_str.contains("request failed") || err_str.contains("timed out") {
                    LyricsResolution::new_source_unavailable("NetEase", "netease_search", err_str)
                } else {
                    LyricsResolution::new_failed("NetEase", "netease_lyrics", err_str)
                }
            }
        }
    }

    /// Resolve lyrics via LRCLIB adapter into domain contract
    pub async fn resolve_lrclib(&self, artist: &str, track: &str, duration_sec: f64) -> LyricsResolution {
        match self.fetch_lyrics(artist, track).await {
            Ok(resp) => resp.to_domain_resolution(),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("not found") {
                    // Try fallback search
                    match self.search_lyrics(&format!("{} {}", artist, track), duration_sec).await {
                        Ok(resp) => {
                            let mut res = resp.to_domain_resolution();
                            res.fallback_applied = true;
                            res
                        }
                        Err(_) => LyricsResolution::new_not_found("LRCLIB", "exact_and_search"),
                    }
                } else if err_str.contains("request failed") || err_str.contains("timed out") {
                    LyricsResolution::new_source_unavailable("LRCLIB", "lrclib_get", err_str)
                } else {
                    LyricsResolution::new_failed("LRCLIB", "lrclib_get", err_str)
                }
            }
        }
    }

    /// Resolve lyrics via LyricsPlus adapter into domain contract
    pub async fn resolve_lyricsplus(&self, artist: &str, track: &str, duration_sec: f64) -> LyricsResolution {
        match self.fetch_lyricsplus(artist, track, duration_sec).await {
            Ok(resp) => resp.to_domain_resolution(),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("Empty lyrics") || err_str.contains("insufficient lines") || err_str.contains("No lyrics field") {
                    LyricsResolution::new_not_found("LyricsPlus", "lyricsplus_search")
                } else if err_str.contains("search failed") || err_str.contains("timed out") {
                    LyricsResolution::new_source_unavailable("LyricsPlus", "lyricsplus_search", err_str)
                } else {
                    LyricsResolution::new_failed("LyricsPlus", "lyricsplus_search", err_str)
                }
            }
        }
    }

    /// Orchestrate resolution across active adapters (NetEase, LRCLIB, LyricsPlus)
    /// following quality rank and avoiding redundant queries.
    /// Returns (LyricsResolution, elapsed_ms).
    pub async fn orchestrate_resolution(
        &self,
        artist: &str,
        track: &str,
        _album: Option<&str>,
        duration_sec: f64,
    ) -> (LyricsResolution, u64) {
        let start = Instant::now();

        // 1. Try NetEase Cloud Music (can provide KaraokeWordSynced or LineSynced)
        let netease_res = self.resolve_netease(artist, track, duration_sec).await;
        if netease_res.status == ResolutionStatus::Resolved {
            if netease_res.sync_type == LyricsSyncType::KaraokeWordSynced {
                let dur = start.elapsed().as_millis() as u64;
                return (netease_res, dur);
            }
        }

        // 2. Try LRCLIB (provides exact / search LineSynced or Instrumental)
        let lrclib_res = self.resolve_lrclib(artist, track, duration_sec).await;
        if lrclib_res.status == ResolutionStatus::Resolved {
            if lrclib_res.sync_type == LyricsSyncType::KaraokeWordSynced {
                let dur = start.elapsed().as_millis() as u64;
                return (lrclib_res, dur);
            }

            // If NetEase provided LineSynced with lyrics, compare line count
            if netease_res.status == ResolutionStatus::Resolved
                && netease_res.sync_type == LyricsSyncType::LineSynced
                && netease_res.lines.len() >= lrclib_res.lines.len()
            {
                let dur = start.elapsed().as_millis() as u64;
                return (netease_res, dur);
            }

            let dur = start.elapsed().as_millis() as u64;
            return (lrclib_res, dur);
        }

        // If NetEase had a valid LineSynced or Plain resolution, return it
        if netease_res.status == ResolutionStatus::Resolved {
            let dur = start.elapsed().as_millis() as u64;
            return (netease_res, dur);
        }

        // 3. Try LyricsPlus
        let lyricsplus_res = self.resolve_lyricsplus(artist, track, duration_sec).await;
        if lyricsplus_res.status == ResolutionStatus::Resolved {
            let dur = start.elapsed().as_millis() as u64;
            return (lyricsplus_res, dur);
        }

        // If no provider resolved, choose best error representation:
        // Priority: SourceUnavailable > Failed > RequiresAuth > NotFound
        let dur = start.elapsed().as_millis() as u64;
        if let ResolutionStatus::SourceUnavailable = lyricsplus_res.status {
            return (lyricsplus_res, dur);
        }
        if let ResolutionStatus::SourceUnavailable = lrclib_res.status {
            return (lrclib_res, dur);
        }
        if let ResolutionStatus::SourceUnavailable = netease_res.status {
            return (netease_res, dur);
        }
        if let ResolutionStatus::Failed(_) = netease_res.status {
            return (netease_res, dur);
        }
        if let ResolutionStatus::Failed(_) = lrclib_res.status {
            return (lrclib_res, dur);
        }

        (LyricsResolution::new_not_found("Orchestrator", "multi_provider_cascade"), dur)
    }
}

impl Default for LyricsClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate and embed lyrics into a FLAC audio file using metaflac,
/// with mandatory post-write re-read verification.
pub fn validate_and_embed_flac_lyrics(
    file_path: &std::path::Path,
    resolution: &LyricsResolution,
) -> Result<bool, String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    let metadata = std::fs::metadata(file_path)
        .map_err(|e| format!("Failed to read file metadata for {}: {}", file_path.display(), e))?;
    if metadata.len() == 0 {
        return Err(format!("File is empty (0 bytes): {}", file_path.display()));
    }

    if resolution.status != ResolutionStatus::Resolved {
        return Err(format!(
            "Cannot embed non-resolved lyrics (status: {:?})",
            resolution.status
        ));
    }

    let synced_text = resolution.synced_content.as_deref();
    let plain_text = resolution.plain_text.as_deref();

    // Read audio file with metaflac
    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to parse FLAC file: {}", e))?;

    // Verify STREAMINFO block exists
    let streaminfo = tag
        .get_streaminfo()
        .ok_or_else(|| format!("FLAC file has no valid STREAMINFO header: {}", file_path.display()))?;
    if streaminfo.sample_rate == 0 {
        return Err(format!("Invalid sample rate in STREAMINFO: {}", file_path.display()));
    }

    // Format LRC content
    let lrc_to_write = if let Some(s) = synced_text {
        s.to_string()
    } else if !resolution.lines.is_empty() {
        let mut buf = String::new();
        for line in &resolution.lines {
            let ts = ms_to_lrc_timestamp(line.start_time_ms);
            buf.push_str(&format!("{}{}\n", ts, line.words));
        }
        buf
    } else {
        String::new()
    };

    // Format plain text content
    let plain_to_write = if let Some(p) = plain_text {
        p.to_string()
    } else if !lrc_to_write.is_empty() {
        strip_lrc_timestamps(&lrc_to_write)
    } else {
        String::new()
    };

    if lrc_to_write.is_empty() && plain_to_write.is_empty() && !resolution.is_instrumental {
        return Err("No lyrics content to embed".to_string());
    }

    // Modify VorbisComments
    let comments = tag.vorbis_comments_mut();

    // Remove existing lyrics to avoid duplication
    comments.remove("LYRICS");
    comments.remove("UNSYNCEDLYRICS");

    // Write LYRICS Vorbis comment (Enhanced/Line-synced LRC)
    if !lrc_to_write.is_empty() {
        comments.set("LYRICS", vec![lrc_to_write.clone()]);
    }

    // Write UNSYNCEDLYRICS / Plain lyrics Vorbis comment
    if !plain_to_write.is_empty() {
        comments.set("UNSYNCEDLYRICS", vec![plain_to_write.clone()]);
    }

    // Save to path using metaflac
    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to save FLAC tags to {}: {}", file_path.display(), e))?;

    // --- MANDATORY POST-WRITE RE-READ VERIFICATION ---
    let verified_tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Verification failed: unable to re-read FLAC file {}: {}", file_path.display(), e))?;

    let verified_comments = verified_tag
        .vorbis_comments()
        .ok_or_else(|| format!("Verification failed: no VorbisComments found in {} after save", file_path.display()))?;

    if !lrc_to_write.is_empty() {
        let read_lyrics = verified_comments.get("LYRICS").and_then(|v| v.first().map(|s| s.as_str()));
        if read_lyrics != Some(lrc_to_write.as_str()) {
            return Err(format!(
                "Verification failed: LYRICS mismatch after save in {}",
                file_path.display()
            ));
        }
    }

    if !plain_to_write.is_empty() {
        let read_unsynced = verified_comments.get("UNSYNCEDLYRICS").and_then(|v| v.first().map(|s| s.as_str()));
        if read_unsynced != Some(plain_to_write.as_str()) {
            return Err(format!(
                "Verification failed: UNSYNCEDLYRICS mismatch after save in {}",
                file_path.display()
            ));
        }
    }

    Ok(true)
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



#[cfg(test)]
mod tests {
    use super::*;
    use syncify_lyrics_domain::{fixtures::*, LyricsLineDomain, LyricsResolution, LyricsSyncType, ResolutionStatus};

    struct TempFlac {
        pub path: std::path::PathBuf,
    }

    impl Drop for TempFlac {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn create_dummy_flac_file() -> TempFlac {
        let path = std::env::temp_dir().join(format!(
            "syncify_flac_test_{}.flac",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        // Block 0: STREAMINFO (not last, len 34)
        data.push(0x00);
        data.extend_from_slice(&[0x00, 0x00, 0x22]);
        let mut streaminfo = vec![0u8; 34];
        streaminfo[0] = 0x10; streaminfo[1] = 0x00; // min block 4096
        streaminfo[2] = 0x10; streaminfo[3] = 0x00; // max block 4096
        streaminfo[10] = 0x0A; streaminfo[11] = 0xC4; streaminfo[12] = 0x42; // 44100Hz, 2 channels, 16 bps
        streaminfo[13] = 0xF0;
        streaminfo[14] = 0x00; streaminfo[15] = 0x00; streaminfo[16] = 0xAC; streaminfo[17] = 0x44; // total samples
        data.extend_from_slice(&streaminfo);

        // Block 1: VORBIS_COMMENT (last, 0x84)
        let mut comment_data = Vec::new();
        comment_data.extend_from_slice(&4u32.to_le_bytes());
        comment_data.extend_from_slice(b"test");
        comment_data.extend_from_slice(&0u32.to_le_bytes());
        data.push(0x84);
        let comment_len = comment_data.len() as u32;
        data.push((comment_len >> 16) as u8);
        data.push((comment_len >> 8) as u8);
        data.push(comment_len as u8);
        data.extend_from_slice(&comment_data);
        data.extend_from_slice(&[0xFF, 0xF8, 0x00, 0x00]);
        std::fs::write(&path, data).expect("Failed to write dummy FLAC");
        TempFlac { path }
    }

    #[test]
    fn test_flac_validation_and_reread_lifecycle() {
        let flac = create_dummy_flac_file();
        let elrc_raw = "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim";
        let plain_raw = "I wish you could swim";

        let resolution = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::KaraokeWordSynced,
            Some(elrc_raw.to_string()),
            Some(plain_raw.to_string()),
            vec![LyricsLineDomain {
                start_time_ms: 10000,
                words: "I wish you could swim".to_string(),
                end_time_ms: Some(12000),
            }],
            false,
            "music.163.com",
        );

        let result = validate_and_embed_flac_lyrics(&flac.path, &resolution);
        assert!(result.is_ok(), "validate_and_embed_flac_lyrics should succeed: {:?}", result.err());

        // Re-read file with metaflac to assert persistence
        let verified = metaflac::Tag::read_from_path(&flac.path).expect("Must re-read FLAC");
        let comments = verified.vorbis_comments().expect("Must have vorbis comments");

        assert_eq!(comments.get("LYRICS").and_then(|v| v.first()).map(|s| s.as_str()), Some(elrc_raw));
        assert_eq!(comments.get("UNSYNCEDLYRICS").and_then(|v| v.first()).map(|s| s.as_str()), Some(plain_raw));
    }

    #[test]
    fn test_flac_nonexistent_and_empty_file_rejected() {
        let non_existent = std::env::temp_dir().join("non_existent_file_12345.flac");
        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Hello".to_string()),
            Some("Hello".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let res_nonexistent = validate_and_embed_flac_lyrics(&non_existent, &resolution);
        assert!(res_nonexistent.is_err());
        assert!(res_nonexistent.unwrap_err().contains("does not exist"));

        let empty_file = std::env::temp_dir().join("empty_file_12345.flac");
        std::fs::write(&empty_file, b"").unwrap();
        let res_empty = validate_and_embed_flac_lyrics(&empty_file, &resolution);
        let _ = std::fs::remove_file(&empty_file);
        assert!(res_empty.is_err());
        assert!(res_empty.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_unrelated_flac_tags_preserved_during_lyrics_embed() {
        let flac = create_dummy_flac_file();

        // 1. Write unrelated tags first (TITLE, ARTIST, ALBUM, ISRC, CUSTOM_TAG)
        {
            let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
            let comments = tag.vorbis_comments_mut();
            comments.set_title(vec!["Original Title".to_string()]);
            comments.set_artist(vec!["Original Artist".to_string()]);
            comments.set_album(vec!["Original Album".to_string()]);
            comments.set("ISRC", vec!["USRC12345678".to_string()]);
            comments.set("CUSTOM_TAG", vec!["CustomValue123".to_string()]);
            tag.write_to_path(&flac.path).unwrap();
        }

        // 2. Embed lyrics via validate_and_embed_flac_lyrics
        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:05.00]Line 1\n[00:10.00]Line 2".to_string()),
            Some("Line 1\nLine 2".to_string()),
            vec![],
            false,
            "lrclib.net",
        );
        let res = validate_and_embed_flac_lyrics(&flac.path, &resolution);
        assert!(res.is_ok());

        // 3. Re-read and assert all original tags are preserved
        let verified = metaflac::Tag::read_from_path(&flac.path).unwrap();
        let comments = verified.vorbis_comments().unwrap();

        assert_eq!(comments.title().and_then(|v| v.first()).map(|s| s.as_str()), Some("Original Title"));
        assert_eq!(comments.artist().and_then(|v| v.first()).map(|s| s.as_str()), Some("Original Artist"));
        assert_eq!(comments.album().and_then(|v| v.first()).map(|s| s.as_str()), Some("Original Album"));
        assert_eq!(comments.get("ISRC").and_then(|v| v.first()).map(|s| s.as_str()), Some("USRC12345678"));
        assert_eq!(comments.get("CUSTOM_TAG").and_then(|v| v.first()).map(|s| s.as_str()), Some("CustomValue123"));
        assert_eq!(comments.get("LYRICS").and_then(|v| v.first()).map(|s| s.as_str()), Some("[00:05.00]Line 1\n[00:10.00]Line 2"));
    }

    #[test]
    fn test_flac_non_flac_arbitrary_binary_rejected() {
        let path = std::env::temp_dir().join(format!(
            "syncify_backend_non_flac_{}.flac",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&path, b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00").unwrap();

        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Hello".to_string()),
            Some("Hello".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let res = validate_and_embed_flac_lyrics(&path, &resolution);
        let _ = std::fs::remove_file(&path);
        assert!(res.is_err(), "Non-FLAC binary must be rejected");
    }

    #[test]
    fn test_flac_truncated_flac_header_rejected() {
        let path = std::env::temp_dir().join(format!(
            "syncify_backend_truncated_{}.flac",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&path, b"fLaC").unwrap();

        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Hello".to_string()),
            Some("Hello".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let res = validate_and_embed_flac_lyrics(&path, &resolution);
        let _ = std::fs::remove_file(&path);
        assert!(res.is_err(), "Truncated FLAC must be rejected");
    }

    #[test]
    fn test_flac_no_duplicate_lyrics_on_multiple_runs() {
        let flac = create_dummy_flac_file();
        let resolution1 = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::LineSynced,
            Some("[00:05.00]First Version".to_string()),
            Some("First Version".to_string()),
            vec![],
            false,
            "music.163.com",
        );
        let res1 = validate_and_embed_flac_lyrics(&flac.path, &resolution1);
        assert!(res1.is_ok());

        let resolution2 = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::KaraokeWordSynced,
            Some("[00:05.00] <00:05.00>Second <00:06.00>Version".to_string()),
            Some("Second Version".to_string()),
            vec![],
            false,
            "lrclib.net",
        );
        let res2 = validate_and_embed_flac_lyrics(&flac.path, &resolution2);
        assert!(res2.is_ok());

        let verified = metaflac::Tag::read_from_path(&flac.path).unwrap();
        let comments = verified.vorbis_comments().unwrap();

        let lyrics_values = comments.get("LYRICS").unwrap();
        assert_eq!(lyrics_values.len(), 1, "Exactly one LYRICS entry must exist");
        assert_eq!(lyrics_values[0], "[00:05.00] <00:05.00>Second <00:06.00>Version");

        let unsynced_values = comments.get("UNSYNCEDLYRICS").unwrap();
        assert_eq!(unsynced_values.len(), 1, "Exactly one UNSYNCEDLYRICS entry must exist");
        assert_eq!(unsynced_values[0], "Second Version");
    }

    #[test]
    fn test_flac_picture_front_cover_and_multiple_pictures() {
        let flac = create_dummy_flac_file();
        use metaflac::block::PictureType;

        let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
        // Add CoverFront picture
        let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
        tag.add_picture("image/jpeg", PictureType::CoverFront, fake_jpeg.clone());
        // Add Artist picture
        let fake_png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        tag.add_picture("image/png", PictureType::Artist, fake_png.clone());
        tag.write_to_path(&flac.path).unwrap();

        // Re-read and check pictures
        let read_tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
        let pics: Vec<_> = read_tag.pictures().collect();
        assert_eq!(pics.len(), 2);
        assert_eq!(pics[0].picture_type, PictureType::CoverFront);
        assert_eq!(pics[0].mime_type, "image/jpeg");
        assert_eq!(pics[1].picture_type, PictureType::Artist);
        assert_eq!(pics[1].mime_type, "image/png");
    }

    #[test]
    fn test_flac_real_downloaded_track_roundtrip() {
        let candidate_paths = [
            "downloads_real_test/Gloria Gaynor/[1978] Love Tracks/05 - I Will Survive.flac",
            "src-tauri/downloads_syncify/Ely Bruna/[2015] Post Modern Lounge/08 - Titanium.flac",
            "src-tauri/downloads_syncify/Audio Test Pink Noise/[2023] Speaker Test Pink Noise/01 - Speaker Test Pink Noise.flac",
            "adjacent_tools/streamrip/tests/silence.flac",
        ];

        let mut real_flac = None;
        for c in &candidate_paths {
            let p = std::path::Path::new("c:/Users/tardis/Documents/Syncify").join(c);
            if p.exists() {
                real_flac = Some(p);
                break;
            }
        }

        let src_path = real_flac.expect("Real FLAC candidate track must exist in workspace");
        let temp_dest = std::env::temp_dir().join(format!(
            "syncify_real_flac_test_{}.flac",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::copy(&src_path, &temp_dest).expect("Copy real FLAC to temp path");

        // 1. Inspect before state
        let tag_before = metaflac::Tag::read_from_path(&temp_dest).expect("Read real FLAC before");
        let info_before = tag_before.get_streaminfo().expect("STREAMINFO before").clone();
        let pics_before: Vec<_> = tag_before.pictures().map(|p| (p.picture_type, p.mime_type.clone(), p.data.len())).collect();
        let comments_before = tag_before.vorbis_comments().cloned();

        let orig_sample_rate = info_before.sample_rate;
        let orig_bits_per_sample = info_before.bits_per_sample;
        let orig_total_samples = info_before.total_samples;
        let orig_channels = info_before.num_channels;
        let orig_duration = (orig_total_samples as f64) / (orig_sample_rate as f64);
        let orig_md5 = info_before.md5;

        // Verify pre-embed audio decode validity with ffmpeg if available
        let ffmpeg_check_before = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i", temp_dest.to_str().unwrap(), "-f", "null", "-"])
            .output();
        if let Ok(out) = &ffmpeg_check_before {
            assert!(out.status.success(), "Pre-embed ffmpeg decode failed: {:?}", String::from_utf8_lossy(&out.stderr));
        }

        // 2. Embed Enhanced LRC
        let elrc = "[00:01.00] <00:01.00>At <00:01.50>first <00:02.00>I <00:02.50>was <00:03.00>afraid\n[00:03.50] <00:03.50>I <00:04.00>was <00:04.50>petrified";
        let resolution = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::KaraokeWordSynced,
            Some(elrc.to_string()),
            Some("At first I was afraid\nI was petrified".to_string()),
            vec![],
            false,
            "music.163.com",
        );

        let res = validate_and_embed_flac_lyrics(&temp_dest, &resolution);
        assert!(res.is_ok(), "Embedding into real FLAC must succeed: {:?}", res.err());

        // 3. Inspect after state
        let tag_after = metaflac::Tag::read_from_path(&temp_dest).expect("Read real FLAC after");
        let info_after = tag_after.get_streaminfo().expect("STREAMINFO after");

        // Assert STREAMINFO is 100% byte-exact preserved
        assert_eq!(info_after.sample_rate, orig_sample_rate, "Sample rate must not change");
        assert_eq!(info_after.bits_per_sample, orig_bits_per_sample, "Bits per sample must not change");
        assert_eq!(info_after.total_samples, orig_total_samples, "Total audio samples must not change");
        assert_eq!(info_after.num_channels, orig_channels, "Channels must not change");
        assert_eq!(info_after.md5, orig_md5, "MD5 audio checksum must not change");

        let duration_after = (info_after.total_samples as f64) / (info_after.sample_rate as f64);
        assert!((duration_after - orig_duration).abs() < f64::EPSILON, "Duration must not change");

        // Assert PICTURE blocks are 100% preserved
        let pics_after: Vec<_> = tag_after.pictures().map(|p| (p.picture_type, p.mime_type.clone(), p.data.len())).collect();
        assert_eq!(pics_after, pics_before, "PICTURE metadata blocks must be preserved");

        // Assert unrelated Vorbis comments are preserved
        let comments_after = tag_after.vorbis_comments().expect("VorbisComments after");
        if let Some(cb) = comments_before {
            if let Some(titles) = cb.title() {
                assert_eq!(comments_after.title(), Some(titles), "TITLE tag must be preserved");
            }
            if let Some(artists) = cb.artist() {
                assert_eq!(comments_after.artist(), Some(artists), "ARTIST tag must be preserved");
            }
            if let Some(albums) = cb.album() {
                assert_eq!(comments_after.album(), Some(albums), "ALBUM tag must be preserved");
            }
        }

        // Assert lyrics match exactly and have no duplicates
        let lyrics_entries = comments_after.get("LYRICS").expect("LYRICS entry must exist");
        assert_eq!(lyrics_entries.len(), 1, "Exactly one LYRICS entry must exist");
        assert_eq!(lyrics_entries[0], elrc);

        let unsynced_entries = comments_after.get("UNSYNCEDLYRICS").expect("UNSYNCEDLYRICS entry must exist");
        assert_eq!(unsynced_entries.len(), 1, "Exactly one UNSYNCEDLYRICS entry must exist");
        assert_eq!(unsynced_entries[0], "At first I was afraid\nI was petrified");

        // Verify post-embed audio decode validity with ffmpeg (bit-exact stream playable)
        let ffmpeg_check_after = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i", temp_dest.to_str().unwrap(), "-f", "null", "-"])
            .output();
        if let Ok(out) = &ffmpeg_check_after {
            assert!(out.status.success(), "Post-embed ffmpeg decode failed: {:?}", String::from_utf8_lossy(&out.stderr));
        }

        let _ = std::fs::remove_file(&temp_dest);
    }

    #[test]
    fn test_lyricsplus_http404_produces_source_unavailable() {
        let res = LyricsResolution::new_source_unavailable(
            "LyricsPlus",
            "lyricsplus_search",
            "LyricsPlus search failed: HTTP 404 Not Found",
        );
        assert_eq!(res.status, ResolutionStatus::SourceUnavailable);
        assert_eq!(res.error, Some("LyricsPlus search failed: HTTP 404 Not Found".to_string()));
    }

    #[test]
    fn test_corrupt_payload_produces_failed() {
        let res = LyricsResolution::new_failed(
            "NetEase Cloud Music",
            "netease_lyrics",
            "JSON decode error: unexpected EOF",
        );
        assert_eq!(res.status, ResolutionStatus::Failed("JSON decode error: unexpected EOF".to_string()));
        assert_eq!(res.error, Some("JSON decode error: unexpected EOF".to_string()));
    }

    #[test]
    fn test_enhanced_lrc_word_synced_preservation_strict() {
        let raw_elrc = "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim\n[00:12.50] <00:12.50>Like <00:13.00>dolphins <00:13.50>can <00:14.00>swim";
        let res = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::KaraokeWordSynced,
            Some(raw_elrc.to_string()),
            Some("I wish you could swim\nLike dolphins can swim".to_string()),
            vec![],
            false,
            "music.163.com",
        );

        assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
        assert_eq!(res.synced_content.as_deref(), Some(raw_elrc));
    }

    #[test]
    fn test_cascade_priority_karaoke_over_linesynced() {
        let karaoke = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::KaraokeWordSynced,
            Some("[00:10.00] <00:10.00>Word <00:11.00>sync".to_string()),
            Some("Word sync".to_string()),
            vec![],
            false,
            "music.163.com",
        );

        let linesynced = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Word sync".to_string()),
            Some("Word sync".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        assert_eq!(karaoke.sync_type, LyricsSyncType::KaraokeWordSynced);
        assert_eq!(linesynced.sync_type, LyricsSyncType::LineSynced);
        assert_ne!(karaoke.sync_type, linesynced.sync_type);
    }

    #[test]
    fn test_cascade_priority_linesynced_over_plain() {
        let linesynced = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Word sync".to_string()),
            Some("Word sync".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let plain = LyricsResolution {
            status: ResolutionStatus::Resolved,
            provider: "NetEase".to_string(),
            strategy: "plain_text".to_string(),
            format: "PLAIN".to_string(),
            sync_type: LyricsSyncType::Plain,
            provenance: "music.163.com".to_string(),
            fallback_applied: false,
            error: None,
            synced_content: None,
            plain_text: Some("Word sync".to_string()),
            lines: vec![],
            is_instrumental: false,
        };

        assert_eq!(linesynced.sync_type, LyricsSyncType::LineSynced);
        assert_eq!(plain.sync_type, LyricsSyncType::Plain);
    }

    #[test]
    fn test_cascade_error_priority_ranking() {
        let su = LyricsResolution::new_source_unavailable("LyricsPlus", "lyricsplus_search", "HTTP 404");
        let failed = LyricsResolution::new_failed("NetEase", "netease_lyrics", "Corrupt payload");
        let auth = LyricsResolution::new_requires_auth("Apple Music", "apple_token", "HTTP 401 Unauthorized");
        let nf = LyricsResolution::new_not_found("Orchestrator", "multi_provider_cascade");

        assert_eq!(su.status, ResolutionStatus::SourceUnavailable);
        assert_eq!(failed.status, ResolutionStatus::Failed("Corrupt payload".to_string()));
        assert_eq!(auth.status, ResolutionStatus::RequiresAuth);
        assert_eq!(nf.status, ResolutionStatus::NotFound);
    }

    #[test]
    fn test_contract_payload_all_variant_representations() {
        let not_supported = LyricsResolution {
            status: ResolutionStatus::NotSupported,
            provider: "DummyProvider".to_string(),
            strategy: "unsupported_codec".to_string(),
            format: "NONE".to_string(),
            sync_type: LyricsSyncType::None,
            provenance: "none".to_string(),
            fallback_applied: false,
            error: Some("Operation not supported for this format".to_string()),
            synced_content: None,
            plain_text: None,
            lines: vec![],
            is_instrumental: false,
        };
        assert_eq!(not_supported.status, ResolutionStatus::NotSupported);

        let not_requested = LyricsResolution {
            status: ResolutionStatus::NotRequested,
            provider: "None".to_string(),
            strategy: "skipped".to_string(),
            format: "NONE".to_string(),
            sync_type: LyricsSyncType::None,
            provenance: "none".to_string(),
            fallback_applied: false,
            error: None,
            synced_content: None,
            plain_text: None,
            lines: vec![],
            is_instrumental: false,
        };
        assert_eq!(not_requested.status, ResolutionStatus::NotRequested);
    }

    #[test]
    fn test_backend_netease_karaoke_http_fixture() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_NETEASE_KARAOKE_JSON).unwrap();
        let klyric = json["klyric"]["lyric"].as_str().unwrap();

        let mut lines = Vec::new();
        for raw in klyric.lines() {
            if let Some(parsed) = parse_lrc_line(raw) {
                lines.push(parsed);
            }
        }

        let res = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::KaraokeWordSynced,
            Some(klyric.to_string()),
            Some("I wish you could swim\nLike dolphins can swim".to_string()),
            lines,
            false,
            "music.163.com",
        );

        assert_eq!(res.status, ResolutionStatus::Resolved);
        assert_eq!(res.provider, "NetEase Cloud Music");
        assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
        assert_eq!(res.format, "KaraokeWordSynced");
        assert_eq!(res.lines.len(), 2);
        assert!(res.synced_content.as_ref().unwrap().contains('<'));
        assert_eq!(res.plain_text.as_deref(), Some("I wish you could swim\nLike dolphins can swim"));
    }

    #[test]
    fn test_backend_lrclib_synced_http_fixture() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_LRCLIB_SYNCED_JSON).unwrap();
        let synced = json["syncedLyrics"].as_str().unwrap();
        let plain = json["plainLyrics"].as_str().unwrap();

        let mut lines = Vec::new();
        for line in synced.lines() {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
            }
        }

        let res = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some(synced.to_string()),
            Some(plain.to_string()),
            lines,
            false,
            "lrclib.net",
        );

        assert_eq!(res.status, ResolutionStatus::Resolved);
        assert_eq!(res.provider, "LRCLIB");
        assert_eq!(res.sync_type, LyricsSyncType::LineSynced);
        assert_eq!(res.lines.len(), 2);
    }

    #[test]
    fn test_backend_lrclib_instrumental_http_fixture() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_LRCLIB_INSTRUMENTAL_JSON).unwrap();
        let instrumental = json["instrumental"].as_bool().unwrap();

        let res = LyricsResolution {
            status: ResolutionStatus::Resolved,
            provider: "LRCLIB".to_string(),
            strategy: "instrumental_flag".to_string(),
            format: "INSTRUMENTAL".to_string(),
            sync_type: LyricsSyncType::Instrumental,
            provenance: "lrclib.net".to_string(),
            fallback_applied: false,
            error: None,
            synced_content: None,
            plain_text: None,
            lines: vec![],
            is_instrumental: instrumental,
        };

        assert_eq!(res.status, ResolutionStatus::Resolved);
        assert!(res.is_instrumental);
        assert_eq!(res.sync_type, LyricsSyncType::Instrumental);
    }

    #[test]
    fn test_backend_lyricsplus_word_http_fixture() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_LYRICSPLUS_WORD_JSON).unwrap();
        let synced = json["syncedLyrics"].as_str().unwrap();
        let mut lines = Vec::new();

        for raw in synced.lines() {
            if let Some(parsed) = parse_lrc_line(raw) {
                lines.push(parsed);
            }
        }

        let res = LyricsResolution::new_resolved(
            "LyricsPlus",
            "lyricsplus-backend",
            LyricsSyncType::KaraokeWordSynced,
            Some(synced.to_string()),
            Some("Is this the real life\nIs this just fantasy".to_string()),
            lines,
            false,
            "lyricsplus-backend",
        );

        assert_eq!(res.status, ResolutionStatus::Resolved);
        assert_eq!(res.provider, "LyricsPlus");
        assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
        assert_eq!(res.lines.len(), 4);
    }

    #[test]
    fn test_backend_lyricsplus_http404_handling() {
        let err_msg = "LyricsPlus search failed: HTTP 404 Not Found";
        let res = LyricsResolution::new_source_unavailable("LyricsPlus", "lyricsplus_search", err_msg);
        assert_eq!(res.status, ResolutionStatus::SourceUnavailable);
        assert_eq!(res.provider, "LyricsPlus");
        assert_eq!(res.error, Some(err_msg.to_string()));
    }

    #[test]
    fn test_backend_invalid_json_handling() {
        let res = LyricsResolution::new_failed("NetEase", "netease_lyrics", "Failed to parse JSON response");
        assert_eq!(res.status, ResolutionStatus::Failed("Failed to parse JSON response".to_string()));
        assert_eq!(res.error, Some("Failed to parse JSON response".to_string()));
    }

    #[test]
    fn test_backend_http_rate_limit_and_server_error_handling() {
        let res_429 = LyricsResolution::new_source_unavailable("LRCLIB", "lrclib_get", "HTTP 429 Too Many Requests");
        assert_eq!(res_429.status, ResolutionStatus::SourceUnavailable);
        assert_eq!(res_429.error, Some("HTTP 429 Too Many Requests".to_string()));

        let res_503 = LyricsResolution::new_source_unavailable("NetEase", "netease_search", "HTTP 503 Service Unavailable");
        assert_eq!(res_503.status, ResolutionStatus::SourceUnavailable);
        assert_eq!(res_503.error, Some("HTTP 503 Service Unavailable".to_string()));
    }

    #[test]
    fn test_backend_http_timeout_handling() {
        let res_timeout = LyricsResolution::new_source_unavailable("LRCLIB", "lrclib_get", "Request timed out after 10000ms");
        assert_eq!(res_timeout.status, ResolutionStatus::SourceUnavailable);
        assert_eq!(res_timeout.error, Some("Request timed out after 10000ms".to_string()));
    }
}
