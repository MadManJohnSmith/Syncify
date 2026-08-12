//! Qobuz Downloader and Download Request DTOs (CLI Standalone)
//!
//! Restored from master backup pipeline `Syncify_FULL_BACKUP_20260810/src-tauri/src/download/qobuz.rs`.
//!
//! Capabilities:
//! - Pure, deterministic request signing (`build_request_signature` & `sign_api_request`)
//! - Multi-tier token resolution hierarchy (`QOBUZ_USER_TOKEN` > SQLite local keychain > explicit RequiresAuth)
//! - Primary native official API integration (`track/getFileUrl`) with signed parameters
//! - Traceable fallback to proxy APIs without leaking credentials
//! - Exact quality mapping (27: 24/192 FLAC, 7: 24/96 FLAC, 6: 16/44.1 FLAC, 5: 320 MP3)
//! - Physical audio validation (size > 0, `fLaC` magic validation, rejection of HTML/error placeholders)
//! - Pure `metaflac` tagging and post-write re-read verification without `lofty`

use crate::download::http_client::{create_http_client, get_user_agent, QOBUZ_LIMITER};
use crate::metadata::tag_writer::{apply_flac_tags, FlacMetadata};
use crate::services::qobuz::{resolve_qobuz_app_id, resolve_qobuz_app_secret, QOBUZ_API_BASE};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use md5;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Windows reserved device names (case-insensitive)
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const MAX_COMPONENT_LEN: usize = 200;

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

/// Qobuz Authentication Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QobuzAuthStatus {
    Authenticated,
    RequiresAuth(String),
    SourceUnavailable(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub item_id: String,
    pub isrc: Option<String>,
    pub spotify_id: Option<String>,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub album_artist: Option<String>,
    pub duration_ms: i64,
    pub track_number: u32,
    pub disc_number: u32,
    pub total_tracks: u32,
    pub total_discs: u32,
    pub release_date: Option<String>,
    pub cover_url: Option<String>,
    pub output_dir: String,
    pub quality: String,
    pub qobuz_token: Option<String>,
    pub embed_lyrics: bool,
    pub embed_artwork: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub file_path: String,
    pub bit_depth: i32,
    pub sample_rate: i32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub release_date: Option<String>,
    pub track_number: u32,
    pub disc_number: u32,
    pub total_discs: i32,
    pub isrc: Option<String>,
    pub service: String,
    pub url_source: Option<StreamUrlSource>,
    pub format_id: String,
}

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
    #[serde(rename = "media_count")]
    pub total_discs: Option<i32>,
    pub artist: Option<QobuzAlbumArtist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzAlbumArtist {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzImage {
    pub small: Option<String>,
    pub large: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QobuzSearchResponse {
    pub tracks: Option<QobuzTracksContainer>,
}

#[derive(Debug, Deserialize)]
pub struct QobuzTracksContainer {
    pub items: Vec<QobuzTrack>,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    url: Option<String>,
    error: Option<String>,
}

// ═══════════════════════════════════════════════════════
// PURE HELPER & SIGNATURE FUNCTIONS
// ═══════════════════════════════════════════════════════

/// Build the Qobuz `track/getFileUrl` request signature.
///
/// Algorithm:
/// MD5("trackgetFileUrlformat_id{quality}intentstreamtrack_id{track_id}{timestamp}{app_secret}")
pub fn build_request_signature(
    quality: &str,
    track_id: &str,
    timestamp: &str,
    app_secret: &str,
) -> String {
    let r_sig = format!(
        "trackgetFileUrlformat_id{}intentstreamtrack_id{}{}{}",
        quality, track_id, timestamp, app_secret
    );
    format!("{:x}", md5::compute(r_sig.as_bytes()))
}

/// Sign a general Qobuz API request.
///
/// MD5(method_without_slashes + sorted(key + val) + app_secret)
pub fn sign_api_request(method: &str, params: &mut Vec<(&str, String)>, app_secret: &str) {
    params.sort_by(|a, b| a.0.cmp(b.0));

    let mut sig_base = method.replace('/', "").to_string();
    for (key, val) in params.iter() {
        sig_base.push_str(key);
        sig_base.push_str(val);
    }
    sig_base.push_str(app_secret);

    let digest = md5::compute(sig_base.as_bytes());
    let sig = format!("{:x}", digest);
    params.push(("request_sig", sig));
}

/// Map user quality request string to official Qobuz format_id
pub fn map_quality_to_format_id(quality: &str) -> &'static str {
    match quality.to_uppercase().trim() {
        "27" | "HI_RES_LOSSLESS" | "24-192" | "24/192" => "27",
        "7" | "HI_RES" | "24-96" | "24/96" => "7",
        "6" | "LOSSLESS" | "16-44" | "16/44" | "16-44.1" | "16/44.1" => "6",
        "5" | "MP3" | "320" | "320KBPS" => "5",
        _ => "27", // Default to master 24-bit 192kHz
    }
}

/// Map user quality request string to allowed Qobuz format_ids in cascade order.
/// By default, lossless quality tiers (16-44, 24-96, 24-192) NEVER downgrade to lossy MP3 (format_id 5)
/// unless allow_lossy_fallback is explicitly set to true.
pub fn map_quality_to_allowed_format_ids(quality: &str) -> &'static [&'static str] {
    map_quality_to_allowed_format_ids_with_lossy_fallback(quality, false)
}

