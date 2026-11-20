//! Fuente `lyricsplus`: letras synced del backend público de LyricsPlus,
//! con umbral mínimo de líneas.

use anyhow::{anyhow, Result};

use super::super::client::LyricsClient;
use crate::types::LyricsResponse;
use syncify_lyrics_domain::parse_lrc_line;

impl LyricsClient {
    /// Fetch lyrics from LyricsPlus API
    pub async fn fetch_lyricsplus(
        &self,
        artist: &str,
        track: &str,
        _duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let query = format!("{} {}", artist, track);
        let url = format!(
            "https://lyricsplus-api.vercel.app/v1/search?q={}",
            urlencoding::encode(&query)
        );

        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(anyhow!("LyricsPlus search failed: HTTP {}", res.status()));
        }

        let json: serde_json::Value = res.json().await?;
        let synced_str = json["syncedLyrics"]
            .as_str()
            .or_else(|| json["lyrics"].as_str())
            .ok_or_else(|| anyhow!("No lyrics field in LyricsPlus response"))?;

        if synced_str.trim().is_empty() {
            return Err(anyhow!("Empty lyrics in LyricsPlus response"));
        }

        let is_karaoke = synced_str.contains('<') && synced_str.contains('>');
        let mut raw_lines: Vec<String> = synced_str.lines().map(|s| s.to_string()).collect();

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

        if lines.len() < 4 {
            return Err(anyhow!("LyricsPlus: insufficient lines ({})", lines.len()));
        }

        Ok(LyricsResponse {
            lines,
            sync_type: if is_karaoke {
                "KARAOKE_WORD_SYNCED".to_string()
            } else {
                "LINE_SYNCED".to_string()
            },
            instrumental: false,
            plain_lyrics: None,
            provider: "LyricsPlus Karaoke".to_string(),
            source: "lyricsplus-api.vercel.app".to_string(),
            elrc_content: if is_karaoke { Some(sorted_buf) } else { None },
        })
    }
}
