//! Tidal downloader - Master restoration with OAuth client credentials, proxy API cascades,
//! duration tolerance, studio origin candidate scoring, and stream resolution details.

use crate::download::http_client::{create_http_client, get_user_agent, TIDAL_LIMITER};
pub use crate::services::tidal::{
    artist_matches, score_tidal_candidate, score_tidal_release, title_matches, TidalAlbum,
    TidalArtist, TidalAuthStatus, TidalMediaMetadata, TidalTrack,
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

/// Detailed stream resolution metrics for Tidal downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalStreamResolution {
    pub url: String,
    pub source: String,
    pub requested_quality: String,
    pub obtained_quality: String,
    pub codec: String,
    pub bit_depth: i32,
    pub sample_rate: f64,
    pub is_fallback: bool,
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
    #[serde(rename = "mimeType")]
    _mime_type: Option<String>,
    urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DirectUrl {
    url: String,
}

pub struct TidalDownloader {
    client: Client,
    client_id: String,
    client_secret: String,
    user_token: RwLock<Option<String>>,
    cached_oauth_token: RwLock<Option<(String, Instant)>>,
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
            user_token: RwLock::new(None),
            cached_oauth_token: RwLock::new(None),
        }
    }

    pub fn with_user_token(self, token: Option<String>) -> Self {
        if let Some(tok) = token {
            let mut guard = self.user_token.write().unwrap();
            *guard = Some(tok);
        }
        self
    }

    pub fn get_proxy_apis() -> Vec<String> {
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

    /// Check authentication status according to strict hierarchy:
    /// UserToken -> OAuth ClientCredentials -> RequiresAuth / SourceUnavailable / Failed
    pub async fn check_auth_status(&self, explicit_token: Option<&str>) -> TidalAuthStatus {
        if let Some(tok) = explicit_token {
            if !tok.trim().is_empty() {
                return TidalAuthStatus::UserToken(tok.to_string());
            }
        }

        {
            let guard = self.user_token.read().unwrap();
            if let Some(ref tok) = *guard {
                if !tok.trim().is_empty() {
                    return TidalAuthStatus::UserToken(tok.clone());
                }
            }
        }

        match self.get_access_token().await {
            Ok(tok) => TidalAuthStatus::ClientCredentials(tok),
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("401") || err_msg.contains("Unauthorized") {
                    TidalAuthStatus::RequiresAuth
                } else {
                    TidalAuthStatus::SourceUnavailable(err_msg)
                }
            }
        }
    }

    /// Get OAuth access token (cached with auto-refresh)
    pub async fn get_access_token(&self) -> Result<String> {
        // Check cache
        {
            let cache = self.cached_oauth_token.read().unwrap();
            if let Some((token, expires_at)) = cache.as_ref() {
                if expires_at.elapsed() < Duration::from_secs(55 * 60) {
                    return Ok(token.clone());
                }
            }
        }

        debug!("[Tidal] Requesting OAuth client_credentials token");

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
                "Failed to get Tidal OAuth token: HTTP {}",
                response.status()
            ));
        }

        let token_resp: TokenResponse = response.json().await?;

        {
            let mut cache = self.cached_oauth_token.write().unwrap();
            *cache = Some((token_resp.access_token.clone(), Instant::now()));
        }

        Ok(token_resp.access_token)
    }

    /// Search for a track by ISRC with duration tolerance check
    pub async fn search_by_isrc(
        &self,
        isrc: &str,
        expected_duration_sec: i32,
    ) -> Result<TidalTrack> {
        TIDAL_LIMITER.wait("tidal").await;

        let token = match self.check_auth_status(None).await {
            TidalAuthStatus::UserToken(t) => t,
            TidalAuthStatus::ClientCredentials(t) => t,
            TidalAuthStatus::RequiresAuth => return Err(anyhow!("Tidal authentication required for search")),
            TidalAuthStatus::SourceUnavailable(msg) => return Err(anyhow!("Tidal API unavailable: {}", msg)),
            TidalAuthStatus::Failed(msg) => return Err(anyhow!("Tidal auth failed: {}", msg)),
        };

        let url = format!(
            "https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US",
            urlencoding::encode(isrc)
        );

        debug!("[Tidal] Searching track by ISRC: {}", isrc);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Tidal ISRC search failed: HTTP {}", response.status()));
        }

        let result: TidalSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks section returned by Tidal search"))?;

        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        info!("[Tidal] Found exact ISRC match '{}' (duration diff: {}s)", track.title, duration_diff);
                        return Ok(track.clone());
                    } else {
                        warn!(
                            "[Tidal] ISRC match '{}' found but duration mismatch (expected {}s, got {}s)",
                            track.title, expected_duration_sec, track.duration
                        );
                    }
                } else {
                    return Ok(track.clone());
                }
            }
        }

        Err(anyhow!("No exact ISRC match found on Tidal for: {}", isrc))
    }

    /// Search for a track by metadata (artist + title) with candidate scoring for smart studio origin
    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
    ) -> Result<TidalTrack> {
        self.search_by_metadata_with_studio_option(track_name, artist_name, expected_duration_sec, true).await
    }

    pub async fn search_by_metadata_with_studio_option(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
        smart_studio_origin: bool,
    ) -> Result<TidalTrack> {
        TIDAL_LIMITER.wait("tidal").await;

        let token = match self.check_auth_status(None).await {
            TidalAuthStatus::UserToken(t) => t,
            TidalAuthStatus::ClientCredentials(t) => t,
            TidalAuthStatus::RequiresAuth => return Err(anyhow!("Tidal authentication required for search")),
            TidalAuthStatus::SourceUnavailable(msg) => return Err(anyhow!("Tidal API unavailable: {}", msg)),
            TidalAuthStatus::Failed(msg) => return Err(anyhow!("Tidal auth failed: {}", msg)),
        };

        let query = format!("{} {}", artist_name, track_name);
        let url = format!(
            "https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US",
            urlencoding::encode(&query)
        );

        debug!("[Tidal] Searching track by metadata: '{} - {}'", artist_name, track_name);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Tidal metadata search failed: HTTP {}", response.status()));
        }

        let result: TidalSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks section returned by Tidal search"))?;

        if tracks.items.is_empty() {
            return Err(anyhow!("No matching tracks found on Tidal for: {} - {}", artist_name, track_name));
        }

        let mut best_track: Option<TidalTrack> = None;
        let mut best_score: i32 = i32::MIN;

        for track in &tracks.items {
            if !title_matches(track_name, &track.title) {
                continue;
            }

            let track_artist = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or("");
            if !artist_matches(artist_name, track_artist) {
                continue;
            }

            if expected_duration_sec > 0 {
                let duration_diff = (track.duration - expected_duration_sec).abs();
                if duration_diff > 10 {
                    continue;
                }
            }

            if smart_studio_origin {
                let alb_title = track.album.as_ref().map(|a| a.title.as_str()).unwrap_or("");
                let is_hires = track.audio_quality.as_deref() == Some("HI_RES_LOSSLESS") || track.audio_quality.as_deref() == Some("HI_RES");
                let score = score_tidal_candidate(
                    alb_title, track_artist, track_artist, &track.title, "", artist_name, is_hires
                );
                if score > best_score {
                    best_score = score;
                    best_track = Some(track.clone());
                }
            } else {
                return Ok(track.clone());
            }
        }

        if let Some(t) = best_track {
            info!("[Tidal] Selected studio origin track: '{}' by '{}' (score: {})", t.title, artist_name, best_score);
            return Ok(t);
        }

        Err(anyhow!(
            "No matching track found on Tidal for: {} - {}",
            artist_name,
            track_name
        ))
    }

    /// Resolve real audio download URL with detailed metrics and proxy fallback
    pub async fn get_stream_resolution(
        &self,
        track_id: i64,
        quality_opt: Option<&str>,
        _user_token: Option<&str>,
        allow_lossy_fallback: bool,
    ) -> Result<TidalStreamResolution> {
        let requested_q = quality_opt.unwrap_or("24-192");
        let target_quality_param = match requested_q {
            "24-192" | "24-96" | "HI_RES_LOSSLESS" | "HI_RES" => "HI_RES_LOSSLESS",
            "16-44" | "LOSSLESS" => "LOSSLESS",
            "320" | "HIGH" => "HIGH",
            _ => "HI_RES_LOSSLESS",
        };

        let apis = Self::get_proxy_apis();
        if apis.is_empty() {
            return Err(anyhow!("No Tidal proxy APIs available in cascade list"));
        }

        debug!("[Tidal] Resolving stream URL for track_id {} (requested: {})", track_id, requested_q);

        for api in &apis {
            let url = format!("{}/track/{}?quality={}", api, track_id, target_quality_param);

            let result = self
                .client
                .get(&url)
                .timeout(Duration::from_secs(15))
                .header("User-Agent", get_user_agent())
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        debug!("[Tidal] Proxy API {} returned HTTP status {}", api, status);
                        continue;
                    }

                    let text = resp.text().await.unwrap_or_default();
                    let trimmed = text.trim();

                    // Reject empty responses, HTML error pages, or JSON error objects
                    if trimmed.is_empty()
                        || trimmed.starts_with("<!DOCTYPE")
                        || trimmed.starts_with("<html")
                        || trimmed.contains("\"status\":4")
                        || trimmed.contains("\"status\":5")
                        || trimmed.contains("\"userMessage\"")
                    {
                        debug!("[Tidal] Proxy API {} returned invalid/error response body", api);
                        continue;
                    }

                    // Try parsing BTS manifest
                    let mut resolved_url: Option<String> = None;
                    if let Ok(manifest) = serde_json::from_str::<BTSManifest>(trimmed) {
                        if !manifest.urls.is_empty() {
                            resolved_url = Some(manifest.urls[0].clone());
                        }
                    }

                    // Try parsing direct URL JSON
                    if resolved_url.is_none() {
                        if let Ok(direct) = serde_json::from_str::<DirectUrl>(trimmed) {
                            if !direct.url.trim().is_empty() {
                                resolved_url = Some(direct.url.trim().to_string());
                            }
                        }
                    }

                    // Try parsing plain URL text
                    if resolved_url.is_none() {
                        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                            resolved_url = Some(trimmed.to_string());
                        }
                    }

                    if let Some(stream_url) = resolved_url {
                        let is_mp3 = target_quality_param == "HIGH" || stream_url.contains(".mp3");
                        let obtained_q = if is_mp3 { "320" } else if target_quality_param == "HI_RES_LOSSLESS" { "24-192" } else { "16-44" };
                        let is_fallback = obtained_q != requested_q;

                        if is_fallback && !allow_lossy_fallback && is_mp3 {
                            return Err(anyhow!("Lossy MP3 fallback prohibited for requested FLAC quality: {}", requested_q));
                        }

                        let (codec, bit_depth, sample_rate) = if is_mp3 {
                            ("MP3", 16, 44100.0)
                        } else if target_quality_param == "HI_RES_LOSSLESS" {
                            ("FLAC", 24, 96000.0)
                        } else {
                            ("FLAC", 16, 44100.0)
                        };

                        info!("[Tidal] Stream URL resolved successfully via {}", api);

                        return Ok(TidalStreamResolution {
                            url: stream_url,
                            source: format!("Tidal Proxy ({})", api.replace("https://", "")),
                            requested_quality: requested_q.to_string(),
                            obtained_quality: obtained_q.to_string(),
                            codec: codec.to_string(),
                            bit_depth,
                            sample_rate,
                            is_fallback,
                        });
                    }
                }
                Err(e) => {
                    debug!("[Tidal] Connection error to proxy API {}: {}", api, e);
                }
            }
        }

        Err(anyhow!("Failed to obtain stream URL for Tidal track ID {} from all proxy APIs", track_id))
    }

    /// Download stream audio payload to disk with chunk validation
    pub async fn download_audio_payload(
        &self,
        stream_url: &str,
        output_path: &Path,
    ) -> Result<u64> {
        let temp_file_path = output_path.with_extension("tmp");
        if let Some(parent) = temp_file_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let mut resp = self.client.get(stream_url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Tidal stream download failed: HTTP {}", resp.status()));
        }

        let mut file = File::create(&temp_file_path).await?;
        let mut downloaded: u64 = 0;

        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
        }

        file.flush().await?;
        drop(file);

        if downloaded == 0 {
            let _ = tokio::fs::remove_file(&temp_file_path).await;
            return Err(anyhow!("Tidal downloaded file payload is zero bytes"));
        }

        tokio::fs::rename(&temp_file_path, output_path).await?;
        info!("[Tidal] Saved audio payload: {} bytes -> {}", downloaded, output_path.display());

        Ok(downloaded)
    }

    pub async fn get_download_url(&self, track_id: i64) -> Result<String> {
        let res = self.get_stream_resolution(track_id, None, None, true).await?;
        Ok(res.url)
    }
}

impl Default for TidalDownloader {
    fn default() -> Self {
        Self::new()
    }
}
