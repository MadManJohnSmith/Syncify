//! Tidal downloader - credential-free downloads via embedded OAuth + proxy APIs (CLI Standalone)

use crate::download::http_client::{create_http_client, get_user_agent, TIDAL_LIMITER};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalTrack {
    pub id: i64,
    pub title: String,
    pub isrc: Option<String>,
    pub duration: i32,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub album: Option<TidalAlbum>,
    pub artist: Option<TidalArtist>,
    pub artists: Option<Vec<TidalArtist>>,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalAlbum {
    pub title: String,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalArtist {
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct TidalSearchResponse {
    tracks: Option<TidalTracksContainer>,
}

#[derive(Debug, Deserialize)]
struct TidalTracksContainer {
    items: Vec<TidalTrack>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct BTSManifest {
    urls: Vec<String>,
}

pub struct TidalDownloader {
    client: Client,
    client_id: String,
    client_secret: String,
    cached_token: RwLock<Option<(String, Instant)>>,
}

impl TidalDownloader {
    pub fn new() -> Self {
        let client_id = BASE64
            .decode("NkJEU1JkcEs5aHFFQlRnVQ==")
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();

        let client_secret = BASE64
            .decode("eGV1UG1ZN25icFo5SUliTEFjUTkzc2hrYTFWTmhlVUFxTjZJY3N6alRHOD0=")
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();

        Self {
            client: create_http_client(),
            client_id,
            client_secret,
            cached_token: RwLock::new(None),
        }
    }

    fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "dGlkYWwua2lub3BsdXMub25saW5l",
            "dGlkYWwtYXBpLmJpbmltdW0ub3Jn",
            "dHJpdG9uLnNxdWlkLnd0Zg==",
            "dm9nZWwucXFkbC5zaXRl",
            "bWF1cy5xcWRsLnNpdGU=",
            "aHVuZC5xcWRsLnNpdGU=",
            "a2F0emUucXFkbC5zaXRl",
            "d29sZi5xcWRsLnNpdGU=",
        ];

        encoded_apis
            .iter()
            .filter_map(|encoded| {
                BASE64.decode(encoded).ok().and_then(|bytes| {
                    String::from_utf8(bytes)
                        .ok()
                        .map(|s| format!("https://{}", s))
                })
            })
            .collect()
    }

    async fn get_access_token(&self) -> Result<String> {
        {
            let cache = self.cached_token.read().unwrap();
            if let Some((token, expires_at)) = cache.as_ref() {
                if expires_at.elapsed() < Duration::from_secs(55 * 60) {
                    return Ok(token.clone());
                }
            }
        }

        let auth_url = BASE64
            .decode("aHR0cHM6Ly9hdXRoLnRpZGFsLmNvbS92MS9vYXV0aDIvdG9rZW4=")
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| anyhow!("Failed to decode auth URL"))?;

        let response = self
            .client
            .post(&auth_url)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&grant_type=client_credentials",
                self.client_id
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get Tidal token: HTTP {}",
                response.status()
            ));
        }

        let token_resp: TokenResponse = response.json().await?;

        {
            let mut cache = self.cached_token.write().unwrap();
            *cache = Some((token_resp.access_token.clone(), Instant::now()));
        }

        Ok(token_resp.access_token)
    }

    pub async fn search_by_isrc(
        &self,
        isrc: &str,
        expected_duration_sec: i32,
    ) -> Result<TidalTrack> {
        TIDAL_LIMITER.wait("tidal").await;
        let token = self.get_access_token().await?;

        let url = format!(
            "https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US",
            urlencoding::encode(isrc)
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Tidal search failed: HTTP {}", response.status()));
        }

        let result: TidalSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks in response"))?;

        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        return Ok(track.clone());
                    }
                } else {
                    return Ok(track.clone());
                }
            }
        }

        Err(anyhow!("No exact ISRC match found for: {}", isrc))
    }

    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
    ) -> Result<TidalTrack> {
        TIDAL_LIMITER.wait("tidal").await;
        let token = self.get_access_token().await?;

        let query = format!("{} {}", artist_name, track_name);
        let url = format!(
            "https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US",
            urlencoding::encode(&query)
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Tidal search failed: HTTP {}", response.status()));
        }

        let result: TidalSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks in response"))?;

        for track in &tracks.items {
            let track_artist = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or("");
            if track.title.to_lowercase().contains(&track_name.to_lowercase()) &&
               track_artist.to_lowercase().contains(&artist_name.to_lowercase()) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff > 10 {
                        continue;
                    }
                }
                return Ok(track.clone());
            }
        }

        Err(anyhow!(
            "No matching track found for: {} - {}",
            artist_name,
            track_name
        ))
    }

    pub async fn get_download_url(&self, track_id: i64) -> Result<String> {
        let apis = Self::get_proxy_apis();
        if apis.is_empty() {
            return Err(anyhow!("No Tidal proxy APIs available"));
        }

        for api in apis {
            let url = format!("{}/track/{}?quality=HI_RES_LOSSLESS", api, track_id);

            let result = self
                .client
                .get(&url)
                .timeout(Duration::from_secs(15))
                .header("User-Agent", get_user_agent())
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().await?;
                    if let Ok(manifest) = serde_json::from_str::<BTSManifest>(&text) {
                        if !manifest.urls.is_empty() {
                            return Ok(manifest.urls[0].clone());
                        }
                    }
                    if text.starts_with("http://") || text.starts_with("https://") {
                        return Ok(text.trim().to_string());
                    }
                }
                _ => continue,
            }
        }

        Err(anyhow!("Failed to get Tidal download URL from all proxy APIs"))
    }
}

impl Default for TidalDownloader {
    fn default() -> Self {
        Self::new()
    }
}
