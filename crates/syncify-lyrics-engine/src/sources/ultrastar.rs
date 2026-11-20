//! Fuente `ultrastar`: karaoke sílaba a sílaba del API abierto de USDB
//! (formato TXT UltraStar).

use anyhow::{anyhow, Result};
use tracing::info;

use super::super::client::LyricsClient;
use crate::types::LyricsResponse;
use syncify_lyrics_domain::parse_ultrastar_to_elrc;

impl LyricsClient {
    /// Fetch UltraStar Karaoke format lyrics from USDB open API
    pub async fn fetch_ultrastar_karaoke(
        &self,
        artist: &str,
        track: &str,
    ) -> Result<LyricsResponse> {
        let search_url = format!(
            "https://usdb.animux.de/api/v1/songs?artist={}&title={}",
            urlencoding::encode(artist),
            urlencoding::encode(track)
        );

        let res = self
            .client
            .get(&search_url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| anyhow!("USDB search request failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("USDB search JSON parse failed: {}", e))?;

        let songs = json
            .as_array()
            .or_else(|| json["songs"].as_array())
            .ok_or_else(|| anyhow!("No songs array in USDB response"))?;

        if songs.is_empty() {
            return Err(anyhow!("USDB: 0 matches for {} - {}", artist, track));
        }

        let song_id = songs[0]["id"]
            .as_i64()
            .or_else(|| songs[0]["id"].as_str().and_then(|s| s.parse().ok()))
            .ok_or_else(|| anyhow!("Missing song ID in USDB result"))?;

        let dl_url = format!(
            "https://usdb.animux.de/index.php?link=gettxt&id={}",
            song_id
        );
        let dl_res = self
            .client
            .get(&dl_url)
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
            return Err(anyhow!(
                "UltraStar TXT parsed 0 lines for {} - {}",
                artist,
                track
            ));
        }

        info!(
            "[UltraStarKaraoke] ✓ Acquired syllable-synced UltraStar lyrics for {} - {} ({} lines)",
            artist,
            track,
            lines.len()
        );

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
}
