//! Tidal service - Authentication and library import
//!
//! Handles Tidal API access and importing favorites.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const TIDAL_API_BASE: &str = "https://api.tidal.com/v1";

/// Tidal track from API
#[derive(Debug, Clone, Deserialize)]
pub struct TidalTrack {
    pub id: i64,
    pub title: String,
    pub duration: i64,
    pub isrc: Option<String>,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub artist: Option<TidalArtist>,
    pub album: Option<TidalAlbum>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalArtist {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalAlbum {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalFavoriteItem {
    pub item: TidalTrack,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPaginated {
    pub items: Vec<TidalFavoriteItem>,
    #[serde(rename = "totalNumberOfItems")]
    pub total: i32,
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
            return Err(format!("Tidal API error {}: {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Import all favorites to database
    pub async fn import_library(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        let mut offset = 0;
        let limit = 100;
        let mut imported = 0;
        let mut skipped = 0;

        let tidal_service_id = self.get_service_id(db, "tidal").await?;

        loop {
            let page = self.get_favorites(offset, limit).await?;

            // Log total on first page
            if offset == 0 {
                tracing::info!("Tidal reports {} total favorite tracks", page.total);
            }

            tracing::debug!(
                "Tidal page: offset={}, got {} items",
                offset,
                page.items.len()
            );

            if page.items.is_empty() {
                break;
            }

            for item in &page.items {
                let track = &item.item;

                // Get or create artist
                let artist_name = track
                    .artist
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
                let (bit_depth, sample_rate) = self.parse_quality(&track.audio_quality);
                let _ = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO track_sources 
                    (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) 
                    VALUES (?, ?, ?, 'FLAC', ?, ?, 1)
                    "#
                )
                .bind(track_id)
                .bind(tidal_service_id)
                .bind(track.id.to_string())
                .bind(bit_depth)
                .bind(sample_rate)
                .execute(db)
                .await;
            }

            offset += limit;

            tracing::info!(
                "Tidal import: {} imported, {} skipped so far (offset: {})",
                imported,
                skipped,
                offset
            );

            // Stop if we've processed all items
            if offset >= page.total {
                break;
            }

            // Also stop if page is empty (safety check)
            if page.items.len() == 0 {
                break;
            }
        }

        Ok(super::ImportResult { imported, skipped })
    }

    pub fn parse_quality(&self, quality: &Option<String>) -> (Option<i32>, Option<i32>) {
        match quality.as_deref() {
            Some("HI_RES") | Some("HI_RES_LOSSLESS") => (Some(24), Some(96000)),
            Some("LOSSLESS") => (Some(16), Some(44100)),
            Some("HIGH") => (Some(16), Some(44100)),
            _ => (None, None),
        }
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
        // Create or get album via upsert
        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title) VALUES (?) 
             ON CONFLICT(title) DO UPDATE SET id = id 
             RETURNING id"
        )
        .bind(&album.title)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Album upsert failed: {}", e))?;

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
                        tracing::info!("Added track {} to Tidal favorites", track_id);
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
