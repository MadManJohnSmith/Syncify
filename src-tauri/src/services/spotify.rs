//! Spotify service - OAuth and library import
//!
//! Handles Spotify authentication and importing liked songs.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::SqlitePool;

/// Helper to deserialize null as empty Vec (Spotify sometimes returns null for arrays)
fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let opt: Option<Vec<T>> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// Helper to deserialize null as empty String (Spotify sometimes returns null for string fields)
fn deserialize_null_as_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// Spotify OAuth configuration
#[derive(Debug, Clone)]
pub struct SpotifyConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

// ═══════════════════════════════════════════════════════
// SPRINT S196: CREDENTIALS RESOLUTION (DB settings > env)
// ═══════════════════════════════════════════════════════

/// Redirect URI the packaged app's OAuth WebView flow binds and expects.
/// Users must register EXACTLY this URI in their Spotify dashboard app;
/// the Accounts view shows it read-only with a copy button.
pub const SPOTIFY_DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";

/// Spotify API credentials persisted by the user from the UI (settings table).
///
/// Unlike OAuth *account* tokens (encrypted in `accounts.credentials_json`),
/// these are the developer-dashboard API credentials shared by every account.
#[derive(Debug, Clone, PartialEq)]
pub struct SpotifyApiCredentials {
    pub client_id: String,
    pub client_secret: String,
    /// None / empty → fall back to [`SPOTIFY_DEFAULT_REDIRECT_URI`].
    pub redirect_uri: Option<String>,
}

/// Process-wide cache of the DB-stored API credentials.
///
/// Hydrated by `commands::settings` whenever the keys are saved or read, and
/// refreshed on every backend resolution. It exists so sync-free code paths
/// that only have `SpotifyConfig::from_env()` available (e.g. the auth WebView
/// command) still resolve BD-settings first without needing a pool handle.
static DB_SPOTIFY_CREDENTIALS: std::sync::RwLock<Option<SpotifyApiCredentials>> =
    std::sync::RwLock::new(None);

/// Replace the cached DB-stored API credentials (`None` clears them).
pub fn set_cached_spotify_credentials(creds: Option<SpotifyApiCredentials>) {
    if let Ok(mut guard) = DB_SPOTIFY_CREDENTIALS.write() {
        *guard = creds;
    }
}

/// Snapshot of the currently cached DB-stored API credentials, if any.
pub fn cached_spotify_credentials() -> Option<SpotifyApiCredentials> {
    DB_SPOTIFY_CREDENTIALS
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

/// Spotify access token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: String,
}

/// Stored credentials format for database persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp (seconds) when the token expires
    pub expires_at: i64,
}

/// Spotify user profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyUser {
    pub id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

