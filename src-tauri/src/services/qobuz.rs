//! Qobuz service - Authentication and library import
//!
//! Handles Qobuz user auth token and importing favorites.

#![allow(dead_code)]

use reqwest::Client;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const QOBUZ_APP_ID: &str = "950096963";
const QOBUZ_API_BASE: &str = "https://www.qobuz.com/api.json/0.2";

/// Qobuz credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzCredentials {
    pub user_auth_token: String,
    pub user_id: Option<String>,
}

/// Qobuz track from API
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzTrack {
    pub id: i64,
    pub title: String,
    pub duration: i64,
    pub isrc: Option<String>,
    pub maximum_bit_depth: Option<i32>,
    pub maximum_sampling_rate: Option<f64>,
    pub performer: Option<QobuzArtist>,
    pub album: Option<QobuzAlbum>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzArtist {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbum {
    pub id: String,
    pub title: String,
    pub released_at: Option<i64>, // Unix timestamp or formatted string? Qobuz API usually returns it
    pub image: Option<QobuzImage>,
    #[serde(default)]
    pub artist: Option<QobuzArtist>, // Present in /favorite/getUserFavorites?type=albums
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzImage {
    pub small: Option<String>,
    pub large: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzFavoritesResponse {
    pub tracks: QobuzTracksContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzTracksContainer {
    pub items: Vec<QobuzTrack>,
    pub total: i32,
}

/// Qobuz albums favorites response (/favorite/getUserFavorites?type=albums)
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbumsResponse {
    pub albums: QobuzAlbumsContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbumsContainer {
    pub items: Vec<QobuzAlbum>,
    pub total: i32,
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

    /// Login with username/password to get user auth token
    pub async fn login(&self, username: &str, password: &str) -> Result<String, String> {
        let url = format!("{}/user/login", QOBUZ_API_BASE);
        tracing::info!("Qobuz API login: {} (app_id={}, user={})", url, &self.app_id, username);

        // Qobuz API requires 'email' (not 'username') and 'app_id' as query
        // parameters, matching the streamrip reference implementation.
        let response = self
            .client
            .get(&url)
            .header("X-App-Id", &self.app_id)
            .query(&[
                ("email", username),
                ("password", password),
                ("app_id", self.app_id.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("Login request failed (timeout/network): {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read login response: {}", e))?;

        if !status.is_success() {
            tracing::error!("Qobuz login failed ({}): {}", status, &text[..text.len().min(500)]);
            return Err(format!("Login failed ({}): {}", status, &text[..text.len().min(200)]));
        }

        #[derive(Deserialize)]
        struct LoginResponse {
            user_auth_token: String,
        }

        let login: LoginResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Parse login response: {} (raw: {})", e, &text[..text.len().min(200)]))?;

        tracing::info!("Qobuz login succeeded, got auth token (len={})", login.user_auth_token.len());
        Ok(login.user_auth_token)
    }

    /// Get user's favorite tracks (paginated)
    pub async fn get_favorites(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzFavoritesResponse, String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/favorite/getUserFavorites", QOBUZ_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("X-User-Auth-Token", token)
            .header("X-App-Id", &self.app_id)
            .query(&[
                ("type", "tracks"),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!(
            "Qobuz favorites response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(format!(
                "Qobuz API error ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Qobuz response: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })
    }

    /// Get user's favorite albums (paginated)
    pub async fn get_favorite_albums(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzAlbumsResponse, String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/favorite/getUserFavorites", QOBUZ_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("X-User-Auth-Token", token)
            .header("X-App-Id", &self.app_id)
            .query(&[
                ("type", "albums"),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!(
            "Qobuz favorite albums response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(format!(
                "Qobuz API error ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Qobuz albums response: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })
    }

    /// Get user's playlists (paginated)
    pub async fn get_playlists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzPlaylistsResponse, String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/playlist/getUserPlaylists", QOBUZ_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("X-User-Auth-Token", token)
            .header("X-App-Id", &self.app_id)
            .query(&[
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!(
            "Qobuz playlists response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(format!(
                "Qobuz API error ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Qobuz playlists response: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })
    }

    /// Get playlist tracks (paginated)
    pub async fn get_playlist_tracks(
        &self,
        playlist_id: i64,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzPlaylistDetail, String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/playlist/get", QOBUZ_API_BASE);

        let playlist_id_str = playlist_id.to_string();
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        let response = self
            .client
            .get(&url)
            .header("X-User-Auth-Token", token)
            .header("X-App-Id", &self.app_id)
            .query(&[
                ("playlist_id", playlist_id_str.as_str()),
                ("extra", "tracks"),
                ("offset", offset_str.as_str()),
                ("limit", limit_str.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!(
            "Qobuz playlist tracks response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(format!(
                "Qobuz API error ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Qobuz playlist detail: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })
    }

    /// Get user's favorite artists (paginated)
    pub async fn get_favorite_artists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<QobuzArtistsResponse, String> {
        let token = self
            .user_auth_token
            .as_ref()
            .ok_or("Not authenticated - call login() first")?;

        let url = format!("{}/favorite/getUserFavorites", QOBUZ_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("X-User-Auth-Token", token)
            .header("X-App-Id", &self.app_id)
            .query(&[
                ("type", "artists"),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!(
            "Qobuz favorite artists response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(format!(
                "Qobuz API error ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Qobuz artists response: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })
    }

    /// Import all favorites to database
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
                    .map(|a| a.name.clone())
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
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM artists WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
            .bind(name)
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
        // Try to find existing by title
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM albums WHERE title = ?")
            .bind(&album.title)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        // Get cover art URL (prefer large)
        let cover_url = album
            .image
            .as_ref()
            .and_then(|i| i.large.clone().or(i.small.clone()));

        // Create new album
        let release_date = album.released_at.map(|ts| {
            // Qobuz often returns year or full date depending on endpoint.
            // If it's a timestamp, we should handle it, but most library endpoints return a string or year
            // We'll use the raw value if it's a string in the API.
            // Wait, I used Option<i64> in struct, let's verify if it's a timestamp.
            ts.to_string()
        });

        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title, cover_art_url, release_date) VALUES (?, ?, ?) RETURNING id",
        )
                .bind(&album.title)
                .bind(&cover_url)
                .bind(&release_date)
                .fetch_one(db)
                .await
                .map_err(|e| format!("Album insert failed: {}", e))?;

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
        // Try to find by ISRC
        if let Some(ref isrc) = track.isrc {
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
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(&track.title)
        .bind(album_id)
        .bind(track.duration * 1000) // Qobuz returns seconds
        .bind(&track.isrc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Insert failed: {}", e))?;

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
                title: track.title.clone(),
                artist: track.performer.map(|p| p.name).unwrap_or_default(),
                album: track.album.map(|a| a.title),
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
