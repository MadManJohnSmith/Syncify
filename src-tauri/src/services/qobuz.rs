//! Qobuz service - Authentication and library import
//!
//! Handles Qobuz user auth token and importing favorites.

#![allow(dead_code)]

use reqwest::Client;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

/// Default placeholder values for development/testing when environment variables are not set.
/// Production deployments must provide valid credentials via `QOBUZ_APP_ID` and `QOBUZ_APP_SECRET`
/// environment variables or through secure database settings.
pub const QOBUZ_APP_ID_FALLBACK: &str = "dev_placeholder_qobuz_app_id";
pub const QOBUZ_APP_SECRET_FALLBACK: &str = "dev_placeholder_qobuz_app_secret";

// Kept for backward compatibility with callers referencing the constants,
// pointing to safe development fallback placeholders.
pub const QOBUZ_APP_ID: &str = QOBUZ_APP_ID_FALLBACK;
pub const QOBUZ_APP_SECRET: &str = QOBUZ_APP_SECRET_FALLBACK;
pub const QOBUZ_API_BASE: &str = "https://www.qobuz.com/api.json/0.2";

/// Resolve Qobuz App ID dynamically from environment or fallback placeholder.
pub fn get_qobuz_app_id() -> String {
    std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| QOBUZ_APP_ID.to_string())
}

/// Resolve Qobuz App Secret dynamically from environment or fallback placeholder.
pub fn get_qobuz_app_secret() -> String {
    std::env::var("QOBUZ_APP_SECRET").unwrap_or_else(|_| QOBUZ_APP_SECRET.to_string())
}

/// Helper to deserialize ID as either string or integer
pub fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(D::Error::custom("expected string or number")),
    }
}

/// Helper to deserialize track ID as i64 from either integer or string
pub fn deserialize_id_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => n.as_i64().ok_or_else(|| D::Error::custom("invalid integer")),
        serde_json::Value::String(s) => s.parse::<i64>().map_err(D::Error::custom),
        _ => Err(D::Error::custom("expected integer or string")),
    }
}

/// Helper to deserialize optional i64 from integer or string
pub fn deserialize_opt_id_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::Number(n)) => Ok(n.as_i64()),
        Some(serde_json::Value::String(s)) => Ok(s.parse::<i64>().ok()),
        _ => Ok(None),
    }
}

/// Helper to deserialize track duration (seconds) from float, integer, or string
pub fn deserialize_duration_secs<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Ok(i)
            } else if let Some(f) = n.as_f64() {
                Ok(f.round() as i64)
            } else {
                Ok(0)
            }
        }
        Some(serde_json::Value::String(s)) => {
            Ok(s.parse::<f64>().map(|f| f.round() as i64).unwrap_or(0))
        }
        _ => Ok(0),
    }
}

/// Helper to deserialize string from either raw string, object, or array
pub fn deserialize_string_or_stringify<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::String(s)) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(s))
            }
        }
        Some(serde_json::Value::Object(map)) => {
            if let Some(t) = map.get("title").and_then(|v| v.as_str()) {
                Ok(Some(t.to_string()))
            } else if let Some(n) = map.get("name").and_then(|v| v.as_str()) {
                Ok(Some(n.to_string()))
            } else {
                Ok(serde_json::to_string(&serde_json::Value::Object(map)).ok())
            }
        }
        Some(serde_json::Value::Array(arr)) => {
            Ok(serde_json::to_string(&serde_json::Value::Array(arr)).ok())
        }
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        _ => Ok(None),
    }
}

/// Helper to deserialize artist from object or string
pub fn deserialize_artist<'de, D>(deserializer: D) -> Result<Option<QobuzArtist>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::Object(map)) => {
            let id = map.get("id").and_then(|v| {
                if let Some(n) = v.as_i64() {
                    Some(n)
                } else if let Some(s) = v.as_str() {
                    s.parse::<i64>().ok()
                } else {
                    None
                }
            });
            let name = map.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(Some(QobuzArtist { id, name }))
        }
        Some(serde_json::Value::String(s)) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(QobuzArtist { id: None, name: Some(s) }))
            }
        }
        _ => Ok(None),
    }
}

/// Helper to deserialize label from object or string
pub fn deserialize_label<'de, D>(deserializer: D) -> Result<Option<QobuzLabel>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::Object(map)) => {
            let id = map.get("id").and_then(|v| {
                if let Some(n) = v.as_i64() {
                    Some(n)
                } else if let Some(s) = v.as_str() {
                    s.parse::<i64>().ok()
                } else {
                    None
                }
            });
            let name = map.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            Ok(Some(QobuzLabel { id, name }))
        }
        Some(serde_json::Value::String(s)) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(QobuzLabel { id: None, name: Some(s) }))
            }
        }
        _ => Ok(None),
    }
}

