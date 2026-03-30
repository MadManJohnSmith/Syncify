// Tidal downloader - credential-free downloads via embedded OAuth + proxy APIs

use crate::download::http_client::{create_http_client, get_user_agent, TIDAL_LIMITER};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Tidal track information
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
    #[serde(rename = "mediaMetadata")]
    pub media_metadata: Option<TidalMediaMetadata>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalMediaMetadata {
    pub tags: Option<Vec<String>>,
}

/// Tidal search response
#[derive(Debug, Deserialize)]
struct TidalSearchResponse {
    tracks: Option<TidalTracksContainer>,
}

#[derive(Debug, Deserialize)]
struct TidalTracksContainer {
    items: Vec<TidalTrack>,
}

/// OAuth token response
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<i64>,
}

/// BTS manifest (used for some download URLs)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct BTSManifest {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    urls: Vec<String>,
}

/// Tidal downloader using embedded OAuth + proxy APIs
pub struct TidalDownloader {
    client: Client,
    client_id: String,
    client_secret: String,
    cached_token: RwLock<Option<(String, Instant)>>,
}

impl TidalDownloader {
    pub fn new() -> Self {
        // Decode embedded credentials (from SpotiFLAC)
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

    /// Get available proxy APIs (decoded from base64)
    fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "dGlkYWwua2lub3BsdXMub25saW5l", // tidal.kinoplus.online
            "dGlkYWwtYXBpLmJpbmltdW0ub3Jn", // tidal-api.binimum.org
            "dHJpdG9uLnNxdWlkLnd0Zg==",     // triton.squid.wtf
            "dm9nZWwucXFkbC5zaXRl",         // vogel.qqdl.site
            "bWF1cy5xcWRsLnNpdGU=",         // maus.qqdl.site
            "aHVuZC5xcWRsLnNpdGU=",         // hund.qqdl.site
            "a2F0emUucXFkbC5zaXRl",         // katze.qqdl.site
            "d29sZi5xcWRsLnNpdGU=",         // wolf.qqdl.site
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

    /// Get OAuth access token (cached with auto-refresh)
    async fn get_access_token(&self) -> Result<String> {
        // Check cache
        {
            let cache = self.cached_token.read().unwrap();
            if let Some((token, expires_at)) = cache.as_ref() {
                if expires_at.elapsed() < Duration::from_secs(55 * 60) {
                    return Ok(token.clone());
                }
            }
        }

        // Get new token
        debug!("[Tidal] Getting new access token");

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

        // Cache the token
        {
            let mut cache = self.cached_token.write().unwrap();
            *cache = Some((token_resp.access_token.clone(), Instant::now()));
        }

        Ok(token_resp.access_token)
    }

    /// Search for a track by ISRC
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

        debug!("[Tidal] Searching by ISRC: {}", isrc);

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

        // Find exact ISRC match
        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                // Verify duration
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        info!(
                            "[Tidal] Found ISRC match: '{}' (duration verified)",
                            track.title
                        );
                        return Ok(track.clone());
                    } else {
                        warn!(
                            "[Tidal] ISRC match but duration mismatch: expected {}s, got {}s",
                            expected_duration_sec, track.duration
                        );
                    }
                } else {
                    return Ok(track.clone());
                }
            }
        }

        Err(anyhow!("No exact ISRC match found for: {}", isrc))
    }

    /// Search for a track by metadata
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

        debug!(
            "[Tidal] Searching by metadata: {} - {}",
            artist_name, track_name
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

        // Find best match
        for track in &tracks.items {
            if !title_matches(track_name, &track.title) {
                continue;
            }

            // Check artist
            let track_artist = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or("");
            if !artist_matches(artist_name, track_artist) {
                continue;
            }

            // Verify duration
            if expected_duration_sec > 0 {
                let duration_diff = (track.duration - expected_duration_sec).abs();
                if duration_diff > 10 {
                    continue;
                }
            }

            info!(
                "[Tidal] Found metadata match: '{}' by '{}'",
                track.title, track_artist
            );
            return Ok(track.clone());
        }

        Err(anyhow!(
            "No matching track found for: {} - {}",
            artist_name,
            track_name
        ))
    }

    /// Get download URL from proxy APIs
    pub async fn get_download_url(&self, track_id: i64) -> Result<String> {
        let apis = Self::get_proxy_apis();
        if apis.is_empty() {
            return Err(anyhow!("No Tidal proxy APIs available"));
        }

        debug!("[Tidal] Getting download URL for track {}", track_id);

        // Try APIs sequentially (Tidal proxies can be rate-limited)
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

                    // Try to parse as BTS manifest
                    if let Ok(manifest) = serde_json::from_str::<BTSManifest>(&text) {
                        if !manifest.urls.is_empty() {
                            info!("[Tidal] Got download URL from BTS manifest");
                            return Ok(manifest.urls[0].clone());
                        }
                    }

                    // Try direct URL parsing
                    #[derive(Deserialize)]
                    struct DirectUrl {
                        url: String,
                    }
                    if let Ok(direct) = serde_json::from_str::<DirectUrl>(&text) {
                        info!("[Tidal] Got direct download URL");
                        return Ok(direct.url);
                    }
                }
                Ok(resp) => {
                    debug!("[Tidal] Proxy {} returned HTTP {}", api, resp.status());
                }
                Err(e) => {
                    debug!("[Tidal] Proxy {} error: {}", api, e);
                }
            }
        }

        Err(anyhow!("All Tidal proxy APIs failed"))
    }

    /// Download a file with progress tracking
    pub async fn download_file(
        &self,
        download_url: &str,
        output_path: &Path,
        item_id: &str,
    ) -> Result<()> {
        debug!("[Tidal] Downloading to {:?}", output_path);

        let response = self
            .client
            .get(download_url)
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed: HTTP {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        PROGRESS_TRACKER.update(DownloadProgress::downloading(
            item_id, "tidal", 0, total_size,
        ));

        // Create output file
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = File::create(output_path).await?;

        // Download with progress
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if downloaded % (64 * 1024) < chunk.len() as u64 {
                PROGRESS_TRACKER.update(DownloadProgress::downloading(
                    item_id, "tidal", downloaded, total_size,
                ));
            }
        }

        file.flush().await?;
        info!("[Tidal] Download complete: {} bytes", downloaded);
        Ok(())
    }

    /// Full download flow
    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        let duration_sec = (request.duration_ms / 1000) as i32;

        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "Tidal"));

        // Find track
        let track = if let Some(isrc) = &request.isrc {
            match self.search_by_isrc(isrc, duration_sec).await {
                Ok(t) => t,
                Err(_) => {
                    self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec)
                        .await?
                }
            }
        } else {
            self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec)
                .await?
        };

        // Get download URL
        let download_url = self.get_download_url(track.id).await?;

        // Build output filename
        let filename = format!(
            "{} - {}.flac",
            sanitize_filename(&request.artist_name),
            sanitize_filename(&request.track_name)
        );
        let output_path = Path::new(&request.output_dir).join(&filename);

        // Download
        self.download_file(&download_url, &output_path, item_id)
            .await?;

        // Determine quality from metadata
        let (bit_depth, sample_rate) = match track.audio_quality.as_deref() {
            Some("HI_RES_LOSSLESS") | Some("HI_RES") => (24, 96000),
            Some("LOSSLESS") => (16, 44100),
            _ => (16, 44100),
        };

        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth,
            sample_rate,
            title: track.title,
            artist: track.artist.map(|a| a.name).unwrap_or_default(),
            album: track
                .album
                .as_ref()
                .map(|a| a.title.clone())
                .unwrap_or_default(),
            release_date: track.album.and_then(|a| a.release_date),
            track_number: track.track_number.unwrap_or(request.track_number),
            disc_number: request.disc_number,
            isrc: track.isrc,
            service: "tidal".to_string(),
        })
    }
}