/// Map user quality request string to allowed Qobuz format_ids with opt-in lossy fallback support
pub fn map_quality_to_allowed_format_ids_with_lossy_fallback(quality: &str, allow_lossy_fallback: bool) -> &'static [&'static str] {
    match (quality.to_uppercase().trim(), allow_lossy_fallback) {
        ("27" | "HI_RES_LOSSLESS" | "24-192" | "24/192", true) => &["27", "7", "6", "5"],
        ("27" | "HI_RES_LOSSLESS" | "24-192" | "24/192", false) => &["27", "7", "6"],

        ("7" | "HI_RES" | "24-96" | "24/96", true) => &["7", "6", "5"],
        ("7" | "HI_RES" | "24-96" | "24/96", false) => &["7", "6"],

        ("6" | "LOSSLESS" | "16-44" | "16/44" | "16-44.1" | "16/44.1", true) => &["6", "5"],
        ("6" | "LOSSLESS" | "16-44" | "16/44" | "16-44.1" | "16/44.1", false) => &["6"],

        ("5" | "MP3" | "320" | "320KBPS", _) => &["5"],

        (_, true) => &["27", "7", "6", "5"],
        (_, false) => &["27", "7", "6"],
    }
}

/// Sanitize a single path component for Windows filesystem safety.
pub fn sanitize_path_component(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect();

    let trimmed = sanitized.trim_end_matches(|c: char| c == '.' || c == ' ');
    let mut result = trimmed.trim().to_string();

    let upper = result.to_uppercase();
    if WINDOWS_RESERVED.iter().any(|r| *r == upper) {
        result = format!("_{}", result);
    }

    if result.len() > MAX_COMPONENT_LEN {
        result.truncate(MAX_COMPONENT_LEN);
        result = result.trim_end_matches(|c: char| c == '.' || c == ' ').to_string();
    }

    if result.is_empty() {
        result = "_".to_string();
    }

    result
}

/// Build the output path for a downloaded track using structured directory layout.
pub fn build_output_path(
    output_dir: &str,
    artist: &str,
    album: &str,
    disc_number: i32,
    track_number: i32,
    title: &str,
    total_discs: i32,
) -> PathBuf {
    let artist_safe = sanitize_path_component(artist);
    let album_safe = sanitize_path_component(album);
    let title_safe = sanitize_path_component(title);
    let filename = format!("{:02} - {}.flac", track_number, title_safe);

    let mut path = PathBuf::from(output_dir);
    path.push(&artist_safe);
    path.push(&album_safe);

    if total_discs > 1 {
        path.push(format!("CD {}", disc_number));
    }

    path.push(filename);
    path
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
        "(remaster", "(remastered", "(deluxe", "(expanded", "(live",
        "(acoustic", "(remix", "(radio edit", "- remaster", "- remastered", "- deluxe",
    ];

    for suffix in suffixes {
        if let Some(pos) = clean.find(suffix) {
            clean = clean[..pos].to_string();
        }
    }

    clean.trim().to_string()
}

