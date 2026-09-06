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

    tracing::info!("perform_get_favorites_tracks called: service={:?}, offset={}, limit={}", service, offset, limit);

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

#[allow(dead_code)]
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

    tracing::info!("perform_get_favorites_albums called: service={:?}, offset={}, limit={}, include_stubs={}", service, offset, limit, include_stubs);

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
    perform_get_favorites_albums_with_options(&state.db, service, offset, limit, include_stubs.unwrap_or(false)).await
}

pub async fn perform_get_favorites_artists(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteArtistItem>, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!("perform_get_favorites_artists called: service={:?}, offset={}, limit={}", service, offset, limit);

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
         RETURNING is_favorite"
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
         RETURNING is_favorite"
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

/// Push a favorite modification (add or remove) to a streaming service (Tidal, Qobuz, Spotify)
#[tauri::command]
pub async fn push_favorite_to_service(
    state: State<'_, AppState>,
    service: String,
    item_type: String,
    service_item_id: String,
    is_favorite: bool,
) -> Result<PushFavoriteResponse, String> {
    let service_lower = service.to_lowercase();
    let item_type_lower = item_type.to_lowercase();

    tracing::info!(
        "push_favorite_to_service: service={}, type={}, id={}, is_fav={}",
        service_lower,
        item_type_lower,
        service_item_id,
        is_favorite
    );

    let (account_id, creds) = load_service_credentials(&state.db, &service_lower).await?;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(&service_lower)
        .fetch_one(&state.db)
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
            let app_id = std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
            let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());
            let user_auth_token = creds["user_auth_token"]
                .as_str()
                .or_else(|| creds["auth_token"].as_str())
                .or_else(|| creds["access_token"].as_str())
                .ok_or("Missing user auth token for Qobuz")?;

            let client = crate::services::QobuzClient::new_with_token(app_id, app_secret, user_auth_token.to_string());

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
            let access_token = get_or_refresh_spotify_token(&state.db, account_id, &creds).await?;
            let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
            let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
            let client = crate::services::SpotifyClient::new(access_token, refresh_token, expires_at);

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
        _ => return Err(format!("Unsupported service for push: {}", service)),
    }

    perform_push_favorite_sync(
        &state.db,
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
            "DELETE FROM favorites WHERE account_id = ? AND item_type = ? AND service_item_id = ?"
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
                "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = ?"
            )
            .bind(service_id)
            .bind(service_item_id)
            .fetch_optional(db)
            .await
            .unwrap_or(None);

            if track_id_opt.is_none() {
                if let Ok(num_id) = service_item_id.parse::<i64>() {
                    let exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM tracks WHERE id = ?")
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
                        "#
                    )
                    .bind(account_id)
                    .bind(track_id)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to update library_entries: {}", e))?;

                    let _ = sqlx::query(
                        "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)"
                    )
                    .bind(track_id)
                    .bind(service_id)
                    .bind(service_item_id)
                    .execute(db)
                    .await;
                } else {
                    let _ = sqlx::query("UPDATE library_entries SET is_liked = 0 WHERE account_id = ? AND track_id = ?")
                        .bind(account_id)
                        .bind(track_id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to update library_entries: {}", e))?;

                    let remaining_likes: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM library_entries WHERE track_id = ? AND is_liked = 1"
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
                        let _ = sqlx::query("UPDATE tracks SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
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
                "spotify" => sqlx::query_scalar("SELECT id FROM albums WHERE spotify_id = ?").bind(service_item_id).fetch_optional(db).await.unwrap_or(None),
                "tidal" => sqlx::query_scalar("SELECT id FROM albums WHERE tidal_id = ?").bind(service_item_id).fetch_optional(db).await.unwrap_or(None),
                "qobuz" => sqlx::query_scalar("SELECT id FROM albums WHERE qobuz_id = ?").bind(service_item_id).fetch_optional(db).await.unwrap_or(None),
                _ => None,
            };

            if album_id_opt.is_none() {
                if let Ok(num_id) = service_item_id.parse::<i64>() {
                    let exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM albums WHERE id = ?")
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
                    let _ = sqlx::query("UPDATE albums SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
                        .bind(album_id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to clear album favorite: {}", e))?;
                }
            }
        }
        "artist" => {
            let mut artist_id_opt: Option<i64> = match service_lower.as_str() {
                "spotify" => sqlx::query_scalar("SELECT id FROM artists WHERE spotify_id = ?").bind(service_item_id).fetch_optional(db).await.unwrap_or(None),
                "tidal" => sqlx::query_scalar("SELECT id FROM artists WHERE tidal_id = ?").bind(service_item_id).fetch_optional(db).await.unwrap_or(None),
                "qobuz" => sqlx::query_scalar("SELECT id FROM artists WHERE qobuz_id = ?").bind(service_item_id).fetch_optional(db).await.unwrap_or(None),
                _ => None,
            };

            if artist_id_opt.is_none() {
                if let Ok(num_id) = service_item_id.parse::<i64>() {
                    let exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM artists WHERE id = ?")
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
                artist_id_opt = sqlx::query_scalar("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
                    .bind(service_item_id)
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
                    let _ = sqlx::query("UPDATE artists SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
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
            syncify_core_domain::quality::classify_audio_tier(Some(16), Some(44100), None, Some("FLAC"))
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
    let target_artist = if clean_artist.is_empty() { artist_name.trim() } else { clean_artist.as_str() };

    let artist_id: i64 = if let Ok(Some((aid,))) = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1",
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
            let has_tracks: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM tracks WHERE album_id = ?")
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
        let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ? LIMIT 1")
            .bind(title)
            .fetch_optional(db)
            .await?;

        if let Some(aid) = existing {
            let has_tracks: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM tracks WHERE album_id = ?")
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

    let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(artist_id)
        .execute(db)
        .await;

    let service_name: Option<String> = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
        .bind(service_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    if let Some(ref svc) = service_name {
        match svc.to_lowercase().as_str() {
            "tidal" => {
                let _ = sqlx::query("UPDATE albums SET tidal_id = ? WHERE id = ? AND (tidal_id IS NULL OR tidal_id = '')")
                    .bind(service_album_id)
                    .bind(album_id)
                    .execute(db)
                    .await;
            }
            "spotify" => {
                let _ = sqlx::query("UPDATE albums SET spotify_id = ? WHERE id = ? AND (spotify_id IS NULL OR spotify_id = '')")
                    .bind(service_album_id)
                    .bind(album_id)
                    .execute(db)
                    .await;
            }
            "qobuz" => {
                let _ = sqlx::query("UPDATE albums SET qobuz_id = ? WHERE id = ? AND (qobuz_id IS NULL OR qobuz_id = '')")
                    .bind(service_album_id)
                    .bind(album_id)
                    .execute(db)
                    .await;
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
    let target_name = if clean_name.is_empty() { name.trim() } else { clean_name.as_str() };

    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1",
    )
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
                let _ = sqlx::query("UPDATE artists SET tidal_id = ? WHERE id = ? AND (tidal_id IS NULL OR tidal_id = '')")
                    .bind(service_artist_id)
                    .bind(artist_id)
                    .execute(db)
                    .await;
            }
            "spotify" => {
                let _ = sqlx::query("UPDATE artists SET spotify_id = ? WHERE id = ? AND (spotify_id IS NULL OR spotify_id = '')")
                    .bind(service_artist_id)
                    .bind(artist_id)
                    .execute(db)
                    .await;
            }
            "qobuz" => {
                let _ = sqlx::query("UPDATE artists SET qobuz_id = ? WHERE id = ? AND (qobuz_id IS NULL OR qobuz_id = '')")
                    .bind(service_artist_id)
                    .bind(artist_id)
                    .execute(db)
                    .await;
            }
            _ => {}
        }
    }

    Ok(artist_id)
}

/// Synchronize favorites from a streaming service (Tidal, Qobuz, Spotify) into SQLite
#[tauri::command]
pub async fn sync_favorites(
    state: State<'_, AppState>,
    window: tauri::Window,
    service: String,
    fav_type: Option<String>,
) -> Result<FavoritesSyncResult, String> {
    let service_lower = service.to_lowercase();
    let type_filter = fav_type.unwrap_or_else(|| "all".to_string()).to_lowercase();

    tracing::info!("sync_favorites called for service '{}', type '{}'", service_lower, type_filter);

    let (account_id, creds) = load_service_credentials(&state.db, &service_lower).await?;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(&service_lower)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Service {} not registered: {}", service_lower, e))?;

    let mut imported = 0i64;
    let mut total_found = 0i64;

    // F2-4: un solo motor por invocación para todos los servicios.
    let enrichment_engine = crate::services::enrichment::EnrichmentEngine::new();

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
                    if track.duration <= 0 && crate::services::import_pagination::is_placeholder_title(&title) {
                        tracing::warn!("Skipping ghost/placeholder track '{}' ({})", title, track_id_str);
                        continue;
                    }
                    let artist_name = track.artist.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown Artist".to_string());
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // F2-4: identidad canónica vía EnrichmentEngine
                    let _ = persist_favorite_track_via_engine(
                        &state.db,
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
                    ).await;
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
                    let artist_name = album.artist.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown Artist".to_string());
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // Canonical library album synchronization with UPC deduplication
                    let _ = upsert_canonical_favorite_album(
                        &state.db,
                        service_id,
                        &album_id_str,
                        &title,
                        &artist_name,
                        upc.as_deref(),
                        image_url.as_deref(),
                    ).await;
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // Canonical library artist synchronization
                    let _ = upsert_canonical_favorite_artist(
                        &state.db,
                        service_id,
                        &artist_id_str,
                        &name,
                    ).await;
                }
            }
        }
        "qobuz" => {
            let app_id = std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
            let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());
            let user_auth_token = creds["user_auth_token"]
                .as_str()
                .or_else(|| creds["auth_token"].as_str())
                .or_else(|| creds["access_token"].as_str())
                .ok_or("Missing user auth token for Qobuz")?;

            let client = crate::services::QobuzClient::new_with_token(app_id, app_secret, user_auth_token.to_string());

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
                    if track.duration <= 0 && crate::services::import_pagination::is_placeholder_title(&title) {
                        tracing::warn!("Skipping ghost/placeholder track '{}' ({})", title, track_id_str);
                        continue;
                    }
                    let artist_name = track.performer.as_ref().and_then(|a| a.name.clone()).unwrap_or_else(|| "Unknown Artist".to_string());
                    let album_name = track.album.as_ref().and_then(|al| al.title.clone());
                    let isrc = track.isrc.clone();
                    let image_url = track.album.as_ref().and_then(|al| al.image.as_ref().and_then(|img| img.large.clone().or_else(|| img.small.clone())));

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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // F2-4: identidad canónica vía EnrichmentEngine
                    let _ = persist_favorite_track_via_engine(
                        &state.db,
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
                    ).await;
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
                    let artist_name = album.artist.as_ref().and_then(|a| a.name.clone()).unwrap_or_else(|| "Unknown Artist".to_string());
                    let upc = album.upc.clone();
                    let image_url = album.image.as_ref().and_then(|img| img.large.clone().or_else(|| img.small.clone()));

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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // Canonical library album synchronization with UPC deduplication
                    let _ = upsert_canonical_favorite_album(
                        &state.db,
                        service_id,
                        &album_id_str,
                        &title,
                        &artist_name,
                        upc.as_deref(),
                        image_url.as_deref(),
                    ).await;
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // Canonical library artist synchronization
                    let _ = upsert_canonical_favorite_artist(
                        &state.db,
                        service_id,
                        &artist_id_str,
                        &name,
                    ).await;
                }
            }
        }
        "spotify" => {
            let access_token = get_or_refresh_spotify_token(&state.db, account_id, &creds).await?;
            let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
            let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
            let client = crate::services::SpotifyClient::new(access_token, refresh_token, expires_at);

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
                    if track.duration_ms <= 0 && crate::services::import_pagination::is_placeholder_title(&title) {
                        tracing::warn!("Skipping ghost/placeholder track '{}' ({})", title, track_id_str);
                        continue;
                    }
                    let artist_name = track.artists.first().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown Artist".to_string());
                    let album_name = track.album.as_ref().map(|al| al.name.clone());
                    let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());
                    let image_url = track.album.as_ref().and_then(|al| al.images.first().map(|i| i.url.clone()));
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // F2-4: identidad canónica vía EnrichmentEngine
                    let _ = persist_favorite_track_via_engine(
                        &state.db,
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
                    ).await;
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // Canonical library album synchronization with UPC deduplication
                    let _ = upsert_canonical_favorite_album(
                        &state.db,
                        service_id,
                        &album_id_str,
                        &title,
                        &artist_name,
                        upc.as_deref(),
                        image_url.as_deref(),
                    ).await;
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
                    .execute(&state.db)
                    .await;

                    if let Ok(r) = res {
                        if r.rows_affected() > 0 { imported += 1; }
                    }

                    // Canonical library artist synchronization
                    let _ = upsert_canonical_favorite_artist(
                        &state.db,
                        service_id,
                        &artist_id_str,
                        &name,
                    ).await;
                }
            }
        }
        _ => return Err(format!("Unsupported service for favorites sync: {}", service)),
    }

    // Update favorites_cache
    let _ = sqlx::query(
        r#"
        INSERT INTO favorites_cache (service_name, item_type, total_count, last_synced_at)
        VALUES (?, ?, ?, datetime('now'))
        ON CONFLICT(service_name, item_type) DO UPDATE SET
            total_count = excluded.total_count,
            last_synced_at = datetime('now')
        "#
    )
    .bind(&service_lower)
    .bind(&type_filter)
    .bind(total_found)
    .execute(&state.db)
    .await;

    let _ = window.emit(
        "syncify:favorites_sync_completed",
        serde_json::json!({
            "service": service_lower,
            "item_type": type_filter,
            "total": total_found,
            "imported": imported
        }),
    );

    Ok(FavoritesSyncResult {
        service: service_lower,
        item_type: type_filter,
        total_found,
        imported,
        cached: total_found,
        message: format!("Synchronized {} favorites for {}", imported, service),
    })
}

