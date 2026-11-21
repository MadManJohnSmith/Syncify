//! Fuente `tekstowo_plain`: letras plain polacas extraídas del HTML de
//! Tekstowo.pl (búsqueda + página de canción).

use anyhow::{anyhow, Result};
use tracing::info;

use super::super::client::LyricsClient;
use crate::types::{LyricsLine, LyricsResponse};

impl LyricsClient {
    /// Fetch Polish plain-text lyrics from Tekstowo.pl fallback
    pub async fn fetch_tekstowo_plain(&self, artist: &str, track: &str) -> Result<LyricsResponse> {
        let search_url = format!(
            "https://www.tekstowo.pl/szukaj,wykonawca,{},tytul,{}.html",
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self
            .client
            .get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| anyhow!("Tekstowo search request failed: {}", e))?;

        let html = res
            .text()
            .await
            .map_err(|e| anyhow!("Tekstowo HTML read failed: {}", e))?;

        let re_link = regex::Regex::new(r#"href="(/piosenka,[^"]+)""#)
            .map_err(|e| anyhow!("Tekstowo regex error: {}", e))?;
        let captures = re_link
            .captures(&html)
            .ok_or_else(|| anyhow!("No song match on Tekstowo.pl for {} - {}", artist, track))?;
        let song_path = captures
            .get(1)
            .map(|m| m.as_str())
            .ok_or_else(|| anyhow!("Failed to extract song path on Tekstowo.pl"))?;

        let song_url = format!("https://www.tekstowo.pl{}", song_path);
        let song_res = self
            .client
            .get(&song_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await
            .map_err(|e| anyhow!("Tekstowo song page request failed: {}", e))?;

        let song_html = song_res.text().await?;
        let re_lyrics = regex::Regex::new(r#"(?s)<div class="inner-text">(.*?)</div>"#)
            .map_err(|e| anyhow!("Tekstowo lyrics regex error: {}", e))?;
        let lyrics_captures = re_lyrics
            .captures(&song_html)
            .ok_or_else(|| anyhow!("No inner-text lyrics block on Tekstowo page"))?;
        let lyrics_html = lyrics_captures
            .get(1)
            .map(|m| m.as_str())
            .ok_or_else(|| anyhow!("Failed to extract lyrics HTML block"))?;

        let re_strip = regex::Regex::new(r"<[^>]+>")
            .map_err(|e| anyhow!("Tekstowo strip regex error: {}", e))?;
        let clean_text = re_strip.replace_all(lyrics_html, "\n");
        let plain_lines: Vec<String> = clean_text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        if plain_lines.is_empty() {
            return Err(anyhow!(
                "Tekstowo.pl lyrics extracted 0 lines for {} - {}",
                artist,
                track
            ));
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
        info!(
            "[Tekstowo.pl] Found Polish plain lyrics for {} - {} ({} lines)",
            artist,
            track,
            lines.len()
        );

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
}