/// Build FlacMetadata from download result and request
pub fn build_flac_metadata(result: &DownloadResult, request: &DownloadRequest) -> FlacMetadata {
    let release_year = result
        .release_date
        .as_ref()
        .and_then(|d| {
            let trimmed = d.trim();
            if trimmed.len() >= 4 {
                Some(trimmed[..4].to_string())
            } else {
                None
            }
        });

    FlacMetadata {
        title: result.title.clone(),
        artist: result.artist.clone(),
        album: result.album.clone(),
        album_artist: result.album_artist.clone().or_else(|| request.album_artist.clone()),
        composer: None,
        performers: None,
        work: None,
        genre: None,
        style: None,
        mood: None,
        release_type: None,
        release_status: None,
        release_country: None,
        language: None,
        copyright: None,
        label: None,
        barcode: None,
        catalog_number: None,
        original_date: None,
        track_number: result.track_number,
        track_total: request.total_tracks,
        disc_number: result.disc_number,
        disc_total: request.total_discs,
        disc_subtitle: None,
        isrc: result.isrc.clone(),
        release_year,
        release_date: result.release_date.clone(),
        explicit: None,
        bpm: None,
        initial_key: None,
        energy: None,
        danceability: None,
        loudness: None,
        replaygain_track_gain: None,
        replaygain_track_peak: None,
        r128_track_gain: None,
        comment: None,
        bit_depth: Some(result.bit_depth),
        sample_rate: Some(result.sample_rate as f64),
        musicbrainz_track_id: None,
        musicbrainz_artist_id: None,
        musicbrainz_album_id: None,
        musicbrainz_release_group_id: None,
        musicbrainz_work_id: None,
        lyrics_lrc: None,
        cover_data: None,
        lyrics_source: None,
        cover_source: None,
        audio_source: Some(result.service.clone()),
    }
}

// ═══════════════════════════════════════════════════════
// DOWNLOADER IMPLEMENTATION
// ═══════════════════════════════════════════════════════

pub struct QobuzDownloader {
    client: Client,
    app_id: String,
    app_secret: String,
}

impl QobuzDownloader {
    pub fn new() -> Self {
        Self {
            client: create_http_client(),
            app_id: resolve_qobuz_app_id(),
            app_secret: resolve_qobuz_app_secret(),
        }
    }

