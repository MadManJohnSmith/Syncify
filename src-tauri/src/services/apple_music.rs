//! Apple Music service - Authentication, library import and migration matching
//!
//! Handles Apple Music API access via MusicKit.

use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;

use super::import_pagination::next_apple_music_offset;

const APPLE_MUSIC_API: &str = "https://amp-api.music.apple.com/v1";

/// Storefront used when the stored credentials carry none (or an unusable one).
pub const DEFAULT_APPLE_MUSIC_STOREFRONT: &str = "us";

/// Environment variable that overrides the storefront when the account
/// credentials were saved before the storefront was captured.
pub const APPLE_MUSIC_STOREFRONT_ENV: &str = "APPLE_MUSIC_STOREFRONT";

/// Environment variable that redirects every [`AppleMusicClient`] at a local
/// mock server. Test seam only (same pattern as `SYNCIFY_S197_TIDAL_BASE_URL`):
/// unset in production, where traffic always goes to the MusicKit API.
pub const APPLE_MUSIC_BASE_URL_ENV: &str = "SYNCIFY_APPLE_MUSIC_BASE_URL";

/// Base URL every client starts from: the mock server when the test seam is
/// set, the public MusicKit API otherwise.
fn base_url_from_env() -> String {
    std::env::var(APPLE_MUSIC_BASE_URL_ENV)
        .ok()
        .map(|base| base.trim().trim_end_matches('/').to_string())
        .filter(|base| !base.is_empty())
        .unwrap_or_else(|| APPLE_MUSIC_API.to_string())
}

/// Page size used by the paginated catalog/library requests.
const PAGE_LIMIT: i32 = 100;

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

/// Normalize a storefront id (Apple uses lowercase ISO 3166-1 alpha-2 codes).
/// Returns `None` for anything that is not a usable storefront id, so callers
/// can fall back to [`DEFAULT_APPLE_MUSIC_STOREFRONT`] instead of building a
/// bogus `/catalog/<garbage>/` URL.
pub fn normalize_storefront(raw: Option<&str>) -> Option<String> {
    let candidate = raw?.trim().to_ascii_lowercase();
    if candidate.len() == 2 && candidate.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(candidate)
    } else {
        None
    }
}

/// Resolve the storefront for a client: stored credentials first, then
/// `APPLE_MUSIC_STOREFRONT`, then the Apple default (`us`).
pub fn resolve_storefront(credentials_value: Option<&str>) -> String {
    if let Some(value) = credentials_value {
        if let Some(storefront) = normalize_storefront(Some(value)) {
            return storefront;
        }
        tracing::warn!(
            "Apple Music: ignoring unusable storefront '{}' from credentials, falling back",
            value
        );
    }

    let from_env = std::env::var(APPLE_MUSIC_STOREFRONT_ENV).ok();
    if from_env.is_some() {
        if let Some(storefront) = normalize_storefront(from_env.as_deref()) {
            return storefront;
        }
        tracing::warn!(
            "Apple Music: ignoring unusable {} value, falling back to {}",
            APPLE_MUSIC_STOREFRONT_ENV,
            DEFAULT_APPLE_MUSIC_STOREFRONT
        );
    }

    DEFAULT_APPLE_MUSIC_STOREFRONT.to_string()
}

/// Outcome of importing one Apple Music entity collection (albums or
/// playlists): the track counters plus how many entities of that kind landed.
#[derive(Debug, Clone, Copy, Default)]
pub struct AppleMusicEntityImport {
    pub imported: i32,
    pub skipped: i32,
    pub entities: i32,
}

/// Apple Music API client
pub struct AppleMusicClient {
    client: Client,
    music_user_token: String,
    developer_token: String,
    base_url: String,
    storefront: String,
}

impl AppleMusicClient {
    pub fn new(developer_token: String, music_user_token: String) -> Self {
        Self {
            client: Client::new(),
            music_user_token,
            developer_token,
            base_url: base_url_from_env(),
            storefront: DEFAULT_APPLE_MUSIC_STOREFRONT.to_string(),
        }
    }

