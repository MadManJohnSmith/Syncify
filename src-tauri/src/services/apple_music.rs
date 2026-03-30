//! Apple Music service - Authentication and library import
//!
//! Handles Apple Music API access via MusicKit.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const APPLE_MUSIC_API: &str = "https://amp-api.music.apple.com/v1";

/// Apple Music track from API
#[derive(Debug, Clone, Deserialize)]
pub struct AppleMusicTrack {
    pub id: String,
    pub attributes: Option<AppleMusicTrackAttributes>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleMusicTrackAttributes {
    pub name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub duration_in_millis: Option<i64>,
    pub isrc: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppleMusicResponse {
    pub data: Option<Vec<AppleMusicTrack>>,
    pub next: Option<String>,
}

/// Search response from catalog API
#[derive(Debug, Clone, Deserialize)]
pub struct AppleMusicSearchResponse {
    pub results: Option<AppleMusicSearchResults>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppleMusicSearchResults {
    pub songs: Option<AppleMusicResponse>,
}

/// Simplified search result for migration matching
#[derive(Debug, Clone, Serialize)]
pub struct AppleMusicSearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i64,
}

/// Apple Music API client
pub struct AppleMusicClient {
    client: Client,
    music_user_token: String,
    developer_token: String,
}

impl AppleMusicClient {
    pub fn new(developer_token: String, music_user_token: String) -> Self {
        Self {
            client: Client::new(),
            music_user_token,
            developer_token,
        }
    }

    /// Get user's library songs (paginated)
    pub async fn get_library_songs(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicResponse, String> {
        let url = format!(
            "{}/me/library/songs?offset={}&limit={}",
            APPLE_MUSIC_API, offset, limit
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.developer_token))
            .header("media-user-token", &self.music_user_token)
            .header("Origin", "https://music.apple.com")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Apple Music API error {}: {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Import all library songs to database
    pub async fn import_library(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        let mut offset = 0;
        let limit = 100;
        let mut imported = 0;
        let mut skipped = 0;

        let service_id = self.get_service_id(db, "apple_music").await?;

        loop {
            let page = self.get_library_songs(offset, limit).await?;

            let tracks = page.data.unwrap_or_default();
            if tracks.is_empty() {
                break;
            }

            for track in &tracks {
                let attrs = match &track.attributes {
                    Some(a) => a,
                    None => continue,
                };

                // Get or create artist
                let artist_id = self.get_or_create_artist(db, &attrs.artist_name).await?;

                // Get or create album (if present)
                let album_id = if let Some(ref album_name) = attrs.album_name {
                    Some(self.get_or_create_album(db, album_name, artist_id).await?)
                } else {
                    None
                };

                // Get or create track using ISRC-first matching
                let track_id = self.get_or_create_track(db, attrs, album_id).await?;

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

                // Add track source
                let _ = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO track_sources 
                    (track_id, service_id, service_track_id, format, quality_score, available) 
                    VALUES (?, ?, ?, 'AAC', 256, 1)
                    "#,
                )
                .bind(track_id)
                .bind(service_id)
                .bind(&track.id)
                .execute(db)
                .await;
            }

            offset += limit;

            tracing::info!("Apple Music import: {} imported so far...", imported);

            if tracks.len() < limit as usize {
                break;
            }
        }

        Ok(super::ImportResult { imported, skipped })
    }

    /// Search the Apple Music catalog
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<AppleMusicSearchResult>, String> {
        let url = format!(
            "{}/catalog/us/search?term={}&types=songs&limit={}",
            APPLE_MUSIC_API,
            urlencoding::encode(query),
            limit
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.developer_token))
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "Apple Music search error {}: {}",
                status,
                &body[..body.len().min(200)]
            ));
        }

        let search_resp: AppleMusicSearchResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse search: {}", e))?;

        let tracks = search_resp
            .results
            .and_then(|r| r.songs)
            .and_then(|s| s.data)
            .unwrap_or_default();

        let results = tracks
            .into_iter()
            .filter_map(|t| {
                let attrs = t.attributes?;
                Some(AppleMusicSearchResult {
                    track_id: t.id,
                    title: attrs.name,
                    artist: attrs.artist_name,
                    album: attrs.album_name,
                    isrc: attrs.isrc,
                    duration_ms: attrs.duration_in_millis.unwrap_or(0),
                })
            })
            .collect();

        Ok(results)
    }

    /// Search for a track by ISRC code
    pub async fn search_by_isrc(
        &self,
        isrc: &str,
    ) -> Result<Option<AppleMusicSearchResult>, String> {
        // Apple Music supports ISRC filtering
        let url = format!("{}/catalog/us/songs?filter[isrc]={}", APPLE_MUSIC_API, isrc);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.developer_token))
            .send()
            .await
            .map_err(|e| format!("ISRC search failed: {}", e))?;

        if !response.status().is_success() {
            // Fallback to regular search
            let results = self.search_track(isrc, 5).await?;
            return Ok(results.into_iter().find(|r| {
                r.isrc
                    .as_ref()
                    .map(|i| i.eq_ignore_ascii_case(isrc))
                    .unwrap_or(false)
            }));
        }

        let resp: AppleMusicResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))?;

        let result = resp
            .data
            .and_then(|tracks| tracks.into_iter().next())
            .and_then(|t| {
                let attrs = t.attributes?;
                Some(AppleMusicSearchResult {
                    track_id: t.id,
                    title: attrs.name,
                    artist: attrs.artist_name,
                    album: attrs.album_name,
                    isrc: attrs.isrc,
                    duration_ms: attrs.duration_in_millis.unwrap_or(0),
                })
            });

        Ok(result)
    }

    /// Match a track by metadata (fallback when no ISRC)
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<AppleMusicSearchResult>, String> {
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
                    || (r_artist.contains(&target_artist) && !r_title.is_empty())
            })
            .next();

        Ok(best_match)
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
        let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM artists WHERE name = ?")
            .bind(name)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        let artist_id: i64 = sqlx::query_scalar(
            "INSERT INTO artists (name) VALUES (?) RETURNING id"
        )
        .bind(name)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Failed to create artist: {}", e))?;

        Ok(artist_id)
    }

    pub async fn get_or_create_album(
        &self,
        db: &SqlitePool,
        title: &str,
        primary_artist_id: i64,
    ) -> Result<i64, String> {
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM albums WHERE title = ?")
            .bind(title)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title) VALUES (?) RETURNING id"
        )
        .bind(title)
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
        attrs: &AppleMusicTrackAttributes,
        album_id: Option<i64>,
    ) -> Result<i64, String> {
        // Try to find by ISRC
        if let Some(ref isrc) = attrs.isrc {
            if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_one(db)
                .await
            {
                // Update album_id if not set
                if let Some(album_id) = album_id {
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

        // Create new track
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(&attrs.name)
        .bind(album_id)
        .bind(attrs.duration_in_millis)
        .bind(&attrs.isrc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(track_id)
    }
}
