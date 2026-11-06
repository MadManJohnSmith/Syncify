#[allow(unused_imports)]
use super::*;

// Favorites Commands - submodule of crate::commands
// Supports Tidal, Qobuz, and Spotify with SQLite persistence and caching

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FavoriteTrackItem {
    pub id: i64,
    pub service_track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub cover_art_url: Option<String>,
    pub service: String,
    pub favorited_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FavoriteAlbumItem {
    pub id: i64,
    pub service_album_id: String,
    pub title: String,
    pub artist: String,
    pub upc: Option<String>,
    pub cover_art_url: Option<String>,
    pub service: String,
    pub total_tracks: Option<i32>,
    pub release_date: Option<String>,
    pub favorited_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FavoriteArtistItem {
    pub id: i64,
    pub service_artist_id: String,
    pub name: String,
    pub image_url: Option<String>,
    pub service: String,
    pub favorited_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoritesSyncResult {
    pub service: String,
    pub item_type: String,
    pub total_found: i64,
    pub imported: i64,
    pub cached: i64,
    pub message: String,
}

pub async fn perform_get_favorites_tracks(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteTrackItem>, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!(
        "perform_get_favorites_tracks called: service={:?}, offset={}, limit={}",
        service,
        offset,
        limit
    );

    let items = match service.as_deref() {
        None | Some("all") => {
            sqlx::query_as::<_, FavoriteTrackItem>(
                r#"
                SELECT
                    t.id,
                    COALESCE(ts.service_track_id, CAST(t.id AS TEXT)) as service_track_id,
                    t.title,
                    COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1), 'Unknown Artist') as artist,
                    al.title as album,
                    t.isrc,
                    al.cover_art_url,
                    COALESCE(s.name, 'local') as service,
                    t.favorite_at as favorited_at
                FROM tracks t
                LEFT JOIN albums al ON al.id = t.album_id
                LEFT JOIN track_sources ts ON ts.track_id = t.id
                LEFT JOIN services s ON s.id = ts.service_id
                WHERE t.is_favorite = 1
                GROUP BY t.id
                ORDER BY t.favorite_at DESC NULLS LAST, t.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
        Some(svc) if svc.eq_ignore_ascii_case("local") => {
            sqlx::query_as::<_, FavoriteTrackItem>(
                r#"
                SELECT
                    t.id,
                    COALESCE(ts.service_track_id, CAST(t.id AS TEXT)) as service_track_id,
                    t.title,
                    COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1), 'Unknown Artist') as artist,
                    al.title as album,
                    t.isrc,
                    al.cover_art_url,
                    COALESCE(s.name, 'local') as service,
                    t.favorite_at as favorited_at
                FROM tracks t
                LEFT JOIN albums al ON al.id = t.album_id
                LEFT JOIN track_sources ts ON ts.track_id = t.id
                LEFT JOIN services s ON s.id = ts.service_id
                WHERE t.is_favorite = 1
                  AND (ts.id IS NULL OR LOWER(s.name) = 'local')
                GROUP BY t.id
                ORDER BY t.favorite_at DESC NULLS LAST, t.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
        Some(svc) => {
            sqlx::query_as::<_, FavoriteTrackItem>(
                r#"
                SELECT
                    t.id,
                    ts.service_track_id as service_track_id,
                    t.title,
                    COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1), 'Unknown Artist') as artist,
                    al.title as album,
                    t.isrc,
                    al.cover_art_url,
                    s.name as service,
                    t.favorite_at as favorited_at
                FROM tracks t
                JOIN track_sources ts ON ts.track_id = t.id
                JOIN services s ON ts.service_id = s.id
                LEFT JOIN albums al ON al.id = t.album_id
                WHERE t.is_favorite = 1 AND LOWER(s.name) = LOWER(?)
                GROUP BY t.id
                ORDER BY t.favorite_at DESC NULLS LAST, t.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(svc)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
    };

    Ok(items)
}

/// Fetch favorite tracks with multi-service support and pagination
#[tauri::command]
pub async fn get_favorites_tracks(
    state: State<'_, AppState>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteTrackItem>, String> {
    perform_get_favorites_tracks(&state.db, service, offset, limit).await
}

#[allow(dead_code)] // Cubierta por `tests/album_stubs_and_placeholders_test.rs` y `tests/favorites_service_filtering_test.rs`.
pub async fn perform_get_favorites_albums(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteAlbumItem>, String> {
    perform_get_favorites_albums_with_options(db, service, offset, limit, false).await
}

pub async fn perform_get_favorites_albums_with_options(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
    include_stubs: bool,
) -> Result<Vec<FavoriteAlbumItem>, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);
    let include_stubs_int = if include_stubs { 1i64 } else { 0i64 };

    tracing::info!(
        "perform_get_favorites_albums called: service={:?}, offset={}, limit={}, include_stubs={}",
        service,
        offset,
        limit,
        include_stubs
    );

    let items = match service.as_deref() {
        None | Some("all") => {
            sqlx::query_as::<_, FavoriteAlbumItem>(
                r#"
                SELECT
                    al.id,
                    COALESCE(al.tidal_id, al.spotify_id, al.qobuz_id, CAST(al.id AS TEXT)) as service_album_id,
                    al.title,
                    COALESCE((SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = al.id AND aa.is_primary = 1 LIMIT 1), 'Unknown Artist') as artist,
                    al.upc,
                    al.cover_art_url,
                    COALESCE(
                        CASE
                            WHEN al.tidal_id IS NOT NULL THEN 'tidal'
                            WHEN al.spotify_id IS NOT NULL THEN 'spotify'
                            WHEN al.qobuz_id IS NOT NULL THEN 'qobuz'
                            ELSE (
                                SELECT s.name FROM tracks t
                                JOIN track_sources ts ON ts.track_id = t.id
                                JOIN services s ON ts.service_id = s.id
                                WHERE t.album_id = al.id LIMIT 1
                            )
                        END,
                        'local'
                    ) as service,
                    al.total_tracks,
                    al.release_date,
                    al.favorite_at as favorited_at
                FROM albums al
                WHERE al.is_favorite = 1
                  AND (?1 = 1 OR al.is_stub = 0)
                ORDER BY al.favorite_at DESC NULLS LAST, al.id DESC
                LIMIT ?2 OFFSET ?3
                "#
            )
            .bind(include_stubs_int)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
        Some(svc) if svc.eq_ignore_ascii_case("local") => {
            sqlx::query_as::<_, FavoriteAlbumItem>(
                r#"
                SELECT
                    al.id,
                    CAST(al.id AS TEXT) as service_album_id,
                    al.title,
                    COALESCE((SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = al.id AND aa.is_primary = 1 LIMIT 1), 'Unknown Artist') as artist,
                    al.upc,
                    al.cover_art_url,
                    'local' as service,
                    al.total_tracks,
                    al.release_date,
                    al.favorite_at as favorited_at
                FROM albums al
                WHERE al.is_favorite = 1
                  AND (?1 = 1 OR al.is_stub = 0)
                  AND (
                    (al.spotify_id IS NULL AND al.tidal_id IS NULL AND al.qobuz_id IS NULL)
                    OR NOT EXISTS (
                        SELECT 1 FROM tracks t
                        JOIN track_sources ts ON ts.track_id = t.id
                        JOIN services s ON ts.service_id = s.id
                        WHERE t.album_id = al.id AND LOWER(s.name) NOT IN ('local')
                    )
                  )
                ORDER BY al.favorite_at DESC NULLS LAST, al.id DESC
                LIMIT ?2 OFFSET ?3
                "#
            )
            .bind(include_stubs_int)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
        Some(svc) => {
            sqlx::query_as::<_, FavoriteAlbumItem>(
                r#"
                SELECT
                    al.id,
                    CAST(COALESCE(
                        CASE
                            WHEN LOWER(?1) = 'spotify' THEN al.spotify_id
                            WHEN LOWER(?1) = 'tidal' THEN al.tidal_id
                            WHEN LOWER(?1) = 'qobuz' THEN al.qobuz_id
                            ELSE NULL
                        END,
                        (
                            SELECT ts.service_track_id FROM tracks t
                            JOIN track_sources ts ON ts.track_id = t.id
                            JOIN services s ON ts.service_id = s.id
                            WHERE t.album_id = al.id AND LOWER(s.name) = LOWER(?1)
                            LIMIT 1
                        ),
                        CAST(al.id AS TEXT)
                    ) AS TEXT) as service_album_id,
                    al.title,
                    COALESCE((SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = al.id AND aa.is_primary = 1 LIMIT 1), 'Unknown Artist') as artist,
                    al.upc,
                    al.cover_art_url,
                    CAST(?1 AS TEXT) as service,
                    al.total_tracks,
                    al.release_date,
                    al.favorite_at as favorited_at
                FROM albums al
                WHERE al.is_favorite = 1
                  AND (?4 = 1 OR al.is_stub = 0)
                  AND (
                    (LOWER(?1) = 'spotify' AND al.spotify_id IS NOT NULL)
                    OR (LOWER(?1) = 'tidal' AND al.tidal_id IS NOT NULL)
                    OR (LOWER(?1) = 'qobuz' AND al.qobuz_id IS NOT NULL)
                    OR EXISTS (
                        SELECT 1 FROM tracks t
                        JOIN track_sources ts ON ts.track_id = t.id
                        JOIN services s ON ts.service_id = s.id
                        WHERE t.album_id = al.id AND LOWER(s.name) = LOWER(?1)
                    )
                  )
                ORDER BY al.favorite_at DESC NULLS LAST, al.id DESC
                LIMIT ?2 OFFSET ?3
                "#
            )
            .bind(svc)
            .bind(limit)
            .bind(offset)
            .bind(include_stubs_int)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
    };

    Ok(items)
}

/// Fetch favorite albums with multi-service support and pagination
#[tauri::command]
pub async fn get_favorites_albums(
    state: State<'_, AppState>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
    include_stubs: Option<bool>,
) -> Result<Vec<FavoriteAlbumItem>, String> {
    perform_get_favorites_albums_with_options(
        &state.db,
        service,
        offset,
        limit,
        include_stubs.unwrap_or(false),
    )
    .await
}

pub async fn perform_get_favorites_artists(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteArtistItem>, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!(
        "perform_get_favorites_artists called: service={:?}, offset={}, limit={}",
        service,
        offset,
        limit
    );

    let items = match service.as_deref() {
        None | Some("all") => {
            sqlx::query_as::<_, FavoriteArtistItem>(
                r#"
                SELECT
                    a.id,
                    COALESCE(a.tidal_id, a.spotify_id, a.qobuz_id, CAST(a.id AS TEXT)) as service_artist_id,
                    a.name,
                    NULL as image_url,
                    COALESCE(
                        CASE
                            WHEN a.tidal_id IS NOT NULL THEN 'tidal'
                            WHEN a.spotify_id IS NOT NULL THEN 'spotify'
                            WHEN a.qobuz_id IS NOT NULL THEN 'qobuz'
                            ELSE (
                                SELECT s.name FROM track_artists ta
                                JOIN track_sources ts ON ts.track_id = ta.track_id
                                JOIN services s ON ts.service_id = s.id
                                WHERE ta.artist_id = a.id LIMIT 1
                            )
                        END,
                        'local'
                    ) as service,
                    a.favorite_at as favorited_at
                FROM artists a
                WHERE a.is_favorite = 1
                ORDER BY a.favorite_at DESC NULLS LAST, a.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
        Some(svc) if svc.eq_ignore_ascii_case("local") => {
            sqlx::query_as::<_, FavoriteArtistItem>(
                r#"
                SELECT
                    a.id,
                    CAST(a.id AS TEXT) as service_artist_id,
                    a.name,
                    NULL as image_url,
                    'local' as service,
                    a.favorite_at as favorited_at
                FROM artists a
                WHERE a.is_favorite = 1
                  AND (
                    (a.spotify_id IS NULL AND a.tidal_id IS NULL AND a.qobuz_id IS NULL)
                    OR NOT EXISTS (
                        SELECT 1 FROM track_artists ta
                        JOIN track_sources ts ON ts.track_id = ta.track_id
                        JOIN services s ON ts.service_id = s.id
                        WHERE ta.artist_id = a.id AND LOWER(s.name) NOT IN ('local')
                    )
                  )
                ORDER BY a.favorite_at DESC NULLS LAST, a.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
        Some(svc) => {
            sqlx::query_as::<_, FavoriteArtistItem>(
                r#"
                SELECT
                    a.id,
                    CAST(COALESCE(
                        CASE
                            WHEN LOWER(?1) = 'spotify' THEN a.spotify_id
                            WHEN LOWER(?1) = 'tidal' THEN a.tidal_id
                            WHEN LOWER(?1) = 'qobuz' THEN a.qobuz_id
                            ELSE NULL
                        END,
                        CAST(a.id AS TEXT)
                    ) AS TEXT) as service_artist_id,
                    a.name,
                    NULL as image_url,
                    CAST(?1 AS TEXT) as service,
                    a.favorite_at as favorited_at
                FROM artists a
                WHERE a.is_favorite = 1
                  AND (
                    (LOWER(?1) = 'spotify' AND a.spotify_id IS NOT NULL)
                    OR (LOWER(?1) = 'tidal' AND a.tidal_id IS NOT NULL)
                    OR (LOWER(?1) = 'qobuz' AND a.qobuz_id IS NOT NULL)
                    OR EXISTS (
                        SELECT 1 FROM track_artists ta
                        JOIN track_sources ts ON ts.track_id = ta.track_id
                        JOIN services s ON ts.service_id = s.id
                        WHERE ta.artist_id = a.id AND LOWER(s.name) = LOWER(?1)
                    )
                    OR EXISTS (
                        SELECT 1 FROM album_artists aa
                        JOIN albums al ON al.id = aa.album_id
                        JOIN tracks t ON t.album_id = al.id
                        JOIN track_sources ts ON ts.track_id = t.id
                        JOIN services s ON ts.service_id = s.id
                        WHERE aa.artist_id = a.id AND LOWER(s.name) = LOWER(?1)
                    )
                  )
                ORDER BY a.favorite_at DESC NULLS LAST, a.id DESC
                LIMIT ?2 OFFSET ?3
                "#
            )
            .bind(svc)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
    };

    Ok(items)
}

/// Fetch favorite artists with multi-service support and pagination
#[tauri::command]
pub async fn get_favorites_artists(
    state: State<'_, AppState>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteArtistItem>, String> {
    perform_get_favorites_artists(&state.db, service, offset, limit).await
}

/// Toggle favorite status of an album (atomic via RETURNING with timestamp)
#[tauri::command]
pub async fn toggle_album_favorite(
    state: State<'_, AppState>,
    album_id: i64,
) -> Result<bool, String> {
    tracing::info!("toggle_album_favorite called: album_id={}", album_id);

    if album_id <= 0 {
        return Err(format!("Invalid album_id: {}", album_id));
    }

    let result: Option<(i32,)> = sqlx::query_as(
        "UPDATE albums \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite",
    )
    .bind(album_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to toggle album favorite: {}", e))?;

    let is_favorite = result
        .map(|(v,)| v != 0)
        .ok_or_else(|| format!("Album {} not found", album_id))?;

    tracing::info!("Album {} favorite toggled to {}", album_id, is_favorite);
    Ok(is_favorite)
}

/// Toggle favorite status of an artist (atomic via RETURNING with timestamp)
#[tauri::command]
pub async fn toggle_artist_favorite(
    state: State<'_, AppState>,
    artist_id: i64,
) -> Result<bool, String> {
    tracing::info!("toggle_artist_favorite called: artist_id={}", artist_id);

    if artist_id <= 0 {
        return Err(format!("Invalid artist_id: {}", artist_id));
    }

    let result: Option<(i32,)> = sqlx::query_as(
        "UPDATE artists \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite",
    )
    .bind(artist_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to toggle artist favorite: {}", e))?;

    let is_favorite = result
        .map(|(v,)| v != 0)
        .ok_or_else(|| format!("Artist {} not found", artist_id))?;

    tracing::info!("Artist {} favorite toggled to {}", artist_id, is_favorite);
    Ok(is_favorite)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushFavoriteResponse {
    pub service: String,
    pub item_type: String,
    pub service_item_id: String,
    pub is_favorite: bool,
    pub status: String,
    pub message: String,
}

/// Push a favorite modification (add or remove) to a streaming service (Tidal,
/// Qobuz, Spotify, Deezer, SoundCloud, Apple Music)
#[tauri::command]
pub async fn push_favorite_to_service(
    state: State<'_, AppState>,
    service: String,
    item_type: String,
    service_item_id: String,
    is_favorite: bool,
) -> Result<PushFavoriteResponse, String> {
    perform_push_favorite_to_service(
        &state.db,
        &service,
        &item_type,
        &service_item_id,
        is_favorite,
    )
    .await
}

/// Provider half of [`push_favorite_to_service`]: propagate the change to the
/// remote service, then mirror it into the local `favorites` table.
///
/// Split from the command so each per-service arm is reachable without a Tauri
/// `State`; `db` is all the state these arms need.
pub async fn perform_push_favorite_to_service(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: &str,
    item_type: &str,
    service_item_id: &str,
    is_favorite: bool,
) -> Result<PushFavoriteResponse, String> {
    let service_lower = service.to_lowercase();
    let item_type_lower = item_type.to_lowercase();
    let service_item_id = service_item_id.to_string();

    tracing::info!(
        "push_favorite_to_service: service={}, type={}, id={}, is_fav={}",
        service_lower,
        item_type_lower,
        service_item_id,
        is_favorite
    );

    let (account_id, creds) = load_service_credentials(db, &service_lower).await?;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(&service_lower)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Service {} not registered: {}", service_lower, e))?;

    match service_lower.as_str() {
        "tidal" => {
            let access_token = creds["access_token"]
                .as_str()
                .ok_or("Missing access token for Tidal")?;
            let user_id = creds["user_id"]
                .as_str()
                .or_else(|| creds["user"]["userId"].as_str())
                .unwrap_or("0");
            let country = creds["country_code"]
                .as_str()
                .or_else(|| creds["user"]["countryCode"].as_str())
                .unwrap_or("US");

            let client = crate::services::TidalClient::new(access_token.to_string())
                .with_user(user_id.to_string(), country.to_string());

            let id_num = service_item_id
                .parse::<i64>()
                .map_err(|_| format!("Invalid Tidal ID format: {}", service_item_id))?;

            match item_type_lower.as_str() {
                "track" => {
                    if is_favorite {
                        client.add_favorite_track(id_num).await?;
                    } else {
                        client.remove_favorite_track(id_num).await?;
                    }
                }
                "album" => {
                    if is_favorite {
                        client.add_favorite_album(id_num).await?;
                    } else {
                        client.remove_favorite_album(id_num).await?;
                    }
                }
                "artist" => {
                    if is_favorite {
                        client.add_favorite_artist(id_num).await?;
                    } else {
                        client.remove_favorite_artist(id_num).await?;
                    }
                }
                _ => return Err(format!("Unsupported item_type: {}", item_type)),
            }
        }
        "qobuz" => {
            let app_id = std::env::var("QOBUZ_APP_ID")
                .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
            let app_secret = std::env::var("QOBUZ_APP_SECRET")
                .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());
            let user_auth_token = creds["user_auth_token"]
                .as_str()
                .or_else(|| creds["auth_token"].as_str())
                .or_else(|| creds["access_token"].as_str())
                .ok_or("Missing user auth token for Qobuz")?;

            let client = crate::services::QobuzClient::new_with_token(
                app_id,
                app_secret,
                user_auth_token.to_string(),
            );

            match item_type_lower.as_str() {
                "track" => {
                    let id_num = service_item_id
                        .parse::<i64>()
                        .map_err(|_| format!("Invalid Qobuz track ID: {}", service_item_id))?;
                    if is_favorite {
                        client.add_favorite_track(id_num).await?;
                    } else {
                        client.remove_favorite_track(id_num).await?;
                    }
                }
                "album" => {
                    if is_favorite {
                        client.add_favorite_album(&service_item_id).await?;
                    } else {
                        client.remove_favorite_album(&service_item_id).await?;
                    }
                }
                "artist" => {
                    let id_num = service_item_id
                        .parse::<i64>()
                        .map_err(|_| format!("Invalid Qobuz artist ID: {}", service_item_id))?;
                    if is_favorite {
                        client.add_favorite_artist(id_num).await?;
                    } else {
                        client.remove_favorite_artist(id_num).await?;
                    }
                }
                _ => return Err(format!("Unsupported item_type: {}", item_type)),
            }
        }
        "spotify" => {
            let access_token = get_or_refresh_spotify_token(db, account_id, &creds).await?;
            let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
            let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
            let client =
                crate::services::SpotifyClient::new(access_token, refresh_token, expires_at);

            match item_type_lower.as_str() {
                "track" => {
                    if is_favorite {
                        client.save_track(&service_item_id).await?;
                    } else {
                        client.remove_saved_track(&service_item_id).await?;
                    }
                }
                "album" => {
                    if is_favorite {
                        client.save_album(&service_item_id).await?;
                    } else {
                        client.remove_saved_album(&service_item_id).await?;
                    }
                }
                "artist" => {
                    if is_favorite {
                        client.follow_artist(&service_item_id).await?;
                    } else {
                        client.unfollow_artist(&service_item_id).await?;
                    }
                }
                _ => return Err(format!("Unsupported item_type: {}", item_type)),
            }
        }
        "deezer" => {
            // Reject what Deezer cannot express before opening a session.
            if item_type_lower != "track" {
                return Err(format!(
                    "Deezer only propagates track favorites (got '{}')",
                    item_type_lower
                ));
            }

            let arl = creds["arl"]
                .as_str()
                .or_else(|| creds["access_token"].as_str())
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .ok_or("Missing Deezer ARL in stored credentials")?;

            let mut client = crate::services::DeezerClient::new(arl.to_string());
            // The gw-light write endpoints need the api_token that `init()`
            // negotiates; without it there is nothing to send.
            client.init().await.map_err(|e| {
                format!(
                    "RequiresAuth: Deezer session could not be initialised: {}",
                    e
                )
            })?;

            if is_favorite {
                client.add_to_favorites(&service_item_id).await?;
            } else {
                client.remove_from_favorites(&service_item_id).await?;
            }
        }
        "soundcloud" => {
            let oauth_token = creds["oauth_token"]
                .as_str()
                .or_else(|| creds["access_token"].as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("Missing SoundCloud OAuth token in stored credentials")?;
            let user_id = creds["user_id"]
                .as_i64()
                .or_else(|| creds["user"]["id"].as_i64())
                .ok_or("Missing SoundCloud user_id in stored credentials")?;

            let client = crate::services::SoundCloudClient::new(oauth_token.to_string())
                .with_user_id(user_id);

            match item_type_lower.as_str() {
                "track" => {
                    if is_favorite {
                        client.add_to_favorites(&service_item_id).await?;
                    } else {
                        client.remove_from_favorites(&service_item_id).await?;
                    }
                }
                other => {
                    return Err(format!(
                        "SoundCloud only exposes track likes (got '{}')",
                        other
                    ))
                }
            }
        }
        "apple_music" => {
            let developer_token = creds["developer_token"]
                .as_str()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("Missing Apple Music developer_token in stored credentials")?;
            let music_user_token = creds["music_user_token"]
                .as_str()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("Missing Apple Music music_user_token in stored credentials")?;

            let client = crate::services::AppleMusicClient::from_credentials(
                developer_token.to_string(),
                music_user_token.to_string(),
                &creds,
            );

            match item_type_lower.as_str() {
                "track" => {
                    if is_favorite {
                        client.add_to_favorites(&service_item_id).await?;
                    } else {
                        client.remove_from_favorites(&service_item_id).await?;
                    }
                }
                "album" => {
                    if is_favorite {
                        client.add_album_to_library(&service_item_id).await?;
                    } else {
                        client.remove_album_from_library(&service_item_id).await?;
                    }
                }
                other => {
                    return Err(format!(
                        "Apple Music library holds no artist favorites (got '{}')",
                        other
                    ))
                }
            }
        }
        _ => return Err(format!("Unsupported service for push: {}", service)),
    }

    perform_push_favorite_sync(
        db,
        account_id,
        service_id,
        &service_lower,
        &item_type_lower,
        &service_item_id,
        is_favorite,
    )
    .await?;

    Ok(PushFavoriteResponse {
        service: service_lower,
        item_type: item_type_lower,
        service_item_id,
        is_favorite,
        status: "success".to_string(),
        message: format!("Successfully propagated favorite state to {}", service),
    })
}

pub async fn perform_push_favorite_sync(
    db: &sqlx::Pool<sqlx::Sqlite>,
    account_id: i64,
    service_id: i64,
    service_name: &str,
    item_type: &str,
    service_item_id: &str,
    is_favorite: bool,
) -> Result<(), String> {
    let service_lower = service_name.to_lowercase();
    let item_type_lower = item_type.to_lowercase();

    // Atomic SQLite synchronization
    if is_favorite {
        let _ = sqlx::query(
            r#"
            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, favorited_at)
            VALUES (?, ?, ?, ?, 'Favorite Item', datetime('now'))
            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                favorited_at = datetime('now')
            "#
        )
        .bind(account_id)
        .bind(service_id)
        .bind(&item_type_lower)
        .bind(service_item_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to insert into favorites: {}", e))?;
    } else {
        let _ = sqlx::query(
            "DELETE FROM favorites WHERE account_id = ? AND item_type = ? AND service_item_id = ?",
        )
        .bind(account_id)
        .bind(&item_type_lower)
        .bind(service_item_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to delete from favorites: {}", e))?;
    }

    match item_type_lower.as_str() {
        "track" => {
            let mut track_id_opt: Option<i64> = sqlx::query_scalar(
                "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = ?",
            )
            .bind(service_id)
            .bind(service_item_id)
            .fetch_optional(db)
            .await
            .unwrap_or(None);

            if track_id_opt.is_none() {
                if let Ok(num_id) = service_item_id.parse::<i64>() {
                    let exists: bool =
                        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM tracks WHERE id = ?")
                            .bind(num_id)
                            .fetch_one(db)
                            .await
                            .unwrap_or(false);
                    if exists {
                        track_id_opt = Some(num_id);
                    }
                }
            }

            if let Some(track_id) = track_id_opt {
                if is_favorite {
                    let _ = sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
                        .bind(track_id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to update track favorite: {}", e))?;

                    let _ = sqlx::query(
                        r#"
                        INSERT INTO library_entries (account_id, track_id, added_at, is_liked)
                        VALUES (?, ?, datetime('now'), 1)
                        ON CONFLICT(account_id, track_id) DO UPDATE SET is_liked = 1
                        "#,
                    )
                    .bind(account_id)
                    .bind(track_id)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to update library_entries: {}", e))?;

                    let _ = crate::db::write_or_log("perform_push_favorite_sync", sqlx::query(
                        "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)"
                    )
                    .bind(track_id)
                    .bind(service_id)
                    .bind(service_item_id)
                    .execute(db)).await;
                } else {
                    let _ = sqlx::query("UPDATE library_entries SET is_liked = 0 WHERE account_id = ? AND track_id = ?")
                        .bind(account_id)
                        .bind(track_id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to update library_entries: {}", e))?;

                    let remaining_likes: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM library_entries WHERE track_id = ? AND is_liked = 1",
                    )
                    .bind(track_id)
                    .fetch_one(db)
                    .await
                    .unwrap_or(0);

                    let remaining_favs: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM favorites f JOIN track_sources ts ON ts.service_id = f.service_id AND ts.service_track_id = f.service_item_id WHERE ts.track_id = ? AND f.item_type = 'track'"
                    )
                    .bind(track_id)
                    .fetch_one(db)
                    .await
                    .unwrap_or(0);

                    if remaining_likes == 0 && remaining_favs == 0 {
                        let _ = sqlx::query(
                            "UPDATE tracks SET is_favorite = 0, favorite_at = NULL WHERE id = ?",
                        )
                        .bind(track_id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to clear track favorite: {}", e))?;
                    }
                }
            }
        }
        "album" => {
            let mut album_id_opt: Option<i64> = match service_lower.as_str() {
                "spotify" => sqlx::query_scalar("SELECT id FROM albums WHERE spotify_id = ?")
                    .bind(service_item_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None),
                "tidal" => sqlx::query_scalar("SELECT id FROM albums WHERE tidal_id = ?")
                    .bind(service_item_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None),
                "qobuz" => sqlx::query_scalar("SELECT id FROM albums WHERE qobuz_id = ?")
                    .bind(service_item_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None),
                _ => None,
            };

            if album_id_opt.is_none() {
                if let Ok(num_id) = service_item_id.parse::<i64>() {
                    let exists: bool =
                        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM albums WHERE id = ?")
                            .bind(num_id)
                            .fetch_one(db)
                            .await
                            .unwrap_or(false);
                    if exists {
                        album_id_opt = Some(num_id);
                    }
                }
            }

            if let Some(album_id) = album_id_opt {
                if is_favorite {
                    let _ = sqlx::query(
                        "UPDATE albums SET is_favorite = 1, favorite_at = datetime('now'), \
                         is_stub = CASE WHEN (SELECT COUNT(*) FROM tracks WHERE album_id = albums.id) = 0 THEN 1 ELSE 0 END \
                         WHERE id = ?"
                    )
                    .bind(album_id)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to update album favorite: {}", e))?;
                } else {
                    let _ = sqlx::query(
                        "UPDATE albums SET is_favorite = 0, favorite_at = NULL WHERE id = ?",
                    )
                    .bind(album_id)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to clear album favorite: {}", e))?;
                }
            }
        }
        "artist" => {
            let mut artist_id_opt: Option<i64> = match service_lower.as_str() {
                "spotify" => sqlx::query_scalar("SELECT id FROM artists WHERE spotify_id = ?")
                    .bind(service_item_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None),
                "tidal" => sqlx::query_scalar("SELECT id FROM artists WHERE tidal_id = ?")
                    .bind(service_item_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None),
                "qobuz" => sqlx::query_scalar("SELECT id FROM artists WHERE qobuz_id = ?")
                    .bind(service_item_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None),
                _ => None,
            };

            if artist_id_opt.is_none() {
                if let Ok(num_id) = service_item_id.parse::<i64>() {
                    let exists: bool =
                        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM artists WHERE id = ?")
                            .bind(num_id)
                            .fetch_one(db)
                            .await
                            .unwrap_or(false);
                    if exists {
                        artist_id_opt = Some(num_id);
                    }
                }
            }

            if artist_id_opt.is_none() {
                let clean_item_id = service_item_id.trim();
                artist_id_opt = sqlx::query_scalar(
                    "SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1",
                )
                .bind(clean_item_id)
                .fetch_optional(db)
                .await
                .unwrap_or(None);
            }

            if let Some(artist_id) = artist_id_opt {
                if is_favorite {
                    let _ = sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
                        .bind(artist_id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to update artist favorite: {}", e))?;
                } else {
                    let _ = sqlx::query(
                        "UPDATE artists SET is_favorite = 0, favorite_at = NULL WHERE id = ?",
                    )
                    .bind(artist_id)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to clear artist favorite: {}", e))?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// F2-4: persistencia de un favorito-track a través del MOTOR UNIFICADO
/// (identidad canónica A→B→C, enriquecimiento, transacciones con retry).
/// Reemplaza la duplicación de `upsert_canonical_favorite_track` que cada
/// servicio repetía con su propia variante de dedup.
#[allow(clippy::too_many_arguments)]
async fn persist_favorite_track_via_engine(
    db: &sqlx::Pool<sqlx::Sqlite>,
    engine: &crate::services::enrichment::EnrichmentEngine,
    service_name: &str,
    service_id: i64,
    account_id: i64,
    service_track_id: &str,
    title: &str,
    artist_name: &str,
    album_name: Option<String>,
    isrc: Option<String>,
    duration_ms: Option<i64>,
) -> Result<crate::services::enrichment::SyncTrackResult, String> {
    let input = crate::services::enrichment::SyncTrackInput {
        origin_meta: crate::services::enrichment::OriginTrackMetadata {
            title: Some(title.to_string()),
            artist: Some(artist_name.to_string()),
            album: album_name.clone(),
            album_artist: album_name.as_ref().map(|_| artist_name.to_string()),
            isrc,
            source_name: service_name.to_string(),
            ..Default::default()
        },
        service_track_id: service_track_id.to_string(),
        service_name: service_name.to_string(),
        service_id,
        account_id,
        is_favorite: true,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        duration_ms,
        audio_quality: Some(
            syncify_core_domain::quality::classify_audio_tier(
                Some(16),
                Some(44100),
                None,
                Some("FLAC"),
            )
            .as_str()
            .to_string(),
        ),
        query_musicbrainz: false,
        ..Default::default()
    };
    enrich_persist_with_locked_retry(engine, db, input).await
}

pub async fn upsert_canonical_favorite_album(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service_id: i64,
    service_album_id: &str,
    title: &str,
    artist_name: &str,
    upc: Option<&str>,
    image_url: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let clean_artist = syncify_core_domain::metadata::sanitize_artist_name(artist_name);
    let target_artist = clean_artist.trim();
    let target_artist = if target_artist.is_empty() {
        artist_name.trim()
    } else {
        target_artist
    };

    let artist_id: i64 = if let Ok(Some((aid,))) = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1",
    )
    .bind(target_artist)
    .fetch_optional(db)
    .await
    {
        aid
    } else {
        sqlx::query_scalar(
            "INSERT INTO artists (name) VALUES (?)
             ON CONFLICT (name COLLATE NOCASE) DO UPDATE SET id = id
             RETURNING id",
        )
        .bind(target_artist)
        .fetch_one(db)
        .await?
    };

    let album_id: i64 = if let Some(upc_val) = upc.filter(|s| !s.trim().is_empty()) {
        let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE upc = ?")
            .bind(upc_val)
            .fetch_optional(db)
            .await?;

        if let Some(aid) = existing {
            let has_tracks: bool =
                sqlx::query_scalar("SELECT COUNT(*) > 0 FROM tracks WHERE album_id = ?")
                    .bind(aid)
                    .fetch_one(db)
                    .await
                    .unwrap_or(false);
            let is_stub = if has_tracks { 0 } else { 1 };
            sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')), cover_art_url = COALESCE(cover_art_url, ?), is_stub = ? WHERE id = ?")
                .bind(image_url)
                .bind(is_stub)
                .bind(aid)
                .execute(db)
                .await?;
            aid
        } else {
            sqlx::query_scalar(
                "INSERT INTO albums (title, upc, cover_art_url, is_favorite, favorite_at, is_stub) VALUES (?, ?, ?, 1, datetime('now'), 1) RETURNING id"
            )
            .bind(title)
            .bind(upc_val)
            .bind(image_url)
            .fetch_one(db)
            .await?
        }
    } else {
        let existing =
            sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ? LIMIT 1")
                .bind(title)
                .fetch_optional(db)
                .await?;

        if let Some(aid) = existing {
            let has_tracks: bool =
                sqlx::query_scalar("SELECT COUNT(*) > 0 FROM tracks WHERE album_id = ?")
                    .bind(aid)
                    .fetch_one(db)
                    .await
                    .unwrap_or(false);
            let is_stub = if has_tracks { 0 } else { 1 };
            sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')), cover_art_url = COALESCE(cover_art_url, ?), is_stub = ? WHERE id = ?")
                .bind(image_url)
                .bind(is_stub)
                .bind(aid)
                .execute(db)
                .await?;
            aid
        } else {
            sqlx::query_scalar(
                "INSERT INTO albums (title, cover_art_url, is_favorite, favorite_at, is_stub) VALUES (?, ?, 1, datetime('now'), 1) RETURNING id"
            )
            .bind(title)
            .bind(image_url)
            .fetch_one(db)
            .await?
        }
    };

    let _ = crate::db::write_or_log("upsert_canonical_favorite_album", sqlx::query(
        "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)",
    )
    .bind(album_id)
    .bind(artist_id)
    .execute(db)).await;

    let service_name: Option<String> = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
        .bind(service_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    if let Some(ref svc) = service_name {
        match svc.to_lowercase().as_str() {
            "tidal" => {
                let _ = crate::db::write_or_log("upsert_canonical_favorite_album", sqlx::query("UPDATE albums SET tidal_id = ? WHERE id = ? AND (tidal_id IS NULL OR tidal_id = '')")
                    .bind(service_album_id)
                    .bind(album_id)
                    .execute(db)).await;
            }
            "spotify" => {
                let _ = crate::db::write_or_log("upsert_canonical_favorite_album", sqlx::query("UPDATE albums SET spotify_id = ? WHERE id = ? AND (spotify_id IS NULL OR spotify_id = '')")
                    .bind(service_album_id)
                    .bind(album_id)
                    .execute(db)).await;
            }
            "qobuz" => {
                let _ = crate::db::write_or_log("upsert_canonical_favorite_album", sqlx::query("UPDATE albums SET qobuz_id = ? WHERE id = ? AND (qobuz_id IS NULL OR qobuz_id = '')")
                    .bind(service_album_id)
                    .bind(album_id)
                    .execute(db)).await;
            }
            _ => {}
        }
    }

    Ok(album_id)
}

pub async fn upsert_canonical_favorite_artist(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service_id: i64,
    service_artist_id: &str,
    name: &str,
) -> Result<i64, sqlx::Error> {
    let clean_name = syncify_core_domain::metadata::sanitize_artist_name(name);
    let target_name = clean_name.trim();
    let target_name = if target_name.is_empty() {
        name.trim()
    } else {
        target_name
    };

    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
            .bind(target_name)
            .fetch_optional(db)
            .await?;

    let artist_id: i64 = if let Some((aid,)) = existing {
        sqlx::query(
            "UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')) WHERE id = ?",
        )
        .bind(aid)
        .execute(db)
        .await?;
        aid
    } else {
        sqlx::query_scalar(
            "INSERT INTO artists (name, is_favorite, favorite_at) VALUES (?, 1, datetime('now'))
             ON CONFLICT (name COLLATE NOCASE) DO UPDATE SET
                 is_favorite = 1,
                 favorite_at = COALESCE(artists.favorite_at, datetime('now'))
             RETURNING id",
        )
        .bind(target_name)
        .fetch_one(db)
        .await?
    };

    let service_name: Option<String> = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
        .bind(service_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    if let Some(ref svc) = service_name {
        match svc.to_lowercase().as_str() {
            "tidal" => {
                let _ = crate::db::write_or_log("upsert_canonical_favorite_artist", sqlx::query("UPDATE artists SET tidal_id = ? WHERE id = ? AND (tidal_id IS NULL OR tidal_id = '')")
                    .bind(service_artist_id)
                    .bind(artist_id)
                    .execute(db)).await;
            }
            "spotify" => {
                let _ = crate::db::write_or_log("upsert_canonical_favorite_artist", sqlx::query("UPDATE artists SET spotify_id = ? WHERE id = ? AND (spotify_id IS NULL OR spotify_id = '')")
                    .bind(service_artist_id)
                    .bind(artist_id)
                    .execute(db)).await;
            }
            "qobuz" => {
                let _ = crate::db::write_or_log("upsert_canonical_favorite_artist", sqlx::query("UPDATE artists SET qobuz_id = ? WHERE id = ? AND (qobuz_id IS NULL OR qobuz_id = '')")
                    .bind(service_artist_id)
                    .bind(artist_id)
                    .execute(db)).await;
            }
            _ => {}
        }
    }

    Ok(artist_id)
}

/// Page size used by the Apple Music favorites sync (its library endpoints cap
/// a page at 100 items).
const APPLE_MUSIC_FAV_PAGE: i32 = 100;

/// Synchronize favorites from a streaming service (Tidal, Qobuz, Spotify,
/// Deezer, SoundCloud, Apple Music) into SQLite
///
/// Multi-cuenta: `account_id` elige QUÉ cuenta se sincroniza. `None` conserva el
/// contrato histórico ("la cuenta activa"), que es lo que usan los llamadores
/// que no tienen contexto de cuenta.
#[tauri::command]
pub async fn sync_favorites(
    state: State<'_, AppState>,
    window: tauri::Window,
    service: String,
    fav_type: Option<String>,
    account_id: Option<i64>,
) -> Result<FavoritesSyncResult, String> {
    let result =
        perform_sync_favorites(&state.db, &service, fav_type.as_deref(), account_id).await?;

    let _ = window.emit(
        "syncify:favorites_sync_completed",
        serde_json::json!({
            "service": result.service,
            "item_type": result.item_type,
            "total": result.total_found,
            "imported": result.imported
        }),
    );

    Ok(result)
}

/// Provider half of [`sync_favorites`]: resolve the account, walk the liked
/// collections the service actually exposes and persist them.
///
/// Split from the command so each per-service arm is reachable without a Tauri
/// window; `db` is all the state these arms need.
///
/// Multi-cuenta: `account_id` = Some(id) resuelve las credenciales de ESA cuenta
/// (validando que pertenezca al servicio); None = la cuenta activa, como antes.
/// Sin este parámetro el `accountId` que pase la UI se ignoraría y el sync de
/// favoritos seguiría anclado a la activa.
pub async fn perform_sync_favorites(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: &str,
    fav_type: Option<&str>,
    account_id: Option<i64>,
) -> Result<FavoritesSyncResult, String> {
    let service_lower = service.to_lowercase();
    let type_filter = fav_type.unwrap_or("all").to_lowercase();

    tracing::info!(
        "sync_favorites called for service '{}', type '{}', account {:?}",
        service_lower,
        type_filter,
        account_id
    );

    let (account_id, creds) = match account_id {
        Some(aid) => load_service_credentials_for_account(db, &service_lower, aid).await?,
        None => load_service_credentials(db, &service_lower).await?,
    };
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(&service_lower)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Service {} not registered: {}", service_lower, e))?;

    let mut imported = 0i64;
    let mut total_found = 0i64;

    // F2-4: un solo motor por invocación para todos los servicios.
    let enrichment_engine = crate::services::enrichment::EnrichmentEngine::with_db(db.clone());

    match service_lower.as_str() {
        "tidal" => {
            let access_token = creds["access_token"]
                .as_str()
                .ok_or("Missing access token for Tidal")?;
            let user_id = creds["user_id"]
                .as_str()
                .or_else(|| creds["user"]["userId"].as_str())
                .unwrap_or("0");
            let country = creds["country_code"]
                .as_str()
                .or_else(|| creds["user"]["countryCode"].as_str())
                .unwrap_or("US");

            let client = crate::services::TidalClient::new(access_token.to_string())
                .with_user(user_id.to_string(), country.to_string());

            if type_filter == "all" || type_filter == "tracks" {
                // F2-4: paginación completa (antes solo la página 0).
                let mut offset: i32 = 0;
                loop {
                    let page = client.get_favorites(offset, 100).await?;
                    total_found += page.total as i64;
                    let items_len = page.items.len();
                    for item in page.items {
                        let track = item.item;
                        let track_id_str = track.id.to_string();
                        let title = track.title.clone();
                        let duration_ms = (track.duration > 0).then(|| track.duration * 1000);
                        if track.duration <= 0
                            && crate::services::import_pagination::is_placeholder_title(&title)
                        {
                            tracing::warn!(
                                "Skipping ghost/placeholder track '{}' ({})",
                                title,
                                track_id_str
                            );
                            continue;
                        }
                        let artist_name = track
                            .artist
                            .as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_name = track.album.as_ref().map(|a| a.title.clone());
                        let isrc = track.isrc.clone();
                        let favorited_at: Option<String> = None;

                        let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, favorited_at)
                        VALUES (?, ?, 'track', ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            album_name = excluded.album_name,
                            isrc = COALESCE(excluded.isrc, favorites.isrc),
                            favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&track_id_str)
                    .bind(&title)
                    .bind(&artist_name)
                    .bind(&album_name)
                    .bind(&isrc)
                    .bind(&favorited_at)
                    .execute(db)
                    .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        // F2-4: identidad canónica vía EnrichmentEngine
                        let _ = persist_favorite_track_via_engine(
                            db,
                            &enrichment_engine,
                            "tidal",
                            service_id,
                            account_id,
                            &track_id_str,
                            &title,
                            &artist_name,
                            album_name.clone(),
                            isrc.clone(),
                            duration_ms,
                        )
                        .await;
                    }
                    match crate::services::import_pagination::next_offset(
                        offset,
                        items_len as i32,
                        100,
                        (page.total > 0).then_some(page.total as i64),
                    ) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }

            if type_filter == "all" || type_filter == "albums" {
                let page = client.get_favorite_albums(0, 100).await?;
                total_found += page.total as i64;
                for item in page.items {
                    let album = item.item;
                    let album_id_str = album.tidal_id.to_string();
                    let title = album.title.clone();
                    let artist_name = album
                        .artist
                        .as_ref()
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| "Unknown Artist".to_string());
                    let upc = album.upc.clone();
                    let image_url = album.cover_url();
                    let favorited_at: Option<String> = None;

                    let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, upc, image_url, favorited_at)
                        VALUES (?, ?, 'album', ?, ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            upc = COALESCE(excluded.upc, favorites.upc),
                            image_url = COALESCE(excluded.image_url, favorites.image_url),
                            favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&album_id_str)
                    .bind(&title)
                    .bind(&artist_name)
                    .bind(&title)
                    .bind(&upc)
                    .bind(&image_url)
                    .bind(&favorited_at)
                    .execute(db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            imported += 1;
                        }
                    }

                    // Canonical library album synchronization with UPC deduplication
                    let _ = upsert_canonical_favorite_album(
                        db,
                        service_id,
                        &album_id_str,
                        &title,
                        &artist_name,
                        upc.as_deref(),
                        image_url.as_deref(),
                    )
                    .await;
                }
            }

            if type_filter == "all" || type_filter == "artists" {
                let page = client.get_favorite_artists(0, 100).await?;
                total_found += page.total as i64;
                for item in page.items {
                    let artist = item.item;
                    let artist_id_str = artist.id.to_string();
                    let name = artist.name.clone();
                    let image_url: Option<String> = None;
                    let favorited_at: Option<String> = None;

                    let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, image_url, favorited_at)
                        VALUES (?, ?, 'artist', ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            image_url = COALESCE(excluded.image_url, favorites.image_url),
                            favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&artist_id_str)
                    .bind(&name)
                    .bind(&name)
                    .bind(&image_url)
                    .bind(&favorited_at)
                    .execute(db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            imported += 1;
                        }
                    }

                    // Canonical library artist synchronization
                    let _ = upsert_canonical_favorite_artist(db, service_id, &artist_id_str, &name)
                        .await;
                }
            }
        }
        "qobuz" => {
            let app_id = std::env::var("QOBUZ_APP_ID")
                .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
            let app_secret = std::env::var("QOBUZ_APP_SECRET")
                .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());
            let user_auth_token = creds["user_auth_token"]
                .as_str()
                .or_else(|| creds["auth_token"].as_str())
                .or_else(|| creds["access_token"].as_str())
                .ok_or("Missing user auth token for Qobuz")?;

            let client = crate::services::QobuzClient::new_with_token(
                app_id,
                app_secret,
                user_auth_token.to_string(),
            );

            if type_filter == "all" || type_filter == "tracks" {
                // F2-4: paginación completa (antes solo la página 0).
                let mut offset: i32 = 0;
                loop {
                    let page = client.get_favorites(offset, 100).await?;
                    total_found += page.tracks.total as i64;
                    let items_len_q = page.tracks.items.len();
                    for track in page.tracks.items {
                        let track_id_str = track.id.to_string();
                        let title = track.title.unwrap_or_else(|| "Unknown Track".to_string());
                        let duration_ms = (track.duration > 0).then(|| track.duration * 1000);
                        if track.duration <= 0
                            && crate::services::import_pagination::is_placeholder_title(&title)
                        {
                            tracing::warn!(
                                "Skipping ghost/placeholder track '{}' ({})",
                                title,
                                track_id_str
                            );
                            continue;
                        }
                        let artist_name = track
                            .performer
                            .as_ref()
                            .and_then(|a| a.name.clone())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_name = track.album.as_ref().and_then(|al| al.title.clone());
                        let isrc = track.isrc.clone();
                        let image_url = track.album.as_ref().and_then(|al| {
                            al.image
                                .as_ref()
                                .and_then(|img| img.large.clone().or_else(|| img.small.clone()))
                        });

                        let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, image_url)
                        VALUES (?, ?, 'track', ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            album_name = excluded.album_name,
                            isrc = COALESCE(excluded.isrc, favorites.isrc),
                            image_url = COALESCE(excluded.image_url, favorites.image_url)
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&track_id_str)
                    .bind(&title)
                    .bind(&artist_name)
                    .bind(&album_name)
                    .bind(&isrc)
                    .bind(&image_url)
                    .execute(db)
                    .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        // F2-4: identidad canónica vía EnrichmentEngine
                        let _ = persist_favorite_track_via_engine(
                            db,
                            &enrichment_engine,
                            "qobuz",
                            service_id,
                            account_id,
                            &track_id_str,
                            &title,
                            &artist_name,
                            album_name.clone(),
                            isrc.clone(),
                            duration_ms,
                        )
                        .await;
                    }
                    match crate::services::import_pagination::next_offset(
                        offset,
                        items_len_q as i32,
                        100,
                        (page.tracks.total > 0).then_some(page.tracks.total as i64),
                    ) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }

            if type_filter == "all" || type_filter == "albums" {
                let page = client.get_favorite_albums(0, 100).await?;
                total_found += page.albums.total as i64;
                for album in page.albums.items {
                    let album_id_str = album.id.clone();
                    let title = album.title.unwrap_or_else(|| "Unknown Album".to_string());
                    let artist_name = album
                        .artist
                        .as_ref()
                        .and_then(|a| a.name.clone())
                        .unwrap_or_else(|| "Unknown Artist".to_string());
                    let upc = album.upc.clone();
                    let image_url = album
                        .image
                        .as_ref()
                        .and_then(|img| img.large.clone().or_else(|| img.small.clone()));

                    let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, upc, image_url)
                        VALUES (?, ?, 'album', ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            upc = COALESCE(excluded.upc, favorites.upc),
                            image_url = COALESCE(excluded.image_url, favorites.image_url)
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&album_id_str)
                    .bind(&title)
                    .bind(&artist_name)
                    .bind(&title)
                    .bind(&upc)
                    .bind(&image_url)
                    .execute(db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            imported += 1;
                        }
                    }

                    // Canonical library album synchronization with UPC deduplication
                    let _ = upsert_canonical_favorite_album(
                        db,
                        service_id,
                        &album_id_str,
                        &title,
                        &artist_name,
                        upc.as_deref(),
                        image_url.as_deref(),
                    )
                    .await;
                }
            }

            if type_filter == "all" || type_filter == "artists" {
                let page = client.get_favorite_artists(0, 100).await?;
                total_found += page.artists.total as i64;
                for artist in page.artists.items {
                    let artist_id_str = artist.id.map(|n| n.to_string()).unwrap_or_default();
                    let name = artist.name.unwrap_or_else(|| "Unknown Artist".to_string());

                    let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name)
                        VALUES (?, ?, 'artist', ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&artist_id_str)
                    .bind(&name)
                    .bind(&name)
                    .execute(db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            imported += 1;
                        }
                    }

                    // Canonical library artist synchronization
                    let _ = upsert_canonical_favorite_artist(db, service_id, &artist_id_str, &name)
                        .await;
                }
            }
        }
        "spotify" => {
            let access_token = get_or_refresh_spotify_token(db, account_id, &creds).await?;
            let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
            let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
            let client =
                crate::services::SpotifyClient::new(access_token, refresh_token, expires_at);

            if type_filter == "all" || type_filter == "tracks" {
                // F2-4: paginación completa (antes solo la página 0).
                let mut offset: i32 = 0;
                loop {
                    let page = client.get_saved_tracks(offset, 50).await?;
                    total_found += page.total as i64;
                    let items_len_s = page.items.len();
                    for saved in page.items {
                        let track = saved.track;
                        let track_id_str = track.id.clone();
                        let title = track.name.clone();
                        let duration_ms = (track.duration_ms > 0).then_some(track.duration_ms);
                        if track.duration_ms <= 0
                            && crate::services::import_pagination::is_placeholder_title(&title)
                        {
                            tracing::warn!(
                                "Skipping ghost/placeholder track '{}' ({})",
                                title,
                                track_id_str
                            );
                            continue;
                        }
                        let artist_name = track
                            .artists
                            .first()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_name = track.album.as_ref().map(|al| al.name.clone());
                        let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());
                        let image_url = track
                            .album
                            .as_ref()
                            .and_then(|al| al.images.first().map(|i| i.url.clone()));
                        let favorited_at = saved.added_at;

                        let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, image_url, favorited_at)
                        VALUES (?, ?, 'track', ?, ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            album_name = excluded.album_name,
                            isrc = COALESCE(excluded.isrc, favorites.isrc),
                            image_url = COALESCE(excluded.image_url, favorites.image_url),
                            favorited_at = excluded.favorited_at
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&track_id_str)
                    .bind(&title)
                    .bind(&artist_name)
                    .bind(&album_name)
                    .bind(&isrc)
                    .bind(&image_url)
                    .bind(&favorited_at)
                    .execute(db)
                    .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        // F2-4: identidad canónica vía EnrichmentEngine
                        let _ = persist_favorite_track_via_engine(
                            db,
                            &enrichment_engine,
                            "spotify",
                            service_id,
                            account_id,
                            &track_id_str,
                            &title,
                            &artist_name,
                            album_name.clone(),
                            isrc.clone(),
                            duration_ms,
                        )
                        .await;
                    }
                    match crate::services::import_pagination::next_offset(
                        offset,
                        items_len_s as i32,
                        50,
                        (page.total > 0).then_some(page.total as i64),
                    ) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }

            if type_filter == "all" || type_filter == "albums" {
                let page = client.get_saved_albums(0, 50).await?;
                total_found += page.total as i64;
                for saved in page.items {
                    let album = saved.album;
                    let album_id_str = album.id.clone();
                    let title = album.name.clone();
                    let artist_name = "Various Artists".to_string();
                    let upc = album.external_ids.as_ref().and_then(|e| e.upc.clone());
                    let image_url = album.images.first().map(|i| i.url.clone());
                    let favorited_at = saved.added_at;

                    let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, upc, image_url, favorited_at)
                        VALUES (?, ?, 'album', ?, ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name,
                            upc = COALESCE(excluded.upc, favorites.upc),
                            image_url = COALESCE(excluded.image_url, favorites.image_url),
                            favorited_at = excluded.favorited_at
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&album_id_str)
                    .bind(&title)
                    .bind(&artist_name)
                    .bind(&title)
                    .bind(&upc)
                    .bind(&image_url)
                    .bind(&favorited_at)
                    .execute(db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            imported += 1;
                        }
                    }

                    // Canonical library album synchronization with UPC deduplication
                    let _ = upsert_canonical_favorite_album(
                        db,
                        service_id,
                        &album_id_str,
                        &title,
                        &artist_name,
                        upc.as_deref(),
                        image_url.as_deref(),
                    )
                    .await;
                }
            }

            if type_filter == "all" || type_filter == "artists" {
                let page = client.get_followed_artists(None, 50).await?;
                let count = page.artists.items.len() as i64;
                total_found += count;
                for artist in page.artists.items {
                    let artist_id_str = artist.id.clone();
                    let name = artist.name.clone();

                    let res = sqlx::query(
                        r#"
                        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name)
                        VALUES (?, ?, 'artist', ?, ?, ?)
                        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                            title = excluded.title,
                            artist_name = excluded.artist_name
                        "#
                    )
                    .bind(account_id)
                    .bind(service_id)
                    .bind(&artist_id_str)
                    .bind(&name)
                    .bind(&name)
                    .execute(db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 {
                            imported += 1;
                        }
                    }

                    // Canonical library artist synchronization
                    let _ = upsert_canonical_favorite_artist(db, service_id, &artist_id_str, &name)
                        .await;
                }
            }
        }
        "deezer" => {
            let arl = creds["arl"]
                .as_str()
                .or_else(|| creds["access_token"].as_str())
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .ok_or("Missing Deezer ARL in stored credentials")?;

            let mut client = crate::services::DeezerClient::new(arl.to_string());
            // `init()` resolves the user id and the api_token the write
            // endpoints need; the read path still works with a stored user_id
            // if the ARL has gone stale, so a failure here is not fatal.
            if let Err(e) = client.init().await {
                tracing::warn!("Deezer init failed during favorites sync: {}", e);
            }
            let user_id = client
                .user_id()
                .or_else(|| creds["user_id"].as_str().map(|s| s.to_string()))
                .ok_or("Deezer User ID not found. Please re-login.")?;

            const DEEZER_PAGE: i32 = 100;

            if type_filter == "all" || type_filter == "tracks" {
                let mut offset: i32 = 0;
                loop {
                    let (page, total) = client
                        .get_favorites_public(&user_id, offset, DEEZER_PAGE)
                        .await?;
                    // Deezer repeats the library total on every page: assign it
                    // rather than accumulating, so `total_found` stays the
                    // count the provider reports.
                    total_found = total as i64;
                    let items_len = page.len() as i32;

                    for track in page {
                        let track_id_str = track.id.clone();
                        let title = track.title.clone();
                        let artist_name = track
                            .artist_name
                            .clone()
                            .filter(|a| !a.trim().is_empty())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_name = track.album_title.clone();
                        let isrc = track.isrc.clone();
                        let duration_ms = track.duration.parse::<i64>().ok().filter(|d| *d > 0);

                        let favorited_at: Option<String> = None;

                        let res = sqlx::query(
                            r#"
                            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, favorited_at)
                            VALUES (?, ?, 'track', ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                                title = excluded.title,
                                artist_name = excluded.artist_name,
                                album_name = excluded.album_name,
                                isrc = COALESCE(excluded.isrc, favorites.isrc),
                                favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                            "#,
                        )
                        .bind(account_id)
                        .bind(service_id)
                        .bind(&track_id_str)
                        .bind(&title)
                        .bind(&artist_name)
                        .bind(&album_name)
                        .bind(&isrc)
                        .bind(&favorited_at)
                        .execute(db)
                        .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        let _ = persist_favorite_track_via_engine(
                            db,
                            &enrichment_engine,
                            "deezer",
                            service_id,
                            account_id,
                            &track_id_str,
                            &title,
                            &artist_name,
                            album_name,
                            isrc,
                            duration_ms.map(|s| s * 1000),
                        )
                        .await;
                    }

                    match crate::services::import_pagination::next_offset(
                        offset,
                        items_len,
                        DEEZER_PAGE,
                        Some(total as i64),
                    ) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }

            if type_filter == "all" || type_filter == "albums" {
                let mut index: i32 = 0;
                loop {
                    let (page, total) = client
                        .get_user_albums_public(&user_id, index, DEEZER_PAGE)
                        .await?;
                    total_found += total;
                    let items_len = page.len() as i32;
                    for album in page {
                        let album_id_str = album.id.clone();
                        let title = album.title.clone();
                        let artist_name = album
                            .artist_name
                            .clone()
                            .filter(|a| !a.trim().is_empty())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let image_url = album.cover.clone();
                        let favorited_at: Option<String> = None;

                        let res = sqlx::query(
                            r#"
                            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, image_url, favorited_at)
                            VALUES (?, ?, 'album', ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                                title = excluded.title,
                                artist_name = excluded.artist_name,
                                image_url = COALESCE(excluded.image_url, favorites.image_url),
                                favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                            "#,
                        )
                        .bind(account_id)
                        .bind(service_id)
                        .bind(&album_id_str)
                        .bind(&title)
                        .bind(&artist_name)
                        .bind(&title)
                        .bind(&image_url)
                        .bind(&favorited_at)
                        .execute(db)
                        .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        let _ = upsert_canonical_favorite_album(
                            db,
                            service_id,
                            &album_id_str,
                            &title,
                            &artist_name,
                            None,
                            image_url.as_deref(),
                        )
                        .await;
                    }

                    match crate::services::import_pagination::next_offset(
                        index,
                        items_len,
                        DEEZER_PAGE,
                        Some(total),
                    ) {
                        Some(next) => index = next,
                        None => break,
                    }
                }
            }

            if type_filter == "all" || type_filter == "artists" {
                let mut index: i32 = 0;
                loop {
                    let (page, total) = client
                        .get_user_artists_public(&user_id, index, DEEZER_PAGE)
                        .await?;
                    total_found += total;
                    let items_len = page.len() as i32;
                    for (artist_id_str, name) in page {
                        let image_url: Option<String> = None;
                        let favorited_at: Option<String> = None;

                        let res = sqlx::query(
                            r#"
                            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, image_url, favorited_at)
                            VALUES (?, ?, 'artist', ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                                title = excluded.title,
                                artist_name = excluded.artist_name,
                                image_url = COALESCE(excluded.image_url, favorites.image_url),
                                favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                            "#,
                        )
                        .bind(account_id)
                        .bind(service_id)
                        .bind(&artist_id_str)
                        .bind(&name)
                        .bind(&name)
                        .bind(&image_url)
                        .bind(&favorited_at)
                        .execute(db)
                        .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        let _ =
                            upsert_canonical_favorite_artist(db, service_id, &artist_id_str, &name)
                                .await;
                    }

                    match crate::services::import_pagination::next_offset(
                        index,
                        items_len,
                        DEEZER_PAGE,
                        Some(total),
                    ) {
                        Some(next) => index = next,
                        None => break,
                    }
                }
            }
        }
        "soundcloud" => {
            let oauth_token = creds["oauth_token"]
                .as_str()
                .or_else(|| creds["access_token"].as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("Missing SoundCloud OAuth token in stored credentials")?;
            let user_id = creds["user_id"]
                .as_i64()
                .or_else(|| creds["user"]["id"].as_i64())
                .ok_or("Missing SoundCloud user_id in stored credentials")?;

            let client = crate::services::SoundCloudClient::new(oauth_token.to_string())
                .with_user_id(user_id);

            if type_filter == "all" || type_filter == "tracks" {
                // SoundCloud exposes exactly one liked collection (tracks), so
                // there is nothing to fetch for the albums/artists filters.
                let mut next: Option<String> = None;
                loop {
                    let page = client.get_likes(next.as_deref()).await?;
                    let items_len = page.collection.len() as i32;
                    total_found += items_len as i64;
                    let has_next = page.next_href.is_some();

                    for like in page.collection {
                        let track_id_str = match like.track.as_ref() {
                            Some(t) => t.id.to_string(),
                            None => continue,
                        };
                        let track = like.track.expect("checked above");
                        let title = track.title.clone();
                        let duration_ms = (track.duration > 0).then_some(track.duration);
                        if track.duration <= 0
                            && crate::services::import_pagination::is_placeholder_title(&title)
                        {
                            tracing::warn!(
                                "Skipping ghost/placeholder SoundCloud track '{}' ({})",
                                title,
                                track_id_str
                            );
                            continue;
                        }
                        // Mismo criterio de atribución que el import: el sello
                        // cuando existe, la cuenta que subió el audio si no.
                        let artist_name = track
                            .attributed_artist()
                            .map(str::trim)
                            .filter(|a| !a.is_empty())
                            .unwrap_or("Unknown Artist")
                            .to_string();
                        let album_name = track.album_title().map(|a| a.to_string());
                        let isrc = track.isrc().map(|i| i.to_string());
                        let image_url = track.cover_art_url();
                        let favorited_at = like.created_at.clone();

                        let res = sqlx::query(
                            r#"
                            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, image_url, favorited_at)
                            VALUES (?, ?, 'track', ?, ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                                title = excluded.title,
                                artist_name = excluded.artist_name,
                                album_name = excluded.album_name,
                                isrc = COALESCE(excluded.isrc, favorites.isrc),
                                image_url = COALESCE(excluded.image_url, favorites.image_url),
                                favorited_at = excluded.favorited_at
                            "#,
                        )
                        .bind(account_id)
                        .bind(service_id)
                        .bind(&track_id_str)
                        .bind(&title)
                        .bind(&artist_name)
                        .bind(&album_name)
                        .bind(&isrc)
                        .bind(&image_url)
                        .bind(&favorited_at)
                        .execute(db)
                        .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        let _ = persist_favorite_track_via_engine(
                            db,
                            &enrichment_engine,
                            "soundcloud",
                            service_id,
                            account_id,
                            &track_id_str,
                            &title,
                            &artist_name,
                            album_name,
                            isrc,
                            duration_ms,
                        )
                        .await;
                    }

                    if !has_next {
                        break;
                    }
                    next = page.next_href;
                }
            } else {
                tracing::warn!(
                    "SoundCloud only exposes liked tracks: ignoring fav_type '{}'",
                    type_filter
                );
            }
        }
        "apple_music" => {
            let developer_token = creds["developer_token"]
                .as_str()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("Missing Apple Music developer_token in stored credentials")?;
            let music_user_token = creds["music_user_token"]
                .as_str()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or("Missing Apple Music music_user_token in stored credentials")?;

            let client = crate::services::AppleMusicClient::from_credentials(
                developer_token.to_string(),
                music_user_token.to_string(),
                &creds,
            );

            if type_filter == "all" || type_filter == "tracks" {
                let mut offset: i32 = 0;
                loop {
                    let page = client
                        .get_library_songs(offset, APPLE_MUSIC_FAV_PAGE)
                        .await?;
                    let items = page.data.clone().unwrap_or_default();
                    let items_len = items.len() as i32;
                    if let Some(meta) = &page.meta {
                        if let Some(total) = meta.total {
                            total_found = total;
                        }
                    }
                    let next_url = page.next.clone();

                    for track in items {
                        let attrs = track.attributes.clone().unwrap_or_default();
                        let track_id_str = track.id.clone();
                        let title = attrs.name.clone();
                        let duration_ms = attrs.duration_in_millis.filter(|d| *d > 0);
                        if attrs.duration_in_millis.unwrap_or(0) <= 0
                            && crate::services::import_pagination::is_placeholder_title(&title)
                        {
                            tracing::warn!(
                                "Skipping ghost/placeholder Apple Music track '{}' ({})",
                                title,
                                track_id_str
                            );
                            continue;
                        }
                        let artist_name = if attrs.artist_name.trim().is_empty() {
                            "Unknown Artist".to_string()
                        } else {
                            attrs.artist_name.clone()
                        };
                        let album_name = attrs.album_name.clone();
                        let isrc = attrs.isrc.clone();
                        let favorited_at = attrs.date_added.clone();

                        let res = sqlx::query(
                            r#"
                            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, favorited_at)
                            VALUES (?, ?, 'track', ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                                title = excluded.title,
                                artist_name = excluded.artist_name,
                                album_name = excluded.album_name,
                                isrc = COALESCE(excluded.isrc, favorites.isrc),
                                favorited_at = excluded.favorited_at
                            "#,
                        )
                        .bind(account_id)
                        .bind(service_id)
                        .bind(&track_id_str)
                        .bind(&title)
                        .bind(&artist_name)
                        .bind(&album_name)
                        .bind(&isrc)
                        .bind(&favorited_at)
                        .execute(db)
                        .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        let _ = persist_favorite_track_via_engine(
                            db,
                            &enrichment_engine,
                            "apple_music",
                            service_id,
                            account_id,
                            &track_id_str,
                            &title,
                            &artist_name,
                            album_name,
                            isrc,
                            duration_ms,
                        )
                        .await;
                    }

                    let total = page.meta.as_ref().and_then(|m| m.total);
                    match crate::services::import_pagination::next_apple_music_offset(
                        offset,
                        items_len,
                        APPLE_MUSIC_FAV_PAGE,
                        next_url.as_deref(),
                        total,
                    ) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }

            if type_filter == "all" || type_filter == "albums" {
                let mut offset: i32 = 0;
                loop {
                    let page = client
                        .get_library_albums(offset, APPLE_MUSIC_FAV_PAGE)
                        .await?;
                    let albums = page.data.clone().unwrap_or_default();
                    let items_len = albums.len() as i32;
                    let total = page.meta.as_ref().and_then(|m| m.total);
                    let next_url = page.next.clone();

                    for album in albums {
                        let attrs = album.attributes.clone().unwrap_or_default();
                        let album_id_str = album.id.clone();
                        let title = attrs.name.clone();
                        let artist_name = if attrs.artist_name.trim().is_empty() {
                            "Unknown Artist".to_string()
                        } else {
                            attrs.artist_name.clone()
                        };
                        let upc = attrs.upc.clone();
                        let favorited_at = attrs.date_added.clone();

                        let res = sqlx::query(
                            r#"
                            INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, upc, favorited_at)
                            VALUES (?, ?, 'album', ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
                                title = excluded.title,
                                artist_name = excluded.artist_name,
                                upc = COALESCE(excluded.upc, favorites.upc),
                                favorited_at = COALESCE(excluded.favorited_at, favorites.favorited_at)
                            "#,
                        )
                        .bind(account_id)
                        .bind(service_id)
                        .bind(&album_id_str)
                        .bind(&title)
                        .bind(&artist_name)
                        .bind(&title)
                        .bind(&upc)
                        .bind(&favorited_at)
                        .execute(db)
                        .await;

                        if let Ok(r) = res {
                            if r.rows_affected() > 0 {
                                imported += 1;
                            }
                        }

                        let _ = upsert_canonical_favorite_album(
                            db,
                            service_id,
                            &album_id_str,
                            &title,
                            &artist_name,
                            upc.as_deref(),
                            None,
                        )
                        .await;
                    }

                    match crate::services::import_pagination::next_apple_music_offset(
                        offset,
                        items_len,
                        APPLE_MUSIC_FAV_PAGE,
                        next_url.as_deref(),
                        total,
                    ) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }
        }
        _ => {
            return Err(format!(
                "Unsupported service for favorites sync: {}",
                service
            ))
        }
    }

    // Update favorites_cache
    let _ = crate::db::write_or_log(
        "perform_sync_favorites",
        sqlx::query(
            r#"
        INSERT INTO favorites_cache (service_name, item_type, total_count, last_synced_at)
        VALUES (?, ?, ?, datetime('now'))
        ON CONFLICT(service_name, item_type) DO UPDATE SET
            total_count = excluded.total_count,
            last_synced_at = datetime('now')
        "#,
        )
        .bind(&service_lower)
        .bind(&type_filter)
        .bind(total_found)
        .execute(db),
    )
    .await;

    Ok(FavoritesSyncResult {
        service: service_lower,
        item_type: type_filter,
        total_found,
        imported,
        cached: total_found,
        message: format!("Synchronized {} favorites for {}", imported, service),
    })
}