    // Seam de test explícito: redirige la API en
    // `tests/service_import_pagination_and_purchases_test.rs` y
    // `tests/apple_music_storefront_pagination_and_migration_test.rs`.
    #[allow(dead_code)]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Set the storefront (catalog) used by every `/catalog/{storefront}/` call.
    /// Unusable values keep the previous storefront instead of building a broken URL.
    pub fn with_storefront(mut self, storefront: &str) -> Self {
        if let Some(value) = normalize_storefront(Some(storefront)) {
            self.storefront = value;
        } else {
            tracing::warn!(
                "Apple Music: ignoring unusable storefront '{}', keeping '{}'",
                storefront,
                self.storefront
            );
        }
        self
    }

    /// Build a client whose storefront comes from the stored account credentials
    /// (`credentials["storefront"]`), then from `APPLE_MUSIC_STOREFRONT`, then `us`.
    pub fn from_credentials(
        developer_token: String,
        music_user_token: String,
        creds: &Value,
    ) -> Self {
        let storefront = resolve_storefront(creds.get("storefront").and_then(|v| v.as_str()));
        Self::new(developer_token, music_user_token).with_storefront(&storefront)
    }

    /// Storefront currently used for catalog lookups.
    pub fn storefront(&self) -> &str {
        &self.storefront
    }

    /// Absolute URL of a catalog resource (`/catalog/{storefront}/{resource}`).
    fn catalog_url(&self, resource: &str) -> String {
        format!(
            "{}/catalog/{}/{}",
            self.base_url.trim_end_matches('/'),
            self.storefront,
            resource
        )
    }

