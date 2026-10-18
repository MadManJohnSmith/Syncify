// Qobuz downloader - deterministic request signing, token resolution, and audio downloads

use crate::download::http_client::{
    calculate_backoff_with_jitter, create_http_client, get_user_agent, parse_retry_after,
    QOBUZ_LIMITER,
};
use crate::download::lyrics::{
    validate_and_embed_flac_lyrics, LyricsPipelineService, LyricsResolution, LyricsSyncType,
    ResolutionStatus,
};
use crate::download::progress::{
    ByteStreamTracker, DownloadPhase, DownloadPhaseTracker, DownloadProgress, DownloadRequest,
    DownloadResult, PROGRESS_TRACKER,
};
use crate::services::animated_cover::{resolve_and_download_animated_cover, AnimatedCoverStatus};
use crate::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use crate::services::qobuz::{QOBUZ_API_BASE, QOBUZ_APP_ID, QOBUZ_APP_SECRET};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_core_domain::quality::FormatId;
use syncify_core_domain::{FolderFileTemplateConfig, LibraryLayout, TrackLayoutContext};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};

/// Qobuz Authentication Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QobuzAuthStatus {
    RequiresAuth(String),
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
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    error: Option<String>,
}

/// Map quality string to Qobuz format_id (deterministic cascade).
///
/// The tier table lives in `syncify_core_domain::quality::FormatId`, so this
/// wrapper only supplies the default: an unrecognized token asks for 16-bit /
/// 44.1 kHz FLAC.
pub fn map_quality_to_format_id(quality: &str) -> &'static str {
    FormatId::from_qobuz_quality_token(quality)
        .unwrap_or(FormatId::LosslessCd)
        .qobuz_format_id()
}

/// Map quality string to allowed Qobuz format_ids in cascade order (identical to CLI).
///
/// Unrecognized tokens start the cascade at the best tier the account may have,
/// so the official endpoint can still answer with whatever it holds.
pub fn map_quality_to_allowed_format_ids_with_lossy_fallback(
    quality: &str,
    allow_lossy_fallback: bool,
) -> &'static [&'static str] {
    FormatId::from_qobuz_quality_token(quality)
        .unwrap_or(FormatId::HiResLossless)
        .qobuz_cascade(allow_lossy_fallback)
}

/// Classifies a failed Qobuz HTTP response onto the shared error taxonomy, so the
/// retry decision (`is_retryable`), the retry budget (`max_attempts`), the wait
/// (`retry_delay_sec`) and the user-facing text (`ui_message`) all come from the
/// domain instead of being decided by hand here.
///
/// The HTTP status is kept verbatim in every message: the download worker and the
/// queue retry commands classify failures by scanning those messages.
pub fn classify_qobuz_http_failure(status: reqwest::StatusCode, item_id: &str) -> ErrorTaxonomy {
    let provider = "qobuz".to_string();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return ErrorTaxonomy::AuthInvalid {
            message: format!("Qobuz rejected the credentials (HTTP {})", status),
        };
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ErrorTaxonomy::RateLimited {
            provider,
            retry_after_sec: None,
        };
    }
    if status.is_server_error() || status == reqwest::StatusCode::REQUEST_TIMEOUT {
        return ErrorTaxonomy::TemporaryNetworkFailure {
            endpoint: format!("qobuz stream (HTTP {})", status),
            message: format!("Qobuz returned HTTP {}", status),
        };
    }
    ErrorTaxonomy::UnavailableFromProvider {
        provider,
        item_id: item_id.to_string(),
        reason: format!("Qobuz returned HTTP {}", status),
    }
}

/// Read the first MPEG-1 Layer III frame bitrate, skipping a bounded ID3v2 tag.
/// An MP3 header alone does not establish the 320 kbps minimum quality floor.
fn mp3_first_frame_bitrate_kbps(header: &[u8]) -> Option<u32> {
    let start = if header.starts_with(b"ID3") && header.len() >= 10 {
        if header[6..10].iter().any(|byte| byte & 0x80 != 0) {
            return None;
        }
        let tag_len = ((header[6] as usize) << 21)
            | ((header[7] as usize) << 14)
            | ((header[8] as usize) << 7)
            | header[9] as usize;
        10 + tag_len + if header[5] & 0x10 != 0 { 10 } else { 0 }
    } else {
        0
    };
    const MPEG1_LAYER3_KBPS: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    if start + 4 > header.len() {
        return None;
    }
    header[start..].windows(4).take(4096).find_map(|frame| {
        if frame[0] == 0xff && frame[1] & 0xfe == 0xfa {
            let bitrate = MPEG1_LAYER3_KBPS[(frame[2] >> 4) as usize];
            let sample_rate = (frame[2] >> 2) & 0x03;
            (bitrate > 0 && sample_rate != 0x03).then_some(bitrate)
        } else {
            None
        }
    })
}

/// Accept only bytes matching the resolved format. A successful getFileUrl response
/// does not prove that the URL carries the full track: Qobuz may serve an MP3
/// sample for a requested FLAC. Never promote that sample as a complete track.
fn validate_qobuz_audio_payload(
    format_id: &str,
    header: &[u8],
    bytes: u64,
    duration_ms: i64,
) -> Result<()> {
    if format_id != "5" {
        if AudioByteValidator::is_flac_magic(header) {
            // A real FLAC is not a preview just because its byte size is small:
            // compression varies widely. Only reject a demonstrably short stream
            // when STREAMINFO reports both sample count and sample rate.
            if let Some(info) = AudioByteValidator::parse_flac_streaminfo(header) {
                if info.sample_rate > 0
                    && info.total_samples > 0
                    && duration_ms >= 60_000
                    && (info.total_samples as u128) * 1_000
                        < (duration_ms as u128 / 2) * info.sample_rate as u128
                    && (info.total_samples as u128) * 1_000 <= 45_000 * info.sample_rate as u128
                {
                    return Err(anyhow!(ErrorTaxonomy::QobuzPreviewPayload {
                        requested_format: format!("FLAC format_id={}", format_id),
                        obtained_format: "short FLAC".to_string(),
                        reason: "FLAC STREAMINFO is shorter than the catalogued full track"
                            .to_string(),
                    }
                    .ui_message()));
                }
            }
            return Ok(());
        }
        if AudioByteValidator::is_mp3_magic(header) {
            return Err(anyhow!(ErrorTaxonomy::QobuzPreviewPayload {
                requested_format: format!("FLAC format_id={}", format_id),
                obtained_format: "MP3".to_string(),
                reason: "stream URL returned lossy audio, not a full lossless track".to_string(),
            }
            .ui_message()));
        }
        return Err(anyhow!(
            "InvalidAudioPayload: Qobuz FLAC format_id={} stream has no FLAC magic",
            format_id
        ));
    }

    if !AudioByteValidator::is_mp3_magic(header) {
        return Err(anyhow!(
            "InvalidAudioPayload: Qobuz format_id=5 stream has no MP3 magic"
        ));
    }
    // A short sample (including a 1.2 MB / ~30s preview) cannot be a full
    // 320 kbps track. Also require an actual 320 kbps frame: size alone could
    // otherwise let a lower-quality MP3 or large ID3 artwork through.
    // Leave a small allowance for Xing/LAME padding and metadata; real 320
    // kbps payloads need not have exactly 40 bytes per catalogued millisecond.
    if duration_ms <= 0
        || (bytes as u128) * 8 < (duration_ms as u128) * 300
        || mp3_first_frame_bitrate_kbps(header) != Some(320)
    {
        return Err(anyhow!(ErrorTaxonomy::QobuzPreviewPayload {
            requested_format: "MP3 320 format_id=5".to_string(),
            obtained_format: "MP3 short/under 320 kbps".to_string(),
            reason: "payload cannot cover the full track duration at the 320 kbps floor"
                .to_string(),
        }
        .ui_message()));
    }
    Ok(())
}

/// Read only a bounded prefix. The full staging file may be hundreds of MB.
async fn validate_staged_qobuz_payload(
    staging_path: &Path,
    format_id: &str,
    duration_ms: i64,
) -> Result<Vec<u8>> {
    let mut file = File::open(staging_path).await?;
    let bytes = file.metadata().await?.len();
    let mut header = vec![0u8; bytes.min(64 * 1024) as usize];
    file.read_exact(&mut header).await?;
    validate_qobuz_audio_payload(format_id, &header, bytes, duration_ms)?;
    Ok(header)
}

