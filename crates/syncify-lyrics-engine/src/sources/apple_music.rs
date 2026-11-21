//! Fuente `apple_music_ttml`: karaoke sílaba a sílaba del catálogo de Apple
//! Music (TTML). El token web llega por los hooks del núcleo, que comparten
//! la caché de sesión con la portada animada.

use anyhow::{anyhow, Result};
use tracing::info;

use super::super::client::LyricsClient;
use crate::types::LyricsResponse;
use syncify_lyrics_domain::{parse_lrc_line, parse_ttml_to_elrc, simplify_track_name};

impl LyricsClient {
    /// Fetch Apple Music TTML Syllable-Synced Karaoke lyrics
    pub async fn fetch_apple_music_ttml(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let am_token = match self.hooks.apple_music_token(&self.client).await {
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

            let req = self
                .client
                .get(&search_url)
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
                                let s_dur =
                                    attrs["durationInMillis"].as_f64().unwrap_or(0.0) / 1000.0;

                                if duration_sec > 0.0
                                    && s_dur > 0.0
                                    && (s_dur - duration_sec).abs() > 3.0
                                {
                                    continue;
                                }

                                if !s_title.contains(&track_lower)
                                    && !track_lower.contains(&s_title)
                                {
                                    continue;
                                }

                                let song_id = song["id"].as_str().unwrap_or("");
                                if song_id.is_empty() {
                                    continue;
                                }

                                let lyrics_url = format!(
                                    "https://amp-api.music.apple.com/v1/catalog/{}/songs/{}/lyrics",
                                    sf, song_id
                                );
                                let l_req = self
                                    .client
                                    .get(&lyrics_url)
                                    .header("Authorization", format!("Bearer {}", am_token))
                                    .header("Origin", "https://music.apple.com")
                                    .header("Referer", "https://music.apple.com/");

                                if let Ok(l_res) = l_req.send().await {
                                    if l_res.status().is_success() {
                                        if let Ok(l_json) = l_res.json::<serde_json::Value>().await
                                        {
                                            if let Some(ttml) =
                                                l_json["data"][0]["attributes"]["ttml"].as_str()
                                            {
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
                                                                sync_type: "KARAOKE_WORD_SYNCED"
                                                                    .to_string(),
                                                                instrumental: false,
                                                                plain_lyrics: None,
                                                                provider: "Apple Music TTML"
                                                                    .to_string(),
                                                                source: "music.apple.com"
                                                                    .to_string(),
                                                                elrc_content: Some(elrc),
                                                            });
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
            }
        }

        Err(anyhow!(
            "No Apple Music TTML lyrics found for {} - {}",
            artist,
            track
        ))
    }
}