    fn absolute_url(&self, path_or_url: &str) -> String {
        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            path_or_url.to_string()
        } else {
            let p = path_or_url.strip_prefix("/v1/").unwrap_or(path_or_url);
            let p = p.strip_prefix('/').unwrap_or(p);
            format!("{}/{}", self.base_url.trim_end_matches('/'), p)
        }
    }

    /// Generic JSON request helper supporting relative paths and full URLs
    pub async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        path_or_url: &str,
    ) -> Result<T, String> {
        self.request_json_with(Method::GET, path_or_url, None).await
    }

    /// Same as [`Self::request_json`] for the write verbs Apple Music needs
    /// (adding a track to the library) and an optional JSON body.
    async fn request_json_with<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        path_or_url: &str,
        body: Option<&Value>,
    ) -> Result<T, String> {
        let response = self.send(method, path_or_url, body).await?;

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Apple Music JSON: {}", e))
    }

    /// Fire a request and discard the response body (Apple Music answers some
    /// writes with an empty payload).
    async fn send_no_content(
        &self,
        method: Method,
        path_or_url: &str,
        body: Option<&Value>,
    ) -> Result<(), String> {
        self.send(method, path_or_url, body).await.map(|_| ())
    }

    /// Authenticated request with the MusicKit headers Apple requires; returns
    /// the raw response for the caller to read (or discard).
    ///
    /// Every Apple Music call funnels through here, so this is where the shared
    /// retryability criterion (`http_retry::is_transient_status`) and the shared
    /// limiter meet: `429`/`408`/`5xx` and transport hiccups are retried with
    /// backoff, while `401`/`403`/`404` reach the caller immediately — a token
    /// or a missing resource is not fixed by trying again.
    async fn send(
        &self,
        method: Method,
        path_or_url: &str,
        body: Option<&Value>,
    ) -> Result<reqwest::Response, String> {
        let url = self.absolute_url(path_or_url);

        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            crate::services::rate_limiter::GLOBAL_RATE_LIMITER
                .acquire("apple_music")
                .await;

            let mut request = self
                .client
                .request(method.clone(), &url)
                .header("Authorization", format!("Bearer {}", self.developer_token))
                .header("media-user-token", &self.music_user_token)
                .header("Origin", "https://music.apple.com")
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                );

            if let Some(body) = body {
                request = request.json(body);
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return Ok(response);
                    } else if crate::services::http_retry::is_transient_status(status) {
                        crate::services::rate_limiter::penalize_on_rate_limit(
                            "apple_music",
                            status,
                            response.headers(),
                        )
                        .await;
                        let text = response.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "Apple Music request attempt {} failed ({}), retrying...",
                            attempt + 1,
                            status
                        );
                    } else {
                        let text = response.text().await.unwrap_or_default();
                        return Err(format!("Apple Music API error {}: {}", status, text));
                    }
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!(
                        "Apple Music request attempt {} failed: {}, retrying...",
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
            "Apple Music request failed after {} retries: {}",
            max_retries, last_error
        ))
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
        let path = format!(
            "me/library/albums?offset={}&limit={}&include=tracks",
            offset, limit
        );
        self.request_json(&path).await
    }

    /// Get user's library playlists (paginated)
    pub async fn get_library_playlists(
        &self,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicPlaylistsResponse, String> {
        let path = format!(
            "me/library/playlists?offset={}&limit={}&include=tracks",
            offset, limit
        );
        self.request_json(&path).await
    }

    /// Get playlist tracks (paginated)
    pub async fn get_playlist_tracks(
        &self,
        playlist_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicResponse, String> {
        let path = format!(
            "me/library/playlists/{}/tracks?offset={}&limit={}",
            playlist_id, offset, limit
        );
        self.request_json(&path).await
    }

    /// Get album tracks (paginated)
    pub async fn get_album_tracks(
        &self,
        album_id: &str,
        offset: i32,
        limit: i32,
    ) -> Result<AppleMusicResponse, String> {
        let path = format!(
            "me/library/albums/{}/tracks?offset={}&limit={}",
            album_id, offset, limit
        );
        self.request_json(&path).await
    }

    /// Every track of a library playlist, following the API `next` cursor until
    /// the playlist is exhausted (a single page caps at `limit` items).
    pub async fn fetch_all_playlist_tracks(
        &self,
        playlist_id: &str,
        limit: i32,
    ) -> Result<Vec<AppleMusicTrack>, String> {
        let mut offset = 0;
        let mut collected = Vec::new();

        loop {
            let page = self.get_playlist_tracks(playlist_id, offset, limit).await?;
            let batch = page.data.clone().unwrap_or_default();
            let batch_len = batch.len() as i32;
            collected.extend(batch);

            match next_apple_music_offset(
                offset,
                batch_len,
                limit,
                page.next.as_deref(),
                page.meta.as_ref().and_then(|m| m.total),
            ) {
                Some(next) => offset = next,
                None => break,
            }
        }

        Ok(collected)
    }

    /// Every track of a library album, following the API `next` cursor.
    pub async fn fetch_all_album_tracks(
        &self,
        album_id: &str,
        limit: i32,
    ) -> Result<Vec<AppleMusicTrack>, String> {
        let mut offset = 0;
        let mut collected = Vec::new();

        loop {
            let page = self.get_album_tracks(album_id, offset, limit).await?;
            let batch = page.data.clone().unwrap_or_default();
            let batch_len = batch.len() as i32;
            collected.extend(batch);

            match next_apple_music_offset(
                offset,
                batch_len,
                limit,
                page.next.as_deref(),
                page.meta.as_ref().and_then(|m| m.total),
            ) {
                Some(next) => offset = next,
                None => break,
            }
        }

        Ok(collected)
    }

    /// Tracks embedded in an album/playlist relationship, but only when Apple
    /// delivered the whole set (`next` absent). A truncated embedded page is
    /// discarded so the caller expands it through the paginated endpoints above.
    fn complete_relationship_tracks(
        relationship_tracks: Option<&AppleMusicResponse>,
    ) -> Option<Vec<AppleMusicTrack>> {
        let page = relationship_tracks?;
        if page.next.is_some() {
            return None;
        }
        let data = page.data.clone().unwrap_or_default();
        if data.is_empty() {
            None
        } else {
            Some(data)
        }
    }

    /// Import all library songs to database
    #[allow(dead_code)] // Cubierta por `tests/backup_restore_e2e_test.rs`, `tests/service_import_pagination_and_purchases_test.rs`.
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
                    attrs.date_added.as_deref(),
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
    ) -> Result<AppleMusicEntityImport, String> {
        let mut offset = 0;
        let limit = 50;
        let mut imported = 0;
        let mut skipped = 0;
        let mut entities = 0;
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
                entities += 1;
                let artist_id = self.get_or_create_artist(db, &attrs.artist_name).await?;
                let album_id = self
                    .get_or_create_album_from_attrs(db, &attrs.name, artist_id, attrs)
                    .await?;

                let tracks = match Self::complete_relationship_tracks(
                    album.relationships.as_ref().and_then(|r| r.tracks.as_ref()),
                ) {
                    Some(embedded) => embedded,
                    // Either no relationship at all or a truncated page: expand
                    // the album through the paginated endpoint so no track is
                    // silently dropped.
                    None => self.fetch_all_album_tracks(&album.id, PAGE_LIMIT).await?,
                };

                for track in &tracks {
                    let track_attrs = match &track.attributes {
                        Some(a) => a,
                        None => continue,
                    };
                    let track_id = self
                        .get_or_create_track(db, track_attrs, Some(album_id))
                        .await?;
                    let _ = sqlx::query(
                        "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                    )
                    .bind(track_id)
                    .bind(artist_id)
                    .execute(db)
                    .await;

                    let normalized_date = crate::services::import_pagination::normalize_added_at(
                        track_attrs
                            .date_added
                            .as_deref()
                            .or(attrs.date_added.as_deref()),
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

        Ok(AppleMusicEntityImport {
            imported,
            skipped,
            entities,
        })
    }

    /// Import user's library playlists and their tracks
    pub async fn import_playlists(
        &self,
        db: &SqlitePool,
        account_id: i64,
    ) -> Result<AppleMusicEntityImport, String> {
        let mut offset = 0;
        let limit = 50;
        let mut imported = 0;
        let mut skipped = 0;
        let mut entities = 0;
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
                entities += 1;

                let playlist_name = &attrs.name;
                let desc = attrs.description.as_ref().and_then(|d| d.standard.clone());

                let tracks = match Self::complete_relationship_tracks(
                    playlist
                        .relationships
                        .as_ref()
                        .and_then(|r| r.tracks.as_ref()),
                ) {
                    Some(embedded) => embedded,
                    // Truncated or absent relationship: expand through the
                    // paginated endpoint so the whole playlist is imported.
                    None => {
                        self.fetch_all_playlist_tracks(&playlist.id, PAGE_LIMIT)
                            .await?
                    }
                };

                // Upsert playlist
                let playlist_db_id: i64 = sqlx::query_scalar(
                    r#"
                    INSERT INTO playlists (account_id, service_playlist_id, name, description, is_public, track_count, last_synced)
                    VALUES (?, ?, ?, ?, 0, ?, CURRENT_TIMESTAMP)
                    ON CONFLICT(account_id, service_playlist_id) DO UPDATE SET
                        name = excluded.name,
                        description = excluded.description,
                        track_count = excluded.track_count,
                        last_synced = CURRENT_TIMESTAMP
                    RETURNING id
                    "#
                )
                .bind(account_id)
                .bind(&playlist.id)
                .bind(playlist_name)
                .bind(&desc)
                .bind(tracks.len() as i32)
                .fetch_one(db)
                .await
                .map_err(|e| format!("Failed to upsert playlist: {}", e))?;

                // Register the remote identity so playlist cross-service sync and
                // the collision guards of migration 0075 can see this playlist.
                sqlx::query(
                    r#"
                    INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
                    VALUES (?, ?, ?, ?)
                    ON CONFLICT(account_id, service_playlist_id) DO UPDATE SET
                        playlist_id = excluded.playlist_id,
                        synced_at = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(playlist_db_id)
                .bind(account_id)
                .bind(service_id)
                .bind(&playlist.id)
                .execute(db)
                .await
                .map_err(|e| format!("Failed to upsert playlist source: {}", e))?;

                for (idx, track) in tracks.iter().enumerate() {
                    let track_attrs = match &track.attributes {
                        Some(a) => a,
                        None => continue,
                    };
                    let artist_id = self
                        .get_or_create_artist(db, &track_attrs.artist_name)
                        .await?;
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
                        track_attrs
                            .date_added
                            .as_deref()
                            .or(attrs.date_added.as_deref()),
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

        Ok(AppleMusicEntityImport {
            imported,
            skipped,
            entities,
        })
    }

    /// Search for a track by ISRC code
    pub async fn search_by_isrc(
        &self,
        isrc: &str,
    ) -> Result<Option<AppleMusicSearchResult>, String> {
        // Apple Music supports ISRC filtering
        let url = self.catalog_url(&format!("songs?filter[isrc]={}", urlencoding::encode(isrc)));

        let resp: AppleMusicResponse = match self.request_json(&url).await {
            Ok(resp) => resp,
            Err(e) => {
                // The ISRC filter is not always available for every storefront or
                // token: fall back to a text search on the same storefront.
                tracing::warn!(
                    "Apple Music ISRC filter failed ({}), falling back to search",
                    e
                );
                let results = self.search_track(isrc, 5).await?;
                return Ok(results.into_iter().find(|r| {
                    r.isrc
                        .as_ref()
                        .map(|i| i.eq_ignore_ascii_case(isrc))
                        .unwrap_or(false)
                }));
            }
        };

        Ok(Self::to_search_results(resp.data.unwrap_or_default())
            .into_iter()
            .next())
    }

    /// Search the Apple Music catalog, following the `next` cursor until
    /// `limit` results are collected.
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<AppleMusicSearchResult>, String> {
        if limit <= 0 {
            return Ok(Vec::new());
        }

        let url = self.catalog_url(&format!(
            "search?term={}&types=songs&limit={}",
            urlencoding::encode(query),
            limit
        ));

        let page: AppleMusicSearchResponse = self
            .request_json(&url)
            .await
            .map_err(|e| format!("Apple Music search failed: {}", e))?;

        let mut songs = page.results.and_then(|r| r.songs).unwrap_or_default();
        let mut results = Self::to_search_results(songs.data.take().unwrap_or_default());
        let mut next_url = songs.next.take();
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

        // A search page holds fewer songs than the caller may ask for: walk the
        // `next` cursor until the quota is met or the catalog runs out. A cursor
        // repeated by the API would loop forever, so already-seen cursors stop
        // the walk.
        while results.len() < limit as usize {
            let Some(cursor) = next_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                break;
            };
            if !visited.insert(cursor.to_string()) {
                tracing::warn!(
                    "Apple Music search pagination stopped: cursor '{}' repeated",
                    cursor
                );
                break;
            }
            let cursor = cursor.to_string();

            let next_page: AppleMusicSearchResponse = match self.request_json(&cursor).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "Apple Music search pagination stopped at '{}': {}",
                        cursor,
                        e
                    );
                    break;
                }
            };

            songs = next_page.results.and_then(|r| r.songs).unwrap_or_default();
            let batch = songs.data.take().unwrap_or_default();
            if batch.is_empty() {
                break;
            }
            results.extend(Self::to_search_results(batch));
            next_url = songs.next.take();
        }

        results.truncate(limit as usize);
        Ok(results)
    }

    /// Flatten Apple Music song resources into the migration matching shape.
    fn to_search_results(tracks: Vec<AppleMusicTrack>) -> Vec<AppleMusicSearchResult> {
        tracks
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
            .collect()
    }

    /// Add a catalog track to the account's Apple Music library — the migration
    /// engine's transfer step for Apple Music as a destination.
    ///
    /// Apple Music exposes no "favorite" endpoint: `POST /v1/me/library` with
    /// `ids[songs]` is the documented way to add a song to the user's library.
    /// Tracks already in the library are detected first so a re-run does not
    /// depend on how the API answers a repeated insert.
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        let trimmed = track_id.trim();
        if trimmed.is_empty() {
            return Err("Apple Music track id is empty".to_string());
        }

        let already_in_library: AppleMusicResponse = self
            .request_json(&format!(
                "me/library/songs?ids[i]={}&limit=1",
                urlencoding::encode(trimmed)
            ))
            .await
            .map_err(|e| format!("Apple Music library lookup failed: {}", e))?;

        if already_in_library
            .data
            .unwrap_or_default()
            .iter()
            .any(|t| t.id == trimmed)
        {
            tracing::info!("Apple Music track {} already in the library", trimmed);
            return Ok(());
        }

        self.send_no_content(
            Method::POST,
            &format!("me/library?ids[songs]={}", urlencoding::encode(trimmed)),
            None,
        )
        .await
        .map_err(|e| format!("Apple Music add to library failed: {}", e))?;

        tracing::info!("Added Apple Music track {} to the library", trimmed);
        Ok(())
    }

    /// Remove a track from the user's library (`DELETE me/library?ids[songs]=…`),
    /// the documented counterpart of [`Self::add_to_favorites`].
    pub async fn remove_from_favorites(&self, track_id: &str) -> Result<(), String> {
        self.change_library("songs", Method::DELETE, track_id, "track")
            .await
    }

    /// Add an album to the user's library (`POST me/library?ids[albums]=…`).
    /// Same `ids[…]` shape as [`Self::add_to_favorites`], different collection.
    pub async fn add_album_to_library(&self, album_id: &str) -> Result<(), String> {
        self.change_library("albums", Method::POST, album_id, "album")
            .await
    }

    /// Remove an album from the user's library (`DELETE me/library?ids[albums]=…`).
    pub async fn remove_album_from_library(&self, album_id: &str) -> Result<(), String> {
        self.change_library("albums", Method::DELETE, album_id, "album")
            .await
    }

    /// Shared `POST`/`DELETE me/library?ids[{collection}]=` path.
    async fn change_library(
        &self,
        collection: &str,
        method: Method,
        id: &str,
        kind: &str,
    ) -> Result<(), String> {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            return Err(format!("Apple Music {} id is empty", kind));
        }

        self.send_no_content(
            method.clone(),
            &format!(
                "me/library?ids[{}]={}",
                collection,
                urlencoding::encode(trimmed)
            ),
            None,
        )
        .await
        .map_err(|e| {
            format!(
                "Apple Music {} {} failed: {}",
                if method == Method::POST {
                    "add"
                } else {
                    "remove"
                },
                kind,
                e
            )
        })?;

        tracing::info!(
            "Apple Music {} {} {}ed in the library",
            kind,
            trimmed,
            if method == Method::POST {
                "add"
            } else {
                "remov"
            }
        );
        Ok(())
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
        let best_match = results.into_iter().find(|r| {
            let r_title = normalize(&r.title);
            let r_artist = normalize(&r.artist);
            r_title.contains(&target_title)
                || target_title.contains(&r_title)
                || (r_artist.contains(&target_artist) && !r_title.is_empty())
        });

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
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
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

        let album_id: i64 =
            sqlx::query_scalar("INSERT INTO albums (title) VALUES (?) RETURNING id")
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

    /// Album upsert that also carries the Apple album identity: the UPC is the
    /// cross-service album key, and `release_date`/`total_tracks` come straight
    /// from the library album attributes.
    pub async fn get_or_create_album_from_attrs(
        &self,
        db: &SqlitePool,
        title: &str,
        primary_artist_id: i64,
        attrs: &AppleMusicAlbumAttributes,
    ) -> Result<i64, String> {
        let upc = attrs
            .upc
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty());

        // Prefer the UPC identity when Apple provided one; fall back to the title.
        let existing: Option<(i64,)> = if let Some(upc) = upc {
            sqlx::query_as::<_, (i64,)>("SELECT id FROM albums WHERE upc = ? LIMIT 1")
                .bind(upc)
                .fetch_optional(db)
                .await
                .map_err(|e| format!("DB error: {}", e))?
        } else {
            None
        };

        let album_id = match existing {
            Some((id,)) => id,
            None => {
                self.get_or_create_album(db, title, primary_artist_id)
                    .await?
            }
        };

        let _ = sqlx::query(
            "UPDATE albums SET
                release_date = COALESCE(?, release_date),
                total_tracks = COALESCE(?, total_tracks),
                upc = COALESCE(?, upc)
             WHERE id = ?",
        )
        .bind(attrs.release_date.as_deref())
        .bind(attrs.track_count)
        .bind(upc)
        .bind(album_id)
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
