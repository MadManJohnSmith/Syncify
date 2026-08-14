//! Tidal Downloader — Core resolution, candidate studio scoring, OAuth client credentials,
//! proxy API cascades, and audio payload verification for Syncify.

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

pub use syncify_core_domain::byte_validators::AudioByteValidator;
pub use syncify_core_domain::errors::{PipelineError, RequiresAuthReason};
pub use syncify_core_domain::events::{PipelineProgressEvent, PipelineStepStatus};
pub use syncify_core_domain::manifest::TrackManifestEntry;
pub use syncify_core_domain::metadata::{
    artist_matches, clean_title, score_tidal_candidate, score_tidal_release, title_matches,
    TidalAlbum, TidalArtist, TidalMediaMetadata, TidalSearchResponse, TidalSearchTracks,
    TidalTrack,
};
pub use syncify_core_domain::quality::{
    QualityClass, QualityPolicy, StreamResolution, StreamSourceType,
};

/// Alias for compatibility
pub type TidalStreamResolution = StreamResolution;

/// Tidal Authentication Status Hierarchy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TidalAuthStatus {
    /// Valid user token provided or stored for private library access
    UserToken(String),
    /// OAuth Client Credentials token acquired for public catalog access
    ClientCredentials(String),
    /// Authentication required but not available
    RequiresAuth,
    /// Tidal API service unavailable
    SourceUnavailable(String),
    /// General failure state
    Failed(String),
}

impl TidalAuthStatus {
    pub fn is_user_authenticated(&self) -> bool {
        matches!(self, TidalAuthStatus::UserToken(_))
    }

    pub fn can_access_public_catalog(&self) -> bool {
        matches!(
            self,
            TidalAuthStatus::UserToken(_) | TidalAuthStatus::ClientCredentials(_)
        )
    }
}

/// Resolution state of GUI account credentials loaded from SQLite
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TidalAuthResolution {
    StoredGuiAccessToken(String),
    RefreshedGuiToken(String),
    ExplicitOverrideToken(String),
    RequiresAuth,
    SourceUnavailable(String),
}

impl std::fmt::Display for TidalAuthResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TidalAuthResolution::StoredGuiAccessToken(_) => write!(f, "Stored GUI Access Token"),
            TidalAuthResolution::RefreshedGuiToken(_) => write!(f, "Refreshed GUI Token"),
            TidalAuthResolution::ExplicitOverrideToken(_) => write!(f, "Explicit Override Token"),
            TidalAuthResolution::RequiresAuth => write!(f, "Requires Authentication"),
            TidalAuthResolution::SourceUnavailable(reason) => {
                write!(f, "Source Unavailable ({})", reason)
            }
        }
    }
}

/// Decrypted structure stored in `accounts.credentials_json` by Syncify GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalGuiCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_expiry: Option<f64>,
    pub expires_at: Option<f64>,
    pub expires_in: Option<f64>,
    pub user_id: Option<serde_json::Value>,
    pub country_code: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

impl TidalGuiCredentials {
    pub fn get_client_id(&self) -> &str {
        if let Some(ref cid) = self.client_id {
            if !cid.trim().is_empty() {
                return cid.as_str();
            }
        }
        // Default Device Code OAuth client_id used by tidal_auth.py
        "fX2JxdmntZWK0ixT"
    }

    pub fn get_client_secret(&self) -> &str {
        if let Some(ref cs) = self.client_secret {
            if !cs.trim().is_empty() {
                return cs.as_str();
            }
        }
        "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8="
    }

    pub fn get_expiry_timestamp(&self) -> Option<f64> {
        if let Some(exp) = self.token_expiry {
            Some(exp)
        } else if let Some(exp) = self.expires_at {
            Some(exp)
        } else {
            None
        }
    }

    pub fn is_expired(&self, now_secs: f64) -> bool {
        if let Some(exp) = self.get_expiry_timestamp() {
            now_secs >= exp - 60.0
        } else {
            self.refresh_token.is_some()
        }
    }
}


/// Mask sensitive identifiers (tokens, client IDs, account IDs) for safe structured logging.
pub fn anonymize_identifier(val: &str) -> String {
    let s = val.trim();
    if s.is_empty() {
        "none".to_string()
    } else if s.len() <= 6 {
        "***".to_string()
    } else {
        format!("{}...{}", &s[..3], &s[s.len().saturating_sub(3)..])
    }
}

/// Refresh an expired Tidal access token using exact client_id and client_secret from credentials.
pub async fn refresh_gui_token(
    client: &reqwest::Client,
    creds: &TidalGuiCredentials,
) -> Result<(String, TidalGuiCredentials), PipelineError> {
    let refresh_tok = creds
        .refresh_token
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| PipelineError::RequiresAuth(RequiresAuthReason::NoCredentialsStored))?;

    let client_id = creds.get_client_id();
    let client_secret = creds.get_client_secret();
    let url = "https://auth.tidal.com/v1/oauth2/token";

    let params = [
        ("client_id", client_id),
        ("refresh_token", refresh_tok),
        ("grant_type", "refresh_token"),
        ("scope", "r_usr+w_usr+w_sub"),
    ];

    let resp = client
        .post(url)
        .basic_auth(client_id, Some(client_secret))
        .form(&params)
        .send()
        .await
        .map_err(|e| PipelineError::NetworkError {
            provider: "tidal".to_string(),
            endpoint: "oauth2_token_refresh".to_string(),
            message: e.to_string(),
        })?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        if status.as_u16() == 401 || status.as_u16() == 400 {
            return Err(PipelineError::RequiresAuth(RequiresAuthReason::TokenExpired));
        }
        return Err(PipelineError::SourceUnavailable {
            provider: "tidal".to_string(),
            message: format!("Token refresh HTTP {}: {}", status, text),
        });
    }

    let json_val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| PipelineError::InternalError(format!("Failed to parse token refresh JSON: {}", e)))?;

    let access_token = json_val["access_token"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| PipelineError::RequiresAuth(RequiresAuthReason::InvalidPayload))?
        .to_string();

    let new_refresh_token = json_val["refresh_token"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .or_else(|| creds.refresh_token.clone());

    let expires_in = json_val["expires_in"].as_f64().unwrap_or(14400.0);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let token_expiry = now_secs + expires_in;

    let updated_creds = TidalGuiCredentials {
        access_token: access_token.clone(),
        refresh_token: new_refresh_token,
        token_expiry: Some(token_expiry),
        expires_at: Some(token_expiry),
        expires_in: Some(expires_in),
        user_id: json_val.get("user").and_then(|u| u.get("userId")).cloned().or_else(|| creds.user_id.clone()),
        country_code: json_val.get("user").and_then(|u| u.get("countryCode")).and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| creds.country_code.clone()),
        client_id: creds.client_id.clone(),
        client_secret: creds.client_secret.clone(),
    };

    Ok((access_token, updated_creds))
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedTidalManifest {
    pub stream_url: String,
    pub mime_type: Option<String>,
    pub codecs: Option<String>,
    pub codec: String, // "FLAC" | "AAC" | "MP3"
    pub quality_class: QualityClass, // Lossless | Lossy
    pub format_id_obtained: String, // "HI_RES_LOSSLESS" | "LOSSLESS" | "HIGH"
    pub container: String, // "FLAC" | "M4A" | "MP3"
    pub extension: String, // "flac" | "m4a" | "mp3"
    pub bit_depth: i32,
    pub sample_rate: f64,
    pub is_dash: bool,
}

