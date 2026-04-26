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
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpotifyExternalIds {
    #[serde(default)]
    pub isrc: Option<String>,
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
pub struct SpotifyPaginated<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
    pub total: i32,
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
    /// Load from environment variables (accepts both SPOTIFY_ and SPOTIPY_ prefixes)
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            client_id: std::env::var("SPOTIFY_CLIENT_ID")
                .or_else(|_| std::env::var("SPOTIPY_CLIENT_ID"))
                .map_err(|_| "SPOTIFY_CLIENT_ID not set")?,
            client_secret: std::env::var("SPOTIFY_CLIENT_SECRET")
                .or_else(|_| std::env::var("SPOTIPY_CLIENT_SECRET"))
                .map_err(|_| "SPOTIFY_CLIENT_SECRET not set")?,
            redirect_uri: std::env::var("SPOTIFY_REDIRECT_URI")
                .or_else(|_| std::env::var("SPOTIPY_REDIRECT_URI"))
                .unwrap_or_else(|_| "http://localhost:8888/callback".to_string()),
        })
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
            .unwrap()
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

/// Spotify API client
pub struct SpotifyClient {
    client: Client,
    access_token: String,
    refresh_token: Option<String>,
    config: Option<SpotifyConfig>,
}

impl SpotifyClient {
    pub fn new(access_token: String, refresh_token: Option<String>) -> Self {
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
            config: SpotifyConfig::from_env().ok(),
        }
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
        max_retries: u32,
    ) -> Result<SpotifyPaginated<SpotifySavedTrack>, String> {
        let url = format!(
            "https://api.spotify.com/v1/me/tracks?offset={}&limit={}",
            offset, limit
        );

        let mut retries = 0;
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

                // Get retry-after header, default to exponential backoff
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << retries); // 1, 2, 4 seconds

                tracing::warn!(
                    "Rate limited at offset {}, waiting {} seconds (retry {}/{})",
                    offset,
                    retry_after,
                    retries + 1,
                    max_retries
                );

                tokio::time::sleep(tokio::time::Duration::from_secs(retry_after)).await;
                retries += 1;
                continue;
            }

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

            return response
                .json()
                .await
                .map_err(|e| format!("Failed to parse: {}", e));
        }
    }

    /// Get user's playlists (paginated)
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
        spotify_track_ids: &[String],
        db: Option<&SqlitePool>,
        account_id: Option<i64>,
    ) -> Result<std::collections::HashMap<String, SpotifyAudioFeatures>, String> {
        use std::collections::HashMap;

        if spotify_track_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Spotify API allows max 100 IDs per request
        let mut all_features: HashMap<String, SpotifyAudioFeatures> = HashMap::new();

        for chunk in spotify_track_ids.chunks(100) {
            let ids = chunk.join(",");
            let url = format!("https://api.spotify.com/v1/audio-features?ids={}", ids);

            let mut retries = 0;
            let max_retries = 3;

            loop {
                let response = self
                    .client
                    .get(&url)
                    .bearer_auth(&self.access_token)
                    .send()
                    .await
                    .map_err(|e| format!("Audio features request failed: {}", e))?;

                let status = response.status();

                // Handle 401 Unauthorized or 403 Forbidden (Auto-Refresh)
                // Spotify sometimes returns 403 for expired tokens or scope issues that might be resolved by refresh
                if status.as_u16() == 401 || status.as_u16() == 403 {
                    if retries >= max_retries {
                        tracing::warn!(
                            "Max retries exceeded for {}, stopping refresh loop",
                            status
                        );
                        break;
                    }

                    // Clone config and refresh token to release borrow on self
                    let refresh_ctx = if let (Some(config), Some(refresh_token)) =
                        (&self.config, &self.refresh_token)
                    {
                        Some((config.clone(), refresh_token.clone()))
                    } else {
                        None
                    };

                    if let Some((config, refresh_token)) = refresh_ctx {
                        tracing::warn!("Spotify API returned {}, attempting refresh...", status);
                        if let Ok(new_token) = config.refresh_access_token(&refresh_token).await {
                            tracing::info!("Refreshed token scopes: {}", new_token.scope);
                            // Update internal token
                            self.access_token = new_token.access_token.clone();

                            // Update refresh token if rotated
                            if let Some(rt) = new_token.refresh_token {
                                self.refresh_token = Some(rt);
                            }

                            // Persist to DB if possible
                            if let (Some(db), Some(account_id)) = (db, account_id) {
                                // Re-encrypt credentials
                                let creds = serde_json::json!({
                                    "access_token": self.access_token,
                                    "refresh_token": self.refresh_token.as_ref().unwrap_or(&refresh_token),
                                    "expires_in": new_token.expires_in,
                                    "scope": new_token.scope,
                                    "token_type": new_token.token_type
                                });

                                if let Ok(encrypted) = crate::crypto::encrypt(&creds.to_string()) {
                                    let _ = sqlx::query(
                                        "UPDATE accounts SET credentials_json = ?, last_refreshed_at = CURRENT_TIMESTAMP WHERE id = ?"
                                    )
                                    .bind(encrypted)
                                    .bind(account_id)
                                    .execute(db)
                                    .await;
                                    tracing::info!("Persisted refreshed Spotify token to DB");
                                }
                            }

                            // Retry request immediately
                            retries += 1;
                            continue;
                        } else {
                            tracing::error!("Failed to auto-refresh token on 401");
                        }
                    } else {
                        tracing::warn!("{} received but missing refresh_token or config", status);
                    }
                }

                let status = response.status();

                // Handle rate limiting (429)
                if status.as_u16() == 429 {
                    if retries >= max_retries {
                        tracing::warn!("Rate limited for audio features, max retries exceeded");
                        break; // Skip this chunk rather than fail entirely
                    }

                    let retry_after = response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(1 << retries);

                    tracing::warn!(
                        "Rate limited for audio features, waiting {} seconds",
                        retry_after
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_after)).await;
                    retries += 1;
                    continue;
                }

                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    tracing::error!(
                        "Audio features API error ({}): {}",
                        status,
                        &body[..body.len().min(200)]
                    );
                    break; // Skip this chunk rather than fail entirely
                }

                // Parse response
                match response.json::<SpotifyAudioFeaturesResponse>().await {
                    Ok(data) => {
                        for feat in data.audio_features.into_iter().flatten() {
                            all_features.insert(feat.id.clone(), feat);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse audio features: {}", e);
                    }
                }
                break;
            }
        }

        tracing::info!("Fetched audio features for {} tracks", all_features.len());
        Ok(all_features)
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

            let primary_artist_id = artist_ids.first().unwrap().0;

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
        // Try to find existing
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM artists WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        // Create new
        let result = sqlx::query("INSERT INTO artists (name) VALUES (?)")
            .bind(name)
            .execute(db)
            .await
            .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_or_create_album(
        &self,
        db: &SqlitePool,
        album: &SpotifyAlbum,
        primary_artist_id: i64,
    ) -> Result<i64, String> {
        // Try to find existing by title (could improve with Spotify album ID later)
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM albums WHERE title = ?")
            .bind(&album.name)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        // Get cover art URL (largest image)
        let cover_url = album.images.first().map(|i| i.url.clone());

        // Create new album
        let result = sqlx::query(
            "INSERT INTO albums (title, release_date, total_tracks, cover_art_url) VALUES (?, ?, ?, ?)"
        )
        .bind(&album.name)
        .bind(&album.release_date)
        .bind(album.total_tracks)
        .bind(&cover_url)
        .execute(db)
        .await
        .map_err(|e| format!("Album insert failed: {}", e))?;

        let album_id = result.last_insert_rowid();

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
        // Try to find by ISRC
        if let Some(isrc) = isrc {
            if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_one(db)
                .await
            {
                // Update album_id if not set
                if album_id.is_some() {
                    let _ = sqlx::query(
                        "UPDATE tracks SET album_id = ? WHERE id = ? AND album_id IS NULL",
                    )
                    .bind(album_id)
                    .bind(row.0)
                    .execute(db)
                    .await;
                }
                return Ok(row.0);
            }
        }

        // Create new track with album_id
        let result = sqlx::query(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc, explicit) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&track.name)
        .bind(album_id)
        .bind(track.duration_ms)
        .bind(isrc)
        .bind(track.explicit)
        .execute(db)
        .await
        .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(result.last_insert_rowid())
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
    /// Includes retry logic with exponential backoff
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

        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let response = self
                .client
                .put(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        tracing::info!("Saved {} tracks to Spotify library", track_ids.len());
                        return Ok(());
                    } else if status.as_u16() == 429 || status.as_u16() >= 500 {
                        let text = resp.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "Spotify save_tracks attempt {} failed ({}), retrying...",
                            attempt + 1,
                            status
                        );
                    } else {
                        let text = resp.text().await.unwrap_or_default();
                        return Err(format!(
                            "Save tracks failed ({}): {}",
                            status,
                            &text[..text.len().min(200)]
                        ));
                    }
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!(
                        "Spotify save_tracks attempt {} failed: {}, retrying...",
                        attempt + 1,
                        e
                    );
                }
            }

            if attempt < max_retries - 1 {
                let delay = 500 * (1 << attempt);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }

        Err(format!(
            "Save tracks failed after {} retries: {}",
            max_retries, last_error
        ))
    }

    /// Add a single track to user's library
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        self.save_tracks(&[track_id.to_string()]).await
    }
}