/// Build pure deterministic MD5 signature for `track/getFileUrl`
pub fn build_request_signature(
    format_id: &str,
    track_id: &str,
    ts: &str,
    app_secret: &str,
) -> String {
    let raw = format!(
        "trackgetFileUrlformat_id{}intentstreamtrack_id{}{}{}",
        format_id, track_id, ts, app_secret
    );
    let digest = md5::compute(raw.as_bytes());
    let sig = format!("{:x}", digest);
    debug!(
        "[Qobuz] Generated signature for format={} track_id={}",
        format_id, track_id
    );
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
    pub async fn resolve_token(
        &self,
        db_opt: Option<&sqlx::SqlitePool>,
    ) -> Result<String, QobuzAuthStatus> {
        let pool = match db_opt {
            Some(p) => p,
            None => {
                return Err(QobuzAuthStatus::RequiresAuth(
                    "No database pool provided to resolve Qobuz account. Please log in via Syncify.".to_string(),
                ));
            }
        };

        self.resolve_token_for_account(pool, None).await
    }

    /// Resolve Qobuz user auth token for a specific account ID or the active account if None.
    pub async fn resolve_token_for_account(
        &self,
        pool: &sqlx::SqlitePool,
        account_id: Option<i64>,
    ) -> Result<String, QobuzAuthStatus> {
        // Query Qobuz account from SQLite
        let account: Result<(i64, String), _> = if let Some(aid) = account_id {
            sqlx::query_as(
                "SELECT a.id, a.credentials_json FROM accounts a
                 JOIN services s ON s.id = a.service_id
                 WHERE LOWER(s.name) = 'qobuz' AND a.id = ?
                 LIMIT 1",
            )
            .bind(aid)
            .fetch_one(pool)
            .await
        } else {
            sqlx::query_as(
                "SELECT a.id, a.credentials_json FROM accounts a
                 JOIN services s ON s.id = a.service_id
                 WHERE LOWER(s.name) = 'qobuz' AND a.is_active = 1
                 ORDER BY a.id DESC LIMIT 1",
            )
            .fetch_one(pool)
            .await
        };

        let (_account_id, encrypted_json) = match account {
            Ok(acc) => acc,
            Err(_) => {
                return Err(QobuzAuthStatus::RequiresAuth(
                    "Tu cuenta de Qobuz está desconectada. Inicia sesión en Syncify (Ajustes > Cuentas).".to_string(),
                ));
            }
        };

        // Decrypt using application Keychain crypto (same as service.rs / auth.rs)
        let decrypted = match crate::crypto::decrypt(&encrypted_json) {
            Ok(d) => d,
            Err(e) => {
                return Err(QobuzAuthStatus::RequiresAuth(format!(
                    "Failed to decrypt Qobuz credentials: {}. Please re-authenticate in Syncify.",
                    e
                )));
            }
        };

        let creds: serde_json::Value = match serde_json::from_str(&decrypted) {
            Ok(c) => c,
            Err(e) => {
                return Err(QobuzAuthStatus::RequiresAuth(format!(
                    "Invalid Qobuz credentials JSON: {}. Please re-authenticate in Syncify.",
                    e
                )));
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
                let client =
                    crate::services::QobuzClient::new(self.app_id.clone(), self.app_secret.clone());
                match client.login_with_streaming_entitlement(user, pass).await {
                    Ok((fresh_token, entitlement)) => {
                        if let Some(can_stream) = entitlement {
                            let value: i64 = if can_stream { 1 } else { 0 };
                            // Login already happened for a missing token. Record only
                            // evidence from that response; never demote on failure.
                            if let Err(e) = sqlx::query(
                                "UPDATE accounts SET streaming_entitled = ? WHERE id = ?",
                            )
                            .bind(value)
                            .bind(_account_id)
                            .execute(pool)
                            .await
                            {
                                warn!(account_id = _account_id, error = %e, "Could not persist Qobuz streaming entitlement");
                            }
                        }
                        return Ok(fresh_token);
                    }
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
            return Err(anyhow!(
                "Cannot query official Qobuz stream URL: user_auth_token is empty"
            ));
        }

        let allowed_formats =
            map_quality_to_allowed_format_ids_with_lossy_fallback(quality, allow_fallback);
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
                    if status == reqwest::StatusCode::UNAUTHORIZED
                        || status == reqwest::StatusCode::FORBIDDEN
                    {
                        let error_body = resp.text().await.unwrap_or_default();
                        warn!(
                            "[Qobuz] Token expired or unauthorized (HTTP {}): {}",
                            status, error_body
                        );
                        return Err(anyhow!("RequiresAuth: Qobuz token expired (HTTP {}). Please re-authenticate via Settings > Accounts.", status));
                    }

                    if status.is_success() {
                        if let Ok(resp_json) = resp.json::<serde_json::Value>().await {
                            if let Some(stream_url) = resp_json["url"].as_str() {
                                if !stream_url.trim().is_empty() {
                                    // A 200 with a URL is not proof of the requested
                                    // quality: an account without streaming entitlement
                                    // receives fmt=5 (MP3) URLs even for FLAC format
                                    // requests. Detect the mismatch HERE so the caller
                                    // can move to a sibling account instead of failing
                                    // only after the payload is already streaming.
                                    let delivered_mime = resp_json["mime_type"]
                                        .as_str()
                                        .unwrap_or("");
                                    let delivered_lossy = delivered_mime
                                        .to_ascii_lowercase()
                                        .contains("mpeg")
                                        || stream_url.contains("fmt=5");
                                    if *format_id != "5" && delivered_lossy {
                                        warn!(
                                            "[Qobuz] Account delivered a lossy/preview stream for format_id={} (mime='{}'): per-account entitlement failure",
                                            format_id, delivered_mime
                                        );
                                        last_error = format!(
                                            "QobuzPreviewPayload: account delivered MP3 instead of the requested lossless stream (format_id={})",
                                            format_id
                                        );
                                        continue;
                                    }
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

        // Keep the taxonomy prefix when every allowed format failed for the
        // same classified reason (e.g. an account-limited preview), so callers
        // can tell a per-account entitlement failure from a generic one.
        if last_error.starts_with("QobuzPreviewPayload:") {
            return Err(anyhow!(last_error));
        }

        Err(anyhow!(
            "Qobuz official getFileUrl failed for all allowed formats: {}",
            last_error
        ))
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

            let result = self
                .client
                .get(&url)
                .timeout(Duration::from_secs(15))
                .send()
                .await;
            if let Ok(resp) = result {
                if resp.status().is_success() {
                    if let Ok(stream_resp) = resp.json::<StreamResponse>().await {
                        if let Some(download_url) = stream_resp.url {
                            if !download_url.trim().is_empty()
                                && !download_url.to_lowercase().contains("error")
                            {
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
                match self
                    .get_official_download_url(track_id, quality, token, allow_fallback)
                    .await
                {
                    Ok(res) => return Ok(res),
                    Err(e) => {
                        let err_str = e.to_string();
                        // If token is expired (RequiresAuth / 401 / 403), fail fast so user can re-authenticate
                        if err_str.contains("RequiresAuth")
                            || err_str.contains("401")
                            || err_str.contains("403")
                        {
                            return Err(e);
                        }
                        // A per-account preview is also a verdict about THIS
                        // account, not about the track: skip the third-party
                        // proxy (which can serve wrong payloads and mask the
                        // problem) so the caller moves to a sibling account
                        // whose official stream really is lossless.
                        if err_str.starts_with("QobuzPreviewPayload:") {
                            return Err(e);
                        }
                        warn!("[Qobuz] Official stream resolution failed: {}. Falling back to proxy...", e);
                    }
                }
            }
        } else if !allow_fallback {
            return Err(anyhow!(
                "RequiresAuth: Tu cuenta de Qobuz está desconectada. Inicia sesión en Syncify (Ajustes > Cuentas)."
            ));
        }

        // Fallback to proxy
        match self.get_proxy_download_url(track_id, quality).await {
            Ok(res) => Ok(res),
            Err(e) => {
                if user_auth_token.is_none()
                    || user_auth_token.map(|t| t.trim().is_empty()).unwrap_or(true)
                {
                    Err(anyhow!(
                        "RequiresAuth: Tu cuenta de Qobuz está desconectada. Falló el proxy de respaldo: {}",
                        e
                    ))
                } else {
                    Err(e)
                }
            }
        }
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

        // Find exact ISRC match with duration verification. When a user token
        // is available, also verify `track/get` so dead index ghosts (404 on
        // retrieval, observed live on retired Trojan/Qobuz catalogue rows) are
        // skipped in favour of a live sibling or a clean miss.
        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff > 10 {
                        warn!(
                            "[Qobuz] ISRC match but duration mismatch: expected {}s, got {}s",
                            expected_duration_sec, track.duration
                        );
                        continue;
                    }
                }
                if let Some(tok) = user_auth_token.filter(|t| !t.trim().is_empty()) {
                    if self.get_track_by_id(track.id, Some(tok)).await.is_err() {
                        debug!(
                            "[Qobuz] Skipping dead ISRC search candidate {} (404 on track/get)",
                            track.id
                        );
                        continue;
                    }
                }
                info!(
                    "[Qobuz] Found ISRC match: '{}' (ID: {}, duration verified)",
                    track.title, track.id
                );
                return Ok(track.clone());
            }
        }

        Err(anyhow!("No exact ISRC match found for: {}", isrc))
    }

    /// Exact-ISRC lookup with a broader query fallback.
    ///
    /// Qobuz's search index does not answer a bare-ISRC query for every ISRC:
    /// a streamable edition can return zero results while the very same ISRC
    /// resolves through an artist+title query (observed live on GBPVV1501060).
    /// An exact `isrc` field match is still exact identity, so when the direct
    /// query misses, retry with the standard artist+title query and accept
    /// only items whose ISRC equals the requested one, with the same duration
    /// verification the direct search applies.
    pub async fn search_by_isrc_broad(
        &self,
        isrc: &str,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzTrack> {
        let tracks = self
            .fetch_metadata_candidates(track_name, artist_name, user_auth_token)
            .await?;

        for track in &tracks {
            if track.isrc.as_deref() != Some(isrc) {
                continue;
            }
            if expected_duration_sec > 0
                && (track.duration - expected_duration_sec).abs() > 10
            {
                warn!(
                    "[Qobuz] Broad-query ISRC match but duration mismatch: expected {}s, got {}s",
                    expected_duration_sec, track.duration
                );
                continue;
            }
            if let Some(tok) = user_auth_token.filter(|t| !t.trim().is_empty()) {
                if self.get_track_by_id(track.id, Some(tok)).await.is_err() {
                    debug!(
                        "[Qobuz] Skipping dead broad-ISRC candidate {} (404 on track/get)",
                        track.id
                    );
                    continue;
                }
            }
            info!(
                "[Qobuz] Found ISRC match via broad metadata query: '{}' (ID: {})",
                track.title, track.id
            );
            return Ok(track.clone());
        }

        Err(anyhow!(
            "No exact ISRC match found for: {} (broad query)",
            isrc
        ))
    }

    /// Search for a track by metadata (artist + track name)
    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzTrack> {
        let tracks = self
            .fetch_metadata_candidates(track_name, artist_name, user_auth_token)
            .await?;

        // Find best match by title and duration
        for track in &tracks {
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

    /// Strict title + artist + duration (±3s) match that must be UNIQUE (or
    /// resolve same-recording re-releases deterministically by studio-origin
    /// score after filtering dead 404 index ghosts).
    ///
    /// Authorized only after every identity-based rule (ISRC/MBID/AcoustID)
    /// failed. Never falls back to an arbitrary first result: a text-only match
    /// is only trusted when live catalog entries agree on the normalized title,
    /// artist and real catalog duration (±3s, so a genuine 30s track like
    /// "(Archive 76)" stays eligible).
    pub async fn search_by_metadata_strict(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
        user_auth_token: Option<&str>,
    ) -> Result<QobuzTrack> {
        let mut tracks = self
            .fetch_metadata_candidates(track_name, artist_name, user_auth_token)
            .await?;

        let filter_strict = |items: Vec<QobuzTrack>| -> Vec<QobuzTrack> {
            items
                .into_iter()
                .filter(|track| {
                    if !title_matches(track_name, &track.title) {
                        return false;
                    }
                    if let Some(performer) = &track.performer {
                        if !artist_matches(artist_name, &performer.name) {
                            return false;
                        }
                    }
                    expected_duration_sec <= 0
                        || (track.duration - expected_duration_sec).abs() <= 3
                })
                .collect()
        };

        let mut matches = filter_strict(std::mem::take(&mut tracks));

        // Filter out dead search-index ghosts (404 on `track/get`) when a token
        // is present.
        let mut live_matches = Vec::new();
        if let Some(tok) = user_auth_token.filter(|t| !t.trim().is_empty()) {
            for cand in matches {
                if self.get_track_by_id(cand.id, Some(tok)).await.is_ok() {
                    live_matches.push(cand);
                } else {
                    debug!(
                        "[Qobuz] Skipping dead strict-metadata candidate {} (404 on track/get)",
                        cand.id
                    );
                }
            }
        } else {
            live_matches = matches;
        }

        // If the literal query only hit dead index ghosts (e.g. a retired
        // parenthetical variant like "(Poor Mi) Israelites" with a duplicated
        // artist credit), retry once with the primary artist and the title
        // stripped of a leading parenthetical alias.
        if live_matches.is_empty() {
            let primary_artist = artist_name
                .split(&[',', ';'][..])
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(artist_name);
            let stripped_title = if track_name.trim_start().starts_with('(') {
                track_name
                    .find(')')
                    .map(|idx| track_name[idx + 1..].trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(track_name)
            } else {
                track_name
            };
            if primary_artist != artist_name || stripped_title != track_name {
                if let Ok(extra) = self
                    .fetch_metadata_candidates(stripped_title, primary_artist, user_auth_token)
                    .await
                {
                    let extra_matches = filter_strict(extra);
                    if let Some(tok) = user_auth_token.filter(|t| !t.trim().is_empty()) {
                        for cand in extra_matches {
                            if self.get_track_by_id(cand.id, Some(tok)).await.is_ok() {
                                live_matches.push(cand);
                            }
                        }
                    } else {
                        live_matches = extra_matches;
                    }
                }
            }
        }

        if live_matches.len() == 1 {
            return Ok(live_matches.remove(0));
        }
        if live_matches.len() > 1 {
            let norm = |s: &str| {
                clean_title(s)
                    .replace(" & ", " and ")
                    .replace(['(', ')'], "")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let first_norm = norm(&live_matches[0].title);
            let same_title = live_matches.iter().all(|m| norm(&m.title) == first_norm);
            if same_title {
                let rank = |t: &QobuzTrack| -> (i32, i32, i32) {
                    let own_album = t
                        .album
                        .as_ref()
                        .and_then(|a| a.artist.as_ref())
                        .map(|art| {
                            !art.name.to_lowercase().contains("various")
                                && artist_matches(artist_name, &art.name)
                        })
                        .unwrap_or(false) as i32;
                    let bd = t.max_bit_depth.unwrap_or(16);
                    let dur_diff = -(t.duration - expected_duration_sec).abs();
                    (own_album, bd, dur_diff)
                };
                live_matches.sort_by(|a, b| rank(b).cmp(&rank(a)));
                return Ok(live_matches.remove(0));
            }
            return Err(anyhow!(
                "AmbiguousSource: {} competing strict metadata matches on Qobuz for '{}' by '{}'",
                live_matches.len(),
                track_name,
                artist_name
            ));
        }

        Err(anyhow!(
            "No matching track found on Qobuz for strict metadata: {} - {}",
            artist_name,
            track_name
        ))
    }

    /// Fetch raw Qobuz metadata-search candidates (signed API, identical
    /// params for the loose and strict matchers).
    async fn fetch_metadata_candidates(
        &self,
        track_name: &str,
        artist_name: &str,
        user_auth_token: Option<&str>,
    ) -> Result<Vec<QobuzTrack>> {
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

        Ok(tracks.items)
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
            return Err(anyhow!(
                "Qobuz track/get failed for ID {}: HTTP {}",
                track_id,
                response.status()
            ));
        }

        let track: QobuzTrack = response.json().await?;
        info!(
            "[Qobuz] ✓ Resolved exact track entity: '{}' (ID: {})",
            track.title, track.id
        );
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
            return Err(anyhow!(
                "Qobuz album/get failed for ID {}: HTTP {}",
                album_id,
                response.status()
            ));
        }

        let album: QobuzAlbum = response.json().await?;
        info!(
            "[Qobuz] ✓ Resolved album entity: '{}' (ID: {:?})",
            album.title, album.id
        );
        Ok(album)
    }

    /// Download stream payload into staging file with byte progress and automatic retries
    #[allow(dead_code)] // Cubierta por `tests/network_resilience_test.rs`.
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

        // The retry budget, the wait and the user-facing wording come from the
        // shared error taxonomy per failure (see `classify_qobuz_http_failure`),
        // not from a hardcoded counter decided in this loop.
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
                        let taxonomy = classify_qobuz_http_failure(status, item_id);
                        attempt += 1;
                        let _ = tokio::fs::remove_file(staging_path).await;
                        let max_attempts = taxonomy.max_attempts();

                        if !taxonomy.is_retryable() {
                            return Err(anyhow!("Download failed: {}", taxonomy.ui_message()));
                        }
                        if attempt >= max_attempts {
                            return Err(anyhow!(
                                "NetworkExhausted: {} after {} attempts",
                                taxonomy.ui_message(),
                                attempt
                            ));
                        }

                        // A rate limit waits exactly as long as the provider asks
                        // (`Retry-After`, else the taxonomy default); anything else
                        // keeps the jittered exponential backoff.
                        let wait_dur = match taxonomy {
                            ErrorTaxonomy::RateLimited { .. } => {
                                parse_retry_after(resp.headers(), std::time::SystemTime::now())
                                    .unwrap_or_else(|| {
                                        Duration::from_secs(taxonomy.retry_delay_sec())
                                    })
                            }
                            _ => calculate_backoff_with_jitter(
                                attempt - 1,
                                initial_backoff,
                                max_backoff,
                            ),
                        };
                        warn!(
                            "[Qobuz] {} for item {}. Retrying in {:?} (attempt {}/{})",
                            taxonomy.ui_message(),
                            item_id,
                            wait_dur,
                            attempt,
                            max_attempts
                        );
                        tokio::time::sleep(wait_dur).await;

                        if let Some((downloader, track_id, quality, token_ref, allow_fallback)) =
                            stream_url_provider
                        {
                            if let Ok(new_stream) = downloader
                                .get_download_url(track_id, quality, token_ref, allow_fallback)
                                .await
                            {
                                current_url = new_stream.url;
                            }
                        }
                        continue;
                    }
                }
                Err(e) => {
                    let taxonomy = ErrorTaxonomy::TemporaryNetworkFailure {
                        endpoint: "qobuz stream".to_string(),
                        message: e.to_string(),
                    };
                    attempt += 1;
                    let _ = tokio::fs::remove_file(staging_path).await;
                    let max_attempts = taxonomy.max_attempts();

                    if attempt >= max_attempts {
                        return Err(anyhow!(
                            "NetworkExhausted: {} after {} attempts",
                            taxonomy.ui_message(),
                            attempt
                        ));
                    }

                    let backoff =
                        calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
                    warn!(
                        "[Qobuz] {} for item {}. Retrying in {:?} (attempt {}/{})",
                        taxonomy.ui_message(),
                        item_id,
                        backoff,
                        attempt,
                        max_attempts
                    );
                    tokio::time::sleep(backoff).await;

                    if let Some((downloader, track_id, quality, token_ref, allow_fallback)) =
                        stream_url_provider
                    {
                        if let Ok(new_stream) = downloader
                            .get_download_url(track_id, quality, token_ref, allow_fallback)
                            .await
                        {
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
                let taxonomy = ErrorTaxonomy::TemporaryNetworkFailure {
                    endpoint: "qobuz stream body".to_string(),
                    message: format!("stream decoding error: {}", err_msg),
                };
                attempt += 1;
                let _ = tokio::fs::remove_file(staging_path).await;
                let max_attempts = taxonomy.max_attempts();

                if attempt >= max_attempts {
                    return Err(anyhow!(
                        "NetworkExhausted: {} after {} attempts",
                        taxonomy.ui_message(),
                        attempt
                    ));
                }

                let backoff =
                    calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
                warn!(
                    "[Qobuz] {} for item {}. Retrying in {:?} (attempt {}/{})",
                    taxonomy.ui_message(),
                    item_id,
                    backoff,
                    attempt,
                    max_attempts
                );
                tokio::time::sleep(backoff).await;

                if let Some((downloader, track_id, quality, token_ref, allow_fallback)) =
                    stream_url_provider
                {
                    if let Ok(new_stream) = downloader
                        .get_download_url(track_id, quality, token_ref, allow_fallback)
                        .await
                    {
                        current_url = new_stream.url;
                    }
                }
                continue;
            }

            if let Err(e) = file.flush().await {
                let taxonomy = ErrorTaxonomy::TemporaryNetworkFailure {
                    endpoint: "qobuz staging file".to_string(),
                    message: format!("flush error: {}", e),
                };
                attempt += 1;
                let _ = tokio::fs::remove_file(staging_path).await;
                let max_attempts = taxonomy.max_attempts();
                if attempt >= max_attempts {
                    return Err(anyhow!(
                        "NetworkExhausted: {} after {} attempts",
                        taxonomy.ui_message(),
                        attempt
                    ));
                }
                let backoff =
                    calculate_backoff_with_jitter(attempt - 1, initial_backoff, max_backoff);
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
    pub fn download_track<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db_opt: Option<&'a sqlx::SqlitePool>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let mut phase_tracker = DownloadPhaseTracker::new();
            let item_id = &request.item_id;
            let duration_sec = (request.duration_ms / 1000) as i32;
            // Capture the queue's track identity once. A later deletion/reuse of the
            // queue row must not redirect lyrics to another track or erase a tombstone.
            let lyrics_identity =
                LyricsTrackIdentity::capture(db_opt, request.queue_id, request.canonical_track_id)
                    .await;

            PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "qobuz"));

            // 1. Auth phase & multi-account fallback resolution
            phase_tracker.start_phase(DownloadPhase::Auth);

            // WF-RES-01: cuando la petición viaja con un plan, la cuenta y la
            // edición ya están decididas. Re-derivarlas aquí sería una segunda
            // selección independiente, que es justo lo que este carril elimina.
            // La política de entitlement sigue viviendo en
            // `get_service_fallback_account_ids` y el resolutor la reutiliza.
            let candidate_account_ids = if let Some(plan) = request.resolution_plan.as_ref() {
                let primary = plan
                    .primary()
                    .ok_or_else(|| anyhow!("SourceIdentityMissing: empty resolution plan"))?;
                let planned_account = primary
                    .account
                    .as_ref()
                    .ok_or_else(|| anyhow!("RequiresAuth: no Qobuz account in resolution plan"))?;
                if !primary.service_name.eq_ignore_ascii_case("qobuz")
                    || request.service_track_id.as_deref()
                        != Some(primary.service_track_id.as_str())
                    || request.quality != plan.quality.effective
                {
                    // El adaptador solo ejecuta el candidato que le dieron; si la
                    // petición no coincide con el plan, es un bug del llamador, no
                    // una decisión que este pueda tomar en silencio.
                    return Err(anyhow!(
                        "SourceIdentityMissing: Qobuz request does not match its resolution plan"
                    ));
                }
                vec![planned_account.account_id]
            } else if let Some(pool) = db_opt {
                crate::commands::accounts::get_service_fallback_account_ids(pool, "qobuz", None)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            let mut last_error: Option<anyhow::Error> = None;
            let mut resolved_stream_context: Option<(i64, String, QobuzTrack, StreamResolution)> =
                None;

            if !candidate_account_ids.is_empty() {
                let pool = db_opt.expect("pool exists if candidate_account_ids is not empty");
                for (attempt_idx, &cand_account_id) in candidate_account_ids.iter().enumerate() {
                    let token = match self
                        .resolve_token_for_account(pool, Some(cand_account_id))
                        .await
                    {
                        Ok(t) => t,
                        Err(QobuzAuthStatus::RequiresAuth(reason)) => {
                            warn!(
                                account_id = cand_account_id,
                                error = %reason,
                                "Qobuz candidate account failed auth resolution"
                            );
                            last_error = Some(anyhow!("RequiresAuth: {}", reason));
                            continue;
                        }
                    };
                    // A browser token cannot reveal subscription flags. Keep
                    // unknown accounts unknown; no per-track entitlement probe.

                    let token_ref = Some(token.as_str());

                    info!(
                        account_id = cand_account_id,
                        attempt = attempt_idx + 1,
                        total_candidates = candidate_account_ids.len(),
                        "Attempting Qobuz stream resolution with candidate user account"
                    );

                    // 2. ResolveStream phase (Entity lookup + Stream URL resolution)
                    phase_tracker.start_phase(DownloadPhase::ResolveStream);
                    let track = if let Some(ref s_track_id) = request.service_track_id {
                        if let Ok(tid) = s_track_id.parse::<i64>() {
                            match self.get_track_by_id(tid, token_ref).await {
                                Ok(t) => t,
                                Err(e) => {
                                    warn!(account_id = cand_account_id, error = %e, "Qobuz get_track_by_id failed with candidate account");
                                    last_error = Some(e);
                                    continue;
                                }
                            }
                        } else {
                            return Err(anyhow!(
                                "SourceIdentityMissing: Non-numeric service_track_id '{}'",
                                s_track_id
                            ));
                        }
                    } else {
                        if !request.allow_fallback || !request.smart_studio_origin {
                            return Err(anyhow!(
                            "SourceIdentityMissing: No locked service_track_id and allow_fallback=false"
                        ));
                        }
                        if let Some(isrc) = &request.isrc {
                            match self.search_by_isrc(isrc, duration_sec, token_ref).await {
                                Ok(t) => t,
                                Err(_) => {
                                    match self
                                        .search_by_metadata(
                                            &request.track_name,
                                            &request.artist_name,
                                            duration_sec,
                                            token_ref,
                                        )
                                        .await
                                    {
                                        Ok(t) => t,
                                        Err(e) => {
                                            warn!(account_id = cand_account_id, error = %e, "Qobuz metadata search failed with candidate account");
                                            last_error = Some(e);
                                            continue;
                                        }
                                    }
                                }
                            }
                        } else {
                            match self
                                .search_by_metadata(
                                    &request.track_name,
                                    &request.artist_name,
                                    duration_sec,
                                    token_ref,
                                )
                                .await
                            {
                                Ok(t) => t,
                                Err(e) => {
                                    warn!(account_id = cand_account_id, error = %e, "Qobuz metadata search failed with candidate account");
                                    last_error = Some(e);
                                    continue;
                                }
                            }
                        }
                    };

                    // 3. Resolve Stream URL
                    let stream_res = match self
                        .get_download_url(
                            track.id,
                            &request.quality,
                            token_ref,
                            request.allow_fallback,
                        )
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => {
                            warn!(
                                account_id = cand_account_id,
                                error = %e,
                                "Candidate Qobuz account failed stream resolution; checking sibling account"
                            );
                            last_error = Some(e);
                            continue;
                        }
                    };

                    resolved_stream_context = Some((cand_account_id, token, track, stream_res));
                    break;
                }
            }

            let (token, track, stream_res) = if let Some((_account_id, token, track, stream_res)) =
                resolved_stream_context
            {
                (token, track, stream_res)
            } else if candidate_account_ids.is_empty() {
                // Fallback for cases with no pool or no candidate accounts in SQLite
                let token = match self.resolve_token(db_opt).await {
                    Ok(token) => token,
                    Err(QobuzAuthStatus::RequiresAuth(reason)) => {
                        let err_msg = format!("RequiresAuth: {}", reason);
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                        return Err(anyhow!("{}", err_msg));
                    }
                };
                let token_ref = Some(token.as_str());

                // 2. ResolveStream phase
                phase_tracker.start_phase(DownloadPhase::ResolveStream);
                let track = if let Some(ref s_track_id) = request.service_track_id {
                    if let Ok(tid) = s_track_id.parse::<i64>() {
                        self.get_track_by_id(tid, token_ref).await?
                    } else {
                        return Err(anyhow!(
                            "SourceIdentityMissing: Non-numeric service_track_id '{}'",
                            s_track_id
                        ));
                    }
                } else {
                    if !request.allow_fallback || !request.smart_studio_origin {
                        return Err(anyhow!(
                        "SourceIdentityMissing: No locked service_track_id and allow_fallback=false"
                    ));
                    }
                    if let Some(isrc) = &request.isrc {
                        match self.search_by_isrc(isrc, duration_sec, token_ref).await {
                            Ok(t) => t,
                            Err(_) => {
                                self.search_by_metadata(
                                    &request.track_name,
                                    &request.artist_name,
                                    duration_sec,
                                    token_ref,
                                )
                                .await?
                            }
                        }
                    } else {
                        self.search_by_metadata(
                            &request.track_name,
                            &request.artist_name,
                            duration_sec,
                            token_ref,
                        )
                        .await?
                    }
                };

                // 3. Resolve Stream URL
                let stream_res = self
                    .get_download_url(
                        track.id,
                        &request.quality,
                        token_ref,
                        request.allow_fallback,
                    )
                    .await?;

                (token, track, stream_res)
            } else {
                let err = last_error.unwrap_or_else(|| {
                anyhow!("RequiresAuth: Tu cuenta de Qobuz está desconectada. Inicia sesión en Syncify (Ajustes > Cuentas).")
            });
                let err_msg = err.to_string();
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                return Err(err);
            };
            let token_ref = Some(token.as_str());

            // 4. Staging path setup
            let out_dir = PathBuf::from(&request.output_dir);
            let staging_dir = out_dir.join(".staging");
            tokio::fs::create_dir_all(&staging_dir).await?;
            let nomedia_path = staging_dir.join(".nomedia");
            if !nomedia_path.exists() {
                let _ = tokio::fs::write(&nomedia_path, b"").await;
            }
            let staging_path =
                staging_dir.join(format!("{}.part", sanitize_filename(&request.item_id)));

            // 5. Transfer phase (Audio streaming to disk until flush)
            phase_tracker.start_phase(DownloadPhase::Transfer);
            let downloaded_bytes = match Box::pin(self.download_to_staging_internal(
                &stream_res.url,
                &staging_path,
                item_id,
                Some((
                    self,
                    track.id,
                    &request.quality,
                    token_ref,
                    request.allow_fallback,
                )),
            ))
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
                return Err(anyhow!(
                    "NetworkExhausted: Qobuz downloaded audio payload is 0 bytes"
                ));
            }
            phase_tracker.set_transfer_metrics(downloaded_bytes, "network");

            self.process_downloaded_staging(
                request,
                &track,
                &stream_res,
                &staging_path,
                downloaded_bytes,
                phase_tracker,
                token_ref,
                db_opt,
                lyrics_identity,
            )
            .await
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn process_downloaded_staging<'a>(
        &'a self,
        request: &'a DownloadRequest,
        track: &'a QobuzTrack,
        stream_res: &'a StreamResolution,
        staging_path: &'a Path,
        _downloaded_bytes: u64,
        mut phase_tracker: DownloadPhaseTracker,
        token_ref: Option<&'a str>,
        db_opt: Option<&'a sqlx::SqlitePool>,
        lyrics_identity: LyricsTrackIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let item_id = &request.item_id;
            let out_dir = PathBuf::from(&request.output_dir);
            let staging_dir = staging_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| out_dir.join(".staging"));

            // 6. ValidateAudio phase
            phase_tracker.start_phase(DownloadPhase::ValidateAudio);
            let is_flac = stream_res.format_id != "5";

            // The resolved format_id only says what was ASKED for; the payload decides
            // what was actually served. Reject short or mismatched samples before tagging.
            let header_bytes = match validate_staged_qobuz_payload(
                staging_path,
                &stream_res.format_id,
                request.duration_ms,
            )
            .await
            {
                Ok(header) => header,
                Err(e) => {
                    let _ = tokio::fs::remove_file(&staging_path).await;
                    return Err(e);
                }
            };

            // F3.4: Inspect STREAMINFO header from downloaded FLAC to extract true physical bit_depth and sample_rate
            let (real_bit_depth, real_sample_rate) = if is_flac {
                if let Some(info) = AudioByteValidator::parse_flac_streaminfo(&header_bytes) {
                    (info.bits_per_sample as i32, info.sample_rate as f64)
                } else if let Ok(tag) = metaflac::Tag::read_from_path(&staging_path) {
                    if let Some(info) = tag.get_streaminfo() {
                        (info.bits_per_sample as i32, info.sample_rate as f64)
                    } else {
                        (
                            track.max_bit_depth.unwrap_or(16),
                            track.max_sample_rate.unwrap_or(44.1) * 1000.0,
                        )
                    }
                } else {
                    (
                        track.max_bit_depth.unwrap_or(16),
                        track.max_sample_rate.unwrap_or(44.1) * 1000.0,
                    )
                }
            } else {
                (
                    track.max_bit_depth.unwrap_or(16),
                    track.max_sample_rate.unwrap_or(44.1) * 1000.0,
                )
            };

            // 7. Tagging with metaflac (VORBIS_COMMENT and PICTURE) + Full Enrichment
            let mut staged_lrc_path: Option<PathBuf> = None;
            let mut resolved_lyrics_res: Option<LyricsResolution> = None;
            let mut staged_cover_jpg_path: Option<PathBuf> = None;
            let mut staged_cover_webp_path: Option<PathBuf> = None;
            let mut staged_booklet_path: Option<PathBuf> = None;
            let mut resolved_musicbrainz_id: Option<String> = None;

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
            let track_num = track.track_number.unwrap_or(request.track_number) as u32;
            let track_tot = track
                .album
                .as_ref()
                .and_then(|a| a.total_tracks)
                .unwrap_or(request.total_tracks) as u32;
            let disc_num = track.disc_number.unwrap_or(request.disc_number) as u32;
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
            let performers_val = track
                .performers
                .clone()
                .or_else(|| Some(artist_name.clone()));
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
                    bit_depth: Some(real_bit_depth),
                    sample_rate: Some(real_sample_rate),
                    comment: Some(format!(
                        "Audio: Qobuz FLAC ({}bit/{:.1}kHz) | Engine: Syncify Production",
                        real_bit_depth,
                        real_sample_rate / 1000.0
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
                                    let cover_staging = staging_dir
                                        .join(format!("{}.cover.jpg", sanitize_filename(item_id)));
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
                match resolve_and_download_animated_cover(
                    &self.client,
                    &artist_name,
                    &album_title,
                    &staging_dir,
                )
                .await
                {
                    AnimatedCoverStatus::Success(webp_path) => {
                        info!(
                        "[Qobuz] ✓ Motion cover art resolved and downloaded from Apple Music: {:?}",
                        webp_path
                    );
                        if let Ok(webp_bytes) = tokio::fs::read(&webp_path).await {
                            if webp_bytes.len() >= 30 {
                                use syncify_core_domain::byte_validators::WebpByteValidator;
                                if let Ok(info) =
                                    WebpByteValidator::validate_animated_webp(&webp_bytes)
                                {
                                    info!(
                                        "[Qobuz] ✓ Validated animated WebP: {} frames, {}x{} px",
                                        info.anmf_frame_count,
                                        info.canvas_width,
                                        info.canvas_height
                                    );
                                    // Preserve animated artwork in sidecar (cover.webp / cover.animated.webp).
                                    // For FLAC embedded PICTURE block, prefer static JPEG with real dimensions and bounded size.
                                    if let Some(jpeg_bytes) = raw_jpeg_bytes {
                                        flac_meta.cover_data = Some(jpeg_bytes);
                                        flac_meta.cover_source =
                                            Some("Qobuz Cover Art".to_string());
                                    } else {
                                        flac_meta.cover_data = Some(webp_bytes);
                                        flac_meta.cover_source =
                                            Some("Apple Music Animated Cover".to_string());
                                    }
                                    staged_cover_webp_path = Some(webp_path);
                                    has_cover_cached = true;
                                } else {
                                    warn!("[Qobuz] Animated WebP failed structural validation (falling back to static cover)");
                                    if let Some(jpeg_bytes) = raw_jpeg_bytes {
                                        flac_meta.cover_data = Some(jpeg_bytes);
                                        flac_meta.cover_source =
                                            Some("Qobuz Cover Art".to_string());
                                    }
                                }
                            } else {
                                warn!("[Qobuz] Animated WebP candidate is smaller than 30 bytes, skipping: {:?}", webp_path);
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
                let lyrics_service = match db_opt {
                    Some(pool) => LyricsPipelineService::from_db(pool).await,
                    None => LyricsPipelineService::new(),
                };
                let duration_sec = if track.duration > 0 {
                    track.duration as f64
                } else {
                    (request.duration_ms / 1000) as f64
                };

                resolved_lyrics_res = None;

                match lyrics_service
                    .resolve_lyrics_and_sidecar(
                        &artist_name,
                        &track.title,
                        Some(&album_title),
                        duration_sec,
                    )
                    .await
                {
                    Ok((res, sidecar_opt)) => {
                        if res.status == ResolutionStatus::Resolved {
                            let track_id = lyrics_identity.track_id();
                            let format = match res.sync_type {
                                syncify_lyrics_domain::LyricsSyncType::KaraokeWordSynced
                                | syncify_lyrics_domain::LyricsSyncType::LineSynced => "lrc",
                                syncify_lyrics_domain::LyricsSyncType::Plain => "plain",
                                syncify_lyrics_domain::LyricsSyncType::Instrumental => {
                                    "instrumental"
                                }
                                syncify_lyrics_domain::LyricsSyncType::None => "none",
                            };
                            let blocked = if lyrics_identity.lyrics_fail_closed() {
                                true
                            } else if let (Some(pool), Some(tid)) = (db_opt, track_id) {
                                crate::commands::lyrics::lyrics_auto_blocked(pool, tid, format)
                                    .await
                                    .map_err(anyhow::Error::msg)?
                            } else {
                                // A direct download has no persisted deletion intent.
                                lyrics_blocked_without_db_identity()
                            };
                            if blocked {
                                debug!("[Qobuz] Skipping automatically resolved lyrics for deleted or unattributed track");
                            } else {
                                let tags = res.to_tag_contract();
                                if let Some(ref lyr) = tags.lyrics {
                                    flac_meta.lyrics_lrc = Some(lyr.clone());
                                }
                                if let Some(ref src) = tags.source {
                                    flac_meta.lyrics_source = Some(src.clone());
                                }

                                if let Some(ref lrc_content) = sidecar_opt {
                                    let lrc_staging = staging_dir
                                        .join(format!("{}.lrc", sanitize_filename(item_id)));
                                    if let Ok(_) = tokio::fs::write(&lrc_staging, lrc_content).await
                                    {
                                        staged_lrc_path = Some(lrc_staging);
                                        info!("[Qobuz] Synced lyrics acquired from {}: embedded and staged as .lrc", res.provider);
                                    }
                                } else {
                                    info!(
                                    "[Qobuz] Plain lyrics acquired from {} (no sidecar created)",
                                    res.provider
                                );
                                }

                                has_lyrics_cached = true;
                                resolved_lyrics_res = Some(res);
                            }
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
                        let is_pdf = g
                            .url
                            .as_deref()
                            .map(|u: &str| {
                                u.ends_with(".pdf") || u.contains("booklet") || u.contains("pdf")
                            })
                            .unwrap_or(false)
                            || g.original_url
                                .as_deref()
                                .map(|u: &str| {
                                    u.ends_with(".pdf")
                                        || u.contains("booklet")
                                        || u.contains("pdf")
                                })
                                .unwrap_or(false)
                            || g.name
                                .as_deref()
                                .map(|n: &str| {
                                    n.to_lowercase().contains("booklet")
                                        || n.to_lowercase().contains("pdf")
                                        || n.to_lowercase().contains("livret")
                                })
                                .unwrap_or(false)
                            || g.file_format_id == Some(21)
                            || g.file_format_id == Some(1);
                        if is_pdf {
                            if let Some(g_url) = g.url.as_deref().or(g.original_url.as_deref()) {
                                if let Ok(resp) = self.client.get(g_url).send().await {
                                    if resp.status().is_success() {
                                        if let Ok(bytes) = resp.bytes().await {
                                            if !bytes.is_empty()
                                                && (bytes.starts_with(b"%PDF") || bytes.len() > 100)
                                            {
                                                let booklet_staging = staging_dir.join(format!(
                                                    "{}.booklet.pdf",
                                                    sanitize_filename(item_id)
                                                ));
                                                let _ = tokio::fs::write(&booklet_staging, &bytes)
                                                    .await;
                                                staged_booklet_path = Some(booklet_staging);
                                                info!(
                                                "[Qobuz] ✓ Staged digital booklet PDF ({} bytes)",
                                                bytes.len()
                                            );
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
                    genre: track
                        .album
                        .as_ref()
                        .and_then(|a| a.genre.as_ref())
                        .map(|g| g.name.clone()),
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

                let enrichment_engine = match db_opt {
                    Some(db) => EnrichmentEngine::with_db(db.clone()),
                    None => EnrichmentEngine::new(),
                };
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
                } else if let Some(ref g) = track
                    .album
                    .as_ref()
                    .and_then(|a| a.genre.as_ref())
                    .map(|g| g.name.clone())
                {
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
                if let Some(art_tags) = enriched.artist_tags.value() {
                    flac_meta.artist_tags = Some(
                        art_tags
                            .split(';')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    );
                }
                if let Some(media_t) = enriched.media_type.value() {
                    flac_meta.media_type = Some(media_t.to_string());
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
                if let Some(bpm) = enriched
                    .bpm
                    .value()
                    .and_then(|s| s.parse::<u32>().ok())
                    .or(track.bpm)
                {
                    flac_meta.bpm = Some(bpm);
                }
                if let Some(mb_rid) = enriched.musicbrainz_recording_id.value() {
                    flac_meta.musicbrainz_track_id = Some(mb_rid.to_string());
                    has_mb_cached = true;
                    resolved_musicbrainz_id = Some(mb_rid.to_string());
                } else if let Some(ref req_mb_id) = request.musicbrainz_recording_id {
                    flac_meta.musicbrainz_track_id = Some(req_mb_id.clone());
                    has_mb_cached = true;
                    resolved_musicbrainz_id = Some(req_mb_id.clone());
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
                // TASK-75: relational acoustic identifiers. The request carries the stored
                // Chromaprint fingerprint (identity anchor used by the fallback matcher);
                // its stable AcoustID (UUIDv4-shaped MD5 of the fingerprint, same derivation
                // as AudioAnalyzer / fingerprint_bridge.py) travels alongside so Symfonium
                // gets ACOUSTID_ID + ACOUSTID_FINGERPRINT for smart radio.
                if let Some(ref fp) = request.acoustid_fingerprint {
                    let fp_trimmed = fp.trim();
                    if !fp_trimmed.is_empty() {
                        flac_meta.acoustid_fingerprint = Some(fp_trimmed.to_string());
                        flac_meta.acoustid_id = derive_acoustid_id(fp_trimmed);
                    }
                }

                // Serialize deletion with tagging and promotion, not only staging.
                let _lyrics_promotion_guard =
                    crate::commands::lyrics::lyrics_file_guard().lock().await;
                let lyrics_track_id = lyrics_identity.track_id();
                let (lrc_blocked, plain_blocked) = if lyrics_identity.lyrics_fail_closed() {
                    (true, true)
                } else if let (Some(pool), Some(tid)) = (db_opt, lyrics_track_id) {
                    (
                        crate::commands::lyrics::lyrics_auto_blocked(pool, tid, "lrc")
                            .await
                            .map_err(anyhow::Error::msg)?,
                        crate::commands::lyrics::lyrics_auto_blocked(pool, tid, "plain")
                            .await
                            .map_err(anyhow::Error::msg)?,
                    )
                } else {
                    // Direct downloads without a DB identity have no deletion mark
                    // to consult; do not discard freshly resolved lyrics by default.
                    (
                        lyrics_blocked_without_db_identity(),
                        lyrics_blocked_without_db_identity(),
                    )
                };
                // The .lrc sidecar is independent of the resolved tag's format.
                if lrc_blocked {
                    staged_lrc_path = None;
                }
                let resolved_format_blocked =
                    resolved_lyrics_res
                        .as_ref()
                        .is_some_and(|res| match res.sync_type {
                            syncify_lyrics_domain::LyricsSyncType::KaraokeWordSynced
                            | syncify_lyrics_domain::LyricsSyncType::LineSynced => lrc_blocked,
                            syncify_lyrics_domain::LyricsSyncType::Plain => plain_blocked,
                            _ => true,
                        });
                if resolved_format_blocked || (lrc_blocked && plain_blocked) {
                    flac_meta.lyrics_lrc = None;
                    flac_meta.lyrics_source = None;
                    resolved_lyrics_res = None;
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
                        return Err(anyhow!(
                            "Failed FLAC tagging and verification in staging: {}",
                            e
                        ));
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
            let track_num = track.track_number.unwrap_or(request.track_number) as u32;
            let track_tot = track
                .album
                .as_ref()
                .and_then(|a| a.total_tracks)
                .unwrap_or(request.total_tracks) as u32;
            let disc_num = track.disc_number.unwrap_or(request.disc_number) as u32;
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
                bit_depth: Some(real_bit_depth),
                sample_rate: Some(real_sample_rate),
            };

            let raw_final_path = layout.resolve_track_path(&layout_ctx);
            let final_path = layout.resolve_unique_path(&raw_final_path);
            let target_dir = final_path.parent().unwrap_or(&out_dir);
            tokio::fs::create_dir_all(target_dir).await?;

            // 8. Atomic promotion from staging to final path
            if let Err(e) = tokio::fs::rename(&staging_path, &final_path).await {
                let staged_bytes = tokio::fs::read(&staging_path).await.map_err(|re| {
                    anyhow!("Failed to read staged file {:?}: {}", staging_path, re)
                })?;
                let staged_sha =
                    crate::services::repair_guardrail::compute_bytes_sha256(&staged_bytes);
                let staged_len = staged_bytes.len() as u64;

                if let Err(ce) = tokio::fs::write(&final_path, &staged_bytes).await {
                    let _ = tokio::fs::remove_file(&staging_path).await;
                    if let Some(ref p) = staged_lrc_path {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    if let Some(ref p) = staged_cover_jpg_path {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    if let Some(ref p) = staged_cover_webp_path {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    if let Some(ref p) = staged_booklet_path {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    return Err(anyhow!(
                        "Failed to promote staging file to final path: rename err={}, copy err={}",
                        e,
                        ce
                    ));
                }

                let dest_meta = tokio::fs::metadata(&final_path).await.map_err(|me| {
                    anyhow!(
                        "Failed to read promoted file metadata {:?}: {}",
                        final_path,
                        me
                    )
                })?;
                let dest_bytes = tokio::fs::read(&final_path).await.map_err(|re| {
                    anyhow!("Failed to reread promoted file {:?}: {}", final_path, re)
                })?;
                let dest_sha = crate::services::repair_guardrail::compute_bytes_sha256(&dest_bytes);

                if dest_meta.len() != staged_len || dest_sha != staged_sha {
                    let _ = tokio::fs::remove_file(&final_path).await;
                    return Err(anyhow!(
                    "Integrity mismatch on cross-volume promotion: size {} vs {}, hash {} vs {}",
                    dest_meta.len(),
                    staged_len,
                    dest_sha,
                    staged_sha
                ));
                }

                let _ = tokio::fs::remove_file(&staging_path).await;
            }

            // 9. Promote sidecars (lyrics, cover artwork, booklets)
            // The tagging-phase guard remains held through this promotion.
            if request.artifact_policy.lyrics_lrc {
                if let Some(ref lrc_staged) = staged_lrc_path {
                    let final_lrc = layout.lyrics_path_for_track(&final_path);
                    if let Err(_) = tokio::fs::rename(lrc_staged, &final_lrc).await {
                        let _ = tokio::fs::copy(lrc_staged, &final_lrc).await;
                    }
                    let _ = tokio::fs::remove_file(lrc_staged).await;

                    // F5.3: Persist synced lyrics (.lrc) in the database upon physical promotion (mitiga A11)
                    if let Some(pool) = db_opt {
                        let track_id_opt = lyrics_identity.track_id();

                        if let Some(tid) = track_id_opt {
                            let content = tokio::fs::read_to_string(&final_lrc)
                                .await
                                .unwrap_or_default();
                            if !content.is_empty() {
                                let (sync_level, source) =
                                    if let Some(ref res) = resolved_lyrics_res {
                                        let s_lvl = match res.sync_type {
                                            LyricsSyncType::KaraokeWordSynced => "word",
                                            LyricsSyncType::LineSynced => "line",
                                            _ => "none",
                                        };
                                        (s_lvl.to_string(), res.provider.clone())
                                    } else {
                                        ("line".to_string(), "qobuz_lyrics".to_string())
                                    };

                                let lyrics_insert = sqlx::query(
                            r#"INSERT INTO lyrics (track_id, format, sync_level, source, content, language, embedded_in_file)
                               VALUES (?, 'lrc', ?, ?, ?, ?, ?)
                               ON CONFLICT(track_id, format) DO UPDATE SET
                                   content = excluded.content,
                                   sync_level = excluded.sync_level,
                                   source = excluded.source,
                                   embedded_in_file = excluded.embedded_in_file"#
                        )
                        .bind(tid)
                        .bind(&sync_level)
                        .bind(&source)
                        .bind(&content)
                        .bind(None::<String>)
                        .bind(if is_flac { 1i64 } else { 0i64 })
                        .execute(pool)
                        .await;

                                if let Err(ref le) = lyrics_insert {
                                    warn!(error = %le, track_id = tid, "[Qobuz] Non-fatal: failed to persist lyrics in DB");
                                }
                            }
                        }
                    }
                }
            }

            let is_valid_file = |p: &std::path::Path| {
                p.exists() && p.metadata().map(|m| m.len() > 0).unwrap_or(false)
            };

            if request.artifact_policy.cover_art {
                if let Some(ref cov_staged) = staged_cover_jpg_path {
                    let final_cover = target_dir.join("cover.jpg");
                    if !is_valid_file(&final_cover) {
                        let _ = tokio::fs::copy(cov_staged, &final_cover).await;
                    }
                    if let Some(parent) = target_dir.parent() {
                        let dir_name = target_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");
                        if dir_name.starts_with("Disc") || dir_name.starts_with("CD") {
                            let parent_cover = parent.join("cover.jpg");
                            if !is_valid_file(&parent_cover) {
                                let _ = tokio::fs::copy(cov_staged, &parent_cover).await;
                            }
                        }
                    }
                    let _ = tokio::fs::remove_file(cov_staged).await;
                }
            }

            if request.artifact_policy.animated_cover {
                if let Some(ref webp_staged) = staged_cover_webp_path {
                    let _ = promote_webp_sidecars(webp_staged, target_dir).await;
                    let _ = tokio::fs::remove_file(staging_dir.join("folder.webp")).await;
                    let _ = tokio::fs::remove_file(staging_dir.join("animated.webp")).await;
                    let _ = tokio::fs::remove_file(staging_dir.join("cover.animated.webp")).await;
                }
            }

            if request.artifact_policy.booklet {
                if let Some(ref booklet_staged) = staged_booklet_path {
                    let final_booklet = target_dir.join("booklet.pdf");
                    if !is_valid_file(&final_booklet) {
                        let _ = tokio::fs::copy(booklet_staged, &final_booklet).await;
                    }
                    let _ = tokio::fs::remove_file(booklet_staged).await;
                }
            }

            // Guard against 0-byte truncated sidecars: regenerate from FLAC PICTURE block if missing or empty
            if is_flac {
                // D-03: sanitize the embedded cover art of the promoted file before the
                // sidecar guard reads its PICTURE blocks, so a cover that had to be
                // re-encoded is the one the sidecars are rebuilt from. Non-fatal.
                let mut cover_ctx =
                    crate::services::flac_cover_sanitizer::FlacCoverSanitizeContext::new(
                        "qobuz.download",
                    );
                cover_ctx.track_id = request.canonical_track_id;
                match crate::services::flac_cover_sanitizer::sanitize_and_audit_flac_cover_art(
                    db_opt,
                    &final_path,
                    &cover_ctx,
                )
                .await
                {
                    Ok(outcome) if outcome.cover_art_lost() => {
                        warn!(
                            path = %final_path.display(),
                            dropped = ?outcome.report.dropped_unrepairable_blocks,
                            sidecars = ?outcome.preserved_sidecars,
                            "[Qobuz] Embedded cover art could not be recovered (D-03); loss recorded in repair history"
                        );
                    }
                    Ok(outcome) if outcome.cover_art_recovered() => {
                        info!(
                            path = %final_path.display(),
                            recovered = outcome.report.recovered_blocks,
                            "[Qobuz] Damaged embedded cover art recovered by re-encoding it on the host"
                        );
                    }
                    Ok(_) => {}
                    Err(sanitize_err) => {
                        warn!(error = %sanitize_err, path = %final_path.display(), "[Qobuz] FLAC cover sanitization failed (non-fatal)");
                    }
                }
                if request.artifact_policy.cover_art {
                    if let Ok(repaired) = crate::services::flac_picture::ensure_flac_sidecars_intact(
                        &final_path,
                        target_dir,
                    ) {
                        if !repaired.is_empty() {
                            info!(count = repaired.len(), "[Qobuz] ✓ Regenerated {} truncated/missing sidecar(s) from FLAC PICTURE block", repaired.len());
                        }
                    }
                }
            }

            info!("[Qobuz] Successfully finalized track: {:?}", final_path);

            // DLX-01: separate the post-promotion SQLite work from filesystem promotion.
            // This phase includes pool acquisition and query time; it is not a pure SQL duration.
            phase_tracker.start_phase(DownloadPhase::Persisting);
            // TASK-84: Retro-propagate MusicBrainz ID to SQLite database if known or present in physical FLAC Vorbis comments
            if let Some(pool) = db_opt {
                let track_id_opt = if let Some(tid) = request.canonical_track_id {
                    Some(tid)
                } else if let Some(qid) = request.queue_id {
                    sqlx::query_scalar("SELECT track_id FROM download_queue WHERE id = ?")
                        .bind(qid)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten()
                } else {
                    // ED-02: no retroactive metadata write by ISRC alone.
                    None
                };

                if let Some(tid) = track_id_opt {
                    let mbid_opt = resolved_musicbrainz_id
                        .as_deref()
                        .or(request.musicbrainz_recording_id.as_deref());

                    if let Some(mbid) = mbid_opt {
                        let clean_mbid = mbid.trim();
                        if !clean_mbid.is_empty() {
                            let update_res =
                                sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                                    .bind(clean_mbid)
                                    .bind(tid)
                                    .execute(pool)
                                    .await;
                            if let Err(ref e) = update_res {
                                warn!(error = %e, track_id = tid, "[Qobuz] Non-fatal: failed to retro-propagate musicbrainz_id to DB");
                            } else {
                                info!(
                                    track_id = tid,
                                    mbid = clean_mbid,
                                    "[Qobuz] ✓ Retro-propagated musicbrainz_id to tracks DB"
                                );
                            }
                        }
                    } else if is_flac && final_path.exists() {
                        let _ = crate::services::musicbrainz::sync_flac_musicbrainz_id_to_track(
                            pool,
                            tid,
                            &final_path,
                        )
                        .await;
                    }
                }
            }

            // F3.4: Inspect STREAMINFO header from promoted physical FLAC to ensure exact ground-truth audio metrics
            let (final_bit_depth, final_sample_rate) = if is_flac {
                if let Ok(tag) = metaflac::Tag::read_from_path(&final_path) {
                    if let Some(info) = tag.get_streaminfo() {
                        (info.bits_per_sample as i32, info.sample_rate as f64)
                    } else {
                        (real_bit_depth, real_sample_rate)
                    }
                } else {
                    (real_bit_depth, real_sample_rate)
                }
            } else {
                (real_bit_depth, real_sample_rate)
            };

            phase_tracker.set_cache_hits(has_lyrics_cached, has_cover_cached, has_mb_cached);
            let phase_timings = phase_tracker.finish_completed();

            Ok(DownloadResult {
                file_path: final_path.to_string_lossy().to_string(),
                bit_depth: final_bit_depth,
                sample_rate: final_sample_rate as i32,
                title: track.title.clone(),
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
        })
    }
}

impl Default for QobuzDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate if WebP sidecar file is valid and meets the minimum size requirement (len >= 30) (TASK-84).
pub fn is_valid_webp_sidecar(p: &std::path::Path) -> bool {
    p.exists() && p.metadata().map(|m| m.len() >= 30).unwrap_or(false)
}

/// Safely promote WebP animated cover sidecars only if valid and >= 30 bytes,
/// preventing 0-byte or corrupted sidecar generation (TASK-84).
pub async fn promote_webp_sidecars(
    staging_webp_path: &std::path::Path,
    target_dir: &std::path::Path,
) -> Result<bool, std::io::Error> {
    if !is_valid_webp_sidecar(staging_webp_path) {
        warn!("[Qobuz] Staged WebP cover is smaller than 30 bytes or corrupt, skipping sidecar promotion: {:?}", staging_webp_path);
        let _ = tokio::fs::remove_file(staging_webp_path).await;
        return Ok(false);
    }

    let final_webp = target_dir.join("cover.webp");
    let final_folder_webp = target_dir.join("folder.webp");
    let final_anim_webp = target_dir.join("animated.webp");

    if !is_valid_webp_sidecar(&final_webp) {
        let _ = tokio::fs::copy(staging_webp_path, &final_webp).await;
    }
    if !is_valid_webp_sidecar(&final_folder_webp) {
        let _ = tokio::fs::copy(staging_webp_path, &final_folder_webp).await;
    }
    if !is_valid_webp_sidecar(&final_anim_webp) {
        let _ = tokio::fs::copy(staging_webp_path, &final_anim_webp).await;
    }

    if let Some(parent) = target_dir.parent() {
        let dir_name = target_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if dir_name.starts_with("Disc") || dir_name.starts_with("CD") {
            for sidecar_name in &["cover.webp", "folder.webp", "animated.webp"] {
                let p = parent.join(sidecar_name);
                if !is_valid_webp_sidecar(&p) {
                    let _ = tokio::fs::copy(staging_webp_path, &p).await;
                }
            }
        }
    }

    let _ = tokio::fs::remove_file(staging_webp_path).await;
    Ok(true)
}

/// Check if two titles match (fuzzy)
pub fn title_matches(expected: &str, found: &str) -> bool {
    let expected_clean = clean_title(expected).replace(" & ", " and ");
    let found_clean = clean_title(found).replace(" & ", " and ");

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
    let unescaped = syncify_core_domain::metadata::decode_html_entities(title);
    let unmojibake = syncify_core_domain::metadata::clean_mojibake(&unescaped);
    let mut clean = unmojibake.to_lowercase();
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

/// TASK-75: derive the stable AcoustID identifier for a Chromaprint fingerprint.
///
/// Without an AcoustID web-service lookup the canonical identity is the UUIDv4-shaped
/// MD5 of the fingerprint string — the exact derivation used by
/// `services::enrichment::AudioAnalyzer::run_fpcalc_binary` and
/// `scripts/fingerprint_bridge.py`, keeping the identifier consistent across every
/// producer of `acoustid_id`.
pub fn derive_acoustid_id(fingerprint: &str) -> Option<String> {
    let fp = fingerprint.trim();
    if fp.is_empty() {
        return None;
    }
    let hash = md5::compute(fp.as_bytes());
    let hex = format!("{:x}", hash);
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// Immutable identity for lyrics across resolution, tagging and promotion.
/// A direct request with no persisted identity remains distinct from a queued
/// request whose identity could not be established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LyricsTrackIdentity {
    Direct,
    Persisted(i64),
    UnattributedQueue,
}

impl LyricsTrackIdentity {
    async fn capture(
        db: Option<&sqlx::SqlitePool>,
        queue_id: Option<i64>,
        canonical_track_id: Option<i64>,
    ) -> Self {
        let queue_track_id = if let (Some(pool), Some(qid)) = (db, queue_id) {
            match sqlx::query_scalar::<_, i64>("SELECT track_id FROM download_queue WHERE id = ?")
                .bind(qid)
                .fetch_optional(pool)
                .await
            {
                Ok(id) => id,
                Err(err) => {
                    warn!(error = %err, "Qobuz queue identity unavailable for lyrics");
                    return Self::UnattributedQueue;
                }
            }
        } else {
            None
        };
        if queue_id.is_some() && db.is_some() {
            match (queue_track_id, canonical_track_id) {
                (Some(id), Some(canonical)) if id != canonical => Self::UnattributedQueue,
                (Some(id), _) => Self::Persisted(id),
                (None, _) => Self::UnattributedQueue,
            }
        } else if let Some(id) = canonical_track_id {
            Self::Persisted(id)
        } else if queue_id.is_some() {
            Self::UnattributedQueue
        } else {
            Self::Direct
        }
    }

    fn track_id(self) -> Option<i64> {
        match self {
            Self::Persisted(id) => Some(id),
            Self::Direct | Self::UnattributedQueue => None,
        }
    }

    fn lyrics_fail_closed(self) -> bool {
        matches!(self, Self::UnattributedQueue)
    }
}

// Without a database track identity there is no deletion intent to attribute.
// Keep this decision shared by resolution and the promotion-time recheck.
fn lyrics_blocked_without_db_identity() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_download_without_db_identity_keeps_resolved_lyrics() {
        // Resolution and promotion must agree when neither canonical_track_id
        // nor queue_id can attribute a persisted deletion mark.
        assert!(!lyrics_blocked_without_db_identity());
    }

    #[tokio::test]
    async fn queued_lyrics_identity_survives_row_deletion_and_reuse() {
        let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE download_queue (id INTEGER PRIMARY KEY, track_id INTEGER NOT NULL)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("CREATE TABLE lyrics_deletion_marks (track_id INTEGER, format TEXT)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE lyrics_restored_formats (track_id INTEGER, format TEXT)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO download_queue VALUES (7, 101)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO lyrics_deletion_marks VALUES (101, 'lrc'), (101, 'plain')")
            .execute(&db)
            .await
            .unwrap();
        let captured = LyricsTrackIdentity::capture(Some(&db), Some(7), None).await;
        assert_eq!(captured.track_id(), Some(101));
        for format in ["lrc", "plain"] {
            assert!(crate::commands::lyrics::lyrics_auto_blocked(
                &db,
                captured.track_id().unwrap(),
                format
            )
            .await
            .unwrap());
        }
        sqlx::query("DELETE FROM download_queue WHERE id = 7")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(captured.track_id(), Some(101));
        assert!(LyricsTrackIdentity::capture(Some(&db), Some(7), None)
            .await
            .lyrics_fail_closed());
        sqlx::query("INSERT INTO download_queue VALUES (7, 202)")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(captured.track_id(), Some(101));
        assert!(LyricsTrackIdentity::capture(Some(&db), Some(7), Some(101))
            .await
            .lyrics_fail_closed());
        assert_eq!(
            LyricsTrackIdentity::capture(Some(&db), None, None).await,
            LyricsTrackIdentity::Direct
        );
        assert!(LyricsTrackIdentity::capture(None, Some(7), None)
            .await
            .lyrics_fail_closed());
        let synthetic_flac = b"fLaC\0\0\0\0";
        assert_eq!(&synthetic_flac[..4], b"fLaC");

        // Separate marks must not be conflated when the queue mutates after capture.
        sqlx::query("DELETE FROM lyrics_deletion_marks")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO lyrics_deletion_marks VALUES (101, 'lrc')")
            .execute(&db)
            .await
            .unwrap();
        assert!(
            crate::commands::lyrics::lyrics_auto_blocked(&db, 101, "lrc")
                .await
                .unwrap()
        );
        assert!(
            !crate::commands::lyrics::lyrics_auto_blocked(&db, 101, "plain")
                .await
                .unwrap()
        );
        sqlx::query("DELETE FROM lyrics_deletion_marks")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO lyrics_deletion_marks VALUES (101, 'plain')")
            .execute(&db)
            .await
            .unwrap();
        assert!(
            !crate::commands::lyrics::lyrics_auto_blocked(&db, 101, "lrc")
                .await
                .unwrap()
        );
        assert!(
            crate::commands::lyrics::lyrics_auto_blocked(&db, 101, "plain")
                .await
                .unwrap()
        );
    }

    #[test]
    fn qobuz_preview_is_not_full_flac_or_320() {
        let preview = b"ID3\x04\x00\x00\x00\x00\x00\x00\xff\xfb\x90\x00";
        let flac_err = validate_qobuz_audio_payload("6", preview, 1_200_588, 210_000)
            .expect_err("Qobuz format_id=6 may still return an MP3 preview");
        assert!(flac_err.to_string().contains("QobuzPreviewPayload:"));
        assert!(!flac_err.to_string().contains("RequiresAuth"));
        let mp3_err = validate_qobuz_audio_payload("5", preview, 1_200_588, 210_000)
            .expect_err("short MP3 must not be promoted as a complete 320 kbps track");
        assert!(mp3_err.to_string().contains("QobuzPreviewPayload:"));
        let full_320 = b"\xff\xfb\xe0\x00";
        assert!(validate_qobuz_audio_payload("5", full_320, 11_000_000, 210_000).is_ok());
        assert!(validate_qobuz_audio_payload("5", preview, 11_000_000, 210_000).is_err());
        // A valid MP3 signature is insufficient: 128 kbps remains below the
        // contractual 320 floor even when the file happens to be very large.
        assert!(
            validate_qobuz_audio_payload("5", b"\xff\xfb\x90\x00", 11_000_000, 210_000).is_err()
        );
    }

    #[test]
    fn genuine_flac_magic_is_not_a_false_positive() {
        let flac = b"fLaC\x80\x00\x00\x22";
        assert!(validate_qobuz_audio_payload("6", flac, 1_200_588, 210_000).is_ok());
        assert!(validate_qobuz_audio_payload("7", flac, 1_200_588, 210_000).is_ok());
        assert!(validate_qobuz_audio_payload("27", flac, 1_200_588, 210_000).is_ok());
        let invalid = validate_qobuz_audio_payload("6", b"<html>error", 1_200_588, 210_000)
            .expect_err("non-audio is invalid, not preview or auth");
        assert!(invalid.to_string().contains("InvalidAudioPayload"));
    }

    #[test]
    fn short_flac_streaminfo_is_preview_but_full_flac_is_accepted() {
        fn flac_header(total_samples: u32) -> [u8; 42] {
            let mut header = [0u8; 42];
            header[..8].copy_from_slice(b"fLaC\x80\x00\x00\x22");
            let sample_rate: u32 = 44_100;
            header[18..20].copy_from_slice(&((sample_rate >> 4) as u16).to_be_bytes());
            header[20] = ((sample_rate & 15) as u8) << 4;
            header[21] = 0xf0; // 16-bit PCM, high total-sample bits = 0
            header[22..26].copy_from_slice(&total_samples.to_be_bytes());
            header
        }
        let preview = flac_header(30 * 44_100);
        assert!(
            validate_qobuz_audio_payload("6", &preview, 1_200_588, 210_000)
                .unwrap_err()
                .to_string()
                .contains("QobuzPreviewPayload:")
        );
        let full = flac_header(210 * 44_100);
        assert!(validate_qobuz_audio_payload("6", &full, 1_200_588, 210_000).is_ok());
    }

    #[tokio::test]
    async fn mock_preview_staging_is_rejected_and_cleaned() {
        let dir = tempfile::tempdir().unwrap();
        let staged = dir.path().join("preview.part");
        let preview = [
            b"ID3\x04\x00\x00\x00\x00\x00\x00".as_slice(),
            &vec![0u8; 1_200_578],
        ]
        .concat();
        tokio::fs::write(&staged, preview).await.unwrap();
        let req = DownloadRequest {
            item_id: "mock-preview".to_string(),
            output_dir: dir.path().to_string_lossy().to_string(),
            duration_ms: 210_000,
            quality: "LOSSLESS".to_string(),
            ..Default::default()
        };
        let track = QobuzTrack {
            id: 1,
            title: "Mock".to_string(),
            isrc: None,
            duration: 210,
            max_bit_depth: None,
            max_sample_rate: None,
            track_number: None,
            disc_number: None,
            copyright: None,
            performers: None,
            composer: None,
            work: None,
            parental_warning: None,
            performer: None,
            bpm: None,
            album: None,
        };
        let stream = StreamResolution {
            url: "mock://unused".to_string(),
            source: StreamUrlSource::QobuzOfficial,
            format_id: "6".to_string(),
        };
        let err = QobuzDownloader::new()
            .process_downloaded_staging(
                &req,
                &track,
                &stream,
                &staged,
                1_200_588,
                DownloadPhaseTracker::new(),
                None,
                None,
                LyricsTrackIdentity::Direct,
            )
            .await
            .expect_err("preview must fail before tagging or promotion");
        assert!(err.to_string().contains("QobuzPreviewPayload:"));
        assert!(
            !staged.exists(),
            "the actual download flow must clean staging"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

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
        assert_eq!(
            sanitize_filename("AC/DC: High Voltage*"),
            "AC_DC_ High Voltage_"
        );
    }

    #[test]
    fn test_derive_acoustid_id() {
        let fp = "AQAA0bmSQIhQJEAiFBCSEceE5McJ8kieBE-OP9qBo0C0kjtQJdKAoiEq7RAzvEDvIPdx_Md5nDl6xTg";
        let id = derive_acoustid_id(fp).expect("non-empty fingerprint must yield an id");
        // UUIDv4-shaped: 8-4-4-4-12 hex groups
        let groups: Vec<usize> = id.split('-').map(|g| g.len()).collect();
        assert_eq!(
            groups,
            vec![8, 4, 4, 4, 12],
            "AcoustID id must be UUID-shaped: {}",
            id
        );
        // Deterministic across invocations
        assert_eq!(id, derive_acoustid_id(fp).unwrap());
        // Honest absence for empty input
        assert!(derive_acoustid_id("").is_none());
        assert!(derive_acoustid_id("   ").is_none());
    }

    #[tokio::test]
    async fn test_resolve_token_no_pool_returns_requires_auth() {
        let downloader = QobuzDownloader::new();
        let res = downloader.resolve_token(None).await;
        match res {
            Err(QobuzAuthStatus::RequiresAuth(msg)) => {
                assert!(msg.contains("No database pool provided"));
            }
            Ok(_) => panic!("Expected RequiresAuth error when no pool is provided"),
        }
    }

    #[tokio::test]
    async fn test_get_download_url_without_token_and_no_fallback_returns_requires_auth() {
        let downloader = QobuzDownloader::new();
        let res = downloader
            .get_download_url(12345, "LOSSLESS", None, false)
            .await;
        assert!(res.is_err());
        let err_str = res.unwrap_err().to_string();
        assert!(err_str.contains("RequiresAuth"));
        assert!(err_str.contains("Tu cuenta de Qobuz está desconectada"));
    }

    #[tokio::test]
    async fn test_download_track_no_db_propagates_requires_auth_and_does_not_call_proxy() {
        let downloader = QobuzDownloader::new();
        let req = DownloadRequest {
            item_id: "test_qobuz_no_db".to_string(),
            service_track_id: Some("12345".to_string()),
            track_name: "Test Track".to_string(),
            artist_name: "Test Artist".to_string(),
            quality: "LOSSLESS".to_string(),
            allow_fallback: false,
            ..Default::default()
        };
        let res = downloader.download_track(&req, None).await;
        assert!(res.is_err());
        let err_str = res.unwrap_err().to_string();
        assert!(err_str.contains("RequiresAuth"));
        assert!(err_str.contains("No database pool provided"));
        assert!(!err_str.contains("proxy fallback APIs failed"));
    }

    #[tokio::test]
    async fn test_qobuz_multi_account_token_resolution_and_fallback_order() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory db");

        sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id),
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                credentials_invalid INTEGER DEFAULT 0,
                invalid_reason TEXT,
                last_auth_error TEXT,
                last_auth_error_at TEXT,
                credentials_json TEXT,
                last_synced TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                streaming_entitled INTEGER NOT NULL DEFAULT -1,
                entitlement_checked_at TEXT
            );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO services (id, name) VALUES (1, 'qobuz');")
            .execute(&pool)
            .await
            .unwrap();

        let _ = crate::crypto::init_crypto([42u8; 32]);
        let creds_active =
            crate::crypto::encrypt(r#"{"user_auth_token":"invalid_short"}"#).unwrap();
        let creds_sibling =
            crate::crypto::encrypt(r#"{"user_auth_token":"qobuz_valid_token_12345678"}"#).unwrap();

        // Account 1: Active, but invalid token
        sqlx::query(
            "INSERT INTO accounts (id, service_id, email, is_active, credentials_json) VALUES (1, 1, 'acc1@test.com', 1, ?);"
        )
        .bind(creds_active)
        .execute(&pool)
        .await
        .unwrap();

        // Account 2: Inactive sibling, but valid token
        sqlx::query(
            "INSERT INTO accounts (id, service_id, email, is_active, credentials_json) VALUES (2, 1, 'acc2@test.com', 0, ?);"
        )
        .bind(creds_sibling)
        .execute(&pool)
        .await
        .unwrap();

        let downloader = QobuzDownloader::new();

        // Account 1 should fail because token length < 16
        let res_acc1 = downloader.resolve_token_for_account(&pool, Some(1)).await;
        assert!(matches!(res_acc1, Err(QobuzAuthStatus::RequiresAuth(_))));

        // Account 2 should succeed
        let token_acc2 = downloader
            .resolve_token_for_account(&pool, Some(2))
            .await
            .expect("sibling token");
        assert_eq!(token_acc2, "qobuz_valid_token_12345678");

        // Fallback account IDs should return [1, 2]
        let candidate_ids =
            crate::commands::accounts::get_service_fallback_account_ids(&pool, "qobuz", None)
                .await
                .expect("candidate ids");
        assert_eq!(candidate_ids, vec![1, 2]);

        // is_active in SQLite must remain unchanged (1 is active, 2 is inactive)
        let active_id: (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = 1 AND is_active = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            active_id.0, 1,
            "is_active exclusivity in SQLite must not be modified"
        );
    }

    #[test]
    fn test_crash_stack_qobuz_staging_download_on_constrained_stack() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let child = std::thread::Builder::new()
            .name("test-qobuz-staging-stack".to_string())
            .stack_size(256 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime");

                rt.block_on(async {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let port = listener.local_addr().unwrap().port();

                    tokio::spawn(async move {
                        if let Ok((mut socket, _)) = listener.accept().await {
                            let mut buf = [0u8; 1024];
                            let _ = socket.read(&mut buf).await;
                            let response = "HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nfLaC_payload";
                            let _ = socket.write_all(response.as_bytes()).await;
                            let _ = socket.flush().await;
                        }
                    });

                    let temp_dir = tempfile::tempdir().unwrap();
                    let staging_path = temp_dir.path().join("constrained_staging.part");
                    let downloader = QobuzDownloader::new();
                    let url = format!("http://127.0.0.1:{}/stream", port);

                    let res = downloader
                        .download_to_staging(&url, &staging_path, "item_constrained_1")
                        .await;

                    assert!(res.is_ok(), "Streaming payload must succeed without stack overflow: {:?}", res.err());
                    assert_eq!(res.unwrap(), 12);
                    assert!(staging_path.exists());
                });
            })
            .expect("spawn thread");

        child
            .join()
            .expect("Constrained stack thread must not abort with stack overflow");
    }
}