/// S203: Normalize a favorites-modal quality label into the download_queue
/// CHECK vocabulary ('hires'|'lossless'|'high'|'any'|NULL) BEFORE inserting.
///
/// The modal used to send 'standard' verbatim, which violates
/// `CHECK(quality_preference IN ('hires','lossless','high','any') OR NULL)`
/// (migrations/0004_production.sql:26): every INSERT failed and the track
/// silently vanished from the enqueue counters. Modal vocabulary first maps to
/// canonical intent ('normal'/'standard' = 320 kbps tier → 'high'), then the
/// canonical normalizer from queue.rs runs; anything unrecognized degrades to
/// NULL (worker default applies) with a traceable warning instead of failing
/// the INSERT.
fn normalize_favorite_queue_quality(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let lowered = trimmed.to_ascii_lowercase();
    let pre_mapped = match lowered.as_str() {
        "normal" | "standard" => "high",
        other => other,
    };
    let normalized = normalize_quality_preference(Some(pre_mapped));
    if normalized.is_none() && !trimmed.is_empty() {
        tracing::warn!(
            raw_quality = %trimmed,
            "[download_favorites] Unrecognized quality preference degraded to NULL (worker default applies); track still enqueued"
        );
    }
    normalized
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadFavoritesResult {
    pub total_candidates: i64,
    pub enqueued: i64,
    pub already_downloaded: i64,
    pub already_queued: i64,
    pub unresolved_sources: i64,
    pub stale_sources: i64,
    pub ambiguous_sources: i64,
    pub is_preflight: bool,
    pub estimated_size_mb: Option<f64>,
    pub message: String,
}

/// Orchestrate mass download of favorite items (tracks, albums, artists) with priority, source identity, preflight guardrail and optional batch limit
#[tauri::command]
pub async fn download_favorites(
    state: State<'_, AppState>,
    service: Option<String>,
    item_type: Option<String>,
    quality_preference: Option<String>,
    priority: Option<i64>,
    limit: Option<i64>,
    dry_run: Option<bool>,
) -> Result<DownloadFavoritesResult, String> {
    let service_filter = service.map(|s| s.to_lowercase());
    let type_filter = item_type.unwrap_or_else(|| "all".to_string()).to_lowercase();
    let quality_pref = quality_preference.unwrap_or_else(|| "lossless".to_string());
    let prio = priority.unwrap_or(60); // Default higher priority for favorites

    let srv_param = service_filter.as_deref().filter(|s| *s != "all" && *s != "local");
    tracing::info!(
        "[download_favorites] Invoked with service={:?}, item_type={:?}, quality={:?}, priority={:?}, limit={:?}, dry_run={:?}",
        srv_param, type_filter, quality_pref, prio, limit, dry_run
    );

    // 1. Gather all candidate track IDs preserving deterministic order
    let mut candidate_track_ids: Vec<i64> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    // Fetch favorites from library_entries, favorites, and tracks
    let raw_tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        LEFT JOIN library_entries le ON le.track_id = t.id
        LEFT JOIN accounts acc_le ON acc_le.id = le.account_id
        LEFT JOIN services s_le ON s_le.id = acc_le.service_id
        LEFT JOIN favorites f ON f.item_type = 'track' AND (f.service_item_id = CAST(t.id AS TEXT) OR f.service_item_id = t.isrc)
        LEFT JOIN accounts acc_f ON acc_f.id = f.account_id
        LEFT JOIN services s_f ON s_f.id = acc_f.service_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1
        LEFT JOIN services s_ts ON s_ts.id = ts.service_id
        WHERE (t.favorite_at IS NOT NULL OR t.is_favorite = 1 OR f.id IS NOT NULL OR le.is_liked = 1)
          AND (? IS NULL OR s_le.name = ? OR s_f.name = ? OR s_ts.name = ?)
        ORDER BY t.id ASC
        "#,
    )
    .bind(srv_param)
    .bind(srv_param)
    .bind(srv_param)
    .bind(srv_param)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error fetching favorite tracks: {}", e))?;

    for (tid,) in raw_tracks {
        if seen.insert(tid) {
            candidate_track_ids.push(tid);
        }
    }

    // If item_type includes albums or is "all", fetch all tracks in favorite albums
    if type_filter == "all" || type_filter == "albums" || type_filter == "album" {
        let album_tracks: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT t.id
            FROM tracks t
            JOIN albums a ON a.id = t.album_id
            LEFT JOIN favorites f ON f.item_type = 'album' AND f.service_item_id = CAST(a.id AS TEXT)
            LEFT JOIN accounts acc_f ON acc_f.id = f.account_id
            LEFT JOIN services s_f ON s_f.id = acc_f.service_id
            LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1
            LEFT JOIN services s_ts ON s_ts.id = ts.service_id
            WHERE (a.favorite_at IS NOT NULL OR a.is_favorite = 1 OR f.id IS NOT NULL)
              AND (? IS NULL OR s_f.name = ? OR s_ts.name = ?)
            ORDER BY t.album_id ASC, t.disc_number ASC, t.track_number ASC
            "#,
        )
        .bind(srv_param)
        .bind(srv_param)
        .bind(srv_param)
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("Database error fetching album tracks: {}", e))?;

        for (tid,) in album_tracks {
            if seen.insert(tid) {
                candidate_track_ids.push(tid);
            }
        }
    }

    // If item_type includes artists or is "all", fetch all tracks by favorite artists
    if type_filter == "all" || type_filter == "artists" || type_filter == "artist" {
        let artist_tracks: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT t.id
            FROM tracks t
            JOIN track_artists ta ON ta.track_id = t.id
            JOIN artists ar ON ar.id = ta.artist_id
            LEFT JOIN favorites f ON f.item_type = 'artist' AND f.service_item_id = CAST(ar.id AS TEXT)
            LEFT JOIN accounts acc_f ON acc_f.id = f.account_id
            LEFT JOIN services s_f ON s_f.id = acc_f.service_id
            LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1
            LEFT JOIN services s_ts ON s_ts.id = ts.service_id
            WHERE (ar.favorite_at IS NOT NULL OR ar.is_favorite = 1 OR f.id IS NOT NULL)
              AND (? IS NULL OR s_f.name = ? OR s_ts.name = ?)
            ORDER BY t.id ASC
            "#,
        )
        .bind(srv_param)
        .bind(srv_param)
        .bind(srv_param)
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("Database error fetching artist tracks: {}", e))?;

        for (tid,) in artist_tracks {
            if seen.insert(tid) {
                candidate_track_ids.push(tid);
            }
        }
    }

    let total_candidates = candidate_track_ids.len() as i64;
    tracing::info!(
        "[download_favorites] Candidate tracks found: total={}, after_limit={}",
        total_candidates,
        limit.map(|l| (l as usize).min(candidate_track_ids.len())).unwrap_or(candidate_track_ids.len())
    );

    // Apply batch limit if specified
    if let Some(lim) = limit {
        if lim > 0 && (lim as usize) < candidate_track_ids.len() {
            candidate_track_ids.truncate(lim as usize);
        }
    }

    if total_candidates == 0 {
        return Ok(DownloadFavoritesResult {
            total_candidates: 0,
            enqueued: 0,
            already_downloaded: 0,
            already_queued: 0,
            unresolved_sources: 0,
            stale_sources: 0,
            ambiguous_sources: 0,
            is_preflight: false,
            estimated_size_mb: Some(0.0),
            message: "No favorite tracks found matching the criteria.".to_string(),
        });
    }

    // Determine if this is a Preflight dry-run (either explicit dry_run=true, or mass batch >100 without explicit dry_run=false)
    let is_preflight = dry_run.unwrap_or_else(|| limit.is_none() && total_candidates > 100);

    // 2. Fetch tracks status: downloaded vs in queue vs unresolved vs stale vs ambiguous
    let mut enqueued = 0i64;
    let mut already_downloaded = 0i64;
    let mut already_queued = 0i64;
    let mut unresolved_sources = 0i64;
    let mut stale_sources = 0i64;
    let mut ambiguous_sources = 0i64;

    // Get current max position in download_queue
    let max_pos: (Option<i64>,) = sqlx::query_as(
        "SELECT MAX(position) FROM download_queue WHERE status = 'queued'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((None,));
    let mut next_pos = max_pos.0.map(|p| p + 1).unwrap_or(0);

    for track_id in candidate_track_ids {
        // Guardrail C3: Check if already downloaded or in queue via ISRC (NOCASE) or canonical signature
        match check_queue_guardrail(&state.db, track_id, None, None, None).await {
            Ok(Some(QueueGuardrailMatch::AlreadyDownloaded { .. })) => {
                already_downloaded += 1;
                continue;
            }
            Ok(Some(QueueGuardrailMatch::AlreadyQueued { .. })) => {
                already_queued += 1;
                continue;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(
                    "[download_favorites] Guardrail check error for track {}: {}",
                    track_id,
                    e
                );
            }
        }

        // Check if already in queue failed with 404/stale/ambiguous
        let queue_item: Option<(i64, String, Option<String>)> = sqlx::query_as(
            "SELECT id, status, error_message FROM download_queue WHERE track_id = ? AND status = 'failed' ORDER BY id DESC LIMIT 1"
        )
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

        if let Some((_, _status, err_opt)) = queue_item {
            let err = err_opt.unwrap_or_default();
            if err.contains("404") || err.contains("NotFound") || err.contains("StaleSource") {
                stale_sources += 1;
                continue;
            } else if err.contains("AmbiguousSource") {
                ambiguous_sources += 1;
                continue;
            }
        }

        let track_info: Option<(
            Option<i64>,    // service_id
            Option<String>, // service_name
            Option<String>, // service_track_id
            Option<String>, // title
            Option<String>, // artist
            Option<String>, // album
            Option<String>, // isrc
        )> = match &service_filter {
            Some(srv) if srv != "all" && srv != "local" => {
                sqlx::query_as(
                    r#"
                    SELECT ts.service_id, s.name, ts.service_track_id,
                           t.title,
                           (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
                           alb.title as album,
                           t.isrc
                    FROM tracks t
                    LEFT JOIN albums alb ON alb.id = t.album_id
                    JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1 AND ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != ''
                    JOIN services s ON s.id = ts.service_id AND s.name = ?
                    WHERE t.id = ?
                    ORDER BY COALESCE(ts.quality_score, 0) DESC, COALESCE(ts.bit_depth, 0) DESC
                    LIMIT 1
                    "#
                )
                .bind(srv)
                .bind(track_id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None)
            }
            _ => {
                sqlx::query_as(
                    r#"
                    SELECT ts.service_id, s.name, ts.service_track_id,
                           t.title,
                           (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
                           alb.title as album,
                           t.isrc
                    FROM tracks t
                    LEFT JOIN albums alb ON alb.id = t.album_id
                    JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1 AND ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != ''
                    JOIN services s ON s.id = ts.service_id
                    WHERE t.id = ?
                    ORDER BY 
                        CASE s.name 
                            WHEN 'qobuz' THEN 1 
                            WHEN 'tidal' THEN 2 
                            WHEN 'deezer' THEN 3 
                            ELSE 4 
                        END ASC,
                        COALESCE(ts.quality_score, 0) DESC
                    LIMIT 1
                    "#
                )
                .bind(track_id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None)
            }
        };

        let (s_id, s_name, s_track_id, t_title, t_artist, t_album, t_isrc) = track_info.unwrap_or((None, None, None, None, None, None, None));

        // When downloading favorites without fallback, source identity MUST be resolved and locked
        if s_track_id.is_none() || s_track_id.as_deref().unwrap_or("").trim().is_empty() {
            tracing::warn!("[download_favorites] Track {} has no resolved service_track_id on {:?}; marked as unresolved", track_id, service_filter);
            unresolved_sources += 1;
            continue;
        }

        // If in Preflight mode, do not write to database
        if is_preflight {
            enqueued += 1;
            continue;
        }

        // Enqueue new item with full source identity.
        // S203: quality label is normalized to the CHECK vocabulary first — a raw
        // 'standard' (modal) used to violate the constraint and drop the INSERT
        // silently below. Unknown labels become NULL, not a failed INSERT.
        let normalized_quality = normalize_favorite_queue_quality(&quality_pref);
        let insert_res = sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference, resumable,
                service_id, service_name, service_track_id,
                target_title, target_artist, target_album, target_isrc,
                allow_fallback, smart_studio_origin, created_at
            )
            VALUES (?, ?, ?, 'queued', ?, 1, ?, ?, ?, ?, ?, ?, ?, 1, 1, CURRENT_TIMESTAMP)
            "#
        )
        .bind(track_id)
        .bind(prio)
        .bind(next_pos)
        .bind(&normalized_quality)
        .bind(s_id)
        .bind(s_name)
        .bind(s_track_id)
        .bind(t_title)
        .bind(t_artist)
        .bind(t_album)
        .bind(t_isrc)
        .execute(&state.db)
        .await;

        if let Err(insert_err) = &insert_res {
            // S203: never swallow enqueue failures again — the favorites counters
            // must stay reconcilable with the queue contents.
            tracing::error!(
                track_id,
                requested_quality = %quality_pref,
                normalized_quality = ?normalized_quality,
                "[download_favorites] Failed to enqueue track: {}",
                insert_err
            );
        }
        if insert_res.is_ok() {
            enqueued += 1;
            next_pos += 1;
        }
    }

    if enqueued > 0 {
        state.worker_state.notify_available();
    }

    let est_mb = (enqueued as f64) * 35.0; // ~35MB per FLAC track estimate

    if is_preflight {
        Ok(DownloadFavoritesResult {
            total_candidates,
            enqueued,
            already_downloaded,
            already_queued,
            unresolved_sources,
            stale_sources,
            ambiguous_sources,
            is_preflight: true,
            estimated_size_mb: Some(est_mb),
            message: format!(
                "Preflight summary: {} candidate(s) total — {} ready to queue (~{:.1} MB), {} already downloaded, {} in queue, {} unresolved, {} stale sources, {} ambiguous. Confirmation required to queue.",
                total_candidates, enqueued, est_mb, already_downloaded, already_queued, unresolved_sources, stale_sources, ambiguous_sources
            ),
        })
    } else {
        Ok(DownloadFavoritesResult {
            total_candidates,
            enqueued,
            already_downloaded,
            already_queued,
            unresolved_sources,
            stale_sources,
            ambiguous_sources,
            is_preflight: false,
            estimated_size_mb: Some(est_mb),
            message: format!(
                "Enqueued {} of {} favorite tracks ({} already downloaded, {} already in queue, {} unresolved, {} stale, {} ambiguous)",
                enqueued, total_candidates, already_downloaded, already_queued, unresolved_sources, stale_sources, ambiguous_sources
            ),
        })
    }
}

