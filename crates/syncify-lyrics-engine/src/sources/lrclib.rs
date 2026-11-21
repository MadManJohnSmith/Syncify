//! Fuente `lrclib_synced` / `lrclib_plain`: búsqueda y descarga de letras en
//! LRCLIB (synced, plain e instrumental).

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, info};

use super::super::client::LyricsClient;
use crate::types::{LyricsLine, LyricsResponse};
use syncify_lyrics_domain::{detect_language_heuristic, parse_lrc_line};

/// LRCLIB API response
#[derive(Debug, Deserialize)]
pub struct LRCLibResponse {
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    id: Option<i64>,
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    name: Option<String>,
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    artist_name: Option<String>,
    #[serde(rename = "albumName")]
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    album_name: Option<String>,
    duration: Option<f64>,
    instrumental: Option<bool>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

impl LyricsClient {
    /// Search for lyrics with duration matching
    pub async fn search_lyrics(&self, query: &str, duration_sec: f64) -> Result<LyricsResponse> {
        self.hooks.wait_rate_limit("lrclib").await;

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

        // Score candidates based on duration, title, synced status, and preferred language
        let config = self.config();
        let pref_lang = config.preferred_language.to_lowercase();
        let track_lower = query.to_lowercase();

        let mut best_score = -1000.0f64;
        let mut best_match: Option<&LRCLibResponse> = None;

        for result in &results {
            let has_synced = result.synced_lyrics.is_some();
            let has_plain = result.plain_lyrics.is_some();
            if !has_synced && !has_plain {
                continue;
            }

            let r_title = result.track_name.as_deref().unwrap_or("").to_lowercase();
            let title_matches = r_title.contains(&track_lower) || track_lower.contains(&r_title);
            if !title_matches && !r_title.is_empty() {
                continue;
            }

            let mut score = 0.0f64;

            // Prioritize synced lyrics
            if has_synced {
                score += 100.0;
            }

            // Exact title match bonus
            if r_title == track_lower {
                score += 30.0;
            }

            // Duration match (only evaluated if duration_sec > 0.0)
            if duration_sec > 0.0 {
                if let Some(d) = result.duration {
                    let diff = (d - duration_sec).abs();
                    if diff <= 3.0 {
                        score += 50.0 - diff * 10.0;
                    } else if diff <= 8.0 {
                        score += 10.0 - diff;
                    } else {
                        // Beyond 8s tolerance
                        continue;
                    }
                }
            }

            // Language matching: if preference is set (not empty, "original", or "any"), boost if matches
            if !pref_lang.is_empty() && pref_lang != "original" && pref_lang != "any" {
                let sample = result
                    .synced_lyrics
                    .as_deref()
                    .or(result.plain_lyrics.as_deref())
                    .unwrap_or("");
                if let Some(detected) = detect_language_heuristic(sample) {
                    if detected.to_lowercase() == pref_lang {
                        score += 60.0;
                    }
                }
            }

            if score > best_score {
                best_score = score;
                best_match = Some(result);
            }
        }

        let best = best_match
            .ok_or_else(|| anyhow!("No title/duration matching lyrics found for query"))?;
        self.parse_response(best)
    }

    /// Fetch plain lyrics from LRCLIB when synced lyrics are absent
    pub async fn fetch_lrclib_plain(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        self.hooks.wait_rate_limit("lrclib").await;

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
            return Err(anyhow!(
                "LRCLIB: plain_lyrics is empty for {} - {}",
                artist,
                track
            ));
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

        info!(
            "[LRCLIB] Found plain lyrics for {} - {} ({} lines)",
            artist,
            track,
            lines.len()
        );

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

        let elrc = if lrc
            .synced_lyrics
            .as_ref()
            .is_some_and(|s| s.contains('<') && s.contains('>'))
        {
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
}