/// Helper to deserialize tracks container from either { items: [...], total: ... } or [ ... ]
pub fn deserialize_tracks_container<'de, D>(deserializer: D) -> Result<Option<QobuzTracksContainer>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    let val = match value {
        Some(v) => v,
        None => return Ok(None),
    };

    match val {
        serde_json::Value::Object(map) => {
            if let Some(items_val) = map.get("items") {
                let items: Vec<QobuzTrack> = serde_json::from_value(items_val.clone()).map_err(D::Error::custom)?;
                let total = map.get("total")
                    .and_then(|t| t.as_i64())
                    .map(|t| t as i32)
                    .unwrap_or(items.len() as i32);
                Ok(Some(QobuzTracksContainer { items, total }))
            } else {
                Ok(Some(QobuzTracksContainer { items: Vec::new(), total: 0 }))
            }
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<QobuzTrack> = serde_json::from_value(serde_json::Value::Array(arr)).map_err(D::Error::custom)?;
            let total = items.len() as i32;
            Ok(Some(QobuzTracksContainer { items, total }))
        }
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

/// Qobuz credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzCredentials {
    pub user_auth_token: String,
    pub user_id: Option<String>,
}

/// Qobuz track from API
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzTrack {
    #[serde(deserialize_with = "deserialize_id_i64")]
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_duration_secs")]
    pub duration: i64,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_stringify")]
    pub copyright: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_stringify")]
    pub performers: Option<String>,
    #[serde(default, deserialize_with = "deserialize_artist")]
    pub composer: Option<QobuzArtist>,
    #[serde(default, deserialize_with = "deserialize_string_or_stringify")]
    pub work: Option<String>,
    #[serde(default)]
    pub track_number: Option<i32>,
    #[serde(default)]
    pub media_number: Option<i32>, // This is the disc number in Qobuz API
    #[serde(default)]
    pub maximum_bit_depth: Option<i32>,
    #[serde(default)]
    pub maximum_sampling_rate: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_artist")]
    pub performer: Option<QobuzArtist>,
    #[serde(default)]
    pub album: Option<QobuzAlbum>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QobuzArtist {
    #[serde(default, deserialize_with = "deserialize_opt_id_i64")]
    pub id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QobuzLabel {
    #[serde(default, deserialize_with = "deserialize_opt_id_i64")]
    pub id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbum {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_id_i64")]
    pub released_at: Option<i64>,
    #[serde(default)]
    pub image: Option<QobuzImage>,
    #[serde(default, deserialize_with = "deserialize_label")]
    pub label: Option<QobuzLabel>,
    #[serde(default, deserialize_with = "deserialize_string_or_stringify")]
    pub upc: Option<String>,
    #[serde(default, deserialize_with = "deserialize_artist")]
    pub artist: Option<QobuzArtist>,
    #[serde(default, deserialize_with = "deserialize_tracks_container")]
    pub tracks: Option<QobuzTracksContainer>,
    // FIX 2026-08-25 (matriz de metadatos): la API de Qobuz trae el género del
    // álbum; antes ni se deserializaba y `tracks.genre` quedaba NULL al 100%.
    #[serde(default)]
    pub genre: Option<QobuzGenre>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzGenre {
    #[serde(default)]
    pub name: Option<String>,
}

impl QobuzAlbum {
    /// Nombre de género plano para `OriginTrackMetadata.genre`.
    pub fn genre_name(&self) -> Option<String> {
        self.genre.as_ref().and_then(|g| g.name.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzImage {
    #[serde(default)]
    pub small: Option<String>,
    #[serde(default)]
    pub large: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzFavoritesResponse {
    pub tracks: QobuzTracksContainer,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QobuzTracksContainer {
    #[serde(default)]
    pub items: Vec<QobuzTrack>,
    #[serde(default)]
    pub total: i32,
}

/// Qobuz albums favorites response (/favorite/getUserFavorites?type=albums)
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbumsResponse {
    pub albums: QobuzAlbumsContainer,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QobuzAlbumsContainer {
    #[serde(default)]
    pub items: Vec<QobuzAlbum>,
    #[serde(default)]
    pub total: i32,
}

/// Qobuz purchases response (/purchase/getUserPurchases)
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzPurchasesResponse {
    pub albums: QobuzAlbumsContainer,
}

/// Qobuz playlists response (/playlist/getUserPlaylists)
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzPlaylistsResponse {
    pub playlists: QobuzPlaylistsContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzPlaylistsContainer {
    pub items: Vec<QobuzPlaylistMeta>,
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzPlaylistMeta {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_public: Option<bool>,
    #[serde(default)]
    pub is_collaborative: Option<bool>,
    #[serde(default)]
    pub owner: Option<QobuzPlaylistOwner>,
    #[serde(default)]
    pub tracks_count: Option<i32>,
    #[serde(default)]
    pub images300: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzPlaylistOwner {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
}

/// Qobuz playlist detail with tracks (/playlist/get?extra=tracks)
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzPlaylistDetail {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub tracks: Option<QobuzTracksContainer>,
}

/// Qobuz artists favorites response (/favorite/getUserFavorites?type=artists)
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzArtistsResponse {
    pub artists: QobuzArtistsContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzArtistsContainer {
    pub items: Vec<QobuzArtist>,
    pub total: i32,
}

/// Search result from Qobuz API
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzSearchResponse {
    pub tracks: Option<QobuzTracksContainer>,
}

/// Simplified search result for migration matching
#[derive(Debug, Clone, Serialize)]
pub struct QobuzSearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i64,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<f64>,
}

/// Qobuz API client
pub struct QobuzClient {
    client: Client,
    app_id: String,
    app_secret: String,
    user_auth_token: Option<String>,
}

impl QobuzClient {
    fn build_client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            client: Self::build_client(),
            app_id,
            app_secret,
            user_auth_token: None,
        }
    }

    pub fn new_with_token(app_id: String, app_secret: String, token: String) -> Self {
        Self {
            client: Self::build_client(),
            app_id,
            app_secret,
            user_auth_token: Some(token),
        }
    }

    /// Sign a Qobuz API request
    fn sign_request(&self, method: &str, params: &mut Vec<(&str, String)>) {
        // 1. Sort parameters alphabetically by key
        params.sort_by(|a, b| a.0.cmp(b.0));

        // 2. Concatenate: key1val1key2val2...
        // Qobuz signs the method WITHOUT slashes (e.g. "userlogin" instead of "user/login")
        let mut sig_base = method.replace('/', "").to_string();
        for (key, val) in params.iter() {
            sig_base.push_str(key);
            sig_base.push_str(val);
        }

        // 3. Append app secret
        sig_base.push_str(&self.app_secret);

        // 4. Calculate MD5 hash
        let digest = md5::compute(sig_base.as_bytes());
        let sig = format!("{:x}", digest);

        // 5. Add request_sig to params
        params.push(("request_sig", sig));
    }

    async fn api_request<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: Vec<(&str, String)>,
        require_auth: bool,
    ) -> Result<T, String> {
        let mut all_params = params;
        
        // Add app_id if not present
        if !all_params.iter().any(|(k, _)| *k == "app_id") {
            all_params.push(("app_id", self.app_id.clone()));
        }

        // Sign the request
        self.sign_request(method, &mut all_params);

        // Build URL
        let mut url = format!("{}/{}", QOBUZ_API_BASE, method);
        for (i, (key, val)) in all_params.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(key);
            url.push('=');
            url.push_str(&urlencoding::encode(val));
        }

        let mut request = self.client.get(&url);
        request = request.header("User-Agent", crate::download::http_client::get_user_agent());
        request = request.header("X-App-Id", &self.app_id);

        if let Some(token) = &self.user_auth_token {
            if !token.trim().is_empty() {
                request = request.header("X-User-Auth-Token", token);
            } else if require_auth {
                return Err("Authentication required".to_string());
            }
        } else if require_auth {
            return Err("Authentication required".to_string());
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            tracing::warn!("[QobuzClient] API error for {} ({}): {}", method, status, text);
            return Err(format!("Qobuz API error ({}): {}", status, text));
        }

        serde_json::from_str(&text).map_err(|e| {
            tracing::error!("[QobuzClient] Deserialization failure for {}: {} (raw: {})", method, e, &text[..text.len().min(300)]);
            format!(
                "Failed to parse Qobuz response for {}: {} (raw: {})",
                method,
                e,
                &text[..text.len().min(300)]
            )
        })
    }

    /// Login with username/password to get user auth token
    pub async fn login(&self, username: &str, password: &str) -> Result<String, String> {
        let params = vec![
            ("email", username.to_string()),
            ("password", password.to_string()),
        ];

        let result: serde_json::Value = self.api_request("user/login", params, false).await?;
        
        let token = result["user"]["auth_token"]
            .as_str()
            .ok_or_else(|| format!("No auth token in login response: {}", result))?;
            
        Ok(token.to_string())
    }
    /// Get user's favorite tracks (paginated)
    pub async fn get_favorites(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzFavoritesResponse, String> {
        let params = vec![
            ("type", "tracks".to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];

        self.api_request("favorite/getUserFavorites", params, true).await
    }


    /// Get user's favorite albums (paginated)
    pub async fn get_favorite_albums(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzAlbumsResponse, String> {
        let params = vec![
            ("type", "albums".to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];

        self.api_request("favorite/getUserFavorites", params, true).await
    }

    /// Get user's purchased albums (paginated)
    pub async fn get_purchases(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzPurchasesResponse, String> {
        let params = vec![
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];

        self.api_request("purchase/getUserPurchases", params, true).await
    }

    /// Get user's playlists (paginated)
    pub async fn get_playlists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzPlaylistsResponse, String> {
        let params = vec![
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];

        self.api_request("playlist/getUserPlaylists", params, true).await
    }

    /// Get playlist tracks (paginated)
    pub async fn get_playlist_tracks(
        &self,
        playlist_id: i64,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzPlaylistDetail, String> {
        let params = vec![
            ("playlist_id", playlist_id.to_string()),
            ("extra", "tracks".to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];

        self.api_request("playlist/get", params, true).await
    }

    /// Get user's favorite artists (paginated)
    pub async fn get_favorite_artists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzArtistsResponse, String> {
        let params = vec![
            ("type", "artists".to_string()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];

        self.api_request("favorite/getUserFavorites", params, true).await
    }

    /// Add a track to Qobuz favorites (favorite/create?track_ids=...)
    pub async fn add_favorite_track(&self, track_id: i64) -> Result<(), String> {
        let params = vec![("track_ids", track_id.to_string())];
        let _: serde_json::Value = self.api_request("favorite/create", params, true).await?;
        Ok(())
    }

    /// Remove a track from Qobuz favorites (favorite/delete?track_ids=...)
    pub async fn remove_favorite_track(&self, track_id: i64) -> Result<(), String> {
        let params = vec![("track_ids", track_id.to_string())];
        let _: serde_json::Value = self.api_request("favorite/delete", params, true).await?;
        Ok(())
    }

    /// Add an album to Qobuz favorites (favorite/create?album_ids=...)
    pub async fn add_favorite_album(&self, album_id: &str) -> Result<(), String> {
        let params = vec![("album_ids", album_id.to_string())];
        let _: serde_json::Value = self.api_request("favorite/create", params, true).await?;
        Ok(())
    }

    /// Remove an album from Qobuz favorites (favorite/delete?album_ids=...)
    pub async fn remove_favorite_album(&self, album_id: &str) -> Result<(), String> {
        let params = vec![("album_ids", album_id.to_string())];
        let _: serde_json::Value = self.api_request("favorite/delete", params, true).await?;
        Ok(())
    }

    /// Add an artist to Qobuz favorites (favorite/create?artist_ids=...)
    pub async fn add_favorite_artist(&self, artist_id: i64) -> Result<(), String> {
        let params = vec![("artist_ids", artist_id.to_string())];
        let _: serde_json::Value = self.api_request("favorite/create", params, true).await?;
        Ok(())
    }

    /// Remove an artist from Qobuz favorites (favorite/delete?artist_ids=...)
    pub async fn remove_favorite_artist(&self, artist_id: i64) -> Result<(), String> {
        let params = vec![("artist_ids", artist_id.to_string())];
        let _: serde_json::Value = self.api_request("favorite/delete", params, true).await?;
        Ok(())
    }

    /// Get full album details (album/get?album_id=...)
    pub async fn get_album_full(&self, album_id: &str) -> Result<QobuzAlbum, String> {
        let params = vec![
            ("album_id", album_id.to_string()),
        ];

        self.api_request("album/get", params, false).await
    }

    /// Mass enrich album metadata (Sequential with rate limit)
    pub async fn enrich_albums(
        &self,
        db: &SqlitePool,
        window: Option<&tauri::Window>
    ) -> Result<super::ImportResult, String> {
        // 1. Find candidate albums
        let candidates: Vec<(String,)> = sqlx::query_as(
            "SELECT qobuz_id FROM albums WHERE qobuz_id IS NOT NULL AND label IS NULL"
        )
        .fetch_all(db)
        .await
        .map_err(|e| e.to_string())?;

        let total = candidates.len();
        if total == 0 {
            return Ok(super::ImportResult { imported: 0, skipped: 0 });
        }

        tracing::info!("Qobuz: Starting enrichment for {} albums", total);
        if let Some(w) = window {
            crate::commands::emit_import_progress(w, "qobuz_enrichment", "started", 0, total as u64, 
                &format!("Enriching metadata for {} Qobuz albums...", total));
        }

        let mut enriched = 0;
        let mut skipped = 0;

        // 2. Process sequentially (1 req/sec to be safe)
        for (i, (qobuz_id,)) in candidates.into_iter().enumerate() {
            match self.get_album_full(&qobuz_id).await {
                Ok(full_album) => {
                    let _ = sqlx::query(
                        "UPDATE albums SET label = ?, upc = ? WHERE qobuz_id = ?"
                    )
                    .bind(full_album.label.as_ref().and_then(|l| l.name.as_deref()))
                    .bind(full_album.upc)
                    .bind(&qobuz_id)
                    .execute(db)
                    .await;
                    
                    enriched += 1;
                },
                Err(e) => {
                    tracing::error!("Qobuz enrichment failed for {}: {}", qobuz_id, e);
                    skipped += 1;
                }
            }

            if let Some(w) = window {
                if (i + 1) % 5 == 0 || i + 1 == total {
                    crate::commands::emit_import_progress(w, "qobuz_enrichment", "progress", (i + 1) as u64, total as u64, 
                        &format!("Enriched {}/{} albums...", i + 1, total));
                }
            }

            // Rate limit: 1 second between requests
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        Ok(super::ImportResult { imported: enriched, skipped })
    }

    pub async fn import_library(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        let mut offset = 0;
        let limit = 50;
        let mut imported = 0;
        let mut skipped = 0;

        let qobuz_service_id = self.get_service_id(db, "qobuz").await?;

        loop {
            let page = self.get_favorites(offset, limit).await?;

            if page.tracks.items.is_empty() {
                break;
            }

            for track in &page.tracks.items {
                // Get or create artist
                let artist_name = track
                    .performer
                    .as_ref()
                    .and_then(|a| a.name.clone())
                    .unwrap_or_default();
                let artist_id = self.get_or_create_artist(db, &artist_name).await?;

                // Get or create album (if present)
                let album_id = if let Some(ref album) = track.album {
                    Some(self.get_or_create_album(db, album, artist_id).await?)
                } else {
                    None
                };

                // Get or create track
                let track_id = self.get_or_create_track(db, track, album_id).await?;

                // Link artist to track
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                )
                .bind(track_id)
                .bind(artist_id)
                .execute(db)
                .await;

                // F4.3: Detect featured artists in track title and link with role = 'featured'
                let track_title = track.title.as_deref().unwrap_or("");
                for feat_name in syncify_core_domain::metadata::extract_featured_artists(track_title) {
                    if let Ok(feat_artist_id) = self.get_or_create_artist(db, &feat_name).await {
                        if feat_artist_id != artist_id {
                            let _ = sqlx::query(
                                "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')"
                            )
                            .bind(track_id)
                            .bind(feat_artist_id)
                            .execute(db)
                            .await;
                        }
                    }
                }

                // Add to library entry
                let result = sqlx::query(
                    "INSERT OR IGNORE INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1)"
                )
                .bind(account_id)
                .bind(track_id)
                .execute(db)
                .await
                .map_err(|e| format!("DB error: {}", e))?;

                if result.rows_affected() > 0 {
                    imported += 1;
                } else {
                    skipped += 1;
                }

                // Add track source with quality info
                let quality_score = self.compute_quality_score(track);
                let _ = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO track_sources 
                    (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) 
                    VALUES (?, ?, ?, 'FLAC', ?, ?, ?, 1)
                    "#
                )
                .bind(track_id)
                .bind(qobuz_service_id)
                .bind(track.id.to_string())
                .bind(track.maximum_bit_depth)
                .bind(track.maximum_sampling_rate.map(|r| (r * 1000.0) as i32))
                .bind(quality_score)
                .execute(db)
                .await;
            }

            offset += limit;

            tracing::info!("Qobuz import: {} imported so far...", imported);

            if page.tracks.items.len() < limit as usize {
                break;
            }
        }

        Ok(super::ImportResult { imported, skipped })
    }

    pub fn compute_quality_score(&self, track: &QobuzTrack) -> i32 {
        let mut score = 1000; // FLAC base

        if let Some(depth) = track.maximum_bit_depth {
            score += depth * 10;
        }

        if let Some(rate) = track.maximum_sampling_rate {
            score += (rate as i32).min(200);
        }

        score
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
        let clean_name = syncify_core_domain::metadata::sanitize_artist_name(name);
        if clean_name.is_empty() {
            return Err("Cannot create artist with empty name".to_string());
        }
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
            .bind(&clean_name)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) ON CONFLICT(name) DO UPDATE SET id=id RETURNING id")
            .bind(&clean_name)
            .fetch_one(db)
            .await
            .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(artist_id)
    }

    pub async fn get_or_create_album(
        &self,
        db: &SqlitePool,
        album: &QobuzAlbum,
        primary_artist_id: i64,
    ) -> Result<i64, String> {
        // Get cover art URL (prefer large)
        let cover_url = album
            .image
            .as_ref()
            .and_then(|i| i.large.clone().or(i.small.clone()));

        let release_date = album.released_at.map(|ts| ts.to_string());

        let label_name = album.label.as_ref().and_then(|l| l.name.as_deref());

        // Create or update album by qobuz_id
        let album_id: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
            r#"
            INSERT INTO albums (title, cover_art_url, release_date, qobuz_id, label, upc) 
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(qobuz_id) WHERE qobuz_id IS NOT NULL DO UPDATE SET
                cover_art_url = COALESCE(albums.cover_art_url, excluded.cover_art_url),
                label = COALESCE(albums.label, excluded.label),
                upc = COALESCE(albums.upc, excluded.upc)
            RETURNING id
            "#
        )
        .bind(album.title.as_deref().unwrap_or_default())
        .bind(&cover_url)
        .bind(&release_date)
        .bind(&album.id)
        .bind(label_name)
        .bind(&album.upc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Album upsert failed: {}", e))?;

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
        track: &QobuzTrack,
        album_id: Option<i64>,
    ) -> Result<i64, String> {
        let qobuz_id = track.id.to_string();

        // 1. Try to find by qobuz_id (Authoritative)
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE qobuz_id = ?")
            .bind(&qobuz_id)
            .fetch_one(db)
            .await
        {
            // Update missing metadata
            let _ = sqlx::query(
                r#"
                UPDATE tracks SET 
                    album_id = COALESCE(album_id, ?),
                    track_number = COALESCE(track_number, ?),
                    disc_number = COALESCE(disc_number, ?)
                WHERE id = ?
                "#
            )
            .bind(album_id)
            .bind(track.track_number)
            .bind(track.media_number)
            .bind(row.0)
            .execute(db)
            .await;
            
            return Ok(row.0);
        }

        // 2. Try to find by ISRC
        if let Some(ref isrc) = track.isrc {
            if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_one(db)
                .await
            {
                // Update qobuz_id and other metadata
                let _ = sqlx::query(
                    r#"
                    UPDATE tracks SET 
                        qobuz_id = COALESCE(qobuz_id, ?),
                        album_id = COALESCE(album_id, ?),
                        track_number = COALESCE(track_number, ?),
                        disc_number = COALESCE(disc_number, ?)
                    WHERE id = ?
                    "#
                )
                .bind(&qobuz_id)
                .bind(album_id)
                .bind(track.track_number)
                .bind(track.media_number)
                .bind(row.0)
                .execute(db)
                .await;
                
                return Ok(row.0);
            }
        }

        // 3. Validate title
        let title = track.title
            .as_ref()
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| format!("Track {} has no title, skipping", track.id))?;
        let clean_title = syncify_core_domain::metadata::sanitize_track_title(title);

        // 4. Create new track
        let track_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO tracks (title, album_id, duration_ms, isrc, track_number, disc_number, qobuz_id) 
            VALUES (?, ?, ?, ?, ?, ?, ?) 
            RETURNING id
            "#,
        )
        .bind(&clean_title)
        .bind(album_id)
        .bind(track.duration * 1000)
        .bind(&track.isrc)
        .bind(track.track_number)
        .bind(track.media_number)
        .bind(&qobuz_id)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Track insert failed: {}", e))?;

        Ok(track_id)
    }

    /// Search for tracks by query string (title, artist, etc.)
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<QobuzSearchResult>, String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/track/search", QOBUZ_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("X-User-Auth-Token", token)
            .header("X-App-Id", &self.app_id)
            .query(&[("query", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Read response: {}", e))?;

        if !status.is_success() {
            return Err(format!(
                "Qobuz search failed ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        let search_resp: QobuzSearchResponse =
            serde_json::from_str(&text).map_err(|e| format!("Parse search response: {}", e))?;

        let results = search_resp
            .tracks
            .map(|t| t.items)
            .unwrap_or_default()
            .into_iter()
            .map(|track| QobuzSearchResult {
                track_id: track.id.to_string(),
                title: track.title.clone().unwrap_or_default(),
                artist: track.performer.and_then(|p| p.name.clone()).unwrap_or_default(),
                album: track.album.and_then(|a| a.title.clone()),
                isrc: track.isrc,
                duration_ms: track.duration * 1000,
                bit_depth: track.maximum_bit_depth,
                sample_rate: track.maximum_sampling_rate,
            })
            .collect();

        Ok(results)
    }

    /// Search for a track by ISRC code (most reliable matching method)
    pub async fn search_by_isrc(&self, isrc: &str) -> Result<Option<QobuzSearchResult>, String> {
        // Qobuz search API supports ISRC queries
        let results = self.search_track(isrc, 5).await?;

        // Find exact ISRC match
        let match_result = results.into_iter().find(|r| {
            r.isrc
                .as_ref()
                .map(|i| i.eq_ignore_ascii_case(isrc))
                .unwrap_or(false)
        });

        Ok(match_result)
    }

    /// Add a track to user's favorites (for migration transfer)
    /// Includes retry logic with exponential backoff for rate limiting
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/favorite/create", QOBUZ_API_BASE);
        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let response = self
                .client
                .get(&url)
                .header("X-User-Auth-Token", token)
                .header("X-App-Id", &self.app_id)
                .query(&[("track_ids", track_id)])
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        tracing::info!("Added track {} to Qobuz favorites", track_id);
                        return Ok(());
                    } else if status.as_u16() == 429 || status.as_u16() >= 500 {
                        // Rate limited or server error - retry
                        let text = resp.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "Qobuz add_to_favorites attempt {} failed ({}), retrying...",
                            attempt + 1,
                            status
                        );
                    } else {
                        // Client error - don't retry
                        let text = resp.text().await.unwrap_or_default();
                        return Err(format!(
                            "Add to favorites failed ({}): {}",
                            status,
                            &text[..text.len().min(200)]
                        ));
                    }
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!(
                        "Qobuz add_to_favorites attempt {} failed: {}, retrying...",
                        attempt + 1,
                        e
                    );
                }
            }

            // Exponential backoff: 500ms, 1s, 2s
            if attempt < max_retries - 1 {
                let delay = 500 * (1 << attempt);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }

        Err(format!(
            "Add to favorites failed after {} retries: {}",
            max_retries, last_error
        ))
    }

    /// Match a track from another service by metadata (fallback when no ISRC)
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<QobuzSearchResult>, String> {
        let query = format!("{} {}", artist, title);
        let results = self.search_track(&query, 10).await?;

        // Normalize for comparison
        let normalize = |s: &str| {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
        };
        let target_title = normalize(title);
        let target_artist = normalize(artist);

        // Find best match
        let best_match = results
            .into_iter()
            .filter(|r| {
                let r_title = normalize(r.title.as_str());
                let r_artist = normalize(&r.artist);
                r_title.contains(&target_title)
                    || target_title.contains(&r_title)
                    || (r_artist.contains(&target_artist) && r_title.len() > 0)
            })
            .next();

        Ok(best_match)
    }

    /// Import user's playlists into the database
    pub async fn import_playlists(
        &self,
        db: &SqlitePool,
        account_id: i64,
        app_handle: &AppHandle,
    ) -> Result<crate::services::ImportResult, String> {
        tracing::info!("Qobuz: Starting playlist import for account {}", account_id);

        // Fetch user's playlists
        let response = self.get_playlists(0, 50).await?;
        let total = response.playlists.total;

        // Emit started event
        let _ = app_handle.emit(
            "import-progress",
            serde_json::json!({
                "service": "qobuz_playlists",
                "status": "started",
                "current": 0,
                "total": total,
                "message": format!("Found {} Qobuz playlists", total)
            }),
        );

        let mut imported = 0;
        let mut skipped = 0;

        for playlist in response.playlists.items {
            tracing::debug!("Qobuz: Importing playlist '{}' ({})", playlist.name, playlist.id);

            let image_url = playlist.images300.as_ref().and_then(|imgs| imgs.first().cloned());
            let pl_id_str = playlist.id.to_string();
            let res = crate::commands::upsert_playlist_and_source(
                db,
                account_id,
                &pl_id_str,
                &playlist.name,
                playlist.description.as_deref(),
                playlist.owner.as_ref().and_then(|o| o.name.as_deref()),
                playlist.is_public.unwrap_or(false) as i32,
                playlist.is_collaborative.unwrap_or(false) as i32,
                image_url.as_deref(),
                playlist.tracks_count.unwrap_or(0),
            )
            .await;

            match res {
                Ok(pid) => {
                    imported += 1;

                    let mut track_offset = 0i32;
                    let track_limit = 50i32;
                    let mut track_position = 0i32;

                    loop {
                            let detail = match self.get_playlist_tracks(playlist.id, track_offset, track_limit).await {
                                Ok(d) => d,
                                Err(e) => {
                                    tracing::warn!("Qobuz: Failed to fetch tracks for playlist '{}': {}", playlist.name, e);
                                    break;
                                }
                            };

                            let tracks = match detail.tracks {
                                Some(t) if !t.items.is_empty() => t,
                                _ => break,
                            };

                            let page_len = tracks.items.len();
                            for track in &tracks.items {
                                let process = async {
                                    let artist_name = track.performer.as_ref()
                                        .and_then(|a| a.name.clone())
                                        .unwrap_or_else(|| "Unknown".to_string());
                                    let artist_id = self.get_or_create_artist(db, &artist_name).await?;
                                    let album_id = if let Some(ref album) = track.album {
                                        Some(self.get_or_create_album(db, album, artist_id).await?)
                                    } else { None };
                                    let track_id = self.get_or_create_track(db, track, album_id).await?;
                                    let _ = sqlx::query(
                                        "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                                    ).bind(track_id).bind(artist_id).execute(db).await;

                                    // F4.3: Detect featured artists in track title and link with role = 'featured'
                                    let track_title = track.title.as_deref().unwrap_or("");
                                    for feat_name in syncify_core_domain::metadata::extract_featured_artists(track_title) {
                                        if let Ok(feat_artist_id) = self.get_or_create_artist(db, &feat_name).await {
                                            if feat_artist_id != artist_id {
                                                let _ = sqlx::query(
                                                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')"
                                                ).bind(track_id).bind(feat_artist_id).execute(db).await;
                                            }
                                        }
                                    }
                                    let _ = sqlx::query(
                                        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                                    ).bind(pid).bind(track_id).bind(track_position).execute(db).await;
                                    Ok::<(), String>(())
                                };
                                if let Err(e) = process.await {
                                    tracing::warn!("Qobuz: Failed to import track in playlist '{}': {}", playlist.name, e);
                                }
                                track_position += 1;
                            }

                            track_offset += track_limit;
                            if page_len < track_limit as usize { break; }
                        }
                },
                Err(e) => {
                    tracing::error!("Qobuz: Failed to insert playlist {}: {}", playlist.id, e);
                    skipped += 1;
                }
            }

            // Emit progress
            let _ = app_handle.emit(
                "import-progress",
                serde_json::json!({
                    "service": "qobuz_playlists",
                    "status": "progress",
                    "current": imported + skipped,
                    "total": total,
                    "message": format!("Importing: {}", playlist.name)
                }),
            );
        }

        // Emit complete
        let _ = app_handle.emit(
            "import-complete",
            serde_json::json!({
                "service": "qobuz_playlists",
                "imported": imported,
                "skipped": skipped,
                "message": format!("Successfully imported {} Qobuz playlists", imported)
            }),
        );

        Ok(crate::services::ImportResult {
            imported: imported as i32,
            skipped: skipped as i32,
        })
    }
}

/// Inspect physical FLAC file STREAMINFO header to extract real bit depth and sample rate (F3.4).
pub fn extract_flac_streaminfo(path: &std::path::Path) -> Option<(i32, f64)> {
    if let Ok(tag) = metaflac::Tag::read_from_path(path) {
        if let Some(info) = tag.get_streaminfo() {
            return Some((info.bits_per_sample as i32, info.sample_rate as f64));
        }
    }
    if let Ok(mut file) = std::fs::File::open(path) {
        use std::io::Read;
        let mut buf = [0u8; 64];
        if let Ok(n) = file.read(&mut buf) {
            if let Some(info) = syncify_core_domain::byte_validators::AudioByteValidator::parse_flac_streaminfo(&buf[..n]) {
                return Some((info.bits_per_sample as i32, info.sample_rate as f64));
            }
        }
    }
    None
}