// ==============================================
// S203: favorites quality_preference CHECK regression tests
// ==============================================
#[cfg(test)]
mod favorites_quality_tests {
    use super::*;

    #[test]
    fn test_s203_favorite_quality_vocab_mapping() {
        // The modal's 'Standard' option (320 kbps MP3 / High AAC) maps to 'high'.
        assert_eq!(normalize_favorite_queue_quality("standard"), Some("high".to_string()));
        assert_eq!(normalize_favorite_queue_quality("Standard"), Some("high".to_string()));
        // 'normal' → 'high' per S203 vocabulary mapping.
        assert_eq!(normalize_favorite_queue_quality("normal"), Some("high".to_string()));
        // Canonical CHECK values pass through untouched.
        assert_eq!(normalize_favorite_queue_quality("lossless"), Some("lossless".to_string()));
        assert_eq!(normalize_favorite_queue_quality("hires"), Some("hires".to_string()));
        assert_eq!(normalize_favorite_queue_quality("any"), Some("any".to_string()));
        // Legacy API-style spellings ride the canonical normalizer.
        assert_eq!(normalize_favorite_queue_quality("HI_RES_LOSSLESS"), Some("hires".to_string()));
        // Unknown → NULL (worker default applies) instead of a failing INSERT.
        assert_eq!(normalize_favorite_queue_quality("ultra_mega"), None);
        assert_eq!(normalize_favorite_queue_quality(""), None);
    }

