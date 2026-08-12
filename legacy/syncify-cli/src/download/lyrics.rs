//! LRCLIB lyrics fetcher (CLI Standalone)

use crate::download::http_client::{create_http_client, LRCLIB_LIMITER};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    #[serde(rename = "startTimeMs")]
    pub start_time_ms: i64,
    pub words: String,
    #[serde(rename = "endTimeMs")]
    pub end_time_ms: Option<i64>,
}

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
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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

pub struct LyricsClient {
    client: Client,
    cache: RwLock<HashMap<String, (LyricsResponse, Instant)>>,
}

impl LyricsClient {
    pub fn new() -> Self {
        Self {
            client: create_http_client(),
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn cache_key(artist: &str, track: &str) -> String {
        format!("{}|{}", artist.to_lowercase(), track.to_lowercase())
    }

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

    fn set_cached(&self, artist: &str, track: &str, lyrics: &LyricsResponse) {
        let key = Self::cache_key(artist, track);
        let mut cache = self.cache.write().unwrap();
        cache.insert(key, (lyrics.clone(), Instant::now()));
    }

    pub async fn fetch_lyrics(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        if let Some(cached) = self.get_cached(artist, track) {
            debug!("[LRCLIB] Cache hit for {} - {}", artist, track);
            return Ok(cached);
        }

        LRCLIB_LIMITER.wait("lrclib").await;

        let url = format!(
            "https://lrclib.net/api/get?artist={}&track={}",
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

        let tolerance = 10.0;
        let mut best_match: Option<&LRCLibResponse> = None;
        let mut best_has_synced = false;

        for result in &results {
            let has_synced = result.synced_lyrics.is_some();
            let duration_matches = if let Some(d) = result.duration {
                (d - duration_sec).abs() <= tolerance
            } else {
                false
            };

            if duration_matches && has_synced {
                best_match = Some(result);
                break;
            }

            if has_synced && !best_has_synced {
                best_match = Some(result);
                best_has_synced = true;
            }

            if best_match.is_none() {
                best_match = Some(result);
            }
        }

        let best = best_match.ok_or_else(|| anyhow!("No suitable lyrics found"))?;
        self.parse_response(best)
    }

    pub async fn fetch_all_sources(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        if let Ok(lyrics) = self.fetch_lyrics(artist, track).await {
            if !lyrics.lines.is_empty() {
                return Ok(lyrics);
            }
        }

        let simplified = simplify_track_name(track);
        if simplified != track {
            if let Ok(lyrics) = self.fetch_lyrics(artist, &simplified).await {
                if !lyrics.lines.is_empty() {
                    return Ok(lyrics);
                }
            }
        }

        let query = format!("{} {}", artist, track);
        if let Ok(lyrics) = self.search_lyrics(&query, duration_sec).await {
            return Ok(lyrics);
        }

        if simplified != track {
            let query = format!("{} {}", artist, simplified);
            if let Ok(lyrics) = self.search_lyrics(&query, duration_sec).await {
                return Ok(lyrics);
            }
        }

        Err(anyhow!("Lyrics not found from any source"))
    }

    fn parse_response(&self, lrc: &LRCLibResponse) -> Result<LyricsResponse> {
        let mut lines = Vec::new();
        let mut sync_type = "UNSYNCED".to_string();

        if let Some(synced) = &lrc.synced_lyrics {
            for line in synced.lines() {
                if let Some(parsed) = parse_lrc_line(line) {
                    lines.push(parsed);
                }
            }
            if !lines.is_empty() {
                sync_type = "LINE_SYNCED".to_string();
            }
        }

        Ok(LyricsResponse {
            lines,
            sync_type,
            instrumental: lrc.instrumental.unwrap_or(false),
            plain_lyrics: lrc.plain_lyrics.clone(),
            provider: "LRCLIB".to_string(),
            source: "lrclib.net".to_string(),
        })
    }

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

fn parse_lrc_line(line: &str) -> Option<LyricsLine> {
    let line = line.trim();
    if !line.starts_with('[') {
        return None;
    }

    let end_bracket = line.find(']')?;
    let timestamp = &line[1..end_bracket];
    let words = line[end_bracket + 1..].to_string();

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

fn ms_to_lrc_timestamp(ms: i64) -> String {
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let centiseconds = (ms % 1000) / 10;
    format!("[{:02}:{:02}.{:02}]", minutes, seconds, centiseconds)
}

fn simplify_track_name(track: &str) -> String {
    let mut simplified = track.to_string();

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

    for pattern in [" (feat.", " (ft.", " feat.", " ft."] {
        if let Some(pos) = simplified.to_lowercase().find(pattern) {
            simplified = simplified[..pos].to_string();
        }
    }

    simplified.trim().to_string()
}
