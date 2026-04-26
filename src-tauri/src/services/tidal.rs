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
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i32>,
    #[serde(rename = "volumeNumber")]
    pub disc_number: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalArtist {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct TidalFavoriteItem {
    pub item: TidalTrack,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalFavoriteAlbumItem {
    pub item: TidalAlbum,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TidalPaginated {
    pub items: Vec<TidalFavoriteItem>,
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
            return Err(format!("Tidal API error {}: {}", status, body));
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
            return Err(format!("Tidal API error {}: {}", status, body));
        }

        response.json::<TidalArtistPaginated>().await.map_err(|e| format!("Failed to parse artists: {}", e))
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
            return Err(format!("Tidal API error {}: {}", status, body));
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
            return Err(format!("Tidal API error {}: {}", status, body));
        }

        response.json().await.map_err(|e| format!("Failed to parse playlist tracks: {}", e))
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

            tracing::info!("Tidal: Processing {} favorites (Batch Start)", page.items.len());
            
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
                    .bind(&album.cover)
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

            tracing::info!("Tidal: Processing {} favorite albums (Batch Start)", page.items.len());
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
                .bind(&album.cover)
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

            let mut tx = db.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;

            for fav_item in page.items.iter() {
                let artist = &fav_item.item;
                
                // Artist Upsert (Target name to link existing artists to tidal_id)
                let res = sqlx::query(
                    r#"
                    INSERT INTO artists (name, tidal_id)
                    VALUES (?, ?)
                    ON CONFLICT(name) 
                    DO UPDATE SET tidal_id = excluded.tidal_id
                    "#
                )
                .bind(&artist.name)
                .bind(artist.id.to_string())
                .execute(&mut *tx)
                .await;

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

            tx.commit().await.map_err(|e| format!("Failed to commit artists: {}", e))?;

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
                            .bind(&album.cover)
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
        .bind(&album.cover)
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
