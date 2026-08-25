//! Deezer service - Authentication and library import
//!
//! Handles Deezer API access using ARL cookie.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const DEEZER_API_BASE: &str = "https://www.deezer.com/ajax/gw-light.php";
const DEEZER_PUBLIC_API: &str = "https://api.deezer.com";

/// Deezer track from API
#[derive(Debug, Clone, Deserialize)]
pub struct DeezerTrack {
    #[serde(rename = "SNG_ID")]
    pub id: String,
    #[serde(rename = "SNG_TITLE")]
    pub title: String,
    #[serde(rename = "DURATION")]
    pub duration: String,
    #[serde(rename = "ISRC")]
    pub isrc: Option<String>,
    #[serde(rename = "ART_NAME")]
    pub artist_name: Option<String>,
    #[serde(rename = "ALB_TITLE")]
    pub album_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeezerApiResponse {
    pub results: Option<DeezerResults>,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeezerResults {
    pub data: Option<Vec<DeezerTrack>>,
    pub total: Option<i32>,
    #[serde(rename = "checkForm")]
    pub check_form: Option<String>,
    #[serde(rename = "USER")]
    pub user: Option<DeezerUser>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeezerUser {
    #[serde(rename = "USER_ID", deserialize_with = "deserialize_id")]
    pub id: String,
}

/// S189-Fase-1: album summary from the public API user-albums listing.
#[derive(Debug, Clone)]
pub struct DeezerAlbumSummary {
    pub id: String,
    pub title: String,
    pub nb_tracks: Option<i32>,
    pub cover: Option<String>,
    pub artist_name: Option<String>,
}

/// S189-Fase-1: playlist summary from the public API user-playlists listing.
#[derive(Debug, Clone)]
pub struct DeezerPlaylistSummary {
    pub id: String,
    pub title: String,
    pub nb_tracks: Option<i32>,
    pub is_public: Option<bool>,
    pub is_collaborative: Option<bool>,
    pub description: Option<String>,
    pub cover: Option<String>,
}

/// Helper to deserialize ID as either string or integer
fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
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

/// Deezer API client using ARL cookie
pub struct DeezerClient {
    client: Client,
    arl: String,
    api_token: Option<String>,
    user_id: Option<String>,
    /// Injectable endpoints (S189 tests): production always uses the consts.
    api_base: String,
    public_api_base: String,
}

impl DeezerClient {
    pub fn new(arl: String) -> Self {
        Self {
            client: Client::new(),
            arl,
            api_token: None,
            user_id: None,
            api_base: DEEZER_API_BASE.to_string(),
            public_api_base: DEEZER_PUBLIC_API.to_string(),
        }
    }

    /// S189 test seam: redirect the gw-light endpoint (init/auth).
    /// Production never calls this; mirrors the S187 Tidal injectable base URL.
    pub fn with_api_base(mut self, base: String) -> Self {
        self.api_base = base;
        self
    }

    /// S189 test seam: redirect the public API endpoint.
    pub fn with_public_api_base(mut self, base: String) -> Self {
        self.public_api_base = base;
        self
    }

    pub fn user_id(&self) -> Option<String> {
        self.user_id.clone()
    }

    /// Initialize the client by getting user data and API token
    pub async fn init(&mut self) -> Result<(), String> {
        let response = self
            .client
            .post(&self.api_base)
            .query(&[
                ("method", "deezer.getUserData"),
                ("api_version", "1.0"),
                ("api_token", ""),
            ])
            .header("Cookie", format!("arl={}", self.arl))
            .json(&serde_json::json!({})) // Empty JSON body to satisfy Content-Length
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        tracing::debug!(
            "Deezer getUserData response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        if !status.is_success() {
            return Err(format!(
                "Deezer API error ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        let data: DeezerApiResponse = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Deezer response: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })?;

        if let Some(results) = data.results {
            self.api_token = results.check_form;
            self.user_id = results.user.map(|u| u.id);
        }

        if self.api_token.is_none() {
            return Err(format!(
                "Failed to get Deezer API token - ARL may be invalid (raw: {})",
                &text[..text.len().min(200)]
            ));
        }

        Ok(())
    }

    /// Get user's favorite tracks
    pub async fn get_favorites(&self, start: i32, count: i32) -> Result<Vec<DeezerTrack>, String> {
        let api_token = self.api_token.as_ref().ok_or("Not initialized")?;
        let user_id = self.user_id.as_ref().ok_or("User ID not set")?;

        // Parse user_id as integer for the API call
        let user_id_int: i64 = user_id.parse().unwrap_or(0);

        tracing::debug!(
            "Deezer get_favorites: user_id={}, start={}, count={}",
            user_id,
            start,
            count
        );

        let response = self
            .client
            .post(DEEZER_API_BASE)
            .query(&[
                ("method", "song.getListByFavorite"),
                ("api_version", "1.0"),
                ("api_token", api_token),
            ])
            .header("Cookie", format!("arl={}", self.arl))
            .json(&serde_json::json!({
                "user_id": user_id_int,
                "start": start,
                "nb": count
            }))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read: {}", e))?;

        tracing::debug!(
            "Deezer favorites response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        let data: DeezerApiResponse = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })?;

        let tracks = data.results.and_then(|r| r.data).unwrap_or_default();
        tracing::info!("Deezer gw-light favorites returned {} tracks", tracks.len());

        Ok(tracks)
    }

    /// Get user's favorite tracks via public API (more reliable)
    pub async fn get_favorites_public(
        &self,
        user_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<(Vec<DeezerTrack>, i32), String> {
        // Use public API: https://api.deezer.com/user/{user_id}/tracks
        let url = format!("{}/user/{}/tracks", DEEZER_PUBLIC_API, user_id);

        tracing::debug!(
            "Deezer public API: {} (offset={}, limit={})",
            url,
            offset,
            limit
        );

        let response = self
            .client
            .get(&url)
            .header("Cookie", format!("arl={}", self.arl))
            .query(&[("index", offset.to_string()), ("limit", limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read: {}", e))?;

        tracing::debug!(
            "Deezer public API response ({}): {}",
            status,
            &text[..text.len().min(500)]
        );

        // Parse public API response format
        #[derive(Deserialize)]
        struct PublicApiResponse {
            data: Option<Vec<PublicApiTrack>>,
            total: Option<i32>,
        }

        #[derive(Deserialize)]
        struct PublicApiTrack {
            id: i64,
            title: String,
            duration: i32,
            #[serde(default)]
            isrc: Option<String>,
            artist: Option<PublicArtist>,
            album: Option<PublicAlbum>,
        }

        #[derive(Deserialize)]
        struct PublicArtist {
            name: String,
        }

        #[derive(Deserialize)]
        struct PublicAlbum {
            title: String,
        }

        let api_resp: PublicApiResponse = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse public API: {} (raw: {})",
                e,
                &text[..text.len().min(200)]
            )
        })?;

        // Convert to internal track format
        let tracks: Vec<DeezerTrack> = api_resp
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|t| DeezerTrack {
                id: t.id.to_string(),
                title: t.title,
                duration: t.duration.to_string(),
                isrc: t.isrc,
                artist_name: t.artist.map(|a| a.name),
                album_title: t.album.map(|a| a.title),
            })
            .collect();

        tracing::info!(
            "Deezer public API returned {} tracks (total: {:?})",
            tracks.len(),
            api_resp.total
        );
        Ok((tracks, api_resp.total.unwrap_or(0)))
    }

    // ==============================================
    // S189-Fase-1: unified-engine public API coverage
    // (albums / artists / playlists / playlist-tracks)
    // ==============================================

    /// Throttled GET against the public API returning raw JSON.
    /// Every call goes through the GLOBAL_RATE_LIMITER with the deezer profile
    /// (50 requests / 5 s) — Fase-0 item: the laggard services never throttled.
    async fn public_get_json(
        &self,
        path: &str,
        index: i32,
        limit: i32,
    ) -> Result<serde_json::Value, String> {
        crate::services::rate_limiter::GLOBAL_RATE_LIMITER
            .acquire("deezer")
            .await;

        let url = format!("{}{}", self.public_api_base, path);
        let response = self
            .client
            .get(&url)
            .header("Cookie", format!("arl={}", self.arl))
            .query(&[("index", index.to_string()), ("limit", limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read: {}", e))?;

        if !status.is_success() {
            return Err(format!("Deezer API error ({}): {}", status, &text[..text.len().min(200)]));
        }

        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Deezer parse error: {}", e))?;
        if let Some(err) = json.get("error").filter(|e| !e.is_null()) {
            if err.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                return Err(format!("Deezer API error payload: {}", err));
            }
        }
        Ok(json)
    }

    /// User's saved albums (public API, paginated). Returns (albums, total).
    pub async fn get_user_albums_public(
        &self,
        user_id: &str,
        index: i32,
        limit: i32,
    ) -> Result<(Vec<DeezerAlbumSummary>, i64), String> {
        #[derive(Deserialize)]
        struct RawAlbum {
            id: i64,
            title: String,
            #[serde(default)]
            nb_tracks: Option<i32>,
            #[serde(default)]
            cover_medium: Option<String>,
            #[serde(default)]
            cover: Option<String>,
            #[serde(default)]
            artist: Option<PublicArtist>,
        }
        #[derive(Deserialize)]
        struct PublicArtist {
            #[allow(dead_code)]
            id: i64,
            name: String,
        }
        #[derive(Deserialize)]
        struct Page {
            #[serde(default)]
            data: Option<Vec<RawAlbum>>,
            #[serde(default)]
            total: Option<i64>,
        }

        let json = self
            .public_get_json(&format!("/user/{}/albums", user_id), index, limit)
            .await?;
        let page: Page = serde_json::from_value(json).map_err(|e| format!("Deezer albums parse error: {}", e))?;
        let albums = page
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|a| DeezerAlbumSummary {
                id: a.id.to_string(),
                title: a.title,
                nb_tracks: a.nb_tracks,
                cover: a.cover_medium.or(a.cover),
                artist_name: a.artist.map(|ar| ar.name),
            })
            .collect();
        Ok((albums, page.total.unwrap_or(0)))
    }

    /// Full track list of an album (public /album/{id}; tracks embed isrc).
    pub async fn get_album_tracks_public(&self, album_id: &str) -> Result<Vec<DeezerTrack>, String> {
        #[derive(Deserialize)]
        struct RawAlbum {
            #[serde(default)]
            tracks: Option<RawTracks>,
        }
        #[derive(Deserialize)]
        struct RawTracks {
            #[serde(default)]
            data: Vec<PublicTrack>,
        }
        #[derive(Deserialize)]
        struct PublicTrack {
            id: i64,
            title: String,
            #[serde(default)]
            duration: Option<i64>,
            #[serde(default)]
            isrc: Option<String>,
            #[serde(default)]
            artist: Option<PublicArtist>,
            #[serde(default)]
            album: Option<PublicAlbumRef>,
        }
        #[derive(Deserialize)]
        struct PublicArtist {
            name: String,
        }
        #[derive(Deserialize)]
        struct PublicAlbumRef {
            #[serde(default)]
            title: Option<String>,
        }

        let json = self.public_get_json(&format!("/album/{}", album_id), 0, 0).await?;
        let raw: RawAlbum =
            serde_json::from_value(json).map_err(|e| format!("Deezer album parse error: {}", e))?;
        Ok(raw
            .tracks
            .map(|t| t.data)
            .unwrap_or_default()
            .into_iter()
            .map(|t| DeezerTrack {
                id: t.id.to_string(),
                title: t.title,
                duration: t.duration.unwrap_or(0).to_string(),
                isrc: t.isrc,
                artist_name: t.artist.map(|a| a.name),
                album_title: t.album.and_then(|a| a.title),
            })
            .collect())
    }

    /// User's followed artists (public API, paginated). Returns ((id, name), total).
    pub async fn get_user_artists_public(
        &self,
        user_id: &str,
        index: i32,
        limit: i32,
    ) -> Result<(Vec<(String, String)>, i64), String> {
        #[derive(Deserialize)]
        struct RawArtist {
            id: i64,
            name: String,
        }
        #[derive(Deserialize)]
        struct Page {
            #[serde(default)]
            data: Option<Vec<RawArtist>>,
            #[serde(default)]
            total: Option<i64>,
        }

        let json = self
            .public_get_json(&format!("/user/{}/artists", user_id), index, limit)
            .await?;
        let page: Page = serde_json::from_value(json).map_err(|e| format!("Deezer artists parse error: {}", e))?;
        let artists = page
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|a| (a.id.to_string(), a.name))
            .collect();
        Ok((artists, page.total.unwrap_or(0)))
    }

    /// User's playlists (public API, paginated). Returns (playlists, total).
    pub async fn get_user_playlists_public(
        &self,
        user_id: &str,
        index: i32,
        limit: i32,
    ) -> Result<(Vec<DeezerPlaylistSummary>, i64), String> {
        #[derive(Deserialize)]
        struct RawPlaylist {
            id: i64,
            title: String,
            #[serde(default)]
            nb_tracks: Option<i32>,
            #[serde(default)]
            public: Option<bool>,
            #[serde(default)]
            collaborative: Option<bool>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            picture_medium: Option<String>,
            #[serde(default)]
            picture: Option<String>,
        }
        #[derive(Deserialize)]
        struct Page {
            #[serde(default)]
            data: Option<Vec<RawPlaylist>>,
            #[serde(default)]
            total: Option<i64>,
        }

        let json = self
            .public_get_json(&format!("/user/{}/playlists", user_id), index, limit)
            .await?;
        let page: Page = serde_json::from_value(json).map_err(|e| format!("Deezer playlists parse error: {}", e))?;
        let playlists = page
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|p| DeezerPlaylistSummary {
                id: p.id.to_string(),
                title: p.title,
                nb_tracks: p.nb_tracks,
                is_public: p.public,
                is_collaborative: p.collaborative,
                description: p.description,
                cover: p.picture_medium.or(p.picture),
            })
            .collect();
        Ok((playlists, page.total.unwrap_or(0)))
    }

    /// Tracks of a playlist (public API, paginated). Returns (tracks, total).
    pub async fn get_playlist_tracks_public(
        &self,
        playlist_id: &str,
        index: i32,
        limit: i32,
    ) -> Result<(Vec<DeezerTrack>, i64), String> {
        #[derive(Deserialize)]
        struct PublicTrack {
            id: i64,
            title: String,
            #[serde(default)]
            duration: Option<i64>,
            #[serde(default)]
            isrc: Option<String>,
            #[serde(default)]
            artist: Option<PublicArtist>,
            #[serde(default)]
            album: Option<PublicAlbumRef>,
        }
        #[derive(Deserialize)]
        struct PublicArtist {
            name: String,
        }
        #[derive(Deserialize)]
        struct PublicAlbumRef {
            #[serde(default)]
            title: Option<String>,
        }
        #[derive(Deserialize)]
        struct Page {
            #[serde(default)]
            data: Option<Vec<PublicTrack>>,
            #[serde(default)]
            total: Option<i64>,
        }

        let json = self
            .public_get_json(&format!("/playlist/{}/tracks", playlist_id), index, limit)
            .await?;
        let page: Page = serde_json::from_value(json).map_err(|e| format!("Deezer playlist tracks parse error: {}", e))?;
        let tracks = page
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|t| DeezerTrack {
                id: t.id.to_string(),
                title: t.title,
                duration: t.duration.unwrap_or(0).to_string(),
                isrc: t.isrc,
                artist_name: t.artist.map(|a| a.name),
                album_title: t.album.and_then(|a| a.title),
            })
            .collect();
        Ok((tracks, page.total.unwrap_or(0)))
    }

    /// Import all favorites to database
    pub async fn import_library(
        &mut self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<super::ImportResult, String> {
        // Initialize first to get user_id
        self.init().await?;

        let user_id = self.user_id.clone().ok_or("User ID not available")?;

        let mut offset = 0;
        let limit = 100;
        let mut imported = 0;
        let mut skipped = 0;

        let deezer_service_id = self.get_service_id(db, "deezer").await?;

        loop {
            // Use public API instead of gw-light (more reliable)
            let (tracks, _) = self.get_favorites_public(&user_id, offset, limit).await?;

            if tracks.is_empty() {
                break;
            }

            for track in &tracks {
                // Get or create artist
                let artist_name = track.artist_name.clone().unwrap_or_default();
                let artist_id = self.get_or_create_artist(db, &artist_name).await?;

                // Get or create album (if present)
                let album_id = if let Some(ref album_title) = track.album_title {
                    Some(
                        self.get_or_create_album_by_title(db, album_title, artist_id)
                            .await?,
                    )
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

                // Add track source (Deezer provides up to FLAC quality)
                let _ = sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO track_sources 
                    (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) 
                    VALUES (?, ?, ?, 'FLAC', 16, 44100, 1)
                    "#
                )
                .bind(track_id)
                .bind(deezer_service_id)
                .bind(&track.id)
                .execute(db)
                .await;
            }

            offset += limit;

            tracing::info!("Deezer import: {} imported so far...", imported);

            if tracks.len() < limit as usize {
                break;
            }
        }

        Ok(super::ImportResult { imported, skipped })
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

    pub async fn get_or_create_album_by_title(
        &self,
        db: &SqlitePool,
        title: &str,
        primary_artist_id: i64,
    ) -> Result<i64, String> {
        // Try to find existing by title
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM albums WHERE title = ?")
            .bind(title)
            .fetch_one(db)
            .await
        {
            return Ok(row.0);
        }

        // Create new album
        let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES (?) RETURNING id")
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
        track: &DeezerTrack,
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

        // Parse duration
        let duration_ms: i64 = track.duration.parse::<i64>().unwrap_or(0) * 1000;

        // Create new track with album_id
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(&track.title)
        .bind(album_id)
        .bind(duration_ms)
        .bind(&track.isrc)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Insert failed: {}", e))?;

        Ok(track_id)
    }

    /// Search for tracks by query string using public API
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<DeezerSearchResult>, String> {
        let url = format!("{}/search/track", DEEZER_PUBLIC_API);

        let response = self
            .client
            .get(&url)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!(
                "Deezer search failed ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            data: Option<Vec<SearchTrack>>,
        }

        #[derive(Deserialize)]
        struct SearchTrack {
            id: i64,
            title: String,
            duration: i32,
            #[serde(default)]
            isrc: Option<String>,
            artist: Option<SearchArtist>,
            album: Option<SearchAlbum>,
        }

        #[derive(Deserialize)]
        struct SearchArtist {
            name: String,
        }

        #[derive(Deserialize)]
        struct SearchAlbum {
            title: String,
        }

        let search_resp: SearchResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse search response: {}", e))?;

        let results = search_resp
            .data
            .unwrap_or_default()
            .into_iter()
            .map(|t| DeezerSearchResult {
                track_id: t.id.to_string(),
                title: t.title,
                artist: t.artist.map(|a| a.name).unwrap_or_default(),
                album: t.album.map(|a| a.title),
                isrc: t.isrc,
                duration_ms: (t.duration as i64) * 1000,
            })
            .collect();

        Ok(results)
    }

    /// Search for a track by ISRC code
    pub async fn search_by_isrc(&self, isrc: &str) -> Result<Option<DeezerSearchResult>, String> {
        let results = self.search_track(isrc, 5).await?;
        let match_result = results.into_iter().find(|r| {
            r.isrc
                .as_ref()
                .map(|i| i.eq_ignore_ascii_case(isrc))
                .unwrap_or(false)
        });
        Ok(match_result)
    }

    /// Match a track by metadata
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<DeezerSearchResult>, String> {
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

    /// Add a track to user's favorites
    /// Note: Deezer requires OAuth token, not just ARL cookie for write operations
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        let api_token = self.api_token.as_ref().ok_or("Not initialized")?;

        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let response = self
                .client
                .post(DEEZER_API_BASE)
                .query(&[
                    ("method", "favorite_song.add"),
                    ("api_version", "1.0"),
                    ("api_token", api_token.as_str()),
                ])
                .header("Cookie", format!("arl={}", self.arl))
                .json(&serde_json::json!({
                    "SNG_ID": track_id
                }))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        tracing::info!("Added track {} to Deezer favorites", track_id);
                        return Ok(());
                    } else if status.as_u16() == 429 || status.as_u16() >= 500 {
                        let text = resp.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "Deezer add_to_favorites attempt {} failed ({}), retrying...",
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
                        "Deezer add_to_favorites attempt {} failed: {}, retrying...",
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
pub struct DeezerSearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i64,
}
