// Unified Playlist Resolver for Spotify and Tidal
// Extracts tracks with ISRC codes, titles, artists, and albums for FLAC lossless conversion

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub isrc: Option<String>,
    pub duration_sec: f64,
    pub track_number: u32,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPlaylist {
    pub name: String,
    pub description: Option<String>,
    pub tracks: Vec<PlaylistTrack>,
}

pub struct PlaylistResolver {
    client: Client,
}

impl PlaylistResolver {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Resolve any supported playlist URL (Spotify or Tidal)
    pub async fn resolve_playlist(&self, url_or_id: &str, auth_token: Option<&str>) -> Result<ResolvedPlaylist> {
        let input = url_or_id.trim();

        if input.contains("spotify.com") || (!input.contains("tidal.com") && input.len() == 22 && !input.contains('/')) {
            let playlist_id = extract_playlist_id(input, "playlist/");
            self.resolve_spotify_playlist(&playlist_id, auth_token).await
        } else if input.contains("tidal.com") || input.contains('-') {
            let playlist_id = extract_playlist_id(input, "playlist/");
            self.resolve_tidal_playlist(&playlist_id, auth_token).await
        } else {
            Err(anyhow!("Unrecognized playlist format or service"))
        }
    }

    /// Resolve a Spotify Playlist to track list with ISRCs
    pub async fn resolve_spotify_playlist(&self, playlist_id: &str, auth_token: Option<&str>) -> Result<ResolvedPlaylist> {
        let mut playlist_name = "Spotify Playlist".to_string();
        let mut tracks = Vec::new();

        // 1. If auth_token is provided, use official Web API
        if let Some(token) = auth_token {
            let mut offset = 0;
            let limit = 100;

            loop {
                let url = format!(
                    "https://api.spotify.com/v1/playlists/{}?market=from_token&limit={}&offset={}",
                    playlist_id, limit, offset
                );

                let res = self
                    .client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await?;

                if !res.status().is_success() {
                    break;
                }

                let json: Value = res.json().await?;
                if let Some(name) = json["name"].as_str() {
                    playlist_name = name.to_string();
                }

                let items = match json["tracks"]["items"].as_array().or_else(|| json["items"].as_array()) {
                    Some(arr) => arr,
                    None => break,
                };

                if items.is_empty() {
                    break;
                }

                let page_count = items.len();
                for (idx, item) in items.iter().enumerate() {
                    let trk = if item["track"].is_object() { &item["track"] } else { item };

                    let title = trk["name"].as_str().unwrap_or("Unknown Track").to_string();
                    let artist = trk["artists"][0]["name"].as_str()
                        .or_else(|| trk["artists"].as_array().and_then(|a| a.first()).and_then(|f| f["name"].as_str()))
                        .unwrap_or("Unknown Artist")
                        .to_string();
                    let album = trk["album"]["name"].as_str().unwrap_or("Single").to_string();
                    let isrc = trk["external_ids"]["isrc"].as_str().map(|s| s.to_string());
                    let duration_sec = trk["duration_ms"].as_f64().map(|ms| ms / 1000.0).unwrap_or(0.0);
                    let cover_url = trk["album"]["images"][0]["url"].as_str().map(|s| s.to_string());

                    tracks.push(PlaylistTrack {
                        title,
                        artist,
                        album,
                        isrc,
                        duration_sec,
                        track_number: (offset + idx as u32) + 1,
                        cover_url,
                    });
                }

                if page_count < limit {
                    break;
                }

                offset += limit as u32;
            }
        }

        // 2. Fallback: Query Spotify Embed API / Public Scraper without authentication
        if tracks.is_empty() {
            let embed_url = format!("https://open.spotify.com/embed/playlist/{}", playlist_id);
            if let Ok(res) = self.client.get(&embed_url).send().await {
                if res.status().is_success() {
                    if let Ok(html) = res.text().await {
                        if let Some(pos) = html.find("id=\"__NEXT_DATA__\"") {
                            if let Some(json_start) = html[pos..].find('>') {
                                let remainder = &html[pos + json_start + 1..];
                                if let Some(json_end) = remainder.find("</script>") {
                                    let json_str = &remainder[..json_end];
                                    if let Ok(next_data) = serde_json::from_str::<Value>(json_str) {
                                        if let Some(entity) = next_data["props"]["pageProps"]["state"]["data"]["entity"].as_object() {
                                            if let Some(name) = entity.get("name").and_then(|v| v.as_str()) {
                                                playlist_name = name.to_string();
                                            }
                                            if let Some(track_list) = entity.get("trackList").and_then(|v| v.as_array()) {
                                                for (idx, trk) in track_list.iter().enumerate() {
                                                    let title = trk["title"].as_str().unwrap_or("Unknown Track").to_string();
                                                    let artist = trk["subtitle"].as_str().unwrap_or("Unknown Artist").to_string();
                                                    let duration_sec = trk["duration"].as_f64().map(|ms| ms / 1000.0).unwrap_or(0.0);

                                                    tracks.push(PlaylistTrack {
                                                        title,
                                                        artist,
                                                        album: "Spotify Playlist Track".to_string(),
                                                        isrc: None,
                                                        duration_sec,
                                                        track_number: (idx + 1) as u32,
                                                        cover_url: None,
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

        if tracks.is_empty() {
            return Err(anyhow!("No tracks found or playlist is private on Spotify"));
        }

        Ok(ResolvedPlaylist {
            name: playlist_name,
            description: None,
            tracks,
        })
    }

    /// Resolve a Tidal Playlist
    pub async fn resolve_tidal_playlist(&self, playlist_uuid: &str, _auth_token: Option<&str>) -> Result<ResolvedPlaylist> {
        let url = format!("https://api.tidal.com/v1/playlists/{}?countryCode=US", playlist_uuid);
        let items_url = format!("https://api.tidal.com/v1/playlists/{}/items?countryCode=US&limit=100", playlist_uuid);

        let mut playlist_name = "Tidal Playlist".to_string();
        let mut tracks = Vec::new();

        // 1. Fetch Playlist metadata
        if let Ok(res) = self.client.get(&url).header("x-tidal-token", "zU4XHVVkc2tDPo4t").send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<Value>().await {
                    if let Some(title) = json["title"].as_str() {
                        playlist_name = title.to_string();
                    }
                }
            }
        }

        // 2. Fetch Playlist items
        if let Ok(res) = self.client.get(&items_url).header("x-tidal-token", "zU4XHVVkc2tDPo4t").send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<Value>().await {
                    if let Some(items) = json["items"].as_array() {
                        for (idx, item) in items.iter().enumerate() {
                            let item_data = &item["item"];
                            let title = item_data["title"].as_str().unwrap_or("Unknown Track").to_string();
                            let artist = item_data["artist"]["name"].as_str()
                                .or_else(|| item_data["artists"][0]["name"].as_str())
                                .unwrap_or("Unknown Artist")
                                .to_string();
                            let album = item_data["album"]["title"].as_str().unwrap_or("Single").to_string();
                            let isrc = item_data["isrc"].as_str().map(|s| s.to_string());
                            let duration_sec = item_data["duration"].as_f64().unwrap_or(0.0);

                            tracks.push(PlaylistTrack {
                                title,
                                artist,
                                album,
                                isrc,
                                duration_sec,
                                track_number: (idx + 1) as u32,
                                cover_url: None,
                            });
                        }
                    }
                }
            }
        }

        if tracks.is_empty() {
            return Err(anyhow!("No tracks found in Tidal playlist"));
        }

        Ok(ResolvedPlaylist {
            name: playlist_name,
            description: None,
            tracks,
        })
    }
}

fn extract_playlist_id(input: &str, prefix: &str) -> String {
    if let Some(pos) = input.find(prefix) {
        let remainder = &input[pos + prefix.len()..];
        remainder.split('?').next().unwrap_or(remainder).split('/').next().unwrap_or(remainder).to_string()
    } else {
        input.to_string()
    }
}
