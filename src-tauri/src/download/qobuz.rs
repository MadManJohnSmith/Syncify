// Qobuz downloader - deterministic request signing, token resolution, and audio downloads

use crate::download::http_client::{create_http_client, get_user_agent, QOBUZ_LIMITER};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use crate::services::qobuz::{QOBUZ_API_BASE, QOBUZ_APP_ID, QOBUZ_APP_SECRET};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use syncify_core_domain::byte_validators::AudioByteValidator;
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
    pub performer: Option<QobuzPerformer>,
    pub album: Option<QobuzAlbum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzPerformer {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzAlbum {
    pub title: String,
    pub release_date_original: Option<String>,
    pub image: Option<QobuzImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzImage {
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
    info!("[Qobuz SIG DEBUG] raw='{}' -> sig='{}'", raw, sig);
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

            info!(
                "[Qobuz] Requesting stream URL: {} (token_len={}, app_id={})",
                get_url, user_auth_token.len(), self.app_id
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

    /// Download stream payload into staging file with byte progress
    pub async fn download_to_staging(
        &self,
        download_url: &str,
        staging_path: &Path,
        item_id: &str,
    ) -> Result<u64> {
        debug!("[Qobuz] Downloading to staging: {:?}", staging_path);

        if let Some(parent) = staging_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

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
            item_id, "qobuz", 0, total_size,
        ));

        let mut file = File::create(staging_path).await?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if downloaded % (64 * 1024) < chunk.len() as u64 {
                PROGRESS_TRACKER.update(DownloadProgress::downloading(
                    item_id, "qobuz", downloaded, total_size,
                ));
            }
        }

        file.flush().await?;
        info!("[Qobuz] Staging payload written: {} bytes", downloaded);
        Ok(downloaded)
    }

    /// Full download flow: search/entity resolution → get stream URL → staging download → validation → tagging → atomic promotion
    pub async fn download_track(
        &self,
        request: &DownloadRequest,
        db_opt: Option<&sqlx::SqlitePool>,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        let duration_sec = (request.duration_ms / 1000) as i32;

        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "qobuz"));

        // 1. Resolve token early so all resolution endpoints (track/get, search) have authenticating context
        let token_opt = self.resolve_token(db_opt).await.ok();
        let token_ref = token_opt.as_deref();

        // 2. Resolve track by exact service_track_id if available, otherwise by ISRC / metadata
        let track = if let Some(ref s_track_id) = request.service_track_id {
            if let Ok(tid) = s_track_id.parse::<i64>() {
                info!("[Qobuz] Resolving exact entity for service_track_id={}", tid);
                self.get_track_by_id(tid, token_ref).await?
            } else if let Some(isrc) = &request.isrc {
                match self.search_by_isrc(isrc, duration_sec, token_ref).await {
                    Ok(t) => t,
                    Err(_) => {
                        self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token_ref)
                            .await?
                    }
                }
            } else {
                self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token_ref).await?
            }
        } else if let Some(isrc) = &request.isrc {
            match self.search_by_isrc(isrc, duration_sec, token_ref).await {
                Ok(t) => t,
                Err(_) => {
                    self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token_ref)
                        .await?
                }
            }
        } else {
            self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token_ref)
                .await?
        };

        // 3. Resolve Stream URL (tries allowed formats in cascade order)
        let stream_res = self
            .get_download_url(track.id, &request.quality, token_ref, request.allow_fallback)
            .await?;

        // 4. Staging path setup
        let out_dir = PathBuf::from(&request.output_dir);
        let staging_dir = out_dir.join(".staging");
        tokio::fs::create_dir_all(&staging_dir).await?;
        let staging_path = staging_dir.join(format!("{}.part", sanitize_filename(&request.item_id)));

        // 4. Download audio payload to staging
        let downloaded_bytes = match self
            .download_to_staging(&stream_res.url, &staging_path, item_id)
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
            return Err(anyhow!("Qobuz downloaded audio payload is 0 bytes"));
        }

        // 5. Audio Byte Validation (magic header verification)
        let header_bytes = tokio::fs::read(&staging_path).await.unwrap_or_default();
        let is_flac = stream_res.format_id != "5";

        if is_flac && !AudioByteValidator::is_flac_magic(&header_bytes) {
            let _ = tokio::fs::remove_file(&staging_path).await;
            return Err(anyhow!("Downloaded audio failed bit-perfect FLAC magic verification"));
        }

        // 6. Tagging with metaflac (VORBIS_COMMENT)
        if is_flac {
            PROGRESS_TRACKER.update(DownloadProgress::finalizing(item_id));

            let flac_meta = FlacMetadata {
                title: track.title.clone(),
                artist: track.performer.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| request.artist_name.clone()),
                album: track.album.as_ref().map(|a| a.title.clone()).unwrap_or_else(|| request.album_name.clone()),
                album_artist: request.album_artist.clone(),
                track_number: track.track_number.unwrap_or(request.track_number as i32) as u32,
                track_total: request.total_tracks as u32,
                disc_number: request.disc_number as u32,
                isrc: track.isrc.clone().or_else(|| request.isrc.clone()),
                release_date: track.album.as_ref().and_then(|a| a.release_date_original.clone()).or_else(|| request.release_date.clone()),
                audio_source: Some("Qobuz".to_string()),
                bit_depth: Some(track.max_bit_depth.unwrap_or(16)),
                sample_rate: Some(track.max_sample_rate.unwrap_or(44.1) * 1000.0),
                ..Default::default()
            };

            let _ = apply_and_verify_flac_tags(&staging_path, &flac_meta);
        }

        // 7. Calculate final destination path
        let ext = if is_flac { "flac" } else { "mp3" };
        let filename = format!(
            "{} - {}.{}",
            sanitize_filename(&request.artist_name),
            sanitize_filename(&request.track_name),
            ext
        );
        tokio::fs::create_dir_all(&out_dir).await?;
        let final_path = out_dir.join(&filename);

        // 8. Atomic promotion from staging to final path
        if let Err(e) = tokio::fs::rename(&staging_path, &final_path).await {
            // If cross-device link fails, copy and remove
            tokio::fs::copy(&staging_path, &final_path).await.map_err(|ce| {
                anyhow!("Failed to promote staging file to final path: rename err={}, copy err={}", e, ce)
            })?;
            let _ = tokio::fs::remove_file(&staging_path).await;
        }

        info!("[Qobuz] Successfully finalized track: {:?}", final_path);

        Ok(DownloadResult {
            file_path: final_path.to_string_lossy().to_string(),
            bit_depth: track.max_bit_depth.unwrap_or(16),
            sample_rate: (track.max_sample_rate.unwrap_or(44.1) * 1000.0) as i32,
            title: track.title,
            artist: track.performer.map(|p| p.name).unwrap_or_else(|| request.artist_name.clone()),
            album: track
                .album
                .as_ref()
                .map(|a| a.title.clone())
                .unwrap_or_else(|| request.album_name.clone()),
            release_date: track.album.and_then(|a| a.release_date_original).or_else(|| request.release_date.clone()),
            track_number: track.track_number.unwrap_or(request.track_number),
            disc_number: request.disc_number,
            isrc: track.isrc.or_else(|| request.isrc.clone()),
            service: "qobuz".to_string(),
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
