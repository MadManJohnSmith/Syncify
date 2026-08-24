// Qobuz downloader - deterministic request signing, token resolution, and audio downloads

use crate::download::http_client::{
    calculate_backoff_with_jitter, create_http_client, get_user_agent, is_transient_status,
    parse_retry_after, QOBUZ_LIMITER,
};
use crate::download::lyrics::{
    validate_and_embed_flac_lyrics, LyricsPipelineService, LyricsResolution, LyricsSyncType,
    ResolutionStatus,
};
use crate::download::progress::{
    ByteStreamTracker, DownloadPhase, DownloadPhaseTracker, DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use crate::services::animated_cover::{resolve_and_download_animated_cover, AnimatedCoverStatus};
use crate::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use crate::services::qobuz::{QOBUZ_API_BASE, QOBUZ_APP_ID, QOBUZ_APP_SECRET};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::{FolderFileTemplateConfig, LibraryLayout, TrackLayoutContext};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Qobuz Authentication Status
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum QobuzAuthStatus {
    Authenticated,
    RequiresAuth(String),
    SourceUnavailable(String),
}

/// Stream URL Origin Source
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamUrlSource {
    QobuzOfficial,
    ProxyFallback(String),
}

/// Resolved Stream Info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResolution {
    pub url: String,
    pub source: StreamUrlSource,
    pub format_id: String,
}