/// Spotify track from API
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyTrack {
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub name: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub external_ids: Option<SpotifyExternalIds>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub artists: Vec<SpotifyArtist>,
    #[serde(default)]
    pub album: Option<SpotifyAlbum>,
    #[serde(default)]
    pub popularity: Option<i32>,
    #[serde(default)]
    pub track_number: Option<i32>,
    #[serde(default)]
    pub disc_number: Option<i32>,
    #[serde(default)]
    pub preview_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyExternalIds {
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub upc: Option<String>,
    #[serde(default)]
    pub ean: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyArtist {
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyAlbum {
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub name: String,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub total_tracks: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub images: Vec<SpotifyImage>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub external_ids: Option<SpotifyExternalIds>,
    #[serde(default)]
    pub tracks: Option<SpotifyPaginated<SpotifyTrack>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyImage {
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub url: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifySavedTrack {
    pub added_at: String,
    pub track: SpotifyTrack,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifySavedAlbum {
    pub added_at: String,
    pub album: SpotifyAlbum,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyFollowedArtistsResponse {
    pub artists: SpotifyArtistsCursorPaginated,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyArtistsCursorPaginated {
    pub items: Vec<SpotifyArtist>,
    pub next: Option<String>,
    pub total: Option<i32>,
    pub cursors: Option<SpotifyCursor>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyCursor {
    pub after: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyPaginated<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyAlbumsResponse {
    pub albums: Vec<Option<SpotifyAlbum>>,
}

/// Spotify playlist from API
#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyPlaylist {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub public: Option<bool>,
    #[serde(default)]
    pub collaborative: bool,
    #[serde(default)]
    pub owner: Option<SpotifyPlaylistOwner>,
    #[serde(default)]
    pub tracks: Option<SpotifyPlaylistTracks>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub images: Vec<SpotifyImage>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyPlaylistOwner {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyPlaylistTracks {
    #[serde(default)]
    pub total: i32,
}

/// Playlist track item from API
#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyPlaylistItem {
    #[serde(default)]
    pub added_at: Option<String>,
    #[serde(default)]
    pub track: Option<SpotifyTrack>,
}

/// Spotify Audio Features (from GET /v1/audio-features)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyAudioFeatures {
    /// Spotify track ID
    pub id: String,
    /// Tempo in BPM
    #[serde(default)]
    pub tempo: f32,
    /// Musical key (0=C, 1=C#, 2=D, ..., 11=B)
    #[serde(default)]
    pub key: i32,
    /// Mode (0=minor, 1=major)
    #[serde(default)]
    pub mode: i32,
    /// Energy (0.0 - 1.0)
    #[serde(default)]
    pub energy: f32,
    /// Danceability (0.0 - 1.0)
    #[serde(default)]
    pub danceability: f32,
    /// Valence/mood (0.0=sad, 1.0=happy)
    #[serde(default)]
    pub valence: f32,
    /// Acousticness (0.0 - 1.0)
    #[serde(default)]
    pub acousticness: f32,
    /// Instrumentalness (0.0 - 1.0)
    #[serde(default)]
    pub instrumentalness: f32,
}

/// Batch audio features response
#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyAudioFeaturesResponse {
    pub audio_features: Vec<Option<SpotifyAudioFeatures>>,
}

impl SpotifyAudioFeatures {
    /// Convert Spotify key code (0-11) to musical notation (C, C#, D, etc.)
    pub fn key_notation(&self) -> String {
        let notes = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let mode_suffix = if self.mode == 0 { "m" } else { "" }; // minor/major
        if self.key >= 0 && (self.key as usize) < notes.len() {
            format!("{}{}", notes[self.key as usize], mode_suffix)
        } else {
            "Unknown".to_string()
        }
    }

    /// Round BPM to nearest integer
    pub fn bpm(&self) -> i32 {
        self.tempo.round() as i32
    }
}

/// Default scopes for Syncify
pub const SPOTIFY_SCOPES: &[&str] = &[
    "user-library-read",
    "user-library-modify",
    "user-read-private",
    "user-read-email",
    "playlist-read-private",
    "playlist-read-collaborative",
];

impl SpotifyConfig {
    /// Single constructor from explicit parts. `redirect_uri` that is None or
    /// blank falls back to [`SPOTIFY_DEFAULT_REDIRECT_URI`].
    pub fn from_parts(client_id: String, client_secret: String, redirect_uri: Option<String>) -> Self {
        let redirect_uri = redirect_uri
            .map(|uri| uri.trim().to_string())
            .filter(|uri| !uri.is_empty())
            .unwrap_or_else(|| SPOTIFY_DEFAULT_REDIRECT_URI.to_string());
        Self { client_id, client_secret, redirect_uri }
    }

    /// Resolve the API credentials with the canonical priority:
    /// 1. DB settings (`spotify_client_id` / `spotify_client_secret` / `spotify_redirect_uri`),
    ///    via the process-wide cache hydrated by `commands::settings`;
    /// 2. environment variables (dev compatibility).
    ///
    /// Kept named `from_env` for call-site compatibility: this is the one and
    /// only place in the codebase where the SPOTIFY_*/SPOTIPY_* env vars are
    /// read, and it always prefers user-configured DB credentials so packaged
    /// builds (which never see a .env) work once configured from the UI.
    pub fn from_env() -> Result<Self, String> {
        if let Some(creds) = cached_spotify_credentials() {
            return Ok(Self::from_parts(creds.client_id, creds.client_secret, creds.redirect_uri));
        }

        Ok(Self::from_parts(
            std::env::var("SPOTIFY_CLIENT_ID")
                .or_else(|_| std::env::var("SPOTIPY_CLIENT_ID"))
                .map_err(|_| "SPOTIFY_CLIENT_ID not set")?,
            std::env::var("SPOTIFY_CLIENT_SECRET")
                .or_else(|_| std::env::var("SPOTIPY_CLIENT_SECRET"))
                .map_err(|_| "SPOTIFY_CLIENT_SECRET not set")?,
            std::env::var("SPOTIFY_REDIRECT_URI")
                .or_else(|_| std::env::var("SPOTIPY_REDIRECT_URI"))
                .ok(),
        ))
    }

    /// Generate the authorization URL
    pub fn auth_url(&self, scopes: &[&str]) -> String {
        let scope = scopes.join(" ");
        format!(
            "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}",
            self.client_id,
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&scope)
        )
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code(&self, code: &str) -> Result<SpotifyTokenResponse, String> {
        let client = Client::new();

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.redirect_uri),
        ];

        let response = client
            .post("https://accounts.spotify.com/api/token")
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Token exchange failed: {}", error));
        }

        response
            .json::<SpotifyTokenResponse>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Refresh an expired access token using the refresh token
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<SpotifyTokenResponse, String> {
        let client = Client::new();

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        tracing::info!("Refreshing Spotify access token...");

        let response = client
            .post("https://accounts.spotify.com/api/token")
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Refresh request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            tracing::error!("Token refresh failed ({}): {}", status, error);
            return Err(format!("Token refresh failed ({}): {}", status, error));
        }

        let token_response = response
            .json::<SpotifyTokenResponse>()
            .await
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        tracing::info!("Spotify token refreshed successfully");
        Ok(token_response)
    }
}

/// Response from Spotify's internal /get_access_token endpoint (sp_dc flow)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpDcTokenResponse {
    pub access_token: String,
    /// Millisecond unix timestamp when the token expires
    #[serde(default)]
    pub access_token_expiration_timestamp_ms: i64,
    #[serde(default)]
    pub is_anonymous: bool,
}

impl SpDcTokenResponse {
    /// Convert expiration from ms to seconds
    pub fn expires_at_secs(&self) -> i64 {
        self.access_token_expiration_timestamp_ms / 1000
    }

    /// Approximate expires_in seconds from now
    pub fn expires_in_secs(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        (self.expires_at_secs() - now).max(0)
    }
}

/// Exchange an sp_dc cookie for a bearer access token.
///
/// Delegates to the Python auth bridge which uses headless Chromium to
/// bypass Spotify's Fastly/Varnish WAF that blocks raw HTTP requests.
pub async fn refresh_from_sp_dc(sp_dc: &str) -> Result<SpDcTokenResponse, String> {
    let project_root = crate::commands::get_project_root();

    let python_cmd = {
        let venv_python = if cfg!(windows) {
            project_root.join(".venv").join("Scripts").join("python.exe")
        } else {
            project_root.join(".venv").join("bin").join("python")
        };
        if venv_python.exists() {
            venv_python.to_string_lossy().to_string()
        } else {
            "python".to_string()
        }
    };

    let script_path = project_root.join("scripts").join("auth_bridge.py");

    let output = tokio::process::Command::new(&python_cmd)
        .arg(&script_path)
        .arg("spotify")
        .arg("refresh")
        .arg(sp_dc)
        .current_dir(&project_root)
        .output()
        .await
        .map_err(|e| format!("Failed to run sp_dc refresh bridge: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        tracing::warn!("sp_dc refresh stderr: {}", stderr);
    }

    // Parse the bridge JSON response
    let json_str = stdout.trim();
    let bridge_start = json_str.find(r#"{"success""#).unwrap_or(0);
    let bridge_json = &json_str[bridge_start..];

    let bridge_result: serde_json::Value = serde_json::from_str(bridge_json)
        .map_err(|e| format!("Failed to parse sp_dc refresh result: {} (raw: {})", e, &stdout[..stdout.len().min(200)]))?;

    if bridge_result["success"].as_bool() != Some(true) {
        let err = bridge_result["error"]
            .as_str()
            .unwrap_or("Unknown refresh error");
        return Err(format!("sp_dc refresh failed: {}", err));
    }

    let data = &bridge_result["data"];
    let access_token = data["accessToken"]
        .as_str()
        .ok_or("Missing accessToken in refresh response")?
        .to_string();
    let expires_ms = data["accessTokenExpirationTimestampMs"]
        .as_i64()
        .unwrap_or(0);
    let is_anonymous = data["isAnonymous"].as_bool().unwrap_or(false);

    if is_anonymous {
        return Err("sp_dc cookie expired — returned anonymous session. Please reconnect.".into());
    }

    tracing::info!(
        "sp_dc token refreshed via Python bridge, expires at {}",
        expires_ms
    );

    Ok(SpDcTokenResponse {
        access_token,
        access_token_expiration_timestamp_ms: expires_ms,
        is_anonymous,
    })
}

use crate::services::http_retry::{HttpRetryPolicy, RetryDecision};
use crate::services::rate_limiter::RateLimiter;
use std::sync::Arc;
use std::time::SystemTime;

/// Spotify API client
pub struct SpotifyClient {
    pub client: Client,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64,
    pub config: Option<SpotifyConfig>,
    pub rate_limiter: Arc<RateLimiter>,
    pub retry_policy: Arc<HttpRetryPolicy>,
}

impl SpotifyClient {
    pub fn new(access_token: String, refresh_token: Option<String>, expires_at: i64) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
            ),
        );
        headers.insert(
            "App-Platform",
            reqwest::header::HeaderValue::from_static("WebPlayer"),
        );

        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .default_headers(headers)
                .build()
                .unwrap_or_else(|_| Client::new()),
            access_token,
            refresh_token,
            expires_at,
            config: SpotifyConfig::from_env().ok(),
            rate_limiter: Arc::new(RateLimiter::new()),
            retry_policy: Arc::new(HttpRetryPolicy::new()),
        }
    }

    /// Ensure the access token is valid, refreshing if necessary
    pub async fn ensure_token_valid(&mut self, db: &SqlitePool, account_id: i64) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        if now >= (self.expires_at - 300) {
            if let Some(rt) = &self.refresh_token {
                if let Some(config) = &self.config {
                    tracing::info!("Spotify: Token expiring soon, refreshing...");
                    match config.refresh_access_token(rt).await {
                        Ok(new_auth) => {
                            let now_new = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                                
                            self.access_token = new_auth.access_token.clone();
                            self.expires_at = now_new + new_auth.expires_in;
                            
                            // Persist new token
                            let _ = sqlx::query(
                                "UPDATE service_credentials SET access_token = ?, expires_at = ? WHERE account_id = ?"
                            )
                            .bind(&self.access_token)
                            .bind(self.expires_at)
                            .bind(account_id)
                            .execute(db)
                            .await;
                            
                            tracing::info!("Spotify: Token refreshed successfully");
                        },
                        Err(e) => return Err(format!("Token refresh failed: {}", e)),
                    }
                }
            }
        }
        Ok(())
    }

    /// Get current user profile
    pub async fn get_current_user(&self) -> Result<SpotifyUser, String> {
        let response = self
            .client
            .get("https://api.spotify.com/v1/me")
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err("Failed to get user profile".into());
        }

        response
            .json::<SpotifyUser>()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Get user's liked songs (paginated) with rate limit handling
    pub async fn get_saved_tracks(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<SpotifyPaginated<SpotifySavedTrack>, String> {
        self.get_saved_tracks_with_retry(offset, limit, 3).await
    }

    /// Get user's liked songs with retry logic for rate limits
    async fn get_saved_tracks_with_retry(
        &self,
        offset: i32,
        limit: i32,
        _max_retries: u32,
    ) -> Result<SpotifyPaginated<SpotifySavedTrack>, String> {
        let url = format!(
            "https://api.spotify.com/v1/me/tracks?offset={}&limit={}",
            offset, limit
        );

        let mut retries = 0;
        loop {
            // 1. Dispatch control via RateLimiter
            self.rate_limiter.acquire("spotify").await;

            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = response.status();
            let headers = response.headers().clone();

            // 2. Response evaluation via HttpRetryPolicy
            let decision = self.retry_policy.evaluate_response(
                &reqwest::Method::GET,
                status,
                &headers,
                retries,
                false,
                false, // is_cancelled
                SystemTime::now(),
            );

            match decision {
                RetryDecision::Success => {
                    return response.json().await.map_err(|e| format!("Parse error: {}", e));
                }
                RetryDecision::RetryAfter(delay) => {
                    tracing::warn!(
                        "Spotify rate limited / transient error at offset {}, retrying in {:?} (attempt {})",
                        offset,
                        delay,
                        retries + 1
                    );
                    tokio::time::sleep(delay).await;
                    retries += 1;
                    continue;
                }
                RetryDecision::DoNotRetry(msg) => {
                    let body = response.text().await.unwrap_or_default();
                    return Err(format!("Spotify API error ({}): {} - {}", status, msg, body));
                }
                RetryDecision::MaxRetriesExceeded => {
                    return Err("Spotify API: Max retries exceeded".into());
                }
            }
        }
    }

    /// Get user's saved albums (paginated)
    pub async fn get_saved_albums(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<SpotifyPaginated<SpotifySavedAlbum>, String> {
        let url = format!(
            "https://api.spotify.com/v1/me/albums?offset={}&limit={}",
            offset, limit
        );

        let mut retries = 0;
        loop {
            self.rate_limiter.acquire("spotify").await;

            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = response.status();
            let headers = response.headers().clone();

            let decision = self.retry_policy.evaluate_response(
                &reqwest::Method::GET,
                status,
                &headers,
                retries,
                false,
                false,
                SystemTime::now(),
            );

            match decision {
                RetryDecision::Success => {
                    return response.json().await.map_err(|e| format!("Parse error: {}", e));
                }
                RetryDecision::RetryAfter(delay) => {
                    tokio::time::sleep(delay).await;
                    retries += 1;
                    continue;
                }
                RetryDecision::DoNotRetry(msg) => {
                    let body = response.text().await.unwrap_or_default();
                    return Err(format!("Spotify API error ({}): {} - {}", status, msg, body));
                }
                RetryDecision::MaxRetriesExceeded => {
                    return Err("Spotify API: Max retries exceeded".into());
                }
            }
        }
    }

    /// Get tracks in a Spotify album (paginated)
    pub async fn get_album_tracks(
        &self,
        album_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<SpotifyPaginated<SpotifyTrack>, String> {
        let url = format!(
            "https://api.spotify.com/v1/albums/{}/tracks?offset={}&limit={}",
            urlencoding::encode(album_id), offset, limit
        );

        let mut retries = 0;
        loop {
            self.rate_limiter.acquire("spotify").await;

            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = response.status();
            let headers = response.headers().clone();

            let decision = self.retry_policy.evaluate_response(
                &reqwest::Method::GET,
                status,
                &headers,
                retries,
                false,
                false,
                SystemTime::now(),
            );

            match decision {
                RetryDecision::Success => {
                    return response.json().await.map_err(|e| format!("Parse error: {}", e));
                }
                RetryDecision::RetryAfter(delay) => {
                    tracing::warn!(
                        "Spotify rate limited at album {} tracks offset {}, retrying in {:?} (attempt {})",
                        album_id, offset, delay, retries + 1
                    );
                    tokio::time::sleep(delay).await;
                    retries += 1;
                    continue;
                }
                RetryDecision::DoNotRetry(msg) => {
                    let body = response.text().await.unwrap_or_default();
                    return Err(format!("Spotify API error ({}): {} - {}", status, msg, body));
                }
                RetryDecision::MaxRetriesExceeded => {
                    return Err("Spotify API: Max retries exceeded".into());
                }
            }
        }
    }

    /// Get user's followed artists (cursor paginated)
    pub async fn get_followed_artists(
        &self,
        after: Option<&str>,
        limit: i32,
    ) -> Result<SpotifyFollowedArtistsResponse, String> {
        let mut url = format!(
            "https://api.spotify.com/v1/me/following?type=artist&limit={}",
            limit
        );
        if let Some(cursor) = after {
            url.push_str(&format!("&after={}", urlencoding::encode(cursor)));
        }

        let mut retries = 0;
        loop {
            self.rate_limiter.acquire("spotify").await;

            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = response.status();
            let headers = response.headers().clone();

            let decision = self.retry_policy.evaluate_response(
                &reqwest::Method::GET,
                status,
                &headers,
                retries,
                false,
                false,
                SystemTime::now(),
            );

            match decision {
                RetryDecision::Success => {
                    return response.json().await.map_err(|e| format!("Parse error: {}", e));
                }
                RetryDecision::RetryAfter(delay) => {
                    tokio::time::sleep(delay).await;
                    retries += 1;
                    continue;
                }
                RetryDecision::DoNotRetry(msg) => {
                    let body = response.text().await.unwrap_or_default();
                    return Err(format!("Spotify API error ({}): {} - {}", status, msg, body));
                }
                RetryDecision::MaxRetriesExceeded => {
                    return Err("Spotify API: Max retries exceeded".into());
                }
            }
        }
    }

    /// Save a track to user's Spotify library (PUT /v1/me/tracks?ids=...)
    pub async fn save_track(&self, id: &str) -> Result<(), String> {
        let url = format!("https://api.spotify.com/v1/me/tracks?ids={}", urlencoding::encode(id));
        self.rate_limiter.acquire("spotify").await;

        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Spotify API error ({}): {}", status, body))
        }
    }

    /// Remove a track from user's Spotify library (DELETE /v1/me/tracks?ids=...)
    pub async fn remove_saved_track(&self, id: &str) -> Result<(), String> {
        let url = format!("https://api.spotify.com/v1/me/tracks?ids={}", urlencoding::encode(id));
        self.rate_limiter.acquire("spotify").await;

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Spotify API error ({}): {}", status, body))
        }
    }

    /// Save an album to user's Spotify library (PUT /v1/me/albums?ids=...)
    pub async fn save_album(&self, id: &str) -> Result<(), String> {
        let url = format!("https://api.spotify.com/v1/me/albums?ids={}", urlencoding::encode(id));
        self.rate_limiter.acquire("spotify").await;

        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Spotify API error ({}): {}", status, body))
        }
    }

    /// Remove an album from user's Spotify library (DELETE /v1/me/albums?ids=...)
    pub async fn remove_saved_album(&self, id: &str) -> Result<(), String> {
        let url = format!("https://api.spotify.com/v1/me/albums?ids={}", urlencoding::encode(id));
        self.rate_limiter.acquire("spotify").await;

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Spotify API error ({}): {}", status, body))
        }
    }

    /// Follow an artist on Spotify (PUT /v1/me/following?type=artist&ids=...)
    pub async fn follow_artist(&self, id: &str) -> Result<(), String> {
        let url = format!("https://api.spotify.com/v1/me/following?type=artist&ids={}", urlencoding::encode(id));
        self.rate_limiter.acquire("spotify").await;

        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Spotify API error ({}): {}", status, body))
        }
    }

    /// Unfollow an artist on Spotify (DELETE /v1/me/following?type=artist&ids=...)
    pub async fn unfollow_artist(&self, id: &str) -> Result<(), String> {
        let url = format!("https://api.spotify.com/v1/me/following?type=artist&ids={}", urlencoding::encode(id));
        self.rate_limiter.acquire("spotify").await;

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Spotify API error ({}): {}", status, body))
        }
    }

    /// Get user's liked albumss playlists (paginated)
    pub async fn get_playlists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<SpotifyPaginated<SpotifyPlaylist>, String> {
        let url = format!(
            "https://api.spotify.com/v1/me/playlists?offset={}&limit={}",
            offset, limit
        );

        tracing::debug!("Requesting playlists from: {}", url);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        tracing::debug!("Spotify API response status: {}", status);

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                "Spotify API error ({}): {}",
                status,
                &body[..body.len().min(300)]
            );
            return Err(format!(
                "Spotify API error ({}): {}",
                status,
                &body[..body.len().min(200)]
            ));
        }

        let body_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        serde_json::from_str(&body_text).map_err(|e| {
            tracing::error!(
                "JSON parse error: {} - Response preview: {}",
                e,
                &body_text[..body_text.len().min(500)]
            );
            format!(
                "Failed to parse playlists: {} - preview: {}",
                e,
                &body_text[..body_text.len().min(100)]
            )
        })
    }

    /// Get playlist tracks (paginated) with rate limit handling
    pub async fn get_playlist_tracks(
        &self,
        playlist_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<SpotifyPaginated<SpotifyPlaylistItem>, String> {
        let url = format!(
            "https://api.spotify.com/v1/playlists/{}/tracks?offset={}&limit={}",
            playlist_id, offset, limit
        );

        let mut retries = 0;
        let max_retries = 3;

        loop {
            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let status = response.status();

            // Handle rate limiting (429)
            if status.as_u16() == 429 {
                if retries >= max_retries {
                    return Err("Rate limited: max retries exceeded".into());
                }

                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << retries);

                tracing::warn!(
                    "Rate limited for playlist {}, waiting {} seconds",
                    playlist_id,
                    retry_after
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(retry_after)).await;
                retries += 1;
                continue;
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!(
                    "Spotify API error ({}): {}",
                    status,
                    &body[..body.len().min(200)]
                ));
            }

            // Read body as text first for better error handling
            let body_text = response
                .text()
                .await
                .map_err(|e| format!("Failed to read response: {}", e))?;

            return serde_json::from_str(&body_text).map_err(|e| {
                // Log more details about the parse error
                tracing::error!(
                    "JSON parse error for playlist {}: {} - Response preview: {}",
                    playlist_id,
                    e,
                    &body_text[..body_text.len().min(500)]
                );
                format!("Failed to parse: {}", e)
            });
        }
    }

    /// Get audio features for multiple tracks (max 100 per request)
    /// Returns a HashMap of track_id -> AudioFeatures for easy lookup
    /// Supports 401 Unauthorized auto-refresh if refresh_token and db access provided
    pub async fn get_audio_features_batch(
        &mut self,
        _spotify_track_ids: &[String],
        _db: Option<&SqlitePool>,
        _account_id: Option<i64>,
    ) -> Result<std::collections::HashMap<String, SpotifyAudioFeatures>, String> {
        // AUDIO FEATURES DEPRECATED (S68): Spotify removed /audio-features endpoint.
        // Return empty map to avoid 403 Forbidden errors.
        Ok(std::collections::HashMap::new())
    }

    /// Import all liked songs to database
    pub async fn import_library(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<ImportResult, String> {
        let mut offset = 0;
        let limit = 50;
        let mut imported = 0;
        let mut skipped = 0;

        loop {
            let page = self.get_saved_tracks(offset, limit).await?;

            if page.items.is_empty() {
                break;
            }

            let batch_result = self
                .process_spotify_import_batch(db, account_id, &page.items)
                .await?;
            imported += batch_result.imported;
            skipped += batch_result.skipped;

            offset += limit;

            if page.next.is_none() {
                break;
            }

            tracing::info!("Imported {} tracks so far...", imported);
        }

        Ok(ImportResult { imported, skipped })
    }

    /// Process a batch of Spotify tracks (public for testing)
    pub async fn process_spotify_import_batch(
        &self,
        db: &SqlitePool,
        account_id: i64,
        items: &[SpotifySavedTrack],
    ) -> Result<ImportResult, String> {
        let mut imported = 0;
        let mut skipped = 0;

        for saved in items {
            let track = &saved.track;

            // Skip tracks without albums (local files, etc.)
            let Some(ref album) = track.album else {
                skipped += 1;
                continue;
            };

            // Skip tracks with empty/invalid data
            if track.id.is_empty() || track.name.is_empty() || track.duration_ms == 0 {
                skipped += 1;
                continue;
            }

            let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());

            // Get or create ALL artists
            let mut artist_ids = Vec::new();
            for (index, artist) in track.artists.iter().enumerate() {
                if artist.name.is_empty() {
                    continue;
                }

                let artist_id = self.get_or_create_artist(db, &artist.name).await?;
                let role = if index == 0 { "primary" } else { "featured" };
                artist_ids.push((artist_id, role));
            }

            // Fallback for no artists
            if artist_ids.is_empty() {
                let artist_id = self.get_or_create_artist(db, "Unknown Artist").await?;
                artist_ids.push((artist_id, "primary"));
            }

            let primary_artist_id = artist_ids
                .first()
                .map(|a| a.0)
                .ok_or_else(|| "Failed to resolve primary artist for Spotify track".to_string())?;

            // Get or create album
            let album_id = self
                .get_or_create_album(db, album, primary_artist_id)
                .await?;

            // Get or create track (by ISRC if available)
            let track_id = self
                .get_or_create_track(db, track, isrc.as_deref(), Some(album_id))
                .await?;

            // Link ALL artists to track (with retry for busy database)
            for (artist_id, role) in artist_ids {
                let mut retries = 0;
                loop {
                    match sqlx::query(
                        "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, ?)"
                    )
                    .bind(track_id)
                    .bind(artist_id)
                    .bind(role)
                    .execute(db)
                    .await {
                        Ok(_) => break,
                        Err(e) => {
                            retries += 1;
                            if retries >= 3 {
                                tracing::error!("Failed to link artist {} to track {} after 3 retries: {}", artist_id, track_id, e);
                                break;
                            }
                            tracing::warn!("Retry {} for track_artists insert: {}", retries, e);
                            tokio::time::sleep(std::time::Duration::from_millis(100 * retries)).await;
                        }
                    }
                }
            }

            // Add to library entry
            let result = sqlx::query(
                "INSERT OR IGNORE INTO library_entries (account_id, track_id, added_at, is_liked) VALUES (?, ?, ?, 1)"
            )
            .bind(account_id)
            .bind(track_id)
            .bind(&saved.added_at)
            .execute(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

            if result.rows_affected() > 0 {
                imported += 1;
            }
            // Note: If track already exists in library, we don't count it as imported (or skipped), just ignored

            // Add track source
            let spotify_service_id = self.get_service_id(db, "spotify").await?;
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, ?, ?, 1)"
            )
            .bind(track_id)
            .bind(spotify_service_id)
            .bind(&track.id)
            .execute(db)
            .await;
        }

        Ok(ImportResult { imported, skipped })
    }

    pub async fn get_service_id(&self, db: &SqlitePool, name: &str) -> Result<i64, String> {
        let row: (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
            .map_err(|e| format!("Service not found: {}", e))?;
        Ok(row.0)
    }

    pub async fn get_or_create_artist(&self, db: &SqlitePool, name: &str) -> Result<i64, String> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO artists (name) VALUES (?)
             ON CONFLICT(name) DO UPDATE SET name=excluded.name
             RETURNING id",
        )
        .bind(name)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Artist get_or_create failed: {}", e))?;

        Ok(id)
    }

    pub async fn get_or_create_album(
        &self,
        db: &SqlitePool,
        album: &SpotifyAlbum,
        primary_artist_id: i64,
    ) -> Result<i64, String> {
        // Get cover art URL (largest image)
        let cover_url = album.images.first().map(|i| i.url.clone());

        let upc = album.external_ids.as_ref().and_then(|ext| ext.upc.clone());

        // Create or update album by spotify_id
        let album_id: (i64,) = sqlx::query_as:: <sqlx::Sqlite, (i64,)>(
            "INSERT INTO albums (title, release_date, total_tracks, cover_art_url, spotify_id, label, upc) 
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(spotify_id) WHERE spotify_id IS NOT NULL DO UPDATE SET
                cover_art_url = COALESCE(albums.cover_art_url, excluded.cover_art_url),
                total_tracks = COALESCE(albums.total_tracks, excluded.total_tracks),
                label = COALESCE(albums.label, excluded.label),
                upc = COALESCE(albums.upc, excluded.upc)
             RETURNING id"
        )
        .bind(&album.name)
        .bind(&album.release_date)
        .bind(album.total_tracks)
        .bind(&cover_url)
        .bind(&album.id)
        .bind(&album.label)
        .bind(&upc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Album get_or_create failed: {}", e))?;

        let album_id = album_id.0;

        // Link album to artist
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)"
        )
        .bind(album_id)
        .bind(primary_artist_id)
        .execute(db)
        .await;

        Ok(album_id)
    }

    pub async fn get_or_create_track(
        &self,
        db: &SqlitePool,
        track: &SpotifyTrack,
        isrc: Option<&str>,
        album_id: Option<i64>,
    ) -> Result<i64, String> {
        let spotify_service_id = self.get_service_id(db, "spotify").await.unwrap_or(1);

        // 1. Check existing source mapping
        if let Ok(Some((existing_id,))) = sqlx::query_as::<_, (i64,)>(
            "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = ? LIMIT 1"
        )
        .bind(spotify_service_id)
        .bind(&track.id)
        .fetch_optional(db)
        .await {
            if let Some(alb_id) = album_id {
                let _ = sqlx::query("UPDATE tracks SET album_id = COALESCE(album_id, ?) WHERE id = ?")
                    .bind(alb_id).bind(existing_id).execute(db).await;
            }
            return Ok(existing_id);
        }

        // 2. Check existing by spotify_id
        if let Ok(Some((existing_id,))) = sqlx::query_as::<_, (i64,)>(
            "SELECT id FROM tracks WHERE spotify_id = ? LIMIT 1"
        )
        .bind(&track.id)
        .fetch_optional(db)
        .await {
            if let Some(alb_id) = album_id {
                let _ = sqlx::query("UPDATE tracks SET album_id = COALESCE(album_id, ?) WHERE id = ?")
                    .bind(alb_id).bind(existing_id).execute(db).await;
            }
            return Ok(existing_id);
        }

        // 3. Check existing by validated ISRC (reject numeric IDs)
        let sanitized_isrc = isrc.and_then(|c| {
            let t = c.trim();
            if syncify_core_domain::metadata::is_valid_isrc(t) {
                Some(t.to_string())
            } else {
                None
            }
        });

        if let Some(ref valid_isrc) = sanitized_isrc {
            if let Ok(Some((existing_id,))) = sqlx::query_as::<_, (i64,)>(
                "SELECT id FROM tracks WHERE isrc = ? LIMIT 1"
            )
            .bind(valid_isrc)
            .fetch_optional(db)
            .await {
                if let Some(alb_id) = album_id {
                    let _ = sqlx::query("UPDATE tracks SET album_id = COALESCE(album_id, ?), spotify_id = COALESCE(spotify_id, ?) WHERE id = ?")
                        .bind(alb_id).bind(&track.id).bind(existing_id).execute(db).await;
                }
                return Ok(existing_id);
            }
        }

        // 4. Create new canonical track
        let id: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc, explicit, spotify_id, popularity, track_number, disc_number, preview_url) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id"
        )
        .bind(&track.name)
        .bind(album_id)
        .bind(track.duration_ms)
        .bind(sanitized_isrc.as_deref())
        .bind(track.explicit)
        .bind(&track.id)
        .bind(track.popularity)
        .bind(track.track_number)
        .bind(track.disc_number)
        .bind(&track.preview_url)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Track get_or_create failed: {}", e))?;

        Ok(id.0)
    }

    /// Fetch multiple albums in a single request (max 20)
    pub async fn get_albums_batch(&self, ids: &[String]) -> Result<Vec<SpotifyAlbum>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("https://api.spotify.com/v1/albums?ids={}", ids.join(","));
        
        let mut retry_count = 0;
        let max_retries = 3;

        loop {
            let response = self.client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if response.status() == 429 {
                let retry_after = response.headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(30);
                
                tracing::warn!("Spotify: Rate limited (429). Retrying after {} seconds...", retry_after);
                tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
                continue;
            }

            if !response.status().is_success() {
                let err_body = response.text().await.unwrap_or_default();
                if retry_count < max_retries {
                    retry_count += 1;
                    tracing::warn!("Spotify: Batch fetch failed ({}). Retrying...", err_body);
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                return Err(format!("Spotify API error: {}", err_body));
            }

            let result: SpotifyAlbumsResponse = response.json().await.map_err(|e| e.to_string())?;
            // Filter out null albums (sometimes Spotify returns null for an ID)
            return Ok(result.albums.into_iter().flatten().collect());
        }
    }

    /// Enrich all albums in the database that are missing label/upc
    pub async fn enrich_albums(
        &mut self, 
        db: &sqlx::SqlitePool, 
        account_id: i64,
        window: Option<&tauri::Window>
    ) -> Result<ImportResult, String> {
        // 1. Find candidate albums
        let candidates: Vec<(String,)> = sqlx::query_as("SELECT spotify_id FROM albums WHERE spotify_id IS NOT NULL AND label IS NULL")
            .fetch_all(db)
            .await
            .map_err(|e| e.to_string())?;

        let total = candidates.len();
        if total == 0 {
            return Ok(super::ImportResult { imported: 0, skipped: 0 });
        }

        tracing::info!("Spotify: Starting enrichment for {} albums", total);
        if let Some(w) = window {
            crate::commands::emit_import_progress(w, "spotify_enrichment", "started", 0, total as u64, 
                &format!("Enriching metadata for {} albums...", total));
        }

        let mut enriched = 0;
        let mut skipped = 0;

        // 2. Process in chunks of 20 (Warp Speed Batch)
        for chunk in candidates.chunks(20) {
            // Check if token needs refresh
            self.ensure_token_valid(db, account_id).await?;

            let ids: Vec<String> = chunk.iter().map(|c| c.0.clone()).collect();
            
            match self.get_albums_batch(&ids).await {
                Ok(albums) => {
                    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
                    
                    for album in albums {
                        let upc = album.external_ids.as_ref().and_then(|ext| ext.upc.clone());
                        
                        let _ = sqlx::query(
                            "UPDATE albums SET label = ?, upc = ? WHERE spotify_id = ?"
                        )
                        .bind(&album.label)
                        .bind(&upc)
                        .bind(&album.id)
                        .execute(&mut *tx)
                        .await;
                        
                        enriched += 1;
                    }
                    
                    tx.commit().await.map_err(|e| e.to_string())?;
                }
                Err(e) => {
                    tracing::error!("Spotify: Batch enrichment failed: {}", e);
                    skipped += ids.len() as i32;
                }
            }

            if let Some(w) = window {
                crate::commands::emit_import_progress(w, "spotify_enrichment", "progress", 
                    (enriched + skipped) as u64, total as u64,
                    &format!("Enriched {}/{} albums", enriched, total));
            }
            
            // Brief pause to be polite to the API (non-429 throttle)
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        if let Some(w) = window {
            crate::commands::emit_import_complete(w, "spotify_enrichment", enriched as u64, skipped as u64);
        }

        Ok(ImportResult { imported: enriched, skipped })
    }
}

