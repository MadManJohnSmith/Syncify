//! Apple Music service - Authentication and library import
//!
//! Handles Apple Music API access via MusicKit.

// F4-1: este allow permanece con evidencia — al retirarlo (2026-08-25) aparecen
// 7 ítems sin llamadores (AppleMusicSearchResponse, import_library, search_track,
// search_by_isrc, match_by_metadata, get_or_create_album/track) porque su
// integración es la Fase 3 del plan de unificación, bloqueada en credenciales
// reales del propietario. Se retira junto con esa fase.
#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const APPLE_MUSIC_API: &str = "https://amp-api.music.apple.com/v1";

/// Apple Music pagination and metadata container
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicMeta {
    pub total: Option<i64>,
}

/// Apple Music track from API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppleMusicTrack {
    pub id: String,
    pub attributes: Option<AppleMusicTrackAttributes>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppleMusicTrackAttributes {
    pub name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub duration_in_millis: Option<i64>,
    pub isrc: Option<String>,
    pub date_added: Option<String>,
    pub track_number: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicResponse {
    pub data: Option<Vec<AppleMusicTrack>>,
    pub next: Option<String>,
    pub meta: Option<AppleMusicMeta>,
}

/// Library Albums response
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicAlbumsResponse {
    pub data: Option<Vec<AppleMusicAlbum>>,
    pub next: Option<String>,
    pub meta: Option<AppleMusicMeta>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppleMusicAlbum {
    pub id: String,
    pub attributes: Option<AppleMusicAlbumAttributes>,
    pub relationships: Option<AppleMusicAlbumRelationships>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppleMusicAlbumAttributes {
    pub name: String,
    pub artist_name: String,
    pub track_count: Option<i32>,
    pub date_added: Option<String>,
    pub release_date: Option<String>,
    pub upc: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicAlbumRelationships {
    pub tracks: Option<AppleMusicResponse>,
}

/// Library Playlists response
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicPlaylistsResponse {
    pub data: Option<Vec<AppleMusicPlaylist>>,
    pub next: Option<String>,
    pub meta: Option<AppleMusicMeta>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppleMusicPlaylist {
    pub id: String,
    pub attributes: Option<AppleMusicPlaylistAttributes>,
    pub relationships: Option<AppleMusicPlaylistRelationships>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppleMusicPlaylistAttributes {
    pub name: String,
    pub description: Option<AppleMusicPlaylistDescription>,
    pub date_added: Option<String>,
    pub can_edit: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicPlaylistDescription {
    pub standard: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppleMusicPlaylistRelationships {
    pub tracks: Option<AppleMusicResponse>,
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
    base_url: String,
}

impl AppleMusicClient {
    pub fn new(developer_token: String, music_user_token: String) -> Self {
        Self {
            client: Client::new(),
            music_user_token,
            developer_token,
            base_url: APPLE_MUSIC_API.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Generic JSON request helper supporting relative paths and full URLs
    pub async fn request_json<T: for<'de> Deserialize<'de>>(&self, path_or_url: &str) -> Result<T, String> {
        let url = if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            path_or_url.to_string()
        } else {
            let p = path_or_url.strip_prefix("/v1/").unwrap_or(path_or_url);
            let p = p.strip_prefix('/').unwrap_or(p);
            format!("{}/{}", self.base_url.trim_end_matches('/'), p)
        };

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
            .map_err(|e| format!("Failed to parse Apple Music JSON: {}", e))
    }

    /// Get user's library songs (paginated)
    pub async fn get_library_songs(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicResponse, String> {
        let path = format!("me/library/songs?offset={}&limit={}", offset, limit);
        self.request_json(&path).await
    }

    /// Get user's library albums (paginated)
    pub async fn get_library_albums(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicAlbumsResponse, String> {
        let path = format!("me/library/albums?offset={}&limit={}&include=tracks", offset, limit);
        self.request_json(&path).await
    }

    /// Get user's library playlists (paginated)
    pub async fn get_library_playlists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicPlaylistsResponse, String> {
        let path = format!("me/library/playlists?offset={}&limit={}&include=tracks", offset, limit);
        self.request_json(&path).await
    }

    /// Get playlist tracks (paginated)
    pub async fn get_playlist_tracks(
        &self,
        playlist_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicResponse, String> {
        let path = format!("me/library/playlists/{}/tracks?offset={}&limit={}", playlist_id, offset, limit);
        self.request_json(&path).await
    }

    /// Get album tracks (paginated)
    pub async fn get_album_tracks(
        &self,
        album_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicResponse, String> {
        let path = format!("me/library/albums/{}/tracks?offset={}&limit={}", album_id, offset, limit);
        self.request_json(&path).await
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

            let tracks = page.data.clone().unwrap_or_default();
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

                // Add to library entry with normalized added_at (TASK-108: never NULL or 1970)
                let normalized_date = crate::services::import_pagination::normalize_added_at(
                    attrs.date_added.as_deref()
                );

                let result = sqlx::query(
                    r#"
                    INSERT INTO library_entries (account_id, track_id, is_liked, is_purchased, added_at)
                    VALUES (?, ?, 1, 0, ?)
                    ON CONFLICT(account_id, track_id) DO UPDATE SET
                        is_liked = 1,
                        added_at = CASE 
                            WHEN library_entries.added_at IS NULL OR library_entries.added_at LIKE '1970-01-01%' THEN excluded.added_at 
                            ELSE library_entries.added_at 
                        END
                    "#
                )
                .bind(account_id)
                .bind(track_id)
                .bind(&normalized_date)
                .execute(db)
                .await
                .map_err(|e| format!("DB error: {}", e))?;

                if result.rows_affected() > 0 {
                    imported += 1;
                } else {
                    skipped += 1;
                }

                // Add track source (Apple Music is typically 256kbps AAC)
                let _ = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO track_sources 
                    (track_id, service_id, service_track_id, format, bitrate, quality_score, available) 
                    VALUES (?, ?, ?, 'AAC', 256, NULL, 1)
                    "#,
                )
                .bind(track_id)
                .bind(service_id)
                .bind(&track.id)
                .execute(db)
                .await;
            }

            let next_decision = crate::services::import_pagination::next_apple_music_offset(
                offset,
                tracks.len() as i32,
                limit,
                page.next.as_deref(),
                page.meta.as_ref().and_then(|m| m.total),
            );

            match next_decision {
                Some(next_off) => {
                    offset = next_off;
                }
                None => break,
            }

            tracing::info!("Apple Music import: {} imported so far...", imported);
        }

        Ok(super::ImportResult { imported, skipped })
    }

    /// Import user's library albums and their constituent tracks
    pub async fn import_albums(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        let mut offset = 0;
        let limit = 50;
        let mut imported = 0;
        let mut skipped = 0;
        let service_id = self.get_service_id(db, "apple_music").await?;

        loop {
            let page = self.get_library_albums(offset, limit).await?;
            let albums = page.data.clone().unwrap_or_default();
            if albums.is_empty() {
                break;
            }

            for album in &albums {
                let attrs = match &album.attributes {
                    Some(a) => a,
                    None => continue,
                };
                let artist_id = self.get_or_create_artist(db, &attrs.artist_name).await?;
                let album_id = self.get_or_create_album(db, &attrs.name, artist_id).await?;

                let tracks = if let Some(rel) = &album.relationships {
                    rel.tracks.as_ref().and_then(|t| t.data.clone()).unwrap_or_default()
                } else {
                    self.get_album_tracks(&album.id, 0, 100).await
                        .ok()
                        .and_then(|r| r.data)
                        .unwrap_or_default()
                };

                for track in &tracks {
                    let track_attrs = match &track.attributes {
                        Some(a) => a,
                        None => continue,
                    };
                    let track_id = self.get_or_create_track(db, track_attrs, Some(album_id)).await?;
                    let _ = sqlx::query(
                        "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                    )
                    .bind(track_id)
                    .bind(artist_id)
                    .execute(db)
                    .await;

                    let normalized_date = crate::services::import_pagination::normalize_added_at(
                        track_attrs.date_added.as_deref().or(attrs.date_added.as_deref())
                    );

                    let result = sqlx::query(
                        r#"
                        INSERT INTO library_entries (account_id, track_id, is_liked, is_purchased, added_at)
                        VALUES (?, ?, 1, 0, ?)
                        ON CONFLICT(account_id, track_id) DO UPDATE SET
                            is_liked = 1,
                            added_at = CASE 
                                WHEN library_entries.added_at IS NULL OR library_entries.added_at LIKE '1970-01-01%' THEN excluded.added_at 
                                ELSE library_entries.added_at 
                            END
                        "#
                    )
                    .bind(account_id)
                    .bind(track_id)
                    .bind(&normalized_date)
                    .execute(db)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?;

                    if result.rows_affected() > 0 {
                        imported += 1;
                    } else {
                        skipped += 1;
                    }

                    let _ = sqlx::query(
                        r#"
                        INSERT OR REPLACE INTO track_sources 
                        (track_id, service_id, service_track_id, format, bitrate, quality_score, available) 
                        VALUES (?, ?, ?, 'AAC', 256, NULL, 1)
                        "#,
                    )
                    .bind(track_id)
                    .bind(service_id)
                    .bind(&track.id)
                    .execute(db)
                    .await;
                }
            }

            let next_decision = crate::services::import_pagination::next_apple_music_offset(
                offset,
                albums.len() as i32,
                limit,
                page.next.as_deref(),
                page.meta.as_ref().and_then(|m| m.total),
            );

            match next_decision {
                Some(next_off) => offset = next_off,
                None => break,
            }
        }

        Ok(super::ImportResult { imported, skipped })
    }

    /// Import user's library playlists and their tracks
    pub async fn import_playlists(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        let mut offset = 0;
        let limit = 50;
        let mut imported = 0;
        let mut skipped = 0;
        let service_id = self.get_service_id(db, "apple_music").await?;

        loop {
            let page = self.get_library_playlists(offset, limit).await?;
            let playlists = page.data.clone().unwrap_or_default();
            if playlists.is_empty() {
                break;
            }

            for playlist in &playlists {
                let attrs = match &playlist.attributes {
                    Some(a) => a,
                    None => continue,
                };

                let playlist_name = &attrs.name;
                let desc = attrs.description.as_ref().and_then(|d| d.standard.clone());

                // Upsert playlist
                let playlist_db_id: i64 = sqlx::query_scalar(
                    r#"
                    INSERT INTO playlists (account_id, service_playlist_id, name, description, is_public, last_synced)
                    VALUES (?, ?, ?, ?, 0, CURRENT_TIMESTAMP)
                    ON CONFLICT(account_id, service_playlist_id) DO UPDATE SET
                        name = excluded.name,
                        description = excluded.description,
                        last_synced = CURRENT_TIMESTAMP
                    RETURNING id
                    "#
                )
                .bind(account_id)
                .bind(&playlist.id)
                .bind(playlist_name)
                .bind(&desc)
                .fetch_one(db)
                .await
                .map_err(|e| format!("Failed to upsert playlist: {}", e))?;

                let tracks = if let Some(rel) = &playlist.relationships {
                    rel.tracks.as_ref().and_then(|t| t.data.clone()).unwrap_or_default()
                } else {
                    self.get_playlist_tracks(&playlist.id, 0, 100).await
                        .ok()
                        .and_then(|r| r.data)
                        .unwrap_or_default()
                };

                for (idx, track) in tracks.iter().enumerate() {
                    let track_attrs = match &track.attributes {
                        Some(a) => a,
                        None => continue,
                    };
                    let artist_id = self.get_or_create_artist(db, &track_attrs.artist_name).await?;
                    let album_id = if let Some(ref alb) = track_attrs.album_name {
                        Some(self.get_or_create_album(db, alb, artist_id).await?)
                    } else {
                        None
                    };

                    let track_id = self.get_or_create_track(db, track_attrs, album_id).await?;
                    let _ = sqlx::query(
                        "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                    )
                    .bind(track_id)
                    .bind(artist_id)
                    .execute(db)
                    .await;

                    // Link to playlist_tracks
                    let _ = sqlx::query(
                        "INSERT OR REPLACE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                    )
                    .bind(playlist_db_id)
                    .bind(track_id)
                    .bind(idx as i32)
                    .execute(db)
                    .await;

                    let normalized_date = crate::services::import_pagination::normalize_added_at(
                        track_attrs.date_added.as_deref().or(attrs.date_added.as_deref())
                    );

                    let result = sqlx::query(
                        r#"
                        INSERT INTO library_entries (account_id, track_id, is_liked, is_purchased, added_at)
                        VALUES (?, ?, 1, 0, ?)
                        ON CONFLICT(account_id, track_id) DO UPDATE SET
                            is_liked = 1,
                            added_at = CASE 
                                WHEN library_entries.added_at IS NULL OR library_entries.added_at LIKE '1970-01-01%' THEN excluded.added_at 
                                ELSE library_entries.added_at 
                            END
                        "#
                    )
                    .bind(account_id)
                    .bind(track_id)
                    .bind(&normalized_date)
                    .execute(db)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?;

                    if result.rows_affected() > 0 {
                        imported += 1;
                    } else {
                        skipped += 1;
                    }

                    let _ = sqlx::query(
                        r#"
                        INSERT OR REPLACE INTO track_sources 
                        (track_id, service_id, service_track_id, format, bitrate, quality_score, available) 
                        VALUES (?, ?, ?, 'AAC', 256, NULL, 1)
                        "#,
                    )
                    .bind(track_id)
                    .bind(service_id)
                    .bind(&track.id)
                    .execute(db)
                    .await;
                }
            }

            let next_decision = crate::services::import_pagination::next_apple_music_offset(
                offset,
                playlists.len() as i32,
                limit,
                page.next.as_deref(),
                page.meta.as_ref().and_then(|m| m.total),
            );

            match next_decision {
                Some(next_off) => offset = next_off,
                None => break,
            }
        }

        Ok(super::ImportResult { imported, skipped })
    }

    /// Import full Apple Music library: songs, albums, and playlists
    pub async fn import_full_library(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        let songs_res = self.import_library(db, account_id).await?;
        let albums_res = self.import_albums(db, account_id).await.unwrap_or(super::ImportResult { imported: 0, skipped: 0 });
        let playlists_res = self.import_playlists(db, account_id).await.unwrap_or(super::ImportResult { imported: 0, skipped: 0 });

        Ok(super::ImportResult {
            imported: songs_res.imported + albums_res.imported + playlists_res.imported,
            skipped: songs_res.skipped + albums_res.skipped + playlists_res.skipped,
        })
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
        let clean_name = syncify_core_domain::metadata::sanitize_artist_name(name);
        if clean_name.is_empty() {
            return Err("Cannot create artist with empty name".to_string());
        }
        let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
            .bind(&clean_name)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        let artist_id: i64 = sqlx::query_scalar(
            "INSERT INTO artists (name) VALUES (?) ON CONFLICT(name) DO UPDATE SET id=id RETURNING id"
        )
        .bind(&clean_name)
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
        let clean_track_title = syncify_core_domain::metadata::sanitize_track_title(&attrs.name);
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(&clean_track_title)
        .bind(album_id)
        .bind(attrs.duration_in_millis)
        .bind(&attrs.isrc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(track_id)
    }
}