    /// Resolve Qobuz user auth token following precedence:
    /// 1. Environment variable `QOBUZ_USER_TOKEN`
    /// 2. SQLite local database `accounts` table decrypted via Keychain
    /// 3. Returns explicit `RequiresAuth`
    pub async fn resolve_token(&self) -> Result<String, QobuzAuthStatus> {
        if let Ok(tok) = std::env::var("QOBUZ_USER_TOKEN") {
            let trimmed = tok.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        let _ = crate::crypto::init_keychain_crypto();
        if let Ok(db_path) = crate::crypto::resolve_syncify_db_path() {
            if let Ok(db) = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await {
                let row_opt: Result<(String,), _> = sqlx::query_as(
                    "SELECT credentials_json FROM accounts WHERE service_id = (SELECT id FROM services WHERE name = 'qobuz' LIMIT 1) AND is_active = 1"
                )
                .fetch_one(&db)
                .await;

                if let Ok((encrypted_json,)) = row_opt {
                    if let Ok(decrypted) = crate::crypto::decrypt(&encrypted_json) {
                        if let Ok(creds) = serde_json::from_str::<crate::services::qobuz::QobuzCredentials>(&decrypted) {
                            if !creds.user_auth_token.trim().is_empty() {
                                return Ok(creds.user_auth_token.trim().to_string());
                            }
                        }
                    }
                }
            }
        }

        Err(QobuzAuthStatus::RequiresAuth(
            "No active Qobuz user auth token found. Set QOBUZ_USER_TOKEN environment variable or log in via Syncify.".to_string(),
        ))
    }

    fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "aHR0cHM6Ly9xb2J1ei5raW5vcGx1cy5vbmxpbmUvdHJhY2svZ2V0P2lkPQ==",
            "aHR0cHM6Ly9xb2J1ei1hcGkuYmluaW11bS5vcmcvdHJhY2svZ2V0P2lkPQ==",
        ];
        encoded_apis
            .iter()
            .filter_map(|encoded| {
                BASE64.decode(encoded).ok().and_then(|bytes| String::from_utf8(bytes).ok())
            })
            .collect()
    }

    /// Primary official Qobuz `track/getFileUrl` endpoint (requires valid user_auth_token)
    pub async fn get_official_download_url(
        &self,
        track_id: i64,
        quality: &str,
        user_auth_token: &str,
    ) -> Result<StreamResolution> {
        if user_auth_token.trim().is_empty() {
            return Err(anyhow!("Cannot query official Qobuz stream URL: user_auth_token is empty"));
        }

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string();

        let track_id_str = track_id.to_string();
        let format_id = map_quality_to_format_id(quality);
        let sig = build_request_signature(format_id, &track_id_str, &ts, &self.app_secret);

        debug!(
            "[Qobuz] Requesting official stream URL for track {} (format_id: {})",
            track_id, format_id
        );

        let url = format!("{}/track/getFileUrl", QOBUZ_API_BASE);
        let response = self
            .client
            .get(&url)
            .header("X-App-Id", &self.app_id)
            .header("X-User-Auth-Token", user_auth_token)
            .query(&[
                ("format_id", format_id),
                ("intent", "stream"),
                ("track_id", &track_id_str),
                ("request_ts", &ts),
                ("request_sig", &sig),
            ])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Qobuz official getFileUrl failed: HTTP {} - {}", status, error_body));
        }

        let resp_json: serde_json::Value = response.json().await?;
        if let Some(stream_url) = resp_json["url"].as_str() {
            if stream_url.trim().is_empty() {
                return Err(anyhow!("Qobuz official API returned empty stream URL"));
            }
            info!("[Qobuz] ✓ Acquired official Qobuz stream URL");
            Ok(StreamResolution {
                url: stream_url.to_string(),
                source: StreamUrlSource::QobuzOfficial,
                format_id: format_id.to_string(),
            })
        } else {
            let error_msg = resp_json["message"]
                .as_str()
                .unwrap_or("Unknown official Qobuz API error (no URL returned)");
            Err(anyhow!("Qobuz official API error: {}", error_msg))
        }
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
            let url = format!("{}{}&quality={}", api, track_id, format_id);
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
    ) -> Result<StreamResolution> {
        if let Some(token) = user_auth_token {
            if !token.trim().is_empty() {
                match self.get_official_download_url(track_id, quality, token).await {
                    Ok(res) => return Ok(res),
                    Err(e) => {
                        warn!("[Qobuz] Official stream resolution failed: {}. Falling back to proxy...", e);
                    }
                }
            }
        }

        // Fallback to proxy
        self.get_proxy_download_url(track_id, quality).await
    }

    pub async fn execute_search_request(
        &self,
        query_str: &str,
        token: Option<&str>,
    ) -> Result<QobuzSearchResponse> {
        let url = format!("{}/track/search", QOBUZ_API_BASE);

        // 1. Try authenticated search with signature
        if let Some(t) = token {
            if !t.is_empty() {
                let mut params = vec![
                    ("app_id", self.app_id.clone()),
                    ("limit", "50".to_string()),
                    ("query", query_str.to_string()),
                    ("user_auth_token", t.to_string()),
                ];
                sign_api_request("track/search", &mut params, &self.app_secret);

                let request = self
                    .client
                    .get(&url)
                    .header("User-Agent", get_user_agent())
                    .header("X-App-Id", &self.app_id)
                    .header("X-User-Auth-Token", t)
                    .query(&params);

                if let Ok(response) = request.send().await {
                    if response.status().is_success() {
                        if let Ok(result) = response.json::<QobuzSearchResponse>().await {
                            return Ok(result);
                        }
                    }
                }
            }
        }

        // 2. Public catalog search fallback
        let mut params = vec![
            ("app_id", self.app_id.clone()),
            ("limit", "50".to_string()),
            ("query", query_str.to_string()),
        ];
        sign_api_request("track/search", &mut params, &self.app_secret);

        let request = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .header("X-App-Id", &self.app_id)
            .query(&params);

        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Qobuz search failed: HTTP {} - {}", status, body));
        }

        let result: QobuzSearchResponse = response.json().await?;
        Ok(result)
    }

    pub async fn search_by_isrc(
        &self,
        isrc: &str,
        expected_duration_sec: i32,
        token: Option<&str>,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;
        debug!("[Qobuz] Searching by ISRC: {}", isrc);

        let result = self.execute_search_request(isrc, token).await?;
        let tracks = result.tracks.ok_or_else(|| anyhow!("No tracks in response"))?;

        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        info!("[Qobuz] Found ISRC match: '{}' (duration verified)", track.title);
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
        token: Option<&str>,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;
        let query = format!("{} {}", artist_name, track_name);
        debug!("[Qobuz] Searching by metadata: {} - {}", artist_name, track_name);

        let result = self.execute_search_request(&query, token).await?;
        let tracks = result.tracks.ok_or_else(|| anyhow!("No tracks in response"))?;

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

            info!("[Qobuz] Found metadata match: '{}' by '{}'", track.title, track.performer.as_ref().map(|p| p.name.as_str()).unwrap_or("Unknown"));
            return Ok(track.clone());
        }

        Err(anyhow!("No matching track found for: {} - {}", artist_name, track_name))
    }

    /// Download audio payload to disk and validate physical integrity
    pub async fn download_file(
        &self,
        download_url: &str,
        output_path: &Path,
        expected_format_id: &str,
    ) -> Result<usize> {
        debug!("[Qobuz] Downloading stream to {:?}", output_path);

        let response = self
            .client
            .get(download_url)
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed: HTTP {}", response.status()));
        }

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let bytes = response.bytes().await?;
        if bytes.is_empty() {
            return Err(anyhow!("Downloaded stream payload is 0 bytes"));
        }

        // Validate expected audio format
        if expected_format_id == "27" || expected_format_id == "7" || expected_format_id == "6" {
            if bytes.len() < 4 || &bytes[0..4] != b"fLaC" {
                // Check if it was an HTML error page or JSON error returned with HTTP 200
                let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(128)]);
                return Err(anyhow!("Downloaded payload is not a valid FLAC file (header mismatch: '{}')", preview.trim()));
            }
        }

        let mut file = File::create(output_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        info!("[Qobuz] Downloaded {} bytes to {:?}", bytes.len(), output_path);
        Ok(bytes.len())
    }

    /// Full download flow for a single track: search → get URL → download → tag
    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        let duration_sec = (request.duration_ms / 1000) as i32;
        let token = request.qobuz_token.as_deref();

        let track = if let Some(isrc) = &request.isrc {
            match self.search_by_isrc(isrc, duration_sec, token).await {
                Ok(t) => t,
                Err(_) => self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token).await?,
            }
        } else {
            self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec, token).await?
        };

        let format_id = map_quality_to_format_id(&request.quality);
        let stream_res = self.get_download_url(track.id, format_id, token).await?;

        let album_artist = track.album.as_ref().and_then(|a| a.artist.as_ref()).and_then(|ar| ar.name.clone());
        let total_discs = track.album.as_ref().and_then(|a| a.total_discs).unwrap_or(1);

        let output_path = build_output_path(
            &request.output_dir,
            &request.artist_name,
            &request.album_name,
            track.disc_number.unwrap_or(request.disc_number as i32),
            track.track_number.unwrap_or(request.track_number as i32),
            &track.title,
            total_discs,
        );

        self.download_file(&stream_res.url, &output_path, format_id).await?;

        let download_result = DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: track.max_bit_depth.unwrap_or(16),
            sample_rate: (track.max_sample_rate.unwrap_or(44.1) * 1000.0) as i32,
            title: track.title,
            artist: track.performer.map(|p| p.name).unwrap_or_default(),
            album: track.album.as_ref().map(|a| a.title.clone()).unwrap_or_default(),
            album_artist,
            release_date: track.album.and_then(|a| a.release_date_original),
            track_number: track.track_number.unwrap_or(request.track_number as i32) as u32,
            disc_number: track.disc_number.unwrap_or(request.disc_number as i32) as u32,
            total_discs,
            isrc: track.isrc,
            service: "qobuz".to_string(),
            url_source: Some(stream_res.source),
            format_id: stream_res.format_id,
        };

        // Tagging with metaflac (non-fatal)
        let flac_meta = build_flac_metadata(&download_result, request);
        if let Err(e) = apply_flac_tags(&output_path, &flac_meta) {
            warn!("[Qobuz] Tagging failed: {}", e);
        }

        Ok(download_result)
    }
}

impl Default for QobuzDownloader {
    fn default() -> Self {
        Self::new()
    }
}