impl Default for TidalDownloader {
    fn default() -> Self {
        Self::new()
    }
}

// Reuse matching functions from qobuz module
fn title_matches(expected: &str, found: &str) -> bool {
    let expected_clean = clean_title(expected);
    let found_clean = clean_title(found);
    expected_clean == found_clean
        || found_clean.contains(&expected_clean)
        || expected_clean.contains(&found_clean)
}

fn artist_matches(expected: &str, found: &str) -> bool {
    let expected_lower = expected.to_lowercase();
    let found_lower = found.to_lowercase();
    if expected_lower == found_lower {
        return true;
    }

    let expected_parts: Vec<&str> = expected_lower
        .split(&[',', ';', '&', '/', '|'][..])
        .collect();
    let found_parts: Vec<&str> = found_lower.split(&[',', ';', '&', '/', '|'][..]).collect();

    expected_parts
        .iter()
        .any(|ep| found_parts.iter().any(|fp| ep.trim() == fp.trim()))
}

fn clean_title(title: &str) -> String {
    let mut clean = title.to_lowercase();
    for suffix in [
        "(remaster",
        "(remastered",
        "(deluxe",
        "(live",
        "(remix",
        "- remaster",
    ] {
        if let Some(pos) = clean.find(suffix) {
            clean = clean[..pos].to_string();
        }
    }
    clean.trim().to_string()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if "/:*?\"<>|\\".contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}