    /// Minimal replica of the migration 0004 `download_queue` DDL restricted to the
    /// columns the mass-enqueue INSERT touches — most importantly the exact
    /// `quality_preference` CHECK that 'standard' used to violate.
    async fn setup_queue_check_db() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::query(
            r#"
            CREATE TABLE download_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL,
                status TEXT DEFAULT 'queued'
                    CHECK(status IN ('queued', 'downloading', 'complete', 'failed', 'cancelled')),
                priority INTEGER DEFAULT 50 CHECK(priority >= 0 AND priority <= 100),
                position INTEGER,
                quality_preference TEXT
                    CHECK(quality_preference IN ('hires', 'lossless', 'high', 'any')
                          OR quality_preference IS NULL),
                progress_percent REAL DEFAULT 0.0,
                bytes_downloaded INTEGER DEFAULT 0,
                total_bytes INTEGER,
                error_message TEXT,
                retry_count INTEGER DEFAULT 0 CHECK(retry_count >= 0),
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                started_at TEXT,
                completed_at TEXT,
                resumable INTEGER DEFAULT 1,
                service_id INTEGER,
                service_name TEXT,
                service_track_id TEXT,
                target_title TEXT,
                target_artist TEXT,
                target_album TEXT,
                target_isrc TEXT,
                allow_fallback INTEGER NOT NULL DEFAULT 0,
                smart_studio_origin INTEGER NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create download_queue");
        pool
    }

