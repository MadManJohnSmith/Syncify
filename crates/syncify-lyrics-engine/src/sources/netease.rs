//! Fuente `netease_synced`: letras synced (klyric preferido sobre lrc) del
//! API de NetEase Cloud Music, con casación por título y duración.

use anyhow::{anyhow, Result};
use tracing::{debug, info};

use super::super::client::LyricsClient;
use crate::types::LyricsResponse;
use syncify_lyrics_domain::{parse_lrc_line, simplify_track_name};

impl LyricsClient {
    /// Fetch lyrics from NetEase Cloud Music API with duration matching
    pub async fn fetch_netease_lyrics(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let query = format!("{} {}", artist, track);
        let search_url = format!(
            "https://music.163.com/api/search/get?s={}&type=1&offset=0&limit=5",
            urlencoding::encode(&query)
        );

        let res = self
            .client
            .get(&search_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .header("Referer", "https://music.163.com")
            .send()
            .await
            .map_err(|e| anyhow!("NetEase search request failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("NetEase search parse failed: {}", e))?;

        let songs = json["result"]["songs"]
            .as_array()
            .ok_or_else(|| anyhow!("No songs in NetEase search response"))?;

        if songs.is_empty() {
            return Err(anyhow!(
                "NetEase: no songs found for {} - {}",
                artist,
                track
            ));
        }

        let track_lower = simplify_track_name(track).to_lowercase();
        let track_words: Vec<&str> = track_lower
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .collect();

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

        let song = matched_song.ok_or_else(|| {
            anyhow!(
                "NetEase: no title/duration matching songs for {} - {}",
                artist,
                track
            )
        })?;
        let song_id = song["id"]
            .as_i64()
            .ok_or_else(|| anyhow!("Missing NetEase song id"))?;

        // Fetch lyrics
        let lyric_url = format!(
            "https://music.163.com/api/song/lyric?id={}&lv=-1&kv=-1&tv=-1",
            song_id
        );

        let l_res = self
            .client
            .get(&lyric_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .header("Referer", "https://music.163.com")
            .send()
            .await
            .map_err(|e| anyhow!("NetEase lyric request failed: {}", e))?;

        let l_json: serde_json::Value = l_res
            .json()
            .await
            .map_err(|e| anyhow!("NetEase lyric parse failed: {}", e))?;

        let klyric = l_json["klyric"]["lyric"].as_str().unwrap_or("");
        let lrc = l_json["lrc"]["lyric"].as_str().unwrap_or("");

        let raw_lyrics = if !klyric.trim().is_empty() {
            klyric
        } else if !lrc.trim().is_empty() {
            lrc
        } else {
            return Err(anyhow!(
                "NetEase: no synced lyrics content for song {}",
                song_id
            ));
        };

        let is_karaoke = raw_lyrics.contains('<') && raw_lyrics.contains('>');
        let mut raw_lines: Vec<String> = raw_lyrics.lines().map(|s| s.to_string()).collect();

        // Sort lines chronologically by timestamp [mm:ss.xx]
        raw_lines.sort_by_key(|line| parse_lrc_line(line).map_or(0, |l| l.start_time_ms));

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
            return Err(anyhow!(
                "NetEase: parsed 0 lines from lyrics for {}",
                song_id
            ));
        }

        info!(
            "[NetEase] Found {} lyrics for {} - {} ({} lines)",
            if is_karaoke {
                "karaoke word-synced"
            } else {
                "line-synced"
            },
            artist,
            track,
            lines.len()
        );

        Ok(LyricsResponse {
            lines,
            sync_type: if is_karaoke {
                "KARAOKE_WORD_SYNCED".to_string()
            } else {
                "LINE_SYNCED".to_string()
            },
            instrumental: false,
            plain_lyrics: None,
            provider: "NetEase Cloud Music".to_string(),
            source: "music.163.com".to_string(),
            elrc_content: if is_karaoke { Some(sorted_buf) } else { None },
        })
    }
}
