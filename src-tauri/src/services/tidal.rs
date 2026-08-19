//! Tidal service - Authentication and library import
//!
//! Handles Tidal API access and importing favorites.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const TIDAL_API_BASE: &str = "https://api.tidal.com/v1";

/// Default TTL for unavailable / unlisted album expansion cache (7 days)
pub const DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS: i64 = 7 * 86400;

/// Tidal Album Expansion Classification Status (S145)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TidalAlbumExpansionStatus {
    Available,
    UnavailableFromProvider,
    RegionRestricted,
    TemporarilyFailed,
    AuthFailed,
    RateLimited,
    MalformedResponse,
}

impl TidalAlbumExpansionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::UnavailableFromProvider => "UnavailableFromProvider",
            Self::RegionRestricted => "RegionRestricted",
            Self::TemporarilyFailed => "TemporarilyFailed",
            Self::AuthFailed => "AuthFailed",
            Self::RateLimited => "RateLimited",
            Self::MalformedResponse => "MalformedResponse",
        }
    }

    pub fn from_str_name(s: &str) -> Self {
        match s {
            "UnavailableFromProvider" => Self::UnavailableFromProvider,
            "RegionRestricted" => Self::RegionRestricted,
            "TemporarilyFailed" => Self::TemporarilyFailed,
            "AuthFailed" => Self::AuthFailed,
            "RateLimited" => Self::RateLimited,
            "MalformedResponse" => Self::MalformedResponse,
            _ => Self::Available,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalArtist {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalAlbum {
    #[serde(rename = "id")]
    pub tidal_id: i64,
    pub title: String,
    pub cover: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(rename = "numberOfTracks")]
    pub total_tracks: Option<i32>,
    pub artist: Option<TidalArtist>,
    #[serde(default)]
    pub upc: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

impl TidalAlbum {
    pub fn cover_url(&self) -> Option<String> {
        self.cover.as_ref().map(|c| {
            if c.starts_with("http") {
                c.clone()
            } else {
                format!("https://resources.tidal.com/images/{}/320x320.jpg", c.replace('-', "/"))
            }
        })
    }
}

/// Tidal track from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalTrack {
    pub id: i64,
    pub title: String,
    pub duration: i64,
    pub isrc: Option<String>,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub artist: Option<TidalArtist>,
    pub album: Option<TidalAlbum>,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i32>,
    #[serde(rename = "volumeNumber")]
    pub disc_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TidalAlbumTrackItem {
    Wrapped { item: TidalTrack },
    Direct(TidalTrack),
}

impl TidalAlbumTrackItem {
    pub fn track(&self) -> &TidalTrack {
        match self {
            Self::Wrapped { item } => item,
            Self::Direct(t) => t,
        }
    }
}

/// Detailed Tidal Album Expansion Result (S145)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalAlbumExpansionResult {
    pub status: TidalAlbumExpansionStatus,
    pub http_status: Option<u16>,
    pub sub_status: Option<i32>,
    pub reason: Option<String>,
    pub tracks: Vec<TidalAlbumTrackItem>,
}

/// Classify HTTP errors during Tidal album track expansion (S145)
pub fn classify_album_expansion_error(status: reqwest::StatusCode, body: &str) -> (TidalAlbumExpansionStatus, Option<i32>, String) {
    let http_status = status.as_u16();
    let parsed_json: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let sub_status = parsed_json.as_ref().and_then(|v| v.get("subStatus").and_then(|s| s.as_i64()).map(|s| s as i32));
    let user_msg = parsed_json.as_ref()
        .and_then(|v| v.get("userMessage").or_else(|| v.get("error")).or_else(|| v.get("message")).and_then(|m| m.as_str()))
        .unwrap_or(body)
        .to_string();

    let msg_lower = user_msg.to_lowercase();
    let body_lower = body.to_lowercase();

    if http_status == 404 || sub_status == Some(2001) || msg_lower.contains("not found") || body_lower.contains("asset not found") {
        (TidalAlbumExpansionStatus::UnavailableFromProvider, sub_status, user_msg)
    } else if (http_status == 400 || http_status == 403) && (sub_status == Some(4005) || msg_lower.contains("not available in") || msg_lower.contains("country") || msg_lower.contains("region")) {
        (TidalAlbumExpansionStatus::RegionRestricted, sub_status, user_msg)
    } else if http_status == 401 || (http_status == 403 && !msg_lower.contains("region") && !msg_lower.contains("country")) {
        (TidalAlbumExpansionStatus::AuthFailed, sub_status, user_msg)
    } else if http_status == 429 {
        (TidalAlbumExpansionStatus::RateLimited, sub_status, user_msg)
    } else if http_status >= 500 {
        (TidalAlbumExpansionStatus::TemporarilyFailed, sub_status, user_msg)
    } else {
        (TidalAlbumExpansionStatus::MalformedResponse, sub_status, user_msg)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalFavoriteItem {
    pub item: TidalTrack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalFavoriteAlbumItem {
    pub item: TidalAlbum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalPaginated {
    pub items: Vec<TidalFavoriteItem>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalAlbumTracksResponse {
    pub items: Vec<TidalAlbumTrackItem>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalAlbumPaginated {
    pub items: Vec<TidalFavoriteAlbumItem>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalFavoriteArtistItem {
    pub item: TidalArtist,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalArtistPaginated {
    pub items: Vec<TidalFavoriteArtistItem>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPlaylist {
    pub uuid: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "numberOfTracks")]
    pub track_count: i32,
    pub creator: Option<TidalPlaylistCreator>,
    pub public_playlist: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPlaylistCreator {
    pub id: i64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPlaylistsResponse {
    pub items: Vec<TidalPlaylist>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPlaylistTracksResponse {
    pub items: Vec<TidalPlaylistTrackItem>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPlaylistTrackItem {
    pub item: TidalTrack,
    #[serde(rename = "type")]
    pub item_type: String,
}

/// Search response from Tidal API
#[derive(Debug, Clone, Deserialize)]
pub struct TidalSearchResponse {
    pub tracks: Option<TidalSearchTracks>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalSearchTracks {
    pub items: Vec<TidalTrack>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
}

/// Simplified search result for migration matching
#[derive(Debug, Clone, Serialize)]
pub struct TidalSearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i64,
    pub quality: Option<String>,
}

/// Tidal API client
pub struct TidalClient {
    client: Client,
    access_token: String,
    user_id: Option<String>,
    country_code: String,
}

impl TidalClient {
    pub fn new(access_token: String) -> Self {
        Self {
            client: Client::new(),
            access_token,
            user_id: None,
            country_code: "MX".into(), // Default, will be updated
        }
    }

    pub fn with_user(mut self, user_id: String, country_code: String) -> Self {
        self.user_id = Some(user_id);
        self.country_code = country_code;
        self
    }

    fn handle_api_error(status: reqwest::StatusCode, body: &str, endpoint: &str) -> String {
        let sanitized_body = crate::commands::redact_auth_payload(body);
        if status.as_u16() == 401 || status.as_u16() == 403 {
            tracing::warn!(
                endpoint = %endpoint,
                http_status = status.as_u16(),
                credentials_invalid = true,
                "[Tidal Auth Diagnostics] Tidal API authentication rejected (HTTP {})", status.as_u16()
            );
            format!("RequiresAuth: Tidal API authentication failed (HTTP {}): {}", status, sanitized_body)
        } else {
            format!("Tidal API error {}: {}", status, sanitized_body)
        }
    }

    /// Get user's favorite tracks (paginated)
    pub async fn get_favorites(&self, offset: i32, limit: i32) -> Result<TidalPaginated, String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;

        let url = format!("{}/users/{}/favorites/tracks", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Self::handle_api_error(status, &body, &url));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Get user's favorite albums (paginated)
    pub async fn get_favorite_albums(&self, offset: i32, limit: i32) -> Result<TidalAlbumPaginated, String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;

        let url = format!("{}/users/{}/favorites/albums", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Self::handle_api_error(status, &body, &url));
        }

        response.json::<TidalAlbumPaginated>().await.map_err(|e| format!("Failed to parse albums: {}", e))
    }

    /// Get user's favorite artists (paginated)
    pub async fn get_favorite_artists(&self, offset: i32, limit: i32) -> Result<TidalArtistPaginated, String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;

        let url = format!("{}/users/{}/favorites/artists", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Self::handle_api_error(status, &body, &url));
        }

        response.json::<TidalArtistPaginated>().await.map_err(|e| format!("Failed to parse artists: {}", e))
    }

    /// Add a track to Tidal favorites (POST /users/{id}/favorites/tracks)
    pub async fn add_favorite_track(&self, track_id: i64) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/favorites/tracks", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .form(&[("trackId", &track_id.to_string()), ("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Self::handle_api_error(status, &body, &url))
        }
    }

    /// Remove a track from Tidal favorites (DELETE /users/{id}/favorites/tracks/{trackId})
    pub async fn remove_favorite_track(&self, track_id: i64) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/favorites/tracks/{}", TIDAL_API_BASE, user_id, track_id);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .query(&[("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Self::handle_api_error(status, &body, &url))
        }
    }

    /// Add an album to Tidal favorites (POST /users/{id}/favorites/albums)
    pub async fn add_favorite_album(&self, album_id: i64) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/favorites/albums", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .form(&[("albumId", &album_id.to_string()), ("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Self::handle_api_error(status, &body, &url))
        }
    }

    /// Remove an album from Tidal favorites (DELETE /users/{id}/favorites/albums/{albumId})
    pub async fn remove_favorite_album(&self, album_id: i64) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/favorites/albums/{}", TIDAL_API_BASE, user_id, album_id);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .query(&[("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Self::handle_api_error(status, &body, &url))
        }
    }

    /// Add an artist to Tidal favorites (POST /users/{id}/favorites/artists)
    pub async fn add_favorite_artist(&self, artist_id: i64) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/favorites/artists", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .form(&[("artistId", &artist_id.to_string()), ("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Self::handle_api_error(status, &body, &url))
        }
    }

    /// Remove an artist from Tidal favorites (DELETE /users/{id}/favorites/artists/{artistId})
    pub async fn remove_favorite_artist(&self, artist_id: i64) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/favorites/artists/{}", TIDAL_API_BASE, user_id, artist_id);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .query(&[("countryCode", &self.country_code)])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(Self::handle_api_error(status, &body, &url))
        }
    }

    /// Get user's playlists (paginated)
    pub async fn get_playlists(&self, offset: i32, limit: i32) -> Result<TidalPlaylistsResponse, String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;
        let url = format!("{}/users/{}/playlists", TIDAL_API_BASE, user_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Self::handle_api_error(status, &body, &url));
        }

        response.json().await.map_err(|e| format!("Parse error: {}", e))
    }

    /// Get tracks in a playlist (paginated)
    pub async fn get_playlist_tracks(&self, playlist_id: &str, offset: i32, limit: i32) -> Result<TidalPlaylistTracksResponse, String> {
        let url = format!("{}/playlists/{}/items", TIDAL_API_BASE, playlist_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Self::handle_api_error(status, &body, &url));
        }

        response.json().await.map_err(|e| format!("Failed to parse playlist tracks: {}", e))
    }

    /// Get tracks in an album (paginated)
    pub async fn get_album_tracks(&self, album_id: i64, offset: i32, limit: i32) -> Result<TidalAlbumTracksResponse, String> {
        let url = format!("{}/albums/{}/tracks", TIDAL_API_BASE, album_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Self::handle_api_error(status, &body, &url));
        }

        response.json::<TidalAlbumTracksResponse>().await.map_err(|e| format!("Failed to parse album tracks: {}", e))
    }

    /// Get tracks in an album with granular status classification (S145)
    pub async fn get_album_tracks_expanded(&self, album_id: i64, offset: i32, limit: i32) -> Result<TidalAlbumExpansionResult, String> {
        let url = format!("{}/albums/{}/tracks", TIDAL_API_BASE, album_id);

        let response = match self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("countryCode", self.country_code.as_str()),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(TidalAlbumExpansionResult {
                    status: TidalAlbumExpansionStatus::TemporarilyFailed,
                    http_status: None,
                    sub_status: None,
                    reason: Some(format!("Request failed: {}", e)),
                    tracks: Vec::new(),
                });
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let (expansion_status, sub_status, reason) = classify_album_expansion_error(status, &body);
            return Ok(TidalAlbumExpansionResult {
                status: expansion_status,
                http_status: Some(status.as_u16()),
                sub_status,
                reason: Some(reason),
                tracks: Vec::new(),
            });
        }

        match response.json::<TidalAlbumTracksResponse>().await {
            Ok(data) => Ok(TidalAlbumExpansionResult {
                status: TidalAlbumExpansionStatus::Available,
                http_status: Some(status.as_u16()),
                sub_status: None,
                reason: None,
                tracks: data.items,
            }),
            Err(e) => Ok(TidalAlbumExpansionResult {
                status: TidalAlbumExpansionStatus::MalformedResponse,
                http_status: Some(status.as_u16()),
                sub_status: None,
                reason: Some(format!("Failed to parse album tracks JSON: {}", e)),
                tracks: Vec::new(),
            }),
        }
    }


    /// Import all favorites to database
    pub async fn import_favorites(
        &self,
        db: &SqlitePool,
        account_id: i64,
        window: Option<&tauri::Window>,
    ) -> Result<super::ImportResult, String> {
        let tidal_service_id = self.get_service_id(db, "tidal").await?;
 
        // First, get total count for progress
        let first_page = self.get_favorites(0, 1).await?;
        let total_tracks = first_page.total;
 
        if let Some(w) = window {
            crate::commands::emit_import_progress(w, "tidal", "started", 0, total_tracks as u64, 
                &format!("Starting import of {} favorite tracks...", total_tracks));
        }
        
        let mut offset = 0;
        let limit = 10; // Reduced for constant feedback (S78)
        let mut imported = 0;
        let mut skipped = 0;

        loop {
            let page = self.get_favorites(offset, limit).await?;
            
            if page.items.is_empty() {
                break;
            }

            tracing::debug!("Tidal: Processing {} favorites (Batch Start)", page.items.len());
            
            let mut tx = db.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;

            for item in page.items.iter() {
                let track = &item.item;
                
                // 1. Artist
                let artist_name = track.artist.as_ref().map(|a| a.name.clone()).unwrap_or_default();
                let artist_res: Option<(i64,)> = sqlx::query_as::<sqlx::Sqlite, (i64,)>("INSERT OR IGNORE INTO artists (name) VALUES (?) RETURNING id")
                    .bind(&artist_name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e: sqlx::Error| e.to_string())?;

                let artist_id = if let Some(row) = artist_res {
                    row.0
                } else {
                    sqlx::query_as::<sqlx::Sqlite, (i64,)>("SELECT id FROM artists WHERE name = ?")
                        .bind(&artist_name)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e: sqlx::Error| e.to_string())?
                        .0
                };

                // 2. Album
                let album_id = if let Some(ref album) = track.album {
                    let aid: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
                        "INSERT INTO albums (title, release_date, total_tracks, cover_art_url, tidal_id, label, upc)
                         VALUES (?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT(tidal_id) WHERE tidal_id IS NOT NULL DO UPDATE SET 
                            label = COALESCE(albums.label, excluded.label),
                            upc = COALESCE(albums.upc, excluded.upc)
                         RETURNING id"
                    )
                    .bind(&album.title)
                    .bind(&album.release_date)
                    .bind(album.total_tracks)
                    .bind(album.cover_url())
                    .bind(album.tidal_id)
                    .bind(&album.label)
                    .bind(&album.upc)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e: sqlx::Error| e.to_string())?;
                    
                    let album_id = aid.0;
                    let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
                        .bind(album_id)
                        .bind(artist_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e: sqlx::Error| e.to_string())?;
                    
                    Some(album_id)
                } else {
                    None
                };

                // 3. Track
                let tid: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
                    r#"
                    INSERT INTO tracks (title, album_id, duration_ms, isrc, track_number, disc_number, audio_quality) 
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(isrc) DO UPDATE SET 
                        album_id = COALESCE(tracks.album_id, excluded.album_id),
                        track_number = COALESCE(tracks.track_number, excluded.track_number),
                        disc_number = COALESCE(tracks.disc_number, excluded.disc_number),
                        audio_quality = COALESCE(tracks.audio_quality, excluded.audio_quality)
                    RETURNING id
                    "#,
                )
                .bind(&track.title)
                .bind(album_id)
                .bind(track.duration * 1000)
                .bind(&track.isrc)
                .bind(track.track_number)
                .bind(track.disc_number)
                .bind(&track.audio_quality)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e: sqlx::Error| e.to_string())?;
                
                let track_id = tid.0;

                // 4. Link artist
                let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
                    .bind(track_id)
                    .bind(artist_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e: sqlx::Error| e.to_string())?;

                // Add to library entry
                let result = sqlx::query(
                    "INSERT OR IGNORE INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1)"
                )
                .bind(account_id)
                .bind(track_id)
                .execute(&mut *tx)
                .await
                .map_err(|e: sqlx::Error| e.to_string())?;

                if result.rows_affected() > 0 {
                    imported += 1;
                } else {
                    skipped += 1;
                }

                // 5. Source
                let (bit_depth, sample_rate) = self.parse_quality(&track.audio_quality);
                let _ = sqlx::query("INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, ?, ?, 'FLAC', ?, ?, 1)")
                    .bind(track_id)
                    .bind(tidal_service_id)
                    .bind(track.id.to_string())
                    .bind(bit_depth)
                    .bind(sample_rate)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e: sqlx::Error| e.to_string())?;

                if let Some(w) = window {
                    let total = page.total as u64;
                    let current = (imported + skipped) as u64;
                    if current % 50 == 0 || current == total {
                        crate::commands::emit_import_progress(w, "tidal", "progress", 
                            current, total,
                            &format!("Processed {} favorites", current));
                    }
                }
            }

            tx.commit().await.map_err(|e| format!("Failed to commit favorites: {}", e))?;

            offset += limit;
            if offset >= page.total {
                break;
            }
        }

        Ok(super::ImportResult {
            imported: imported as i32,
            skipped: skipped as i32,
        })
    }

    pub async fn import_favorite_albums(
        &self,
        db: &SqlitePool,
        _account_id: i64,
        window: Option<&tauri::Window>,
    ) -> Result<super::ImportResult, String> {
        // First, get total count
        let first_page = self.get_favorite_albums(0, 1).await?;
        let total_albums = first_page.total;

        if let Some(w) = window {
            crate::commands::emit_import_progress(w, "tidal_albums", "started", 0, total_albums as u64, 
                &format!("Starting import of {} favorite albums...", total_albums));
        }

        let mut offset = 0;
        let limit = 20; // Warp Speed Batch
        let mut imported = 0;
        let skipped = 0;

        loop {
            let page = self.get_favorite_albums(offset, limit).await?;
            if page.items.is_empty() {
                break;
            }

            tracing::debug!("Tidal: Processing {} favorite albums (Batch Start)", page.items.len());
            let mut tx = db.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;

            for fav_item in page.items.iter() {
                let album = &fav_item.item;

                // 1. Artist (if available)
                let artist_id = if let Some(ref artist) = album.artist {
                    let artist_res: Option<(i64,)> = sqlx::query_as::<sqlx::Sqlite, (i64,)>("INSERT OR IGNORE INTO artists (name) VALUES (?) RETURNING id")
                        .bind(&artist.name)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e: sqlx::Error| e.to_string())?;

                    if let Some(row) = artist_res {
                        row.0
                    } else {
                        sqlx::query_as::<sqlx::Sqlite, (i64,)>("SELECT id FROM artists WHERE name = ?")
                            .bind(&artist.name)
                            .fetch_one(&mut *tx)
                            .await
                            .map_err(|e: sqlx::Error| e.to_string())?
                            .0
                    }
                } else {
                    1 // Default "Unknown Artist" ID
                };

                // 2. Album Upsert (S77 pattern with S79 blind protection + S81 metadata)
                let aid: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
                    r#"
                    INSERT INTO albums (title, release_date, total_tracks, cover_art_url, tidal_id, label, upc)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(tidal_id) WHERE tidal_id IS NOT NULL 
                    DO UPDATE SET 
                        label = COALESCE(albums.label, excluded.label),
                        upc = COALESCE(albums.upc, excluded.upc)
                    RETURNING id
                    "#
                )
                .bind(&album.title)
                .bind(&album.release_date)
                .bind(album.total_tracks)
                .bind(album.cover_url())
                .bind(album.tidal_id.to_string())
                .bind(&album.label)
                .bind(&album.upc)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e: sqlx::Error| e.to_string())?;

                let album_id = aid.0;

                // 3. Link Artist
                let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
                    .bind(album_id)
                    .bind(artist_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e: sqlx::Error| e.to_string())?;

                imported += 1;

                if let Some(w) = window {
                    let total = page.total as u64;
                    let current = (imported + skipped) as u64;
                    if current % 10 == 0 || current == total {
                        crate::commands::emit_import_progress(w, "tidal_albums", "progress", 
                            current, total,
                            &format!("Processed {} albums", current));
                    }
                }
            }

            tx.commit().await.map_err(|e| format!("Failed to commit albums: {}", e))?;

            offset += limit;
            if offset >= page.total {
                break;
            }
        }

        if let Some(w) = window {
            crate::commands::emit_import_complete(w, "tidal_albums", imported as u64, skipped as u64);
        }

        Ok(super::ImportResult {
            imported: imported as i32,
            skipped: skipped as i32,
        })
    }

    /// Get user's playlists (paginated)
    pub async fn import_favorite_artists(
        &self,
        db: &SqlitePool,
        _account_id: i64,
        window: Option<&tauri::Window>,
    ) -> Result<super::ImportResult, String> {
        // First, get total count
        let first_page = self.get_favorite_artists(0, 1).await?;
        let total_artists = first_page.total;
        
        tracing::info!("Tidal: Detected {} favorite artists", total_artists);

        if let Some(w) = window {
            crate::commands::emit_import_progress(w, "tidal_artists", "started", 0, total_artists as u64, 
                &format!("Starting import of {} favorite artists...", total_artists));
        }

        let mut offset = 0;
        let limit = 20; // Warp Speed Batch
        let mut imported = 0;
        let mut skipped = 0;

        loop {
            let page = self.get_favorite_artists(offset, limit).await?;
            if page.items.is_empty() {
                break;
            }

            for fav_item in page.items.iter() {
                let artist = &fav_item.item;
                
                // Fix 1: Robust deduplication by tidal_id (auth identifier)
                let res = self.get_or_create_artist_by_tidal_id(db, &artist.name, &artist.id.to_string()).await;

                match res {
                    Ok(_) => imported += 1,
                    Err(e) => {
                        tracing::error!("Tidal: Failed to import artist {}: {}", artist.name, e);
                        skipped += 1;
                    }
                }

                if let Some(w) = window {
                    let total = page.total as u64;
                    let current = (imported + skipped) as u64;
                    if current % 10 == 0 || current == total {
                        crate::commands::emit_import_progress(w, "tidal_artists", "progress", 
                            current, total,
                            &format!("Processed {} artists", current));
                    }
                }
            }


            offset += limit;
            if offset >= page.total {
                break;
            }
        }

        if let Some(w) = window {
            crate::commands::emit_import_complete(w, "tidal_artists", imported as u64, skipped as u64);
        }

        Ok(super::ImportResult {
            imported: imported as i32,
            skipped: skipped as i32,
        })
    }

    pub async fn import_playlists(
        &self,
        db: &SqlitePool,
        account_id: i64,
        window: Option<&tauri::Window>,
    ) -> Result<(), String> {
        let mut offset = 0;
        let limit = 50;
        let mut playlists_processed = 0;

        let tidal_service_id = self.get_service_id(db, "tidal").await?;

        if let Some(w) = window {
            crate::commands::emit_import_progress(w, "tidal_playlists", "started", 0, 0, "Fetching Tidal playlists...");
        }

        loop {
            let page = self.get_playlists(offset, limit).await?;

            if page.items.is_empty() {
                break;
            }

            for playlist in &page.items {
                playlists_processed += 1;
                // 1. Insert or update playlist
                let result = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO playlists 
                    (account_id, service_playlist_id, name, description, owner_name, track_count, last_synced) 
                    VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                    "#
                )
                .bind(account_id)
                .bind(&playlist.uuid)
                .bind(&playlist.title)
                .bind(&playlist.description)
                .bind(playlist.creator.as_ref().and_then(|c| c.name.as_deref()))
                .bind(playlist.track_count)
                .execute(db)
                .await;

                let playlist_db_id = match result {
                    Ok(_) => {
                        let id: (i64,) = sqlx::query_as("SELECT id FROM playlists WHERE account_id = ? AND service_playlist_id = ?")
                            .bind(account_id)
                            .bind(&playlist.uuid)
                            .fetch_one(db)
                            .await
                            .map_err(|e| format!("Failed to get playlist ID: {}", e))?;
                        id.0
                    }
                    Err(e) => {
                        tracing::error!("Failed to insert playlist {}: {}", playlist.title, e);
                        continue;
                    }
                };

                if let Some(w) = window {
                    crate::commands::emit_import_progress(w, "tidal_playlists", "progress", 
                        playlists_processed as u64, page.total as u64, 
                        &format!("Importing playlist: {}", playlist.title));
                }

                // 2. Import tracks for this playlist
                let mut track_offset = 0;
                let track_limit = 20; // Reduced for better UI/Console feedback during batch

                loop {
                    tracing::info!("Tidal: Fetching tracks for playlist {} (offset: {}, limit: {})", playlist.title, track_offset, track_limit);
                    let tracks_page = self.get_playlist_tracks(&playlist.uuid, track_offset, track_limit).await?;
                    
                    if tracks_page.items.is_empty() {
                        tracing::info!("Tidal: No more tracks in playlist {}", playlist.title);
                        break;
                    }

                    tracing::info!("Tidal: Processing {} tracks for playlist {} (Transaction Start)", tracks_page.items.len(), playlist.title);
                    
                    // --- BATCH TRANSACTION START ---
                    let mut tx = db.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
                    
                    for (pos, track_item) in tracks_page.items.iter().enumerate() {
                        let track = &track_item.item;
                        
                        // 1. Artist
                        let artist_name = track.artist.as_ref().map(|a| a.name.clone()).unwrap_or_default();
                        let artist_res: Option<(i64,)> = sqlx::query_as::<sqlx::Sqlite, (i64,)>("INSERT OR IGNORE INTO artists (name) VALUES (?) RETURNING id")
                            .bind(&artist_name)
                            .fetch_optional(&mut *tx)
                            .await
                            .map_err(|e: sqlx::Error| e.to_string())?;

                        let artist_id = if let Some(row) = artist_res {
                            row.0
                        } else {
                            sqlx::query_as::<sqlx::Sqlite, (i64,)>("SELECT id FROM artists WHERE name = ?")
                                .bind(&artist_name)
                                .fetch_one(&mut *tx)
                                .await
                                .map_err(|e: sqlx::Error| e.to_string())?
                                .0
                        };

                        // 2. Album
                        let album_id = if let Some(ref album) = track.album {
                            let aid: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
                                "INSERT INTO albums (title, release_date, total_tracks, cover_art_url, tidal_id, label, upc)
                                 VALUES (?, ?, ?, ?, ?, ?, ?)
                                 ON CONFLICT(tidal_id) WHERE tidal_id IS NOT NULL DO UPDATE SET 
                                    label = COALESCE(albums.label, excluded.label),
                                    upc = COALESCE(albums.upc, excluded.upc)
                                 RETURNING id"
                            )
                            .bind(&album.title)
                            .bind(&album.release_date)
                            .bind(album.total_tracks)
                            .bind(album.cover_url())
                            .bind(album.tidal_id.to_string())
                            .bind(&album.label)
                            .bind(&album.upc)
                            .fetch_one(&mut *tx)
                            .await
                            .map_err(|e: sqlx::Error| e.to_string())?;
                            
                            let album_id = aid.0;
                            let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
                                .bind(album_id)
                                .bind(artist_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e: sqlx::Error| e.to_string())?;
                            
                            Some(album_id)
                        } else {
                            None
                        };

                        // 3. Track
                        let tid: (i64,) = sqlx::query_as::<sqlx::Sqlite, (i64,)>(
                            r#"
                            INSERT INTO tracks (title, album_id, duration_ms, isrc, track_number, disc_number, audio_quality) 
                            VALUES (?, ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(isrc) DO UPDATE SET 
                                album_id = COALESCE(tracks.album_id, excluded.album_id),
                                track_number = COALESCE(tracks.track_number, excluded.track_number),
                                disc_number = COALESCE(tracks.disc_number, excluded.disc_number),
                                audio_quality = COALESCE(tracks.audio_quality, excluded.audio_quality)
                            RETURNING id
                            "#,
                        )
                        .bind(&track.title)
                        .bind(album_id)
                        .bind(track.duration * 1000)
                        .bind(&track.isrc)
                        .bind(track.track_number)
                        .bind(track.disc_number)
                        .bind(&track.audio_quality)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e: sqlx::Error| e.to_string())?;
                        
                        let track_id = tid.0;

                        // 4. Link artist
                        let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
                            .bind(track_id)
                            .bind(artist_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e: sqlx::Error| e.to_string())?;

                        // 5. Source
                        let (bit_depth, sample_rate) = self.parse_quality(&track.audio_quality);
                        let _ = sqlx::query("INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, ?, ?, 'FLAC', ?, ?, 1)")
                            .bind(track_id)
                            .bind(tidal_service_id)
                            .bind(track.id.to_string())
                            .bind(bit_depth)
                            .bind(sample_rate)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e: sqlx::Error| e.to_string())?;

                        // 6. Link to playlist
                        let _ = sqlx::query("INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
                            .bind(playlist_db_id)
                            .bind(track_id)
                            .bind((track_offset + pos as i32) as i32)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e: sqlx::Error| e.to_string())?;
                    }

                    tx.commit().await.map_err(|e| format!("Failed to commit transaction: {}", e))?;
                    // --- BATCH TRANSACTION END ---

                    track_offset += track_limit;
                    if track_offset >= tracks_page.total || tracks_page.items.len() < track_limit as usize {
                        tracing::info!("Tidal: Reached end of playlist {} (total: {})", playlist.title, tracks_page.total);
                        break;
                    }
                }
            }

            offset += limit;
            if offset >= page.total || page.items.len() < limit as usize {
                break;
            }
        }

        tracing::info!("Tidal: Playlist import complete. Processed {} playlists.", playlists_processed);
        Ok(())
    }

    pub fn parse_quality(&self, quality: &Option<String>) -> (Option<i32>, Option<i32>) {
        match quality.as_deref() {
            Some("HI_RES") | Some("HI_RES_LOSSLESS") => (Some(24), Some(96000)),
            Some("LOSSLESS") => (Some(16), Some(44100)),
            Some("HIGH") => (Some(16), Some(44100)),
            _ => (None, None),
        }
    }

    pub async fn get_or_create_artist_by_tidal_id(
        &self,
        db: &SqlitePool,
        name: &str,
        tidal_id: &str,
    ) -> Result<i64, String> {
        // Step 1: Lookup by tidal_id (Authoritative ID)
        if let Some(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM artists WHERE tidal_id = ?")
            .bind(tidal_id)
            .fetch_optional(db)
            .await
            .map_err(|e| e.to_string())?
        {
            return Ok(row.0);
        }

        // Step 2: Upsert by name, only assign tidal_id if currently NULL
        // COALESCE(artists.tidal_id, excluded.tidal_id) ensures we don't overwrite existing IDs
        sqlx::query(
            "INSERT INTO artists (name, tidal_id) VALUES (?, ?)
             ON CONFLICT(name) DO UPDATE SET
               tidal_id = COALESCE(artists.tidal_id, excluded.tidal_id)"
        )
        .bind(name)
        .bind(tidal_id)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

        // Return the final ID
        let id: (i64,) = sqlx::query_as("SELECT id FROM artists WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(id.0)
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
             ON CONFLICT(name) DO UPDATE SET id = id 
             RETURNING id"
        )
        .bind(name)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Get/Create artist failed: {}", e))?;

        Ok(id)
    }

    pub async fn get_or_create_album(
        &self,
        db: &SqlitePool,
        album: &TidalAlbum,
        primary_artist_id: i64,
    ) -> Result<i64, String> {
        // Logic for S77: Atomic upsert if tidal_id is available
        let tid_str = album.tidal_id.to_string();
        
        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title, release_date, total_tracks, cover_art_url, tidal_id)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(tidal_id) WHERE tidal_id IS NOT NULL DO UPDATE SET id = id
             RETURNING id"
        )
        .bind(&album.title)
        .bind(&album.release_date)
        .bind(album.total_tracks)
        .bind(album.cover_url())
        .bind(&tid_str)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Album upsert (tidal_id) failed: {}", e))?;

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
        track: &TidalTrack,
        album_id: Option<i64>,
    ) -> Result<i64, String> {
        // Try to find by ISRC first if available
        if let Some(ref isrc) = track.isrc {
            let id: i64 = sqlx::query_scalar(
                r#"INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES (?, ?, ?, ?)
                   ON CONFLICT(isrc) DO UPDATE SET 
                     album_id = COALESCE(tracks.album_id, excluded.album_id),
                     id = id
                   RETURNING id"#
            )
            .bind(&track.title)
            .bind(album_id)
            .bind(track.duration * 1000)
            .bind(isrc)
            .fetch_one(db)
            .await
            .map_err(|e| format!("Track upsert failed: {}", e))?;

            return Ok(id);
        }

        // Fallback for tracks without ISRC (create new every time for now as per soundcloud.rs logic)
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(&track.title)
        .bind(album_id)
        .bind(track.duration * 1000) // Tidal returns seconds
        .bind(&track.isrc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(id)
    }

    /// Search for tracks by query string
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<TidalSearchResult>, String> {
        let url = format!("{}/search/tracks", TIDAL_API_BASE);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(&[
                ("query", query),
                ("limit", &limit.to_string()),
                ("countryCode", &self.country_code),
            ])
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
                "Tidal search failed ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        let search_resp: TidalSearchResponse =
            serde_json::from_str(&text).map_err(|e| format!("Parse search response: {}", e))?;

        let results = search_resp
            .tracks
            .map(|t| t.items)
            .unwrap_or_default()
            .into_iter()
            .map(|track| TidalSearchResult {
                track_id: track.id.to_string(),
                title: track.title.clone(),
                artist: track.artist.map(|a| a.name).unwrap_or_default(),
                album: track.album.map(|a| a.title),
                isrc: track.isrc,
                duration_ms: track.duration * 1000,
                quality: track.audio_quality,
            })
            .collect();

        Ok(results)
    }

    /// Search for a track by ISRC code
    pub async fn search_by_isrc(&self, isrc: &str) -> Result<Option<TidalSearchResult>, String> {
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

    /// Add a track to user's favorites
    /// Includes retry logic with exponential backoff for rate limiting
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;

        let url = format!("{}/users/{}/favorites/tracks", TIDAL_API_BASE, user_id);
        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let response = self
                .client
                .post(&url)
                .bearer_auth(&self.access_token)
                .query(&[("countryCode", &self.country_code)])
                .form(&[("trackIds", track_id)])
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        tracing::debug!("Added track {} to Tidal favorites", track_id);
                        return Ok(());
                    } else if status.as_u16() == 429 || status.as_u16() >= 500 {
                        // Rate limited or server error - retry
                        let text = resp.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "Tidal add_to_favorites attempt {} failed ({}), retrying...",
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
                        "Tidal add_to_favorites attempt {} failed: {}, retrying...",
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

    /// Match a track by metadata (fallback when no ISRC)
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<TidalSearchResult>, String> {
        let query = format!("{} {}", artist, title);
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
}

/// Check if an album has a cached availability status within TTL (S145)
pub async fn check_album_availability(
    db: &SqlitePool,
    service_id: i64,
    service_album_id: &str,
    ttl_seconds: i64,
) -> Result<Option<(TidalAlbumExpansionStatus, String)>, String> {
    let row: Option<(String, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT availability_status, reason, last_checked
        FROM service_album_availability
        WHERE service_id = ? AND service_album_id = ?
        "#
    )
    .bind(service_id)
    .bind(service_album_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database query error in check_album_availability: {}", e))?;

    if let Some((status_str, reason_opt, last_checked_str)) = row {
        let status = TidalAlbumExpansionStatus::from_str_name(&status_str);
        if status == TidalAlbumExpansionStatus::Available {
            return Ok(None);
        }

        let is_valid_ttl = if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&last_checked_str, "%Y-%m-%d %H:%M:%S") {
            let now = chrono::Utc::now().naive_utc();
            let elapsed = (now - dt).num_seconds();
            elapsed >= 0 && elapsed < ttl_seconds
        } else {
            true
        };

        if is_valid_ttl {
            let reason = reason_opt.unwrap_or_else(|| status.as_str().to_string());
            return Ok(Some((status, reason)));
        }
    }

    Ok(None)
}

/// Record or update album availability in SQLite (S145)
pub async fn record_album_availability(
    db: &SqlitePool,
    service_id: i64,
    service_album_id: &str,
    status: TidalAlbumExpansionStatus,
    http_status: Option<u16>,
    sub_status: Option<i32>,
    reason: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO service_album_availability
            (service_id, service_album_id, availability_status, http_status, sub_status, reason, last_checked)
        VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(service_id, service_album_id) DO UPDATE SET
            availability_status = excluded.availability_status,
            http_status = excluded.http_status,
            sub_status = excluded.sub_status,
            reason = excluded.reason,
            last_checked = CURRENT_TIMESTAMP
        "#
    )
    .bind(service_id)
    .bind(service_album_id)
    .bind(status.as_str())
    .bind(http_status.map(|s| s as i64))
    .bind(sub_status.map(|s| s as i64))
    .bind(reason)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to record album availability: {}", e))?;

    Ok(())
}

/// Clear album availability (e.g. on 200 OK recovery) (S145)
pub async fn clear_album_availability(
    db: &SqlitePool,
    service_id: i64,
    service_album_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM service_album_availability WHERE service_id = ? AND service_album_id = ?"
    )
    .bind(service_id)
    .bind(service_album_id)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to clear album availability: {}", e))?;

    Ok(())
}

