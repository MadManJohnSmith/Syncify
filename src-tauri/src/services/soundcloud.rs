//! SoundCloud service - Authentication and library import
//!
//! Handles SoundCloud API access and importing favorites.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const SOUNDCLOUD_API_BASE: &str = "https://api.soundcloud.com";
const SOUNDCLOUD_API_V2: &str = "https://api-v2.soundcloud.com";

/// SoundCloud track from API
#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudTrack {
    pub id: i64,
    pub title: String,
    pub duration: i64, // in milliseconds
    pub user: Option<SoundCloudUser>,
    pub permalink_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudUser {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudCollection {
    pub collection: Vec<SoundCloudLike>,
    pub next_href: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudLike {
    pub track: Option<SoundCloudTrack>,
}

/// SoundCloud API client
pub struct SoundCloudClient {
    client: Client,
    oauth_token: String,
    user_id: Option<i64>,
}

impl SoundCloudClient {
    pub fn new(oauth_token: String) -> Self {
        Self {
            client: Client::new(),
            oauth_token,
            user_id: None,
        }
    }

    pub fn with_user_id(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Get user's liked tracks (paginated via next_href)
    pub async fn get_likes(&self, url: Option<&str>) -> Result<SoundCloudCollection, String> {
        let user_id = self.user_id.ok_or("User ID not set")?;

        let request_url = url
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/users/{}/likes?limit=100", SOUNDCLOUD_API_V2, user_id));

        let response = self
            .client
            .get(&request_url)
            .header("Authorization", format!("OAuth {}", self.oauth_token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("SoundCloud API error {}: {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Get current user info
    pub async fn get_me(&self) -> Result<SoundCloudUser, String> {
        let response = self
            .client
            .get(&format!("{}/me", SOUNDCLOUD_API_V2))
            .header("Authorization", format!("OAuth {}", self.oauth_token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("SoundCloud API error {}: {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    // Helper methods for database operations
    pub async fn get_service_id(&self, db: &SqlitePool, name: &str) -> Result<i64, String> {
        let result: (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
            .map_err(|e| format!("Service not found: {}", e))?;
        Ok(result.0)
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
        .map_err(|e| format!("Failed to get/create artist: {}", e))?;

        Ok(id)
    }

    /// Import all liked tracks to database
    pub async fn import_library(
        &mut self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        // Get user info first
        let user = self.get_me().await?;
        self.user_id = Some(user.id);

        let mut imported = 0;
        let mut skipped = 0;
        let mut next_href: Option<String> = None;

        let soundcloud_service_id = self.get_service_id(db, "soundcloud").await?;

        loop {
            let page = self.get_likes(next_href.as_deref()).await?;

            if page.collection.is_empty() {
                break;
            }

            for like in &page.collection {
                let Some(ref track) = like.track else {
                    skipped += 1;
                    continue;
                };

                // Get or create artist
                let artist_name = track
                    .user
                    .as_ref()
                    .map(|u| u.username.clone())
                    .unwrap_or_default();
                let artist_id = self.get_or_create_artist(db, &artist_name).await?;

                // Get or create track (SoundCloud doesn't have ISRC)
                let track_id = self.get_or_create_track(db, track).await?;

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

                // Add track source (SoundCloud is typically 128kbps MP3)
                let _ = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO track_sources 
                    (track_id, service_id, service_track_id, format, quality_score, available) 
                    VALUES (?, ?, ?, 'MP3', 128, 1)
                    "#,
                )
                .bind(track_id)
                .bind(soundcloud_service_id)
                .bind(track.id.to_string())
                .execute(db)
                .await;
            }

            tracing::info!("SoundCloud import: {} imported so far...", imported);

            next_href = page.next_href;
            if next_href.is_none() {
                break;
            }
        }

        Ok(super::ImportResult { imported, skipped })
    }

    /// Get or create a track in the database (SoundCloud doesn't have ISRC)
    pub async fn get_or_create_track(
        &self,
        db: &SqlitePool,
        track: &SoundCloudTrack,
    ) -> Result<i64, String> {
        // SoundCloud doesn't provide ISRC, so we match by title (simplified)
        // For now, using RETURNING id directly.
        let id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, duration_ms) VALUES (?, ?) RETURNING id")
            .bind(&track.title)
            .bind(track.duration)
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
    ) -> Result<Vec<SoundCloudSearchResult>, String> {
        let url = format!("{}/search/tracks", SOUNDCLOUD_API_V2);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("OAuth {}", self.oauth_token))
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!(
                "SoundCloud search failed ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            collection: Option<Vec<SoundCloudTrack>>,
        }

        let search_resp: SearchResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse search response: {}", e))?;

        let results = search_resp
            .collection
            .unwrap_or_default()
            .into_iter()
            .map(|t| SoundCloudSearchResult {
                track_id: t.id.to_string(),
                title: t.title.clone(),
                artist: t.user.map(|u| u.username).unwrap_or_default(),
                duration_ms: t.duration,
                permalink_url: t.permalink_url,
            })
            .collect();

        Ok(results)
    }

    /// Search for a track by title and artist (SoundCloud doesn't have ISRC)
    pub async fn search_by_isrc(
        &self,
        _isrc: &str,
    ) -> Result<Option<SoundCloudSearchResult>, String> {
        // SoundCloud doesn't support ISRC, return None
        Ok(None)
    }

    /// Match a track by metadata
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<SoundCloudSearchResult>, String> {
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

    /// Like a track (add to favorites)
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        let user_id = self.user_id.ok_or("User ID not set")?;

        let url = format!(
            "{}/users/{}/track_likes/{}",
            SOUNDCLOUD_API_V2, user_id, track_id
        );

        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let response = self
                .client
                .put(&url)
                .header("Authorization", format!("OAuth {}", self.oauth_token))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() || status.as_u16() == 201 {
                        tracing::info!("Added track {} to SoundCloud likes", track_id);
                        return Ok(());
                    } else if status.as_u16() == 429 || status.as_u16() >= 500 {
                        let text = resp.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "SoundCloud add_to_favorites attempt {} failed ({}), retrying...",
                            attempt + 1,
                            status
                        );
                    } else {
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
                        "SoundCloud add_to_favorites attempt {} failed: {}, retrying...",
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
            "Add to favorites failed after {} retries: {}",
            max_retries, last_error
        ))
    }
}

/// Search result for migration matching
#[derive(Debug, Clone, Serialize)]
pub struct SoundCloudSearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
    pub permalink_url: Option<String>,
}