/// Qobuz track information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzTrack {
    pub id: i64,
    pub title: String,
    pub isrc: Option<String>,
    pub duration: i32,
    #[serde(rename = "maximum_bit_depth")]
    pub max_bit_depth: Option<i32>,
    #[serde(rename = "maximum_sampling_rate")]
    pub max_sample_rate: Option<f64>,
    pub track_number: Option<i32>,
    #[serde(rename = "media_number")]
    pub disc_number: Option<i32>,
    pub copyright: Option<String>,
    pub performers: Option<String>,
    pub composer: Option<QobuzPerformer>,
    pub work: Option<String>,
    pub parental_warning: Option<bool>,
    pub performer: Option<QobuzPerformer>,
    pub bpm: Option<u32>,
    pub album: Option<QobuzAlbum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzGenre {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzPerformer {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzGoodie {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub url: Option<String>,
    pub original_url: Option<String>,
    pub file_format_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzAlbum {
    pub id: Option<String>,
    pub title: String,
    pub release_date_original: Option<String>,
    pub released_at: Option<i64>,
    pub image: Option<QobuzImage>,
    pub label: Option<QobuzLabel>,
    pub upc: Option<String>,
    pub genre: Option<QobuzGenre>,
    #[serde(rename = "media_count")]
    pub total_discs: Option<i32>,
    #[serde(rename = "tracks_count")]
    pub total_tracks: Option<i32>,
    pub artist: Option<QobuzPerformer>,
    pub goodies: Option<Vec<QobuzGoodie>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzLabel {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzImage {
    pub small: Option<String>,
    pub large: Option<String>,
}

/// Qobuz search response
#[derive(Debug, Deserialize)]
pub struct QobuzSearchResponse {
    pub tracks: Option<QobuzTracksContainer>,
}

#[derive(Debug, Deserialize)]
pub struct QobuzTracksContainer {
    pub items: Vec<QobuzTrack>,
}

/// Download URL response from proxy API
#[derive(Debug, Deserialize)]
struct StreamResponse {
    url: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

/// Map quality string to Qobuz format_id (deterministic cascade)
pub fn map_quality_to_format_id(quality: &str) -> &'static str {
    match quality.to_uppercase().as_str() {
        "24-192" | "HI_RES_LOSSLESS" | "27" => "27", // 24-bit / up to 192kHz FLAC
        "24-96" | "HI_RES" | "HIRES" | "7" => "7",   // 24-bit / up to 96kHz FLAC
        "16-44" | "16-44.1" | "LOSSLESS" | "6" => "6", // 16-bit / 44.1kHz FLAC
        "320" | "HIGH" | "5" => "5",                 // 320kbps MP3
        _ => "6",                                    // Default 16-bit / 44.1kHz FLAC
    }
}

/// Map quality string to allowed Qobuz format_ids in cascade order (identical to CLI)
pub fn map_quality_to_allowed_format_ids_with_lossy_fallback(quality: &str, allow_lossy_fallback: bool) -> &'static [&'static str] {
    match (quality.to_uppercase().trim(), allow_lossy_fallback) {
        ("27" | "HI_RES_LOSSLESS" | "24-192" | "24/192", true) => &["27", "7", "6", "5"],
        ("27" | "HI_RES_LOSSLESS" | "24-192" | "24/192", false) => &["27", "7", "6"],

        ("7" | "HI_RES" | "HIRES" | "24-96" | "24/96", true) => &["7", "6", "5"],
        ("7" | "HI_RES" | "HIRES" | "24-96" | "24/96", false) => &["7", "6"],

        ("6" | "LOSSLESS" | "16-44" | "16/44" | "16-44.1" | "16/44.1", true) => &["6", "5"],
        ("6" | "LOSSLESS" | "16-44" | "16/44" | "16-44.1" | "16/44.1", false) => &["6"],

        ("5" | "MP3" | "320" | "320KBPS" | "HIGH", _) => &["5"],

        (_, true) => &["27", "7", "6", "5"],
        (_, false) => &["27", "7", "6"],
    }
}

/// Build pure deterministic MD5 signature for `track/getFileUrl`
pub fn build_request_signature(format_id: &str, track_id: &str, ts: &str, app_secret: &str) -> String {
    let raw = format!(
        "trackgetFileUrlformat_id{}intentstreamtrack_id{}{}{}",
        format_id, track_id, ts, app_secret
    );
    let digest = md5::compute(raw.as_bytes());
    let sig = format!("{:x}", digest);
    debug!("[Qobuz] Generated signature for format={} track_id={}", format_id, track_id);
    sig
}

/// Pure parameter signing for arbitrary endpoints
pub fn sign_api_request(endpoint_path: &str, params: &mut Vec<(&str, String)>, app_secret: &str) {
    params.sort_by(|a, b| a.0.cmp(b.0));
    let mut concat = endpoint_path.replace('/', "");
    for (k, v) in params.iter() {
        concat.push_str(k);
        concat.push_str(v);
    }
    concat.push_str(app_secret);
    let digest = md5::compute(concat.as_bytes());
    params.push(("request_sig", format!("{:x}", digest)));
}

/// Qobuz downloader using official signed API with proxy fallback
pub struct QobuzDownloader {
    client: Client,
    app_id: String,
    app_secret: String,
}

impl QobuzDownloader {
    pub fn new() -> Self {
        let app_id = std::env::var("QOBUZ_APP_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| QOBUZ_APP_ID.to_string());

        let app_secret = std::env::var("QOBUZ_APP_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "05a4851e74ee47fda346f50cfdfc4f09")
            .unwrap_or_else(|| QOBUZ_APP_SECRET.to_string());

        Self {
            client: create_http_client(),
            app_id,
            app_secret,
        }
    }

    /// Resolve Qobuz user auth token exclusively from SQLite accounts table (same logic as service.rs)
    pub async fn resolve_token(&self, db_opt: Option<&sqlx::SqlitePool>) -> Result<String, QobuzAuthStatus> {
        let pool = match db_opt {
            Some(p) => p,
            None => {
                return Err(QobuzAuthStatus::RequiresAuth(
                    "No database pool provided to resolve Qobuz account. Please log in via Syncify.".to_string(),
                ));
            }
        };

        // Query active Qobuz account from SQLite (identical to service.rs:load_service_credentials)
        let account: Result<(i64, String), _> = sqlx::query_as(
            "SELECT a.id, a.credentials_json FROM accounts a 
             JOIN services s ON s.id = a.service_id 
             WHERE s.name = 'qobuz' AND a.is_active = 1 
             ORDER BY a.id DESC LIMIT 1"
        )
        .fetch_one(pool)
        .await;

        let (_account_id, encrypted_json) = match account {
            Ok(acc) => acc,
            Err(_) => {
                return Err(QobuzAuthStatus::RequiresAuth(
                    "Qobuz account not connected. Please log in to Qobuz in Syncify (Settings > Accounts).".to_string(),
                ));
            }
        };

        // Decrypt using application Keychain crypto (same as service.rs / auth.rs)
        let decrypted = match crate::crypto::decrypt(&encrypted_json) {
            Ok(d) => d,
            Err(e) => {
                return Err(QobuzAuthStatus::RequiresAuth(
                    format!("Failed to decrypt Qobuz credentials: {}. Please re-authenticate in Syncify.", e),
                ));
            }
        };

        let creds: serde_json::Value = match serde_json::from_str(&decrypted) {
            Ok(c) => c,
            Err(e) => {
                return Err(QobuzAuthStatus::RequiresAuth(
                    format!("Invalid Qobuz credentials JSON: {}. Please re-authenticate in Syncify.", e),
                ));
            }
        };

/// Validate that a Qobuz auth token is usable (filters browser cookie artifacts)
fn is_viable_qobuz_token(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() || t == "browser_cookies" || t == "null" || t == "undefined" {
        return false;
    }
    if t.starts_with('{') || t.starts_with('[') || t.starts_with("eyJ") {
        return false;
    }
    if t.len() < 16 {
        return false;
    }
    !t.chars().any(|c| c.is_whitespace())
}

        // Extract token matching service.rs logic
        let stored_token = creds["user_auth_token"]
            .as_str()
            .or_else(|| creds["auth_token"].as_str())
            .or_else(|| creds["access_token"].as_str());

        if let Some(token) = stored_token {
            if is_viable_qobuz_token(token) {
                return Ok(token.trim().to_string());
            }
        }

        // Check environment variable fallback
        if let Ok(env_token) = std::env::var("QOBUZ_USER_TOKEN") {
            if is_viable_qobuz_token(&env_token) {
                return Ok(env_token.trim().to_string());
            }
        }

        // If token is missing / browser_cookies, check if username and password exist to perform API login
        let username = creds["username"].as_str();
        let password = creds["password"].as_str();

        if let (Some(user), Some(pass)) = (username, password) {
            if !user.trim().is_empty() && !pass.trim().is_empty() {
                info!("[Qobuz] Performing direct login with stored username/password...");
                let client = crate::services::QobuzClient::new(self.app_id.clone(), self.app_secret.clone());
                match client.login(user, pass).await {
                    Ok(fresh_token) => return Ok(fresh_token),
                    Err(e) => {
                        return Err(QobuzAuthStatus::RequiresAuth(
                            format!("Qobuz auto-login failed: {}. Please re-authenticate in Settings > Accounts.", e),
                        ));
                    }
                }
            }
        }

        Err(QobuzAuthStatus::RequiresAuth(
            "No valid Qobuz auth token found in stored credentials. Please log in via Settings > Accounts.".to_string(),
        ))
    }

    /// Get available proxy APIs (decoded from base64)
    fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "aHR0cHM6Ly9xb2J1ei5raW5vcGx1cy5vbmxpbmUvdHJhY2svZ2V0P2lkPQ==",
            "aHR0cHM6Ly9xb2J1ei1hcGkuYmluaW11bS5vcmcvdHJhY2svZ2V0P2lkPQ==",
            "ZGFiLnllZXQuc3UvYXBpL3N0cmVhbT90cmFja0lkPQ==", // dab.yeet.su
            "ZGFibXVzaWMueHl6L2FwaS9zdHJlYW0/dHJhY2tJZD0=", // dabmusic.xyz
        ];

        encoded_apis
            .iter()
            .filter_map(|encoded| {
                BASE64.decode(encoded).ok().and_then(|bytes| {
                    let s = String::from_utf8(bytes).ok()?;
                    if s.starts_with("http") {
                        Some(s)
                    } else {
                        Some(format!("https://{}", s))
                    }
                })
            })
            .collect()
    }

    /// Primary official Qobuz `track/getFileUrl` endpoint (requires valid user_auth_token)
    /// Tries allowed format IDs in cascade order (identical to CLI parity)
    pub async fn get_official_download_url(
        &self,
        track_id: i64,
        quality: &str,
        user_auth_token: &str,
        allow_fallback: bool,
    ) -> Result<StreamResolution> {
        if user_auth_token.trim().is_empty() {
            return Err(anyhow!("Cannot query official Qobuz stream URL: user_auth_token is empty"));
        }

        let allowed_formats = map_quality_to_allowed_format_ids_with_lossy_fallback(quality, allow_fallback);
        let track_id_str = track_id.to_string();
        let mut last_error = String::new();

        for format_id in allowed_formats {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
                .to_string();

            let sig = build_request_signature(format_id, &track_id_str, &ts, &self.app_secret);

            let get_url = format!(
                "{}/track/getFileUrl?format_id={}&intent=stream&track_id={}&request_ts={}&request_sig={}",
                QOBUZ_API_BASE, format_id, track_id_str, ts, sig
            );

            debug!(
                "[Qobuz] Requesting stream URL for format_id={} track_id={}",
                format_id, track_id_str
            );

            let response = self
                .client
                .get(&get_url)
                .header("X-App-Id", &self.app_id)
                .header("X-User-Auth-Token", user_auth_token)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                        let error_body = resp.text().await.unwrap_or_default();
                        warn!("[Qobuz] Token expired or unauthorized (HTTP {}): {}", status, error_body);
                        return Err(anyhow!("RequiresAuth: Qobuz token expired (HTTP {}). Please re-authenticate via Settings > Accounts.", status));
                    }

                    if status.is_success() {
                        if let Ok(resp_json) = resp.json::<serde_json::Value>().await {
                            if let Some(stream_url) = resp_json["url"].as_str() {
                                if !stream_url.trim().is_empty() {
                                    info!("[Qobuz] ✓ Acquired official Qobuz stream URL (format_id: {})", format_id);
                                    return Ok(StreamResolution {
                                        url: stream_url.to_string(),
                                        source: StreamUrlSource::QobuzOfficial,
                                        format_id: format_id.to_string(),
                                    });
                                }
                            }
                        }
                    } else {
                        let error_body = resp.text().await.unwrap_or_default();
                        last_error = format!("HTTP {} - {}", status, error_body);
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }
        }

        Err(anyhow!("Qobuz official getFileUrl failed for all allowed formats: {}", last_error))
    }

    /// Secondary proxy fallback stream endpoint (never receives user_auth_token)
    pub async fn get_proxy_download_url(
        &self,
        track_id: i64,
        quality: &str,
    ) -> Result<StreamResolution> {
        let format_id = map_quality_to_format_id(quality);
        let apis = Self::get_proxy_apis();

        for api in apis {
            let url = if api.contains("trackId=") {
                format!("{}{}&quality={}", api, track_id, format_id)
            } else {
                format!("{}{}&quality={}", api, track_id, format_id)
            };

            let result = self.client.get(&url).timeout(Duration::from_secs(15)).send().await;
            if let Ok(resp) = result {
                if resp.status().is_success() {
                    if let Ok(stream_resp) = resp.json::<StreamResponse>().await {
                        if let Some(download_url) = stream_resp.url {
                            if !download_url.trim().is_empty() && !download_url.to_lowercase().contains("error") {
                                info!("[Qobuz] ✓ Acquired fallback stream URL via proxy {}", api);
                                return Ok(StreamResolution {
                                    url: download_url,
                                    source: StreamUrlSource::ProxyFallback(api),
                                    format_id: format_id.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Err(anyhow!("All Qobuz proxy fallback APIs failed"))
    }

    /// Resolve stream URL with official primary route and explicit proxy fallback
    pub async fn get_download_url(
        &self,
        track_id: i64,
        quality: &str,
        user_auth_token: Option<&str>,
        allow_fallback: bool,
    ) -> Result<StreamResolution> {
        if let Some(token) = user_auth_token {
            if !token.trim().is_empty() {
                match self.get_official_download_url(track_id, quality, token, allow_fallback).await {
                    Ok(res) => return Ok(res),
                    Err(e) => {
                        let err_str = e.to_string();
                        // If token is expired (RequiresAuth / 401 / 403), fail fast so user can re-authenticate
                        if err_str.contains("RequiresAuth") || err_str.contains("401") || err_str.contains("403") {
                            return Err(e);
                        }
                        warn!("[Qobuz] Official stream resolution failed: {}. Falling back to proxy...", e);
                    }
                }
            }
        }

        // Fallback to proxy
        self.get_proxy_download_url(track_id, quality).await
    }

    /// Search for a track by ISRC
    pub async fn search_by_isrc(
        &self,
        isrc: &str,
        expected_duration_sec: i32,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;

        let url = format!("{}/track/search", QOBUZ_API_BASE);
        let mut params = vec![
            ("app_id", self.app_id.clone()),
            ("limit", "50".to_string()),
            ("query", isrc.to_string()),
        ];
        sign_api_request("track/search", &mut params, &self.app_secret);

        debug!("[Qobuz] Searching by ISRC: {}", isrc);

        let mut req = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .header("X-App-Id", &self.app_id)
            .query(&params);

        if let Some(token) = user_auth_token {
            if !token.trim().is_empty() {
                req = req.header("X-User-Auth-Token", token);
            }
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz search failed: HTTP {}", response.status()));
        }

        let result: QobuzSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks in response"))?;

        // Find exact ISRC match with duration verification
        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        info!(
                            "[Qobuz] Found ISRC match: '{}' (duration verified)",
                            track.title
                        );
                        return Ok(track.clone());
                    } else {
                        warn!(
                            "[Qobuz] ISRC match but duration mismatch: expected {}s, got {}s",
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

    /// Search for a track by metadata (artist + track name)
    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;

        let query = format!("{} {}", artist_name, track_name);
        let url = format!("{}/track/search", QOBUZ_API_BASE);
        let mut params = vec![
            ("app_id", self.app_id.clone()),
            ("limit", "50".to_string()),
            ("query", query.clone()),
        ];
        sign_api_request("track/search", &mut params, &self.app_secret);

        debug!(
            "[Qobuz] Searching by metadata: {} - {}",
            artist_name, track_name
        );

        let mut req = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .header("X-App-Id", &self.app_id)
            .query(&params);

        if let Some(token) = user_auth_token {
            if !token.trim().is_empty() {
                req = req.header("X-User-Auth-Token", token);
            }
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz search failed: HTTP {}", response.status()));
        }

        let result: QobuzSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks in search response"))?;

        // Find best match by title and duration
        for track in &tracks.items {
            if !title_matches(track_name, &track.title) {
                continue;
            }

            if let Some(performer) = &track.performer {
                if !artist_matches(artist_name, &performer.name) {
                    continue;
                }
            }

            if expected_duration_sec > 0 {
                let duration_diff = (track.duration - expected_duration_sec).abs();
                if duration_diff > 10 {
                    continue;
                }
            }

            info!(
                "[Qobuz] Found metadata match: '{}' by '{}'",
                track.title,
                track
                    .performer
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("Unknown")
            );
            return Ok(track.clone());
        }

        Err(anyhow!(
            "No matching track found for: {} - {}",
            artist_name,
            track_name
        ))
    }

    /// Get track metadata directly by Qobuz track ID (deterministic entity resolution)
    pub async fn get_track_by_id(
        &self,
        track_id: i64,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;

        let url = format!("{}/track/get", QOBUZ_API_BASE);
        let mut params = vec![
            ("app_id", self.app_id.clone()),
            ("track_id", track_id.to_string()),
        ];
        sign_api_request("track/get", &mut params, &self.app_secret);

        debug!("[Qobuz] Fetching track entity directly by ID: {}", track_id);

        let mut req = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .header("X-App-Id", &self.app_id)
            .query(&params);

        if let Some(token) = user_auth_token {
            if !token.trim().is_empty() {
                req = req.header("X-User-Auth-Token", token);
            }
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz track/get failed for ID {}: HTTP {}", track_id, response.status()));
        }

        let track: QobuzTrack = response.json().await?;
        info!("[Qobuz] ✓ Resolved exact track entity: '{}' (ID: {})", track.title, track.id);
        Ok(track)
    }

    /// Get album metadata directly by Qobuz album ID (fetches full album entity with goodies)
    pub async fn get_album_by_id(
        &self,
        album_id: &str,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzAlbum> {
        QOBUZ_LIMITER.wait("qobuz").await;

        let url = format!("{}/album/get", QOBUZ_API_BASE);
        let mut params = vec![
            ("app_id", self.app_id.clone()),
            ("album_id", album_id.to_string()),
        ];
        sign_api_request("album/get", &mut params, &self.app_secret);

        debug!("[Qobuz] Fetching album entity directly by ID: {}", album_id);

        let mut req = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .header("X-App-Id", &self.app_id)
            .query(&params);

        if let Some(token) = user_auth_token {
            if !token.trim().is_empty() {
                req = req.header("X-User-Auth-Token", token);
            }
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz album/get failed for ID {}: HTTP {}", album_id, response.status()));
        }

        let album: QobuzAlbum = response.json().await?;
        info!("[Qobuz] ✓ Resolved album entity: '{}' (ID: {:?})", album.title, album.id);
        Ok(album)
    }

    /// Download stream payload into staging file with byte progress and automatic retries
    #[allow(dead_code)]
    pub async fn download_to_staging(
        &self,
        download_url: &str,
        staging_path: &Path,
        item_id: &str,
    ) -> Result<u64> {
        self.download_to_staging_internal(download_url, staging_path, item_id, None)
            .await
    }

    /// Internal stream download helper with exponential backoff, staging cleanup, rate limiting, and NetworkExhausted classification
    pub async fn download_to_staging_internal(
        &self,
        initial_download_url: &str,
        staging_path: &Path,
        item_id: &str,
        stream_url_provider: Option<(&Self, i64, &str, Option<&str>, bool)>,
    ) -> Result<u64> {
        debug!("[Qobuz] Downloading to staging: {:?}", staging_path);

        if let Some(parent) = staging_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let max_retries: u32 = 3;
        let mut attempt: u32 = 0;
        let mut current_url = initial_download_url.to_string();
        let initial_backoff = Duration::from_millis(500);
        let max_backoff = Duration::from_secs(10);

        loop {
            // Clean up partial staging file before each attempt
            if staging_path.exists() {
                let _ = tokio::fs::remove_file(staging_path).await;
            }

            QOBUZ_LIMITER.wait("qobuz").await;

            let response_res = self
                .client
                .get(&current_url)
                .header("User-Agent", get_user_agent())
                .send()
                .await;

            let response = match response_res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        resp
                    } else {
                        let is_transient = is_transient_status(status);
                        attempt += 1;
                        let _ = tokio::fs::remove_file(staging_path).await;

                        if !is_transient || attempt >= max_retries {
                            if is_transient {
                                return Err(anyhow!("NetworkExhausted: HTTP {} after {} attempts", status, attempt));
                            } else {
                                return Err(anyhow!("Download failed: HTTP {}", status));
                            }
                        }

                        let server_retry = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                            parse_retry_after(resp.headers(), std::time::SystemTime::now())
                        } else {
                            None
                        };

                        let backoff = calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
                        let wait_dur = server_retry.unwrap_or(backoff);
                        warn!(
                            "[Qobuz] Transient HTTP status {} for item {}. Retrying in {:?} (attempt {}/{})",
                            status, item_id, wait_dur, attempt, max_retries
                        );
                        tokio::time::sleep(wait_dur).await;

                        if let Some((downloader, track_id, quality, token_ref, allow_fallback)) = stream_url_provider {
                            if let Ok(new_stream) = downloader.get_download_url(track_id, quality, token_ref, allow_fallback).await {
                                current_url = new_stream.url;
                            }
                        }
                        continue;
                    }
                }
                Err(e) => {
                    attempt += 1;
                    let _ = tokio::fs::remove_file(staging_path).await;
                    let err_msg = e.to_string();

                    if attempt >= max_retries {
                        return Err(anyhow!("NetworkExhausted: Network error after {} attempts: {}", max_retries, err_msg));
                    }

                    let backoff = calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
                    warn!(
                        "[Qobuz] Network error for item {}: '{}'. Retrying in {:?} (attempt {}/{})",
                        item_id, err_msg, backoff, attempt, max_retries
                    );
                    tokio::time::sleep(backoff).await;

                    if let Some((downloader, track_id, quality, token_ref, allow_fallback)) = stream_url_provider {
                        if let Ok(new_stream) = downloader.get_download_url(track_id, quality, token_ref, allow_fallback).await {
                            current_url = new_stream.url;
                        }
                    }
                    continue;
                }
            };

            let total_size_opt = response.content_length();
            let mut tracker = ByteStreamTracker::new(item_id, "qobuz", total_size_opt);
            PROGRESS_TRACKER.update(DownloadProgress::downloading_bytes(
                item_id,
                "qobuz",
                0,
                total_size_opt,
                0.0,
                0.0,
            ));

            let raw_file = match File::create(staging_path).await {
                Ok(f) => f,
                Err(e) => {
                    let _ = tokio::fs::remove_file(staging_path).await;
                    return Err(e.into());
                }
            };
            let mut file = tokio::io::BufWriter::with_capacity(256 * 1024, raw_file);
            let mut downloaded: u64 = 0;
            let mut stream = response.bytes_stream();
            use futures_util::StreamExt;
            let mut stream_failed: Option<String> = None;

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        if let Err(e) = file.write_all(&chunk).await {
                            stream_failed = Some(e.to_string());
                            break;
                        }
                        downloaded += chunk.len() as u64;

                        if let Some(progress) = tracker.on_bytes(downloaded, false) {
                            PROGRESS_TRACKER.update(progress);
                        }
                    }
                    Err(e) => {
                        stream_failed = Some(e.to_string());
                        break;
                    }
                }
            }

            if let Some(err_msg) = stream_failed {
                attempt += 1;
                let _ = tokio::fs::remove_file(staging_path).await;

                if attempt >= max_retries {
                    return Err(anyhow!("NetworkExhausted: Stream decoding error after {} attempts: {}", max_retries, err_msg));
                }

                let backoff = calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
                warn!(
                    "[Qobuz] Stream error for item {}: '{}'. Retrying in {:?} (attempt {}/{})",
                    item_id, err_msg, backoff, attempt, max_retries
                );
                tokio::time::sleep(backoff).await;

                if let Some((downloader, track_id, quality, token_ref, allow_fallback)) = stream_url_provider {
                    if let Ok(new_stream) = downloader.get_download_url(track_id, quality, token_ref, allow_fallback).await {
                        current_url = new_stream.url;
                    }
                }
                continue;
            }

            if let Err(e) = file.flush().await {
                attempt += 1;
                let _ = tokio::fs::remove_file(staging_path).await;
                if attempt >= max_retries {
                    return Err(anyhow!("NetworkExhausted: Flush error after {} attempts: {}", max_retries, e));
                }
                let backoff = calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
                tokio::time::sleep(backoff).await;
                continue;
            }

            if let Some(progress) = tracker.on_bytes(downloaded, true) {
                PROGRESS_TRACKER.update(progress);
            }

            info!("[Qobuz] Staging payload written: {} bytes", downloaded);
            return Ok(downloaded);
        }
    }

    /// Full download flow: search/entity resolution → get stream URL → staging download → validation → tagging → atomic promotion
    pub async fn download_track(
        &self,
        request: &DownloadRequest,
        db_opt: Option<&sqlx::SqlitePool>,
    ) -> Result<DownloadResult> {
        let mut phase_tracker = DownloadPhaseTracker::new();
        let item_id = &request.item_id;
        let duration_sec = (request.duration_ms / 1000) as i32;

        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "qobuz"));

        // 1. Auth phase
        phase_tracker.start_phase(DownloadPhase::Auth);
        let token_opt = self.resolve_token(db_opt).await.ok();
        let token_ref = token_opt.as_deref();

        // 2. ResolveStream phase (Entity lookup + Stream URL resolution)
        phase_tracker.start_phase(DownloadPhase::ResolveStream);
        let track = if let Some(ref s_track_id) = request.service_track_id {
            if let Ok(tid) = s_track_id.parse::<i64>() {
                info!("[Qobuz] Resolving exact entity for service_track_id={}", tid);
                self.get_track_by_id(tid, token_ref).await?
            } else {
                return Err(anyhow!("SourceIdentityMissing: Non-numeric service_track_id '{}'", s_track_id));
            }
        } else {
            // Only allow ISRC search if explicitly authorized with BOTH allow_fallback and smart_studio_origin
            if !request.allow_fallback || !request.smart_studio_origin {
                return Err(anyhow!("SourceIdentityMissing: No locked service_track_id and allow_fallback=false"));
            }
            if let Some(isrc) = &request.isrc {
                info!("[Qobuz] Using resolution_strategy='isrc_fallback' for isrc={}", isrc);
                match self.search_by_isrc(isrc, duration_sec, token_ref).await {
                    Ok(t) => t,
                    Err(_) => {
                        self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token_ref)
                            .await?
                    }
                }
            } else {
                info!("[Qobuz] Using resolution_strategy='metadata_fallback' for title='{}' artist='{}'", request.track_name, request.artist_name);
                self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token_ref)
                    .await?
            }
        };

        // 3. Resolve Stream URL (tries allowed formats in cascade order)
        let stream_res = self
            .get_download_url(track.id, &request.quality, token_ref, request.allow_fallback)
            .await?;

        // 4. Staging path setup
        let out_dir = PathBuf::from(&request.output_dir);
        let staging_dir = out_dir.join(".staging");
        tokio::fs::create_dir_all(&staging_dir).await?;
        let nomedia_path = staging_dir.join(".nomedia");
        if !nomedia_path.exists() {
            let _ = tokio::fs::write(&nomedia_path, b"").await;
        }
        let staging_path = staging_dir.join(format!("{}.part", sanitize_filename(&request.item_id)));

        // 5. Transfer phase (Audio streaming to disk until flush)
        phase_tracker.start_phase(DownloadPhase::Transfer);
        let downloaded_bytes = match self
            .download_to_staging_internal(
                &stream_res.url,
                &staging_path,
                item_id,
                Some((self, track.id, &request.quality, token_ref, request.allow_fallback)),
            )
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                let _ = tokio::fs::remove_file(&staging_path).await;
                return Err(e);
            }
        };

        if downloaded_bytes == 0 {
            let _ = tokio::fs::remove_file(&staging_path).await;
            return Err(anyhow!("NetworkExhausted: Qobuz downloaded audio payload is 0 bytes"));
        }
        phase_tracker.set_transfer_metrics(downloaded_bytes, "network");

        // 6. ValidateAudio phase
        phase_tracker.start_phase(DownloadPhase::ValidateAudio);
        let header_bytes = tokio::fs::read(&staging_path).await.unwrap_or_default();
        let is_flac = stream_res.format_id != "5";

        if is_flac && !AudioByteValidator::is_flac_magic(&header_bytes) {
            let _ = tokio::fs::remove_file(&staging_path).await;
            return Err(anyhow!("Downloaded audio failed bit-perfect FLAC magic verification"));
        }

        // 7. Tagging with metaflac (VORBIS_COMMENT and PICTURE) + Full Enrichment
        let mut staged_lrc_path: Option<PathBuf> = None;
        let mut staged_cover_jpg_path: Option<PathBuf> = None;
        let mut staged_cover_webp_path: Option<PathBuf> = None;
        let mut staged_booklet_path: Option<PathBuf> = None;

        let artist_name = track
            .performer
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| request.artist_name.clone());
        let album_title = track
            .album
            .as_ref()
            .map(|a| a.title.clone())
            .unwrap_or_else(|| request.album_name.clone());
        let album_artist = track
            .album
            .as_ref()
            .and_then(|a| a.artist.as_ref())
            .map(|ar| ar.name.clone())
            .or_else(|| request.album_artist.clone())
            .or_else(|| Some(artist_name.clone()));
        let composer = track.composer.as_ref().map(|c| c.name.clone());
        let track_num = track.track_number.unwrap_or(request.track_number as i32) as u32;
        let track_tot = track
            .album
            .as_ref()
            .and_then(|a| a.total_tracks)
            .unwrap_or(request.total_tracks as i32) as u32;
        let disc_num = track.disc_number.unwrap_or(request.disc_number as i32) as u32;
        let disc_tot = track
            .album
            .as_ref()
            .and_then(|a| a.total_discs)
            .unwrap_or(1) as u32;
        let isrc_val = track.isrc.clone().or_else(|| request.isrc.clone());
        let rel_date = track
            .album
            .as_ref()
            .and_then(|a| a.release_date_original.clone())
            .or_else(|| request.release_date.clone());
        let rel_year = rel_date
            .as_ref()
            .and_then(|d| d.split('-').next().map(|y| y.to_string()));
        let label_val = track
            .album
            .as_ref()
            .and_then(|a| a.label.as_ref())
            .and_then(|l| l.name.clone());
        let barcode_val = track.album.as_ref().and_then(|a| a.upc.clone());
        let explicit_val = track.parental_warning;
        let copyright_val = track.copyright.clone();
        let performers_val = track.performers.clone().or_else(|| Some(artist_name.clone()));
        let work_val = track.work.clone();

        let mut has_lyrics_cached = false;
        let mut has_cover_cached = false;
        let mut has_mb_cached = false;

        if is_flac {
            PROGRESS_TRACKER.update(DownloadProgress::finalizing(item_id));

            let mut flac_meta = FlacMetadata {
                title: track.title.clone(),
                artist: artist_name.clone(),
                album: album_title.clone(),
                album_artist: album_artist.clone(),
                composer: composer.clone(),
                performers: performers_val.clone(),
                work: work_val.clone(),
                track_number: track_num,
                track_total: track_tot,
                disc_number: disc_num,
                disc_total: disc_tot,
                isrc: isrc_val.clone(),
                release_date: rel_date.clone(),
                release_year: rel_year.clone(),
                original_date: rel_date.clone(),
                copyright: copyright_val.clone(),
                label: label_val.clone(),
                barcode: barcode_val.clone(),
                explicit: explicit_val,
                audio_source: Some("Qobuz".to_string()),
                bit_depth: Some(track.max_bit_depth.unwrap_or(16)),
                sample_rate: Some(track.max_sample_rate.unwrap_or(44.1) * 1000.0),
                comment: Some(format!(
                    "Audio: Qobuz FLAC ({}bit/{}kHz) | Engine: Syncify Production",
                    track.max_bit_depth.unwrap_or(16),
                    track.max_sample_rate.unwrap_or(44.1)
                )),
                ..Default::default()
            };

            // 7a. ResolveCover phase
            phase_tracker.start_phase(DownloadPhase::ResolveCover);
            let cover_url = track
                .album
                .as_ref()
                .and_then(|a| a.image.as_ref())
                .and_then(|img| img.large.clone())
                .or_else(|| request.cover_url.clone());

            let mut raw_jpeg_bytes: Option<Vec<u8>> = None;
            if let Some(ref curl) = cover_url {
                match self.client.get(curl).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(bytes) = resp.bytes().await {
                            if !bytes.is_empty() {
                                let cover_staging = staging_dir.join(format!("{}.cover.jpg", sanitize_filename(item_id)));
                                let _ = tokio::fs::write(&cover_staging, &bytes).await;
                                staged_cover_jpg_path = Some(cover_staging);
                                raw_jpeg_bytes = Some(bytes.to_vec());
                                has_cover_cached = true;
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Attempt Apple Music Animated Cover resolution for motion artwork
            match resolve_and_download_animated_cover(&self.client, &artist_name, &album_title, &staging_dir).await {
                AnimatedCoverStatus::Success(webp_path) => {
                    info!("[Qobuz] ✓ Motion cover art resolved and downloaded from Apple Music: {:?}", webp_path);
                    if let Ok(webp_bytes) = tokio::fs::read(&webp_path).await {
                        use syncify_core_domain::byte_validators::WebpByteValidator;
                        if let Ok(info) = WebpByteValidator::validate_animated_webp(&webp_bytes) {
                            info!("[Qobuz] ✓ Validated animated WebP: {} frames, {}x{} px", info.anmf_frame_count, info.canvas_width, info.canvas_height);
                            flac_meta.cover_data = Some(webp_bytes);
                            flac_meta.cover_source = Some("Apple Music Animated Cover".to_string());
                            staged_cover_webp_path = Some(webp_path);
                            has_cover_cached = true;
                        } else {
                            warn!("[Qobuz] Animated WebP failed structural validation (falling back to static cover)");
                            if let Some(jpeg_bytes) = raw_jpeg_bytes {
                                flac_meta.cover_data = Some(jpeg_bytes);
                                flac_meta.cover_source = Some("Qobuz Cover Art".to_string());
                            }
                        }
                    }
                }
                AnimatedCoverStatus::NotFound => {
                    debug!("[Qobuz] No motion cover art found for '{} - {}' (falling back to static cover)", artist_name, album_title);
                    if let Some(jpeg_bytes) = raw_jpeg_bytes {
                        flac_meta.cover_data = Some(jpeg_bytes);
                        flac_meta.cover_source = Some("Qobuz Cover Art".to_string());
                    }
                }
                AnimatedCoverStatus::SourceUnavailable(reason) => {
                    warn!("[Qobuz] Animated cover source unavailable: {} (falling back to static cover)", reason);
                    if let Some(jpeg_bytes) = raw_jpeg_bytes {
                        flac_meta.cover_data = Some(jpeg_bytes);
                        flac_meta.cover_source = Some("Qobuz Cover Art".to_string());
                    }
                }
                AnimatedCoverStatus::Failed(reason) => {
                    warn!("[Qobuz] Animated cover resolution/conversion failed: {} (falling back to static cover)", reason);
                    if let Some(jpeg_bytes) = raw_jpeg_bytes {
                        flac_meta.cover_data = Some(jpeg_bytes);
                        flac_meta.cover_source = Some("Qobuz Cover Art".to_string());
                    }
                }
            }

            // 7b. ResolveLyrics phase
            phase_tracker.start_phase(DownloadPhase::ResolveLyrics);
            let lyrics_service = LyricsPipelineService::new();
            let duration_sec = if track.duration > 0 {
                track.duration as f64
            } else {
                (request.duration_ms / 1000) as f64
            };

            let mut resolved_lyrics_res: Option<LyricsResolution> = None;

            match lyrics_service
                .resolve_lyrics_and_sidecar(&artist_name, &track.title, Some(&album_title), duration_sec)
                .await
            {
                Ok((res, sidecar_opt)) => {
                    if res.status == ResolutionStatus::Resolved {
                        let tags = res.to_tag_contract();
                        if let Some(ref lyr) = tags.lyrics {
                            flac_meta.lyrics_lrc = Some(lyr.clone());
                        }
                        if let Some(ref src) = tags.source {
                            flac_meta.lyrics_source = Some(src.clone());
                        }

                        if let Some(ref lrc_content) = sidecar_opt {
                            let lrc_staging = staging_dir.join(format!("{}.lrc", sanitize_filename(item_id)));
                            if let Ok(_) = tokio::fs::write(&lrc_staging, lrc_content).await {
                                staged_lrc_path = Some(lrc_staging);
                                info!("[Qobuz] Synced lyrics acquired from {}: embedded and staged as .lrc", res.provider);
                            }
                        } else {
                            info!("[Qobuz] Plain lyrics acquired from {} (no sidecar created)", res.provider);
                        }

                        has_lyrics_cached = true;
                        resolved_lyrics_res = Some(res);
                    } else {
                        debug!("[Qobuz] Lyrics not resolved (status: {:?})", res.status);
                    }
                }
                Err(e) => {
                    debug!("[Qobuz] Lyrics resolution error (best-effort): {}", e);
                }
            }

            // 7c. Digital Booklet (Goodies) PDF Resolution (Best-Effort)
            let mut goodies_list = track.album.as_ref().and_then(|a| a.goodies.clone());
            if goodies_list.as_ref().map(|g| g.is_empty()).unwrap_or(true) {
                if let Some(album_id) = track.album.as_ref().and_then(|a| a.id.as_deref()) {
                    if let Ok(album_entity) = self.get_album_by_id(album_id, token_ref).await {
                        goodies_list = album_entity.goodies;
                    }
                }
            }

            if let Some(goodies) = goodies_list {
                for g in goodies {
                    let is_pdf = g.url.as_deref().map(|u: &str| u.ends_with(".pdf") || u.contains("booklet") || u.contains("pdf")).unwrap_or(false)
                        || g.original_url.as_deref().map(|u: &str| u.ends_with(".pdf") || u.contains("booklet") || u.contains("pdf")).unwrap_or(false)
                        || g.name.as_deref().map(|n: &str| n.to_lowercase().contains("booklet") || n.to_lowercase().contains("pdf") || n.to_lowercase().contains("livret")).unwrap_or(false)
                        || g.file_format_id == Some(21)
                        || g.file_format_id == Some(1);
                    if is_pdf {
                        if let Some(g_url) = g.url.as_deref().or(g.original_url.as_deref()) {
                            if let Ok(resp) = self.client.get(g_url).send().await {
                                if resp.status().is_success() {
                                    if let Ok(bytes) = resp.bytes().await {
                                        if !bytes.is_empty() && (bytes.starts_with(b"%PDF") || bytes.len() > 100) {
                                            let booklet_staging = staging_dir.join(format!("{}.booklet.pdf", sanitize_filename(item_id)));
                                            let _ = tokio::fs::write(&booklet_staging, &bytes).await;
                                            staged_booklet_path = Some(booklet_staging);
                                            info!("[Qobuz] ✓ Staged digital booklet PDF ({} bytes)", bytes.len());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 7d. EnrichMetadata phase
            phase_tracker.start_phase(DownloadPhase::EnrichMetadata);
            let origin_meta = OriginTrackMetadata {
                title: Some(track.title.clone()),
                artist: Some(artist_name.clone()),
                album: Some(album_title.clone()),
                album_artist: album_artist.clone(),
                composer: composer.clone(),
                performers: performers_val.clone(),
                work: work_val.clone(),
                genre: track.album.as_ref().and_then(|a| a.genre.as_ref()).map(|g| g.name.clone()),
                bpm: track.bpm,
                track_number: Some(track_num),
                disc_number: Some(disc_num),
                track_total: Some(track_tot),
                disc_total: Some(disc_tot),
                release_year: rel_year.clone(),
                release_date: rel_date.clone(),
                original_date: rel_date.clone(),
                isrc: isrc_val.clone(),
                label: label_val.clone(),
                barcode: barcode_val.clone(),
                copyright: copyright_val.clone(),
                explicit: explicit_val,
                audio_source: Some("Qobuz".to_string()),
                source_name: "Qobuz".to_string(),
                ..Default::default()
            };

            let enrichment_engine = EnrichmentEngine::new();
            let enriched = enrichment_engine
                .resolve_track_metadata(
                    &artist_name,
                    &album_title,
                    &track.title,
                    isrc_val.as_deref(),
                    Some(&origin_meta),
                )
                .await;

            if let Some(lbl) = enriched.label.value() {
                flac_meta.label = Some(lbl.to_string());
            }
            if let Some(cat) = enriched.catalog_number.value() {
                flac_meta.catalog_number = Some(cat.to_string());
            }
            if let Some(bc) = enriched.barcode.value() {
                flac_meta.barcode = Some(bc.to_string());
            }
            if let Some(od) = enriched.original_date.value() {
                flac_meta.original_date = Some(od.to_string());
            }
            if let Some(rtype) = enriched.release_type.value() {
                flac_meta.release_type = Some(rtype.to_string());
            }
            if let Some(rstat) = enriched.release_status.value() {
                flac_meta.release_status = Some(rstat.to_string());
            }
            if let Some(rcntry) = enriched.release_country.value() {
                flac_meta.release_country = Some(rcntry.to_string());
            }
            if let Some(genre) = enriched.genre.value() {
                flac_meta.genre = Some(genre.to_string());
            } else if let Some(ref g) = track.album.as_ref().and_then(|a| a.genre.as_ref()).map(|g| g.name.clone()) {
                flac_meta.genre = Some(g.clone());
            }
            if let Some(style) = enriched.style.value() {
                flac_meta.style = Some(style.to_string());
            }
            if let Some(mood) = enriched.mood.value() {
                flac_meta.mood = Some(mood.to_string());
            }
            if let Some(tags) = enriched.tags.value() {
                flac_meta.tags = Some(tags.to_string());
            }
            if let Some(lang) = enriched.language.value() {
                flac_meta.language = Some(lang.to_string());
            }
            if let Some(comp) = enriched.compilation.value() {
                flac_meta.compilation = Some(comp == "1");
            }
            if let Some(grp) = enriched.grouping.value() {
                flac_meta.grouping = Some(grp.to_string());
            }
            if let Some(bpm) = enriched.bpm.value().and_then(|s| s.parse::<u32>().ok()).or(track.bpm) {
                flac_meta.bpm = Some(bpm);
            }
            if let Some(mb_rid) = enriched.musicbrainz_recording_id.value() {
                flac_meta.musicbrainz_track_id = Some(mb_rid.to_string());
                has_mb_cached = true;
            }
            if let Some(mb_relid) = enriched.musicbrainz_release_id.value() {
                flac_meta.musicbrainz_album_id = Some(mb_relid.to_string());
            }
            if let Some(mb_rgid) = enriched.musicbrainz_release_group_id.value() {
                flac_meta.musicbrainz_release_group_id = Some(mb_rgid.to_string());
            }
            if let Some(mb_aid) = enriched.musicbrainz_artist_id.value() {
                flac_meta.musicbrainz_artist_id = Some(mb_aid.to_string());
            }
            if let Some(mb_aaid) = enriched.musicbrainz_albumartist_id.value() {
                flac_meta.musicbrainz_albumartist_id = Some(mb_aaid.to_string());
            }
            if let Some(mb_wid) = enriched.musicbrainz_work_id.value() {
                flac_meta.musicbrainz_work_id = Some(mb_wid.to_string());
            }

            // 7e. Tagging phase
            phase_tracker.start_phase(DownloadPhase::Tagging);
            match apply_and_verify_flac_tags(&staging_path, &flac_meta) {
                Ok(verified) => {
                    info!(
                        "[Qobuz] Tagged and verified FLAC (flac_valid: {}, tags_match: {}, cover: {}, lyrics: {})",
                        verified.flac_valid, verified.tags_match, verified.cover_present, verified.lyrics_present
                    );

                    if let Some(ref res) = resolved_lyrics_res {
                        if res.sync_type == LyricsSyncType::Plain {
                            let _ = validate_and_embed_flac_lyrics(&staging_path, res);
                        }
                    }
                }
                Err(e) => {
                    let _ = tokio::fs::remove_file(&staging_path).await;
                    if let Some(ref lrc_p) = staged_lrc_path {
                        let _ = tokio::fs::remove_file(lrc_p).await;
                    }
                    if let Some(ref cov_p) = staged_cover_jpg_path {
                        let _ = tokio::fs::remove_file(cov_p).await;
                    }
                    if let Some(ref webp_p) = staged_cover_webp_path {
                        let _ = tokio::fs::remove_file(webp_p).await;
                    }
                    if let Some(ref bkt_p) = staged_booklet_path {
                        let _ = tokio::fs::remove_file(bkt_p).await;
                    }
                    return Err(anyhow!("Failed FLAC tagging and verification in staging: {}", e));
                }
            }
        }

        // 8. Promotion phase
        phase_tracker.start_phase(DownloadPhase::Promotion);
        let ext = if is_flac { "flac" } else { "mp3" };

        let template_config = if let Some(pool) = db_opt {
            sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
                "SELECT folder_template, file_template, artist_separator, replace_spaces_with, max_path_length FROM folder_settings WHERE id = 1"
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .map(|(f_tpl, file_tpl, art_sep, r_sp, max_l)| FolderFileTemplateConfig {
                folder_template: f_tpl,
                file_template: file_tpl,
                artist_separator: art_sep,
                replace_spaces_with: r_sp,
                max_path_length: max_l as usize,
            })
            .unwrap_or_default()
        } else {
            FolderFileTemplateConfig::default()
        };

        let layout = LibraryLayout::with_config(&out_dir, template_config);

        let artist_name = track
            .performer
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| request.artist_name.clone());
        let album_title = track
            .album
            .as_ref()
            .map(|a| a.title.clone())
            .unwrap_or_else(|| request.album_name.clone());
        let album_artist = track
            .album
            .as_ref()
            .and_then(|a| a.artist.as_ref())
            .map(|ar| ar.name.clone())
            .or_else(|| request.album_artist.clone())
            .or_else(|| Some(artist_name.clone()));
        let track_num = track.track_number.unwrap_or(request.track_number as i32) as u32;
        let track_tot = track
            .album
            .as_ref()
            .and_then(|a| a.total_tracks)
            .unwrap_or(request.total_tracks as i32) as u32;
        let disc_num = track.disc_number.unwrap_or(request.disc_number as i32) as u32;
        let disc_tot = track
            .album
            .as_ref()
            .and_then(|a| a.total_discs)
            .unwrap_or(1) as u32;
        let rel_date = track
            .album
            .as_ref()
            .and_then(|a| a.release_date_original.clone())
            .or_else(|| request.release_date.clone());
        let rel_year_i32 = rel_date
            .as_ref()
            .and_then(|d| d.split('-').next().and_then(|y| y.parse::<i32>().ok()));

        let layout_ctx = TrackLayoutContext {
            artist: &artist_name,
            album_artist: album_artist.as_deref(),
            album: &album_title,
            title: &track.title,
            year: rel_year_i32,
            original_date: rel_date.as_deref(),
            track_number: track_num,
            track_total: Some(track_tot),
            disc_number: disc_num,
            total_discs: disc_tot,
            format: ext,
            bit_depth: track.max_bit_depth,
            sample_rate: track.max_sample_rate.map(|s| s * 1000.0),
        };

        let raw_final_path = layout.resolve_track_path(&layout_ctx);
        let final_path = layout.resolve_unique_path(&raw_final_path);
        let target_dir = final_path.parent().unwrap_or(&out_dir);
        tokio::fs::create_dir_all(target_dir).await?;

        // 8. Atomic promotion from staging to final path
        if let Err(e) = tokio::fs::rename(&staging_path, &final_path).await {
            let staged_bytes = tokio::fs::read(&staging_path).await
                .map_err(|re| anyhow!("Failed to read staged file {:?}: {}", staging_path, re))?;
            let staged_sha = crate::services::repair_guardrail::compute_bytes_sha256(&staged_bytes);
            let staged_len = staged_bytes.len() as u64;

            if let Err(ce) = tokio::fs::write(&final_path, &staged_bytes).await {
                let _ = tokio::fs::remove_file(&staging_path).await;
                if let Some(ref p) = staged_lrc_path { let _ = tokio::fs::remove_file(p).await; }
                if let Some(ref p) = staged_cover_jpg_path { let _ = tokio::fs::remove_file(p).await; }
                if let Some(ref p) = staged_cover_webp_path { let _ = tokio::fs::remove_file(p).await; }
                if let Some(ref p) = staged_booklet_path { let _ = tokio::fs::remove_file(p).await; }
                return Err(anyhow!("Failed to promote staging file to final path: rename err={}, copy err={}", e, ce));
            }

            let dest_meta = tokio::fs::metadata(&final_path).await
                .map_err(|me| anyhow!("Failed to read promoted file metadata {:?}: {}", final_path, me))?;
            let dest_bytes = tokio::fs::read(&final_path).await
                .map_err(|re| anyhow!("Failed to reread promoted file {:?}: {}", final_path, re))?;
            let dest_sha = crate::services::repair_guardrail::compute_bytes_sha256(&dest_bytes);

            if dest_meta.len() != staged_len || dest_sha != staged_sha {
                let _ = tokio::fs::remove_file(&final_path).await;
                return Err(anyhow!("Integrity mismatch on cross-volume promotion: size {} vs {}, hash {} vs {}", dest_meta.len(), staged_len, dest_sha, staged_sha));
            }

            let _ = tokio::fs::remove_file(&staging_path).await;
        }

        // 9. Promote sidecars (lyrics, cover artwork, booklets)
        if let Some(ref lrc_staged) = staged_lrc_path {
            let final_lrc = layout.lyrics_path_for_track(&final_path);
            if let Err(_) = tokio::fs::rename(lrc_staged, &final_lrc).await {
                let _ = tokio::fs::copy(lrc_staged, &final_lrc).await;
            }
            let _ = tokio::fs::remove_file(lrc_staged).await;
        }

        if let Some(ref cov_staged) = staged_cover_jpg_path {
            let final_cover = target_dir.join("cover.jpg");
            if !final_cover.exists() {
                let _ = tokio::fs::copy(cov_staged, &final_cover).await;
            }
            if let Some(parent) = target_dir.parent() {
                let dir_name = target_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if dir_name.starts_with("Disc") || dir_name.starts_with("CD") {
                    let parent_cover = parent.join("cover.jpg");
                    if !parent_cover.exists() {
                        let _ = tokio::fs::copy(cov_staged, &parent_cover).await;
                    }
                }
            }
            let _ = tokio::fs::remove_file(cov_staged).await;
        }

        if let Some(ref webp_staged) = staged_cover_webp_path {
            let final_webp = target_dir.join("cover.webp");
            let final_folder_webp = target_dir.join("folder.webp");
            let final_anim_webp = target_dir.join("animated.webp");
            if !final_webp.exists() {
                let _ = tokio::fs::copy(webp_staged, &final_webp).await;
            }
            if !final_folder_webp.exists() {
                let _ = tokio::fs::copy(webp_staged, &final_folder_webp).await;
            }
            if !final_anim_webp.exists() {
                let _ = tokio::fs::copy(webp_staged, &final_anim_webp).await;
            }
            if let Some(parent) = target_dir.parent() {
                let dir_name = target_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if dir_name.starts_with("Disc") || dir_name.starts_with("CD") {
                    for sidecar_name in &["cover.webp", "folder.webp", "animated.webp"] {
                        let p = parent.join(sidecar_name);
                        if !p.exists() {
                            let _ = tokio::fs::copy(webp_staged, &p).await;
                        }
                    }
                }
            }
            let _ = tokio::fs::remove_file(webp_staged).await;
            let _ = tokio::fs::remove_file(staging_dir.join("folder.webp")).await;
            let _ = tokio::fs::remove_file(staging_dir.join("animated.webp")).await;
            let _ = tokio::fs::remove_file(staging_dir.join("cover.animated.webp")).await;
        }

        if let Some(ref booklet_staged) = staged_booklet_path {
            let final_booklet = target_dir.join("booklet.pdf");
            if !final_booklet.exists() {
                let _ = tokio::fs::copy(booklet_staged, &final_booklet).await;
            }
            let _ = tokio::fs::remove_file(booklet_staged).await;
        }

        info!("[Qobuz] Successfully finalized track: {:?}", final_path);

        phase_tracker.set_cache_hits(has_lyrics_cached, has_cover_cached, has_mb_cached);
        let phase_timings = phase_tracker.finish_completed();

        Ok(DownloadResult {
            file_path: final_path.to_string_lossy().to_string(),
            bit_depth: track.max_bit_depth.unwrap_or(16),
            sample_rate: (track.max_sample_rate.unwrap_or(44.1) * 1000.0) as i32,
            title: track.title,
            artist: artist_name,
            album: album_title,
            release_date: rel_date,
            track_number: track_num as i32,
            disc_number: disc_num as i32,
            isrc: isrc_val,
            service: "qobuz".to_string(),
            phase_timings: Some(phase_timings),
            ..Default::default()
        })
    }
}

impl Default for QobuzDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if two titles match (fuzzy)
pub fn title_matches(expected: &str, found: &str) -> bool {
    let expected_clean = clean_title(expected);
    let found_clean = clean_title(found);

    if expected_clean == found_clean {
        return true;
    }

    if found_clean.contains(&expected_clean) || expected_clean.contains(&found_clean) {
        return true;
    }

    false
}

/// Check if two artist names match (fuzzy)
pub fn artist_matches(expected: &str, found: &str) -> bool {
    let expected_lower = expected.to_lowercase();
    let found_lower = found.to_lowercase();

    if expected_lower == found_lower {
        return true;
    }

    let expected_parts: Vec<&str> = expected_lower
        .split(&[',', ';', '&', '/', '|'][..])
        .collect();
    let found_parts: Vec<&str> = found_lower.split(&[',', ';', '&', '/', '|'][..]).collect();

    for ep in &expected_parts {
        for fp in &found_parts {
            if ep.trim() == fp.trim() {
                return true;
            }
        }
    }

    false
}

/// Clean title for comparison
pub fn clean_title(title: &str) -> String {
    let mut clean = title.to_lowercase();
    let suffixes = [
        "(remaster",
        "(remastered",
        "(deluxe",
        "(expanded",
        "(live",
        "(acoustic",
        "(remix",
        "(radio edit",
        "- remaster",
        "- remastered",
        "- deluxe",
    ];

    for suffix in suffixes {
        if let Some(pos) = clean.find(suffix) {
            clean = clean[..pos].to_string();
        }
    }

    clean.trim().to_string()
}

/// Sanitize filename for filesystem
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_computation() {
        let sig = build_request_signature("6", "123456", "1700000000", "testsecret");
        assert_eq!(sig.len(), 32);
    }

    #[test]
    fn test_quality_mapping() {
        assert_eq!(map_quality_to_format_id("16-44"), "6");
        assert_eq!(map_quality_to_format_id("LOSSLESS"), "6");
        assert_eq!(map_quality_to_format_id("24-96"), "7");
        assert_eq!(map_quality_to_format_id("HI_RES"), "7");
        assert_eq!(map_quality_to_format_id("24-192"), "27");
        assert_eq!(map_quality_to_format_id("HI_RES_LOSSLESS"), "27");
        assert_eq!(map_quality_to_format_id("320"), "5");
    }

    #[test]
    fn test_title_and_artist_matching() {
        assert!(title_matches("Heroes", "Heroes (2017 Remaster)"));
        assert!(artist_matches("David Bowie", "David Bowie"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("AC/DC: High Voltage*"), "AC_DC_ High Voltage_");
    }
}
