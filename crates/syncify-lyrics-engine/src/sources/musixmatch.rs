//! Fuente `musixmatch_richsync` / `musixmatch_plain`: token de escritorio
//! auto-refrescado, richsync word-synced convertido a Enhanced LRC y letras
//! oficiales plain.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use tracing::{debug, info};

use super::super::client::LyricsClient;
use crate::types::{LyricsLine, LyricsResponse};
use syncify_lyrics_domain::ms_to_lrc_timestamp;

/// Musixmatch cached token
pub(crate) struct MxmToken {
    token: String,
    obtained_at: Instant,
}

impl LyricsClient {
    /// Obtain a Musixmatch usertoken (auto-refreshed, cached for 10 minutes)
    async fn get_musixmatch_token(&self) -> Result<String> {
        let mut guard = self.mxm_token.lock().await;
        if let Some(ref cached) = *guard {
            if cached.obtained_at.elapsed() < Duration::from_secs(600) {
                return Ok(cached.token.clone());
            }
        }

        let url =
            "https://apic-desktop.musixmatch.com/ws/1.1/token.get?app_id=web-desktop-app-v1.0";
        let res = self
            .client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch token request failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("Musixmatch token parse failed: {}", e))?;

        let status = json["message"]["header"]["status_code"]
            .as_i64()
            .unwrap_or(0);
        if status != 200 {
            return Err(anyhow!("Musixmatch token.get returned status {}", status));
        }

        let token = json["message"]["body"]["user_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No user_token in Musixmatch response"))?
            .to_string();

        if token.is_empty() {
            return Err(anyhow!("Empty Musixmatch token"));
        }

        debug!(
            "[Musixmatch] Obtained usertoken: {}...",
            &token[..token.len().min(16)]
        );
        *guard = Some(MxmToken {
            token: token.clone(),
            obtained_at: Instant::now(),
        });
        Ok(token)
    }

    /// Fetch Musixmatch Richsync word-synced lyrics and convert to Enhanced LRC
    pub async fn fetch_musixmatch_richsync(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let token = self.get_musixmatch_token().await?;

        // Step 1: Search for the track
        let search_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.search?app_id=web-desktop-app-v1.0&usertoken={}&q_artist={}&q_track={}&page_size=5&page=1&s_track_rating=desc",
            token,
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self
            .client
            .get(&search_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch search failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("Musixmatch search parse failed: {}", e))?;

        let track_list = json["message"]["body"]["track_list"]
            .as_array()
            .ok_or_else(|| anyhow!("No track_list in Musixmatch search response"))?;

        if track_list.is_empty() {
            return Err(anyhow!(
                "Musixmatch: no tracks found for {} - {}",
                artist,
                track
            ));
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
            let artist_matches =
                t_artist.contains(&artist_lower) || artist_lower.contains(&t_artist);

            if title_matches && artist_matches {
                exact_match = Some(t);
                break;
            } else if title_matches && fallback_match.is_none() {
                fallback_match = Some(t);
            }
        }

        let matched = exact_match.or(fallback_match).ok_or_else(|| {
            anyhow!(
                "No Musixmatch track with richsync for {} - {}",
                artist,
                track
            )
        })?;
        let commontrack_id = matched["commontrack_id"]
            .as_i64()
            .ok_or_else(|| anyhow!("Missing commontrack_id"))?;

        // Step 2: Fetch richsync data
        // Small delay for rate limiting
        tokio::time::sleep(Duration::from_millis(300)).await;

        let rs_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.richsync.get?app_id=web-desktop-app-v1.0&usertoken={}&commontrack_id={}",
            token, commontrack_id
        );

        let rs_res = self
            .client
            .get(&rs_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch richsync request failed: {}", e))?;

        let rs_json: serde_json::Value = rs_res
            .json()
            .await
            .map_err(|e| anyhow!("Musixmatch richsync parse failed: {}", e))?;

        let rs_body_str = rs_json["message"]["body"]["richsync"]["richsync_body"]
            .as_str()
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

        info!(
            "[Musixmatch] Found Richsync word-synced lyrics for {} - {} ({} lines)",
            artist,
            track,
            lines.len()
        );

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
    pub async fn fetch_musixmatch_plain(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let token = self.get_musixmatch_token().await?;

        // Search for track
        let search_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.search?app_id=web-desktop-app-v1.0&usertoken={}&q_artist={}&q_track={}&page_size=5&page=1&s_track_rating=desc",
            token,
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self
            .client
            .get(&search_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch search failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("Musixmatch search parse failed: {}", e))?;

        let track_list = json["message"]["body"]["track_list"]
            .as_array()
            .ok_or_else(|| anyhow!("No track_list in Musixmatch search response"))?;

        if track_list.is_empty() {
            return Err(anyhow!(
                "Musixmatch: no tracks found for {} - {}",
                artist,
                track
            ));
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
            let artist_matches =
                t_artist.contains(&artist_lower) || artist_lower.contains(&t_artist);

            if title_matches && artist_matches {
                matched_id = t["commontrack_id"].as_i64();
                break;
            } else if title_matches && matched_id.is_none() {
                matched_id = t["commontrack_id"].as_i64();
            }
        }

        let commontrack_id = matched_id.ok_or_else(|| {
            anyhow!(
                "No matching commontrack_id in Musixmatch for {} - {}",
                artist,
                track
            )
        })?;

        tokio::time::sleep(Duration::from_millis(300)).await;

        let lyrics_url = format!(
            "https://apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get?app_id=web-desktop-app-v1.0&usertoken={}&commontrack_id={}",
            token, commontrack_id
        );

        let lyr_res = self
            .client
            .get(&lyrics_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| anyhow!("Musixmatch lyrics request failed: {}", e))?;

        let lyr_json: serde_json::Value = lyr_res
            .json()
            .await
            .map_err(|e| anyhow!("Musixmatch lyrics parse failed: {}", e))?;

        let raw_body = lyr_json["message"]["body"]["lyrics"]["lyrics_body"]
            .as_str()
            .ok_or_else(|| anyhow!("No lyrics_body in Musixmatch track.lyrics.get"))?;

        // Strip Musixmatch commercial disclaimer
        let clean_body = raw_body.split("*******").next().unwrap_or(raw_body).trim();

        if clean_body.is_empty() {
            return Err(anyhow!(
                "Empty lyrics_body in Musixmatch for {} - {}",
                artist,
                track
            ));
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

        info!(
            "[Musixmatch] Found official plain lyrics for {} - {} ({} lines)",
            artist,
            track,
            lines.len()
        );

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
}
