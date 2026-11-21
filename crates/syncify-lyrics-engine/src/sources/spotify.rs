//! Fuente `spotify_color`: Color Lyrics nativo de Spotify con token web
//! obtenido de la cookie `sp_dc` (fallback de variable de entorno).

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use tracing::info;

use super::super::client::LyricsClient;
use crate::types::{LyricsLine, LyricsResponse};
use syncify_lyrics_domain::{ms_to_lrc_timestamp, simplify_track_name};

impl LyricsClient {
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

        let url =
            "https://open.spotify.com/get_access_token?reason=transport&productType=web_player";
        let res = self.client
            .get(url)
            .header("Cookie", format!("sp_dc={}", sp_dc))
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://open.spotify.com/")
            .send()
            .await
            .map_err(|e| anyhow!("Spotify token request failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("Spotify token parse failed: {}", e))?;

        let access_token = json["accessToken"]
            .as_str()
            .ok_or_else(|| anyhow!("No accessToken in Spotify response (sp_dc may be expired)"))?
            .to_string();

        *guard = Some((access_token.clone(), Instant::now()));
        info!("[SpotifyLyrics] Successfully acquired fresh Spotify Web Player access token");
        Ok(access_token)
    }

    /// Fetch official native syllable/word-synced and line-synced lyrics directly from Spotify
    pub async fn fetch_spotify_lyrics(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let access_token = self.get_spotify_access_token().await?;

        // Step 1: Search for track ID on Spotify
        let query = format!("{} {}", artist, track);
        let search_url = format!(
            "https://api.spotify.com/v1/search?q={}&type=track&limit=5",
            urlencoding::encode(&query)
        );

        let s_res = self
            .client
            .get(&search_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Spotify search request failed: {}", e))?;

        let s_json: serde_json::Value = s_res
            .json()
            .await
            .map_err(|e| anyhow!("Spotify search JSON parse failed: {}", e))?;

        let tracks = s_json["tracks"]["items"]
            .as_array()
            .ok_or_else(|| anyhow!("No tracks array in Spotify search"))?;

        if tracks.is_empty() {
            return Err(anyhow!(
                "No tracks found on Spotify for {} - {}",
                artist,
                track
            ));
        }

        let track_lower = simplify_track_name(track).to_lowercase();
        let artist_lower = artist.to_lowercase();

        let mut matched_id: Option<String> = None;
        for t in tracks {
            let t_name = t["name"].as_str().unwrap_or("").to_lowercase();
            let t_dur_sec = t["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0;
            let t_artists = t["artists"].as_array();

            let artist_matches = t_artists.is_some_and(|arr| {
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

        let spotify_track_id = matched_id
            .ok_or_else(|| anyhow!("No matching track ID on Spotify for {} - {}", artist, track))?;

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
            return Err(anyhow!(
                "No lyrics on Spotify for track {}",
                spotify_track_id
            ));
        }

        let lyr_json: serde_json::Value = lyr_res
            .json()
            .await
            .map_err(|e| anyhow!("Spotify Color Lyrics parse failed: {}", e))?;

        let lines_array = lyr_json["lyrics"]["lines"]
            .as_array()
            .ok_or_else(|| anyhow!("No lines in Spotify Color Lyrics payload"))?;

        if lines_array.is_empty() {
            return Err(anyhow!("Spotify Color Lyrics lines array is empty"));
        }

        let mut lines = Vec::new();
        let mut elrc_buf = String::new();
        let mut has_syllables = false;

        for line_item in lines_array {
            let start_ms: i64 = line_item["startTimeMs"]
                .as_str()
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
                        let syl_ms: i64 = syl["startTimeMs"]
                            .as_i64()
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
            if has_syllables {
                "Karaoke word-synced"
            } else {
                "line-synced"
            },
            artist,
            track,
            lines.len()
        );

        Ok(LyricsResponse {
            lines,
            sync_type: if has_syllables {
                "KARAOKE_WORD_SYNCED".to_string()
            } else {
                "LINE_SYNCED".to_string()
            },
            instrumental: false,
            plain_lyrics: None,
            provider: "Spotify Color Lyrics".to_string(),
            source: "spotify.com".to_string(),
            elrc_content: if has_syllables { Some(elrc_buf) } else { None },
        })
    }
}