/// Robust parser for Tidal playback info manifests (BTS base64 JSON, MPEG-DASH XML, and direct URLs)
pub fn parse_tidal_playback_manifest(
    raw_response_text: &str,
    target_quality_param: &str,
) -> Result<ParsedTidalManifest, anyhow::Error> {
    let mut resolved_url: Option<String> = None;
    let mut detected_mime: Option<String> = None;
    let mut detected_codecs: Option<String> = None;
    let mut is_dash = false;

    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(raw_response_text) {
        if let Some(u) = json_val["url"].as_str() {
            resolved_url = Some(u.to_string());
        } else if let Some(arr) = json_val["urls"].as_array() {
            if let Some(u) = arr.first().and_then(|v| v.as_str()) {
                resolved_url = Some(u.to_string());
            }
        } else if let Some(b64_manifest) = json_val["manifest"].as_str() {
            if let Ok(decoded_bytes) = BASE64.decode(b64_manifest) {
                if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                    if let Ok(m_json) = serde_json::from_str::<serde_json::Value>(&decoded_str) {
                        if let Some(m) = m_json["mimeType"].as_str() {
                            detected_mime = Some(m.to_lowercase());
                        }
                        if let Some(c) = m_json["codecs"].as_str() {
                            detected_codecs = Some(c.to_lowercase());
                        }
                        if let Some(u) = m_json["urls"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()) {
                            resolved_url = Some(u.to_string());
                        }
                    }

                    if resolved_url.is_none() {
                        if decoded_str.contains("<MPD") || decoded_str.contains("<?xml") {
                            is_dash = true;
                            if decoded_str.contains("codecs=\"flac\"") || decoded_str.contains("codecs=\"fLaC\"") || decoded_str.contains("FLAC") {
                                detected_codecs = Some("flac".to_string());
                                detected_mime = Some("audio/flac".to_string());
                            } else if decoded_str.contains("codecs=\"mp4a") {
                                detected_codecs = Some("mp4a.40.2".to_string());
                                detected_mime = Some("audio/mp4".to_string());
                            }

                            let mut init_url_opt: Option<&str> = None;
                            let mut media_tmpl_opt: Option<&str> = None;
                            let mut total_segs: u32 = 0;

                            if let Some(init_idx) = decoded_str.find("initialization=\"http") {
                                let start = init_idx + "initialization=\"".len();
                                if let Some(end) = decoded_str[start..].find('"') {
                                    init_url_opt = Some(&decoded_str[start..start + end]);
                                }
                            }

                            if let Some(media_idx) = decoded_str.find("media=\"http") {
                                let start = media_idx + "media=\"".len();
                                if let Some(end) = decoded_str[start..].find('"') {
                                    media_tmpl_opt = Some(&decoded_str[start..start + end]);
                                }
                            }

                            let mut pos = 0;
                            while let Some(s_idx) = decoded_str[pos..].find("<S ") {
                                let abs_s = pos + s_idx;
                                if let Some(close_idx) = decoded_str[abs_s..].find('>') {
                                    let tag_str = &decoded_str[abs_s..abs_s + close_idx];
                                    let repeat_count = if let Some(r_idx) = tag_str.find("r=\"") {
                                        let r_start = r_idx + "r=\"".len();
                                        tag_str[r_start..].split('"').next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
                                    } else {
                                        0
                                    };
                                    total_segs += repeat_count + 1;
                                    pos = abs_s + close_idx + 1;
                                } else {
                                    break;
                                }
                            }

                            if let (Some(init_u), Some(media_u)) = (init_url_opt, media_tmpl_opt) {
                                if total_segs == 0 { total_segs = 1; }
                                resolved_url = Some(format!("DASH_MANIFEST|{}|{}|{}", init_u, media_u, total_segs));
                            } else if let Some(init_u) = init_url_opt {
                                resolved_url = Some(init_u.to_string());
                            }
                        }
                    }

                    if resolved_url.is_none() {
                        for line in decoded_str.lines() {
                            let tr = line.trim();
                            if tr.starts_with("http://") || tr.starts_with("https://") {
                                resolved_url = Some(tr.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let stream_url = resolved_url.ok_or_else(|| anyhow!("Failed to extract audio stream URL from Tidal manifest"))?;
    let mime_str = detected_mime.as_deref().unwrap_or("");
    let codec_str = detected_codecs.as_deref().unwrap_or("");

    let is_flac = codec_str == "flac" || codec_str == "flac" || mime_str == "audio/flac" || mime_str == "audio/x-flac" || stream_url.ends_with(".flac");
    let is_mp4_aac = !is_flac && (mime_str == "audio/mp4" || codec_str.starts_with("mp4a") || codec_str.starts_with("aac") || stream_url.contains(".m4a") || stream_url.contains(".mp4"));
    let is_mp3 = !is_flac && !is_mp4_aac && (mime_str == "audio/mpeg" || codec_str == "mp3" || stream_url.contains(".mp3"));

    let (codec, container, extension, quality_class, format_id_obtained, bit_depth, sample_rate) = if is_flac {
        let is_hi_res = target_quality_param == "HI_RES_LOSSLESS" || is_dash;
        (
            "FLAC".to_string(),
            "FLAC".to_string(),
            "flac".to_string(),
            QualityClass::Lossless,
            if is_hi_res { "HI_RES_LOSSLESS".to_string() } else { "LOSSLESS".to_string() },
            if is_hi_res { 24 } else { 16 },
            if is_hi_res { 96000.0 } else { 44100.0 },
        )
    } else if is_mp4_aac {
        (
            "AAC".to_string(),
            "M4A".to_string(),
            "m4a".to_string(),
            QualityClass::Lossy,
            "HIGH".to_string(),
            16,
            44100.0,
        )
    } else if is_mp3 {
        (
            "MP3".to_string(),
            "MP3".to_string(),
            "mp3".to_string(),
            QualityClass::Lossy,
            "HIGH".to_string(),
            16,
            44100.0,
        )
    } else {
        if target_quality_param == "HIGH" || target_quality_param == "LOW" {
            (
                "AAC".to_string(),
                "M4A".to_string(),
                "m4a".to_string(),
                QualityClass::Lossy,
                "HIGH".to_string(),
                16,
                44100.0,
            )
        } else {
            (
                "FLAC".to_string(),
                "FLAC".to_string(),
                "flac".to_string(),
                QualityClass::Lossless,
                "LOSSLESS".to_string(),
                16,
                44100.0,
            )
        }
    };

    Ok(ParsedTidalManifest {
        stream_url,
        mime_type: detected_mime,
        codecs: detected_codecs,
        codec,
        quality_class,
        format_id_obtained,
        container,
        extension,
        bit_depth,
        sample_rate,
        is_dash,
    })
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

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
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

    /// Read-only access to user token if set
    pub fn user_token(&self) -> Option<String> {
        self.user_token.read().unwrap().clone()
    }


    /// Read-only access to the internal HTTP client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Refresh an expired Tidal access token using the downloader's HTTP client
    pub async fn refresh_gui_token(
        &self,
        creds: &TidalGuiCredentials,
    ) -> Result<(String, TidalGuiCredentials), PipelineError> {
        refresh_gui_token(&self.client, creds).await
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
    /// Explicit Override -> User Token -> OAuth ClientCredentials -> RequiresAuth / SourceUnavailable
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

        if let Ok(env_tok) = std::env::var("TIDAL_USER_TOKEN") {
            let clean = env_tok.trim().trim_matches('"').trim_matches('\'').to_string();
            if !clean.is_empty() {
                return TidalAuthStatus::UserToken(clean);
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
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
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
        let client_creds_token = self.get_access_token().await.ok();
        let user_tok = match self.check_auth_status(None).await {
            TidalAuthStatus::UserToken(t) => Some(t),
            _ => None,
        };

        let search_tokens = match (client_creds_token, user_tok) {
            (Some(cc), Some(ut)) => vec![cc, ut],
            (Some(cc), None) => vec![cc],
            (None, Some(ut)) => vec![ut],
            (None, None) => return Err(anyhow!("Tidal authentication required for search")),
        };

        let query = format!("{} {}", artist_name, track_name);
        let mut candidate_tracks: Vec<TidalTrack> = Vec::new();

        for token in &search_tokens {
            let official_urls = [
                format!("https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US", urlencoding::encode(&query)),
                format!("https://api.tidal.com/v1/search?query={}&types=TRACKS&limit=50&countryCode=US", urlencoding::encode(&query)),
            ];

            for official_url in &official_urls {
                if let Ok(response) = self
                    .client
                    .get(official_url)
                    .timeout(Duration::from_secs(5))
                    .header("Authorization", format!("Bearer {}", token))
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        if let Ok(result) = response.json::<TidalSearchResponse>().await {
                            if let Some(tracks) = result.tracks {
                                if !tracks.items.is_empty() {
                                    candidate_tracks = tracks.items;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if !candidate_tracks.is_empty() {
                break;
            }
        }

        // 2. If official search yielded no items, cascade through proxy search APIs with 2s timeout
        if candidate_tracks.is_empty() {
            let apis = Self::get_proxy_apis();
            for api in apis {
                let proxy_search_url = format!("{}/search?query={}&type=tracks", api, urlencoding::encode(&query));
                if let Ok(response) = self
                    .client
                    .get(&proxy_search_url)
                    .timeout(Duration::from_secs(2))
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        let text = response.text().await.unwrap_or_default();
                        if let Ok(result) = serde_json::from_str::<TidalSearchResponse>(&text) {
                            if let Some(tracks) = result.tracks {
                                if !tracks.items.is_empty() {
                                    candidate_tracks = tracks.items;
                                    break;
                                }
                            }
                        }
                        if let Ok(items) = serde_json::from_str::<Vec<TidalTrack>>(&text) {
                            if !items.is_empty() {
                                candidate_tracks = items;
                                break;
                            }
                        }
                    }
                }
            }
        }

        if candidate_tracks.is_empty() {
            return Err(anyhow!("No matching tracks found on Tidal for: {} - {}", artist_name, track_name));
        }

        let mut best_track: Option<TidalTrack> = None;
        let mut best_score: i32 = i32::MIN;

        for track in &candidate_tracks {
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

        if let Some(first_track) = candidate_tracks.first() {
            info!("[Tidal] Selected top candidate track fallback: '{}'", first_track.title);
            return Ok(first_track.clone());
        }

        Err(anyhow!(
            "No matching track found on Tidal for: {} - {}",
            artist_name,
            track_name
        ))
    }

    pub async fn get_stream_resolution(
        &self,
        track_id: i64,
        quality_opt: Option<&str>,
        user_token_opt: Option<&str>,
        allow_lossy_fallback: bool,
    ) -> Result<TidalStreamResolution> {
        let creds = user_token_opt.map(|tok| TidalGuiCredentials {
            access_token: tok.to_string(),
            refresh_token: None,
            token_expiry: None,
            expires_at: None,
            expires_in: None,
            user_id: None,
            country_code: None,
            client_id: None,
            client_secret: None,
        });

        self.get_stream_resolution_with_credentials(
            track_id,
            quality_opt,
            creds.as_ref(),
            allow_lossy_fallback,
        ).await
    }

    pub async fn get_stream_resolution_with_credentials(
        &self,
        track_id: i64,
        quality_opt: Option<&str>,
        creds_opt: Option<&TidalGuiCredentials>,
        allow_lossy_fallback: bool,
    ) -> Result<TidalStreamResolution> {
        let requested_q = quality_opt.unwrap_or("24-192");
        let target_quality_param = match requested_q {
            "24-192" | "24-96" | "HI_RES_LOSSLESS" | "HI_RES" => "HI_RES_LOSSLESS",
            "16-44" | "LOSSLESS" => "LOSSLESS",
            "320" | "HIGH" | "LOSSY" => "HIGH",
            _ => "HI_RES_LOSSLESS",
        };

        let quality_class_requested = match requested_q {
            "320" | "HIGH" | "LOSSY" => QualityClass::Lossy,
            _ => QualityClass::Lossless,
        };

        let effective_creds: Option<TidalGuiCredentials> = if creds_opt.is_some() {
            creds_opt.cloned()
        } else if let Some(ref tok) = *self.user_token.read().unwrap() {
            if !tok.trim().is_empty() {
                Some(TidalGuiCredentials {
                    access_token: tok.clone(),
                    refresh_token: None,
                    token_expiry: None,
                    expires_at: None,
                    expires_in: None,
                    user_id: None,
                    country_code: None,
                    client_id: None,
                    client_secret: None,
                })
            } else {
                None
            }
        } else {
            None
        };

        if effective_creds.is_none() && quality_class_requested == QualityClass::Lossless {
            return Err(anyhow!(
                "RequiresAuth: No active Tidal user session available; Lossless playback requires an authenticated Tidal account"
            ));
        }

        let client_id_raw = effective_creds.as_ref().map(|c| c.get_client_id()).unwrap_or(&self.client_id);
        let client_id_anon = anonymize_identifier(client_id_raw);
        let account_id_anon = effective_creds
            .as_ref()
            .and_then(|c| c.user_id.as_ref())
            .map(|u| anonymize_identifier(&u.to_string()))
            .unwrap_or_else(|| "none".to_string());
        let country_code = effective_creds
            .as_ref()
            .and_then(|c| c.country_code.as_deref())
            .unwrap_or("US");

        // 1. Try Official Tidal API endpoints if user credentials / token is present
        if let Some(ref creds) = effective_creds {
            let user_tok = &creds.access_token;
            let official_endpoints = vec![
                (
                    "playbackinfopostpaywall",
                    format!(
                        "https://api.tidal.com/v1/tracks/{}/playbackinfopostpaywall?audioquality={}&playbackmode=STREAM&assetpresentation=FULL&countryCode={}",
                        track_id, target_quality_param, country_code
                    ),
                ),
                (
                    "streamUrl",
                    format!(
                        "https://api.tidal.com/v1/tracks/{}/streamUrl?soundQuality={}&countryCode={}",
                        track_id, target_quality_param, country_code
                    ),
                ),
                (
                    "url",
                    format!(
                        "https://api.tidal.com/v1/tracks/{}/url?soundQuality={}&countryCode={}",
                        track_id, target_quality_param, country_code
                    ),
                ),
            ];

            let mut last_auth_error: Option<String> = None;


            for (endpoint_name, official_url) in &official_endpoints {
                match self.client.get(official_url)
                    .header("Authorization", format!("Bearer {}", user_tok))
                    .header("X-Tidal-SessionId", user_tok)
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();

                        if status.is_success() {
                            if let Ok(parsed) = parse_tidal_playback_manifest(&text, target_quality_param) {
                                QualityPolicy::evaluate_downgrade(quality_class_requested, parsed.quality_class, &parsed.codec, allow_lossy_fallback)
                                    .map_err(|e| anyhow!(e))?;

                                let obtained_q = if parsed.quality_class == QualityClass::Lossy {
                                    "320"
                                } else if parsed.format_id_obtained == "HI_RES_LOSSLESS" {
                                    "24-192"
                                } else {
                                    "16-44"
                                };
                                let is_fallback = obtained_q != requested_q;

                                info!(
                                    account_id_anon = %account_id_anon,
                                    provider = "tidal",
                                    track_id = track_id,
                                    region = %country_code,
                                    requested_quality = requested_q,
                                    client_id_anon = %client_id_anon,
                                    endpoint = endpoint_name,
                                    http_status = status.as_u16(),
                                    audio_quality = target_quality_param,
                                    manifest_mime_type = %parsed.mime_type.as_deref().unwrap_or("direct"),
                                    final_error_classification = "None",
                                    "[Tidal] Stream URL resolved successfully via Official Tidal API"
                                );

                                return Ok(TidalStreamResolution {
                                    url: parsed.stream_url,
                                    source: StreamSourceType::TidalOfficial,
                                    source_name: "Tidal Official API".to_string(),
                                    requested_quality: requested_q.to_string(),
                                    obtained_quality: obtained_q.to_string(),
                                    format_id_requested: target_quality_param.to_string(),
                                    format_id_obtained: parsed.format_id_obtained,
                                    quality_class_requested,
                                    quality_class_obtained: parsed.quality_class,
                                    codec: parsed.codec,
                                    container: parsed.container,
                                    extension: parsed.extension,
                                    bit_depth: parsed.bit_depth,
                                    sample_rate: parsed.sample_rate,
                                    is_fallback,
                                });
                            }
                        } else {
                            let is_401 = status.as_u16() == 401;
                            let substatus = if text.contains("11002") {
                                Some("11002".to_string())
                            } else if text.contains("11003") {
                                Some("11003".to_string())
                            } else {
                                None
                            };

                            let classification = if is_401 {
                                "PlaybackUnauthorized"
                            } else {
                                "SourceUnavailable"
                            };

                            warn!(
                                account_id_anon = %account_id_anon,
                                provider = "tidal",
                                track_id = track_id,
                                region = %country_code,
                                requested_quality = requested_q,
                                client_id_anon = %client_id_anon,
                                endpoint = endpoint_name,
                                http_status = status.as_u16(),
                                audio_quality = target_quality_param,
                                manifest_mime_type = "none",
                                final_error_classification = classification,
                                "[Tidal] Official stream endpoint failed"
                            );

                            if is_401 {
                                last_auth_error = Some(format!(
                                    "PlaybackUnauthorized: HTTP 401 on {} (subStatus: {:?}): {}",
                                    endpoint_name, substatus, text.chars().take(150).collect::<String>()
                                ));
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            account_id_anon = %account_id_anon,
                            provider = "tidal",
                            track_id = track_id,
                            region = %country_code,
                            requested_quality = requested_q,
                            client_id_anon = %client_id_anon,
                            endpoint = endpoint_name,
                            http_status = 0,
                            audio_quality = target_quality_param,
                            manifest_mime_type = "none",
                            final_error_classification = "NetworkError",
                            "[Tidal] Network error requesting official stream: {}", e
                        );
                    }
                }
            }

            if let Some(err_msg) = last_auth_error {
                return Err(anyhow!("{}", err_msg));
            }
            return Err(anyhow!(
                "SourceUnavailable: Official Tidal playback endpoints failed to return a valid stream URL for track_id {}",
                track_id
            ));
        }

        // 2. Cascade through Proxy APIs (only for non-authenticated / lossy public access)
        let apis = Self::get_proxy_apis();
        if apis.is_empty() {
            return Err(anyhow!("RequiresAuth: No active Tidal user session available and proxy cascade list is empty"));
        }


        debug!("[Tidal] Resolving stream URL via proxy cascade for track_id {} (requested: {})", track_id, requested_q);

        for api in &apis {
            let domain = api.replace("https://", "");
            let url = format!("{}/track/{}?quality={}", api, track_id, target_quality_param);

            let result = self
                .client
                .get(&url)
                .timeout(Duration::from_secs(4))
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
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

                    let mut resolved_url: Option<String> = None;
                    if let Ok(manifest) = serde_json::from_str::<BTSManifest>(trimmed) {
                        if !manifest.urls.is_empty() {
                            resolved_url = Some(manifest.urls[0].clone());
                        }
                    }

                    if resolved_url.is_none() {
                        if let Ok(direct) = serde_json::from_str::<DirectUrl>(trimmed) {
                            if !direct.url.trim().is_empty() {
                                resolved_url = Some(direct.url.trim().to_string());
                            }
                        }
                    }

                    if resolved_url.is_none() {
                        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                            resolved_url = Some(trimmed.to_string());
                        }
                    }

                    if let Some(stream_url) = resolved_url {
                        let is_mp3 = target_quality_param == "HIGH" || stream_url.contains(".mp3");
                        let is_m4a = stream_url.contains(".m4a") || stream_url.contains(".mp4");
                        let final_codec = if is_mp3 {
                            "MP3".to_string()
                        } else if is_m4a {
                            "AAC".to_string()
                        } else {
                            "FLAC".to_string()
                        };

                        let quality_class_obtained = if final_codec == "FLAC" {
                            QualityClass::Lossless
                        } else {
                            QualityClass::Lossy
                        };

                        if quality_class_requested == QualityClass::Lossless && quality_class_obtained == QualityClass::Lossy && !allow_lossy_fallback {
                            return Err(anyhow!("Quality rejection: requested_lossless_but_received_{}", final_codec.to_lowercase()));
                        }

                        let container = if final_codec == "AAC" {
                            "M4A".to_string()
                        } else if final_codec == "MP3" {
                            "MP3".to_string()
                        } else {
                            "FLAC".to_string()
                        };

                        let extension = if final_codec == "AAC" {
                            "m4a".to_string()
                        } else if final_codec == "MP3" {
                            "mp3".to_string()
                        } else {
                            "flac".to_string()
                        };

                        let obtained_q = if quality_class_obtained == QualityClass::Lossy { "320" } else if target_quality_param == "HI_RES_LOSSLESS" { "24-192" } else { "16-44" };
                        let is_fallback = obtained_q != requested_q;

                        info!("[Tidal] Stream URL resolved via TidalProxy ({})", domain);

                        return Ok(TidalStreamResolution {
                            url: stream_url,
                            source: StreamSourceType::TidalProxy(domain.clone()),
                            source_name: format!("Tidal Proxy ({})", domain),
                            requested_quality: requested_q.to_string(),
                            obtained_quality: obtained_q.to_string(),
                            format_id_requested: target_quality_param.to_string(),
                            format_id_obtained: target_quality_param.to_string(),
                            quality_class_requested,
                            quality_class_obtained,
                            codec: final_codec,
                            container,
                            extension,
                            bit_depth: if quality_class_obtained == QualityClass::Lossy { 16 } else if target_quality_param == "HI_RES_LOSSLESS" { 24 } else { 16 },
                            sample_rate: if quality_class_obtained == QualityClass::Lossy { 44100.0 } else if target_quality_param == "HI_RES_LOSSLESS" { 96000.0 } else { 44100.0 },
                            is_fallback,
                        });

                    }
                }
                Err(e) => {
                    debug!("[Tidal] Connection error to proxy API {}: {}", api, e);
                }
            }
        }

        Err(anyhow!("Failed to obtain stream URL for Tidal track ID {} from official & proxy APIs", track_id))
    }

    /// Download stream audio payload to disk with strict chunk & format header validation
    pub async fn download_audio_payload(
        &self,
        stream_url: &str,
        output_path: &Path,
    ) -> Result<u64> {
        self.download_audio_payload_with_progress(stream_url, output_path, |_, _, _| {}).await
    }

    /// Download stream audio payload with per-segment progress reporting and strict error classification
    pub async fn download_audio_payload_with_progress<P>(
        &self,
        stream_url: &str,
        output_path: &Path,
        progress_callback: P,
    ) -> Result<u64>
    where
        P: Fn(u32, u32, u64) + Send + Sync + 'static,
    {
        const TOTAL_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
        const SEGMENT_TIMEOUT: Duration = Duration::from_secs(15);
        const MAX_SEGMENT_RETRIES: usize = 3;

        let total_download_future = async {
            let temp_file_path = output_path.with_extension("stream.tmp");
            if let Some(parent) = temp_file_path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            let mut downloaded: u64 = 0;

            if stream_url.starts_with("DASH_MANIFEST|") {
                let parts: Vec<&str> = stream_url.split('|').collect();
                if parts.len() < 4 {
                    return Err(anyhow!("Invalid DASH manifest stream URL spec"));
                }
                let init_url = parts[1];
                let media_template = parts[2];
                let total_segments: u32 = parts[3].parse().unwrap_or(1);

                info!(
                    total_segments = total_segments,
                    "[Tidal DASH] Starting DASH MPD stream download"
                );
                let mut file = File::create(&temp_file_path).await?;

                // 1. Download Init Segment with retries
                let mut init_downloaded = false;
                let mut last_init_err = String::new();
                for attempt in 1..=MAX_SEGMENT_RETRIES {
                    let init_start = Instant::now();
                    let init_res = tokio::time::timeout(SEGMENT_TIMEOUT, async {
                        let resp = self.client.get(init_url).send().await?;
                        let status = resp.status();
                        if !status.is_success() {
                            return Err(anyhow!("HTTP {}", status));
                        }
                        let bytes = resp.bytes().await?;
                        Ok((status, bytes))
                    }).await;

                    match init_res {
                        Ok(Ok((status, bytes))) => {
                            let seg_bytes_len = bytes.len();
                            file.write_all(&bytes).await?;
                            downloaded += seg_bytes_len as u64;
                            info!(
                                segment_idx = 0,
                                total_segments = total_segments,
                                http_status = status.as_u16(),
                                bytes = seg_bytes_len,
                                elapsed_ms = init_start.elapsed().as_millis(),
                                retries = attempt - 1,
                                "[Tidal DASH] Init segment downloaded successfully"
                            );
                            progress_callback(0, total_segments, downloaded);
                            init_downloaded = true;
                            break;
                        }
                        Ok(Err(e)) => {
                            last_init_err = format!("Attempt {}: {}", attempt, e);
                            warn!(
                                segment_idx = 0,
                                attempt = attempt,
                                error = %e,
                                "[Tidal DASH] Init segment attempt failed; retrying"
                            );
                        }
                        Err(_) => {
                            last_init_err = format!("Attempt {}: timed out after {:?}", attempt, SEGMENT_TIMEOUT);
                            warn!(
                                segment_idx = 0,
                                attempt = attempt,
                                "[Tidal DASH] Init segment attempt timed out; retrying"
                            );
                        }
                    }

                    if attempt < MAX_SEGMENT_RETRIES {
                        tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                    }
                }

                if !init_downloaded {
                    let _ = tokio::fs::remove_file(&temp_file_path).await;
                    return Err(anyhow!(
                        "SegmentDownloadFailed: DASH init segment download failed after {} retries: {}",
                        MAX_SEGMENT_RETRIES, last_init_err
                    ));
                }

                // 2. Download Media Segments with structured logging & retries
                for seg_num in 1..=total_segments {
                    let seg_url = media_template.replace("$Number$", &seg_num.to_string());
                    let mut seg_success = false;
                    let mut last_seg_err = String::new();

                    for attempt in 1..=MAX_SEGMENT_RETRIES {
                        let seg_start = Instant::now();
                        let seg_res = tokio::time::timeout(SEGMENT_TIMEOUT, async {
                            let resp = self.client.get(&seg_url).send().await?;
                            let status = resp.status();
                            if !status.is_success() {
                                return Err(anyhow!("HTTP {}", status));
                            }
                            let bytes = resp.bytes().await?;
                            Ok((status, bytes))
                        }).await;

                        match seg_res {
                            Ok(Ok((status, bytes))) => {
                                let seg_bytes_len = bytes.len();
                                file.write_all(&bytes).await?;
                                downloaded += seg_bytes_len as u64;
                                info!(
                                    segment_idx = seg_num,
                                    total_segments = total_segments,
                                    http_status = status.as_u16(),
                                    bytes = seg_bytes_len,
                                    elapsed_ms = seg_start.elapsed().as_millis(),
                                    retries = attempt - 1,
                                    "[Tidal DASH] Segment downloaded successfully"
                                );
                                progress_callback(seg_num, total_segments, downloaded);
                                seg_success = true;
                                break;
                            }
                            Ok(Err(e)) => {
                                last_seg_err = format!("Attempt {}: {}", attempt, e);
                                warn!(
                                    segment_idx = seg_num,
                                    attempt = attempt,
                                    error = %e,
                                    "[Tidal DASH] Segment attempt failed; retrying"
                                );
                            }
                            Err(_) => {
                                last_seg_err = format!("Attempt {}: timed out after {:?}", attempt, SEGMENT_TIMEOUT);
                                warn!(
                                    segment_idx = seg_num,
                                    attempt = attempt,
                                    "[Tidal DASH] Segment attempt timed out; retrying"
                                );
                            }
                        }

                        if attempt < MAX_SEGMENT_RETRIES {
                            tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                        }
                    }

                    if !seg_success {
                        let _ = tokio::fs::remove_file(&temp_file_path).await;
                        return Err(anyhow!(
                            "SegmentDownloadFailed: segment {}/{} failed after {} retries: {}",
                            seg_num, total_segments, MAX_SEGMENT_RETRIES, last_seg_err
                        ));
                    }
                }

                file.flush().await?;
                drop(file);
            } else {
                let mut file = File::create(&temp_file_path).await?;
                let mut resp = self.client.get(stream_url).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Tidal stream download failed: HTTP {}", resp.status()));
                }

                while let Some(chunk) = resp.chunk().await? {
                    file.write_all(&chunk).await?;
                    downloaded += chunk.len() as u64;
                    progress_callback(1, 1, downloaded);
                }

                file.flush().await?;
                drop(file);
            }

            if downloaded == 0 {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow!("ValidationFailed: Tidal downloaded file payload is zero bytes"));
            }

            let header_bytes = tokio::fs::read(&temp_file_path).await.unwrap_or_default();
            if header_bytes.len() < 4 {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow!("ValidationFailed: Downloaded file is too small to contain valid audio headers"));
            }

            let ext_str = output_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_flac_path = ext_str == "flac";
            let is_mp3_path = ext_str == "mp3";
            let is_m4a_path = ext_str == "m4a" || ext_str == "mp4";

            if is_flac_path && !AudioByteValidator::is_flac_magic(&header_bytes) && !AudioByteValidator::is_isobmff_container(&header_bytes) {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow!("ValidationFailed: Downloaded file fails FLAC magic header verification ('fLaC' or ISOBMFF expected)"));
            }

            if is_mp3_path && !AudioByteValidator::is_mp3_magic(&header_bytes) {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow!("ValidationFailed: Downloaded file fails MP3 frame header verification"));
            }

            if is_m4a_path && !AudioByteValidator::is_m4a_magic(&header_bytes) {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow!("ValidationFailed: Downloaded file fails MP4/AAC magic header verification ('ftyp' expected)"));
            }

            let is_isobmff = AudioByteValidator::is_isobmff_container(&header_bytes);
            if is_flac_path && is_isobmff {
                info!("[Tidal] Remuxing ISOBMFF FLAC container to native FLAC container via ffmpeg...");
                let native_temp_path = output_path.with_extension("native.tmp");
                let remux_output = tokio::process::Command::new("ffmpeg")
                    .args(&[
                        "-y",
                        "-i", temp_file_path.to_str().unwrap_or(""),
                        "-c:a", "copy",
                        "-f", "flac",
                        native_temp_path.to_str().unwrap_or(""),
                    ])
                    .output()
                    .await;

                match remux_output {
                    Ok(out) if out.status.success() && native_temp_path.exists() => {
                        let native_bytes = tokio::fs::read(&native_temp_path).await.unwrap_or_default();
                        if !AudioByteValidator::is_flac_magic(&native_bytes) {
                            let _ = tokio::fs::remove_file(&native_temp_path).await;
                            let _ = tokio::fs::remove_file(&temp_file_path).await;
                            return Err(anyhow!("RemuxError: ffmpeg output is not a valid native FLAC bitstream"));
                        }
                        let _ = tokio::fs::remove_file(&temp_file_path).await;
                        tokio::fs::rename(&native_temp_path, output_path).await?;
                        let final_len = tokio::fs::metadata(output_path).await.map(|m| m.len()).unwrap_or(downloaded);
                        info!("[Tidal] Remuxed & saved native FLAC payload: {} bytes -> {}", final_len, output_path.display());
                        return Ok(final_len);
                    }
                    Ok(out) => {
                        let stderr_msg = String::from_utf8_lossy(&out.stderr);
                        let _ = tokio::fs::remove_file(&temp_file_path).await;
                        let _ = tokio::fs::remove_file(&native_temp_path).await;
                        return Err(anyhow!("RemuxError: ffmpeg remuxing failed (exit code {:?}): {}", out.status.code(), stderr_msg));
                    }
                    Err(e) => {
                        let _ = tokio::fs::remove_file(&temp_file_path).await;
                        return Err(anyhow!("RemuxError: failed to invoke ffmpeg: {}", e));
                    }
                }
            }

            tokio::fs::rename(&temp_file_path, output_path).await?;
            info!("[Tidal] Verified & saved audio payload: {} bytes -> {}", downloaded, output_path.display());

            Ok(downloaded)
        };

        match tokio::time::timeout(TOTAL_DOWNLOAD_TIMEOUT, total_download_future).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("NetworkError: Download timed out after {:?}", TOTAL_DOWNLOAD_TIMEOUT)),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tidal_auth_status_hierarchy() {
        let user = TidalAuthStatus::UserToken("secret".to_string());
        assert!(user.is_user_authenticated());
        assert!(user.can_access_public_catalog());

        let client = TidalAuthStatus::ClientCredentials("token".to_string());
        assert!(!client.is_user_authenticated());
        assert!(client.can_access_public_catalog());

        let unauth = TidalAuthStatus::RequiresAuth;
        assert!(!unauth.is_user_authenticated());
        assert!(!unauth.can_access_public_catalog());
    }

    #[test]
    fn test_proxy_api_cascade_list_decoding() {
        let apis = TidalDownloader::get_proxy_apis();
        assert!(!apis.is_empty());
        assert!(apis.iter().any(|u| u.contains("tidal.kinoplus.online")));
        assert!(apis.iter().any(|u| u.contains("triton.squid.wtf")));
    }

    #[test]
    fn test_anonymize_identifier() {
        assert_eq!(anonymize_identifier(""), "none");
        assert_eq!(anonymize_identifier("short"), "***");
        assert_eq!(anonymize_identifier("fX2JxdmntZWK0ixT"), "fX2...ixT");
        assert_eq!(anonymize_identifier("user_secret_account_id"), "use..._id");
    }

    #[test]
    fn test_tidal_gui_credentials_expiry_and_defaults() {
        let creds = TidalGuiCredentials {
            access_token: "test_tok".to_string(),
            refresh_token: Some("refresh_123".to_string()),
            token_expiry: Some(1000.0),
            expires_at: None,
            expires_in: Some(3600.0),
            user_id: None,
            country_code: Some("ES".to_string()),
            client_id: None,
            client_secret: None,
        };

        assert_eq!(creds.get_client_id(), "fX2JxdmntZWK0ixT");
        assert_eq!(creds.get_client_secret(), "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8=");
        assert!(!creds.is_expired(900.0));
        assert!(creds.is_expired(950.0)); // within 60s buffer
        assert!(creds.is_expired(1050.0));
    }

    #[test]
    fn test_fixture_bts_flac_16_44() {
        let bts_payload = r#"{"mimeType":"audio/flac","codecs":"flac","encryptionType":"NONE","urls":["https://sp-pr-cf.audio.tidal.com/data/12345.flac"]}"#;
        let b64_manifest = BASE64.encode(bts_payload);
        let resp_json = format!(
            r#"{{"trackId":80654035,"audioQuality":"LOSSLESS","manifestMimeType":"application/vnd.tidal.bts","manifest":"{}"}}"#,
            b64_manifest
        );

        let parsed = parse_tidal_playback_manifest(&resp_json, "LOSSLESS").expect("Parse BTS FLAC");
        assert_eq!(parsed.codec, "FLAC");
        assert_eq!(parsed.container, "FLAC");
        assert_eq!(parsed.extension, "flac");
        assert_eq!(parsed.quality_class, QualityClass::Lossless);
        assert_eq!(parsed.format_id_obtained, "LOSSLESS");
        assert_eq!(parsed.bit_depth, 16);
        assert_eq!(parsed.sample_rate, 44100.0);
        assert!(!parsed.is_dash);
        assert_eq!(parsed.stream_url, "https://sp-pr-cf.audio.tidal.com/data/12345.flac");

        // Evaluates without downgrade error
        assert!(QualityPolicy::evaluate_downgrade(QualityClass::Lossless, parsed.quality_class, &parsed.codec, false).is_ok());
    }

    #[test]
    fn test_fixture_dash_flac_24_96() {
        let dash_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" minBufferTime="PT2.0S" type="static">
  <Period>
    <AdaptationSet mimeType="audio/mp4" codecs="flac" lang="en">
      <SegmentTemplate timescale="96000" initialization="https://sp-pr-cf.audio.tidal.com/init.mp4" media="https://sp-pr-cf.audio.tidal.com/seg_$Number$.mp4">
        <SegmentTimeline>
          <S d="96000" r="10" />
        </SegmentTimeline>
      </SegmentTemplate>
      <Representation id="1" bandwidth="2800000" audioSamplingRate="96000" />
    </AdaptationSet>
  </Period>
</MPD>"#;
        let b64_manifest = BASE64.encode(dash_xml);
        let resp_json = format!(
            r#"{{"trackId":80654035,"audioQuality":"HI_RES_LOSSLESS","manifestMimeType":"application/dash+xml","manifest":"{}"}}"#,
            b64_manifest
        );

        let parsed = parse_tidal_playback_manifest(&resp_json, "HI_RES_LOSSLESS").expect("Parse DASH FLAC");
        assert_eq!(parsed.codec, "FLAC");
        assert_eq!(parsed.container, "FLAC");
        assert_eq!(parsed.extension, "flac");
        assert_eq!(parsed.quality_class, QualityClass::Lossless);
        assert_eq!(parsed.format_id_obtained, "HI_RES_LOSSLESS");
        assert_eq!(parsed.bit_depth, 24);
        assert_eq!(parsed.sample_rate, 96000.0);
        assert!(parsed.is_dash);
        assert!(parsed.stream_url.starts_with("DASH_MANIFEST|https://sp-pr-cf.audio.tidal.com/init.mp4|https://sp-pr-cf.audio.tidal.com/seg_$Number$.mp4|11"));

        assert!(QualityPolicy::evaluate_downgrade(QualityClass::Lossless, parsed.quality_class, &parsed.codec, false).is_ok());
    }

    #[test]
    fn test_fixture_mp4_aac_320() {
        let bts_payload = r#"{"mimeType":"audio/mp4","codecs":"mp4a.40.2","encryptionType":"NONE","urls":["https://sp-pr-cf.audio.tidal.com/data/12345.m4a"]}"#;
        let b64_manifest = BASE64.encode(bts_payload);
        let resp_json = format!(
            r#"{{"trackId":80654035,"audioQuality":"HIGH","manifestMimeType":"application/vnd.tidal.bts","manifest":"{}"}}"#,
            b64_manifest
        );

        let parsed = parse_tidal_playback_manifest(&resp_json, "HIGH").expect("Parse MP4 AAC");
        assert_eq!(parsed.codec, "AAC");
        assert_eq!(parsed.container, "M4A");
        assert_eq!(parsed.extension, "m4a");
        assert_eq!(parsed.quality_class, QualityClass::Lossy);
        assert_eq!(parsed.format_id_obtained, "HIGH");
        assert_eq!(parsed.bit_depth, 16);
        assert_eq!(parsed.sample_rate, 44100.0);
        assert!(!parsed.is_dash);

        // When HIGH is requested (Lossy), it is accepted
        assert!(QualityPolicy::evaluate_downgrade(QualityClass::Lossy, parsed.quality_class, &parsed.codec, false).is_ok());
    }

    #[test]
    fn test_fixture_ambiguous_high_response_resolves_to_flac_if_manifest_is_flac() {
        // Even if Tidal declares audioQuality: "HIGH" in the outer JSON, if the decoded BTS manifest says "audio/flac",
        // we must NOT assume AAC. It must resolve to FLAC 16/44 Lossless.
        let bts_payload = r#"{"mimeType":"audio/flac","codecs":"flac","encryptionType":"NONE","urls":["https://sp-pr-cf.audio.tidal.com/data/cd_quality.flac"]}"#;
        let b64_manifest = BASE64.encode(bts_payload);
        let resp_json = format!(
            r#"{{"trackId":80654035,"audioQuality":"HIGH","manifestMimeType":"application/vnd.tidal.bts","manifest":"{}"}}"#,
            b64_manifest
        );

        let parsed = parse_tidal_playback_manifest(&resp_json, "HIGH").expect("Parse ambiguous HIGH FLAC");
        assert_eq!(parsed.codec, "FLAC", "Must evaluate actual codec, not commercial HIGH label");
        assert_eq!(parsed.quality_class, QualityClass::Lossless);
        assert_eq!(parsed.format_id_obtained, "LOSSLESS");
    }

    #[test]
    fn test_fixture_real_downgrade_rejection() {
        // Requested LOSSLESS (16-44), but Tidal API returns AAC stream (as happens on track 80654035)
        let bts_payload = r#"{"mimeType":"audio/mp4","codecs":"mp4a.40.2","encryptionType":"NONE","urls":["https://sp-pr-cf.audio.tidal.com/data/lossy.m4a"]}"#;
        let b64_manifest = BASE64.encode(bts_payload);
        let resp_json = format!(
            r#"{{"trackId":80654035,"audioQuality":"HIGH","manifestMimeType":"application/vnd.tidal.bts","manifest":"{}"}}"#,
            b64_manifest
        );

        let parsed = parse_tidal_playback_manifest(&resp_json, "LOSSLESS").expect("Parse Lossy response");
        assert_eq!(parsed.codec, "AAC");
        assert_eq!(parsed.quality_class, QualityClass::Lossy);

        // Strict policy rejection
        let downgrade_eval = QualityPolicy::evaluate_downgrade(QualityClass::Lossless, parsed.quality_class, &parsed.codec, false);
        assert!(downgrade_eval.is_err(), "Must reject lossy AAC when lossless was requested without fallback");
        let err_msg = downgrade_eval.unwrap_err();
        assert!(err_msg.contains("requested_lossless_but_received_aac"), "Error detail: {}", err_msg);
    }
}