    async fn insert_with_quality(pool: &sqlx::SqlitePool, quality: Option<&str>) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference, resumable,
                service_id, service_name, service_track_id,
                target_title, target_artist, target_album, target_isrc,
                allow_fallback, smart_studio_origin, created_at
            )
            VALUES (?, ?, ?, 'queued', ?, 1, ?, ?, ?, ?, ?, ?, ?, 1, 1, CURRENT_TIMESTAMP)
            "#,
        )
        // Columns service_id..target_isrc are nullable in production; NULL everywhere
        // isolates the quality_preference behaviour under test.
        .bind(42_i64)
        .bind(60_i64)
        .bind(0_i64)
        .bind(quality)
        .bind(Option::<i64>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .execute(pool)
        .await
    }

    #[tokio::test]
    async fn test_s203_standard_raw_insert_violates_check() {
        let pool = setup_queue_check_db().await;
        // Reproduce the silent drop: the raw modal label violates the CHECK and the
        // INSERT fails — exactly what made favorite tracks vanish from counters.
        let raw = insert_with_quality(&pool, Some("standard")).await;
        assert!(raw.is_err(), "raw 'standard' must violate the CHECK constraint");

        // The normalized path succeeds and persists canonical 'high'.
        insert_with_quality(&pool, normalize_favorite_queue_quality("standard").as_deref())
            .await
            .expect("normalized 'standard'→'high' must insert cleanly");

        // Unknown labels degrade to NULL and still enqueue (worker default applies).
        insert_with_quality(&pool, normalize_favorite_queue_quality("bogus").as_deref())
            .await
            .expect("NULL quality must insert cleanly");

        // Assert on stored VALUES (not ids: the failed INSERT consumes an AUTOINCREMENT id).
        let rows: Vec<(Option<String>,)> =
            sqlx::query_as("SELECT quality_preference FROM download_queue ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2, "exactly the two normalized inserts must persist");
        assert_eq!(rows[0].0.as_deref(), Some("high"), "'standard' must be stored as canonical 'high'");
        assert_eq!(rows[1].0, None, "unknown label must be stored as NULL (worker default applies)");
    }
}