/// Import result
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub imported: i32,
    pub skipped: i32,
}

/// Search result for migration matching
#[derive(Debug, Clone, Serialize)]
pub struct SpotifySearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i64,
}

/// Search response from Spotify API
#[derive(Debug, Clone, Deserialize)]
pub struct SpotifySearchResponse {
    pub tracks: Option<SpotifyPaginated<SpotifyTrack>>,
}

impl SpotifyClient {
    /// Search for tracks by query string
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<SpotifySearchResult>, String> {
        let url = format!(
            "https://api.spotify.com/v1/search?q={}&type=track&limit={}",
            urlencoding::encode(query),
            limit
        );

        let mut retries = 0;
        let max_retries = 3;

        loop {
            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| format!("Search request failed: {}", e))?;

            let status = response.status();

            if status.as_u16() == 429 {
                if retries >= max_retries {
                    return Err("Rate limited: max retries exceeded".into());
                }
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << retries);
                tokio::time::sleep(tokio::time::Duration::from_secs(retry_after)).await;
                retries += 1;
                continue;
            }

            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                return Err(format!(
                    "Spotify search failed ({}): {}",
                    status,
                    &text[..text.len().min(200)]
                ));
            }

            let search_resp: SpotifySearchResponse = response
                .json()
                .await
                .map_err(|e| format!("Parse search response: {}", e))?;

            let results = search_resp
                .tracks
                .map(|t| t.items)
                .unwrap_or_default()
                .into_iter()
                .map(|track| SpotifySearchResult {
                    track_id: track.id.clone(),
                    title: track.name.clone(),
                    artist: track
                        .artists
                        .first()
                        .map(|a| a.name.clone())
                        .unwrap_or_default(),
                    album: track.album.map(|a| a.name),
                    isrc: track.external_ids.and_then(|e| e.isrc),
                    duration_ms: track.duration_ms,
                })
                .collect();

            return Ok(results);
        }
    }

    /// Search for a track by ISRC code
    pub async fn search_by_isrc(&self, isrc: &str) -> Result<Option<SpotifySearchResult>, String> {
        // Spotify supports ISRC in search with isrc: prefix
        let query = format!("isrc:{}", isrc);
        let results = self.search_track(&query, 1).await?;
        Ok(results.into_iter().next())
    }

    /// Match a track by metadata (fallback when no ISRC)
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<SpotifySearchResult>, String> {
        let query = format!("track:{} artist:{}", title, artist);
        let results = self.search_track(&query, 10).await?;

        let normalize = |s: &str| {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
        };
        let target_title = normalize(title);
        let target_artist = normalize(artist);

        let best_match = results
            .into_iter()
            .filter(|r| {
                let r_title = normalize(&r.title);
                let r_artist = normalize(&r.artist);
                r_title.contains(&target_title)
                    || target_title.contains(&r_title)
                    || (r_artist.contains(&target_artist) && r_title.len() > 0)
            })
            .next();

        Ok(best_match)
    }

    /// Save tracks to user's library (add to Liked Songs)
    /// Uses RateLimiter for dispatch and HttpRetryPolicy for resilience
    pub async fn save_tracks(&self, track_ids: &[String]) -> Result<(), String> {
        if track_ids.is_empty() {
            return Ok(());
        }

        // Spotify allows up to 50 tracks per request
        let ids = track_ids
            .iter()
            .take(50)
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("https://api.spotify.com/v1/me/tracks?ids={}", ids);

        let mut attempt = 0;
        loop {
            // 1. Dispatch control via RateLimiter
            self.rate_limiter.acquire("spotify").await;

            let response = self
                .client
                .put(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();

                    let decision = self.retry_policy.evaluate_response(
                        &reqwest::Method::PUT,
                        status,
                        &headers,
                        attempt,
                        false, // Method::PUT is inherently idempotent
                        false, // is_cancelled
                        SystemTime::now(),
                    );

                    match decision {
                        RetryDecision::Success => {
                            tracing::info!("Saved {} tracks to Spotify library", track_ids.len());
                            return Ok(());
                        }
                        RetryDecision::RetryAfter(delay) => {
                            tracing::warn!(
                                "Spotify save_tracks attempt {} failed ({}), retrying in {:?}...",
                                attempt + 1,
                                status,
                                delay
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                        RetryDecision::DoNotRetry(msg) => {
                            let text = resp.text().await.unwrap_or_default();
                            return Err(format!(
                                "Save tracks failed ({}): {} - {}",
                                status,
                                msg,
                                &text[..text.len().min(200)]
                            ));
                        }
                        RetryDecision::MaxRetriesExceeded => {
                            return Err("Save tracks failed: max retries exceeded".into());
                        }
                    }
                }
                Err(e) => {
                    let decision = self.retry_policy.evaluate_network_error(
                        &reqwest::Method::PUT,
                        attempt,
                        false,
                        false,
                    );

                    match decision {
                        RetryDecision::RetryAfter(delay) => {
                            tracing::warn!(
                                "Spotify save_tracks network error ({}), retrying in {:?}...",
                                e,
                                delay
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                        _ => return Err(format!("Request failed: {}", e)),
                    }
                }
            }
        }
    }

    /// Add a single track to user's library
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        self.save_tracks(&[track_id.to_string()]).await
    }
}

// ═══════════════════════════════════════════════════════
// TESTS — SPRINT S196 credential resolution
// ═══════════════════════════════════════════════════════

#[cfg(test)]
mod credentials_resolution_tests {
    use super::*;

    #[test]
    fn from_parts_uses_explicit_redirect_uri() {
        let config = SpotifyConfig::from_parts(
            "id123".to_string(),
            "secret456".to_string(),
            Some("http://localhost:9999/cb".to_string()),
        );
        assert_eq!(config.client_id, "id123");
        assert_eq!(config.client_secret, "secret456");
        assert_eq!(config.redirect_uri, "http://localhost:9999/cb");
    }

    #[test]
    fn from_parts_falls_back_to_default_redirect_uri() {
        let none = SpotifyConfig::from_parts("id".to_string(), "sec".to_string(), None);
        assert_eq!(none.redirect_uri, SPOTIFY_DEFAULT_REDIRECT_URI);

        // Blank / whitespace-only values must behave like None.
        let blank = SpotifyConfig::from_parts("id".to_string(), "sec".to_string(), Some("   ".to_string()));
        assert_eq!(blank.redirect_uri, SPOTIFY_DEFAULT_REDIRECT_URI);

        // Values are trimmed.
        let padded = SpotifyConfig::from_parts(
            "id".to_string(),
            "sec".to_string(),
            Some("  http://x/cb ".to_string()),
        );
        assert_eq!(padded.redirect_uri, "http://x/cb");
    }

    #[test]
    fn cached_db_credentials_take_priority_over_env() {
        // Deterministic regardless of the developer machine's env: the DB
        // cache, when hydrated, must always win over any SPOTIFY_* variable.
        let creds = SpotifyApiCredentials {
            client_id: "db_client_id".to_string(),
            client_secret: "db_client_secret".to_string(),
            redirect_uri: None,
        };
        set_cached_spotify_credentials(Some(creds.clone()));

        let resolved = SpotifyConfig::from_env()
            .expect("cached credentials must resolve without env vars");
        assert_eq!(resolved.client_id, "db_client_id");
        assert_eq!(resolved.client_secret, "db_client_secret");
        assert_eq!(resolved.redirect_uri, SPOTIFY_DEFAULT_REDIRECT_URI);

        // Leave the process-wide state exactly as we found it.
        set_cached_spotify_credentials(None);
        assert!(cached_spotify_credentials().is_none());
    }
}
