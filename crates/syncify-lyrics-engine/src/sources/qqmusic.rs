//! Fuente `qqmusic`: letras synced del API abierto de QQ Music (respuesta
//! lyric en base64).

use base64::Engine as _;

use anyhow::{anyhow, Result};
use tracing::info;

use super::super::client::LyricsClient;
use crate::types::LyricsResponse;
use syncify_lyrics_domain::parse_lrc_line;

impl LyricsClient {
    /// Fetch synced lyrics from QQ Music API (Word-Synced & Line-Synced)
    pub async fn fetch_qqmusic_lyrics(
        &self,
        artist: &str,
        track: &str,
        _duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let query = format!("{} {}", artist, track);
        let search_url = format!(
            "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?w={}&format=json&n=5",
            urlencoding::encode(&query)
        );

        let res = self
            .client
            .get(&search_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .header("Referer", "https://y.qq.com/")
            .send()
            .await
            .map_err(|e| anyhow!("QQMusic search request failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("QQMusic search parse failed: {}", e))?;

        let songs = json["data"]["song"]["list"]
            .as_array()
            .ok_or_else(|| anyhow!("No songs found in QQMusic response"))?;

        if songs.is_empty() {
            return Err(anyhow!("QQMusic: 0 matches for {} - {}", artist, track));
        }

        let song_mid = songs[0]["songmid"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing songmid in QQMusic result"))?;

        let l_url = format!(
            "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&format=json",
            song_mid
        );

        let l_res = self
            .client
            .get(&l_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .header("Referer", "https://y.qq.com/")
            .send()
            .await
            .map_err(|e| anyhow!("QQMusic lyric request failed: {}", e))?;

        let l_json: serde_json::Value = l_res
            .json()
            .await
            .map_err(|e| anyhow!("QQMusic lyric parse failed: {}", e))?;

        let l_b64 = l_json["lyric"]
            .as_str()
            .ok_or_else(|| anyhow!("No lyric field in QQMusic response"))?;

        let raw_bytes = base64::engine::general_purpose::STANDARD
            .decode(l_b64)
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
            return Err(anyhow!(
                "QQMusic: parsed 0 lines for {} - {}",
                artist,
                track
            ));
        }

        info!(
            "[QQMusic] ✓ Acquired synced lyrics for {} - {} ({} lines)",
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
            provider: "QQ Music".to_string(),
            source: "y.qq.com".to_string(),
            elrc_content: if is_karaoke { Some(raw_text) } else { None },
        })
    }
}
