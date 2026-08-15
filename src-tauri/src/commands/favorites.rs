// Favorites Commands - included via include!() in mod.rs
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

/// Fetch favorite tracks with multi-service support and pagination
#[tauri::command]
pub async fn get_favorites_tracks(
    state: State<'_, AppState>,
    service: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<FavoriteTrackItem>, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!("get_favorites_tracks called: service={:?}, offset={}, limit={}", service, offset, limit);

    let items = if let Some(ref svc) = service {
        if svc == "all" || svc == "local" {
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
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        } else {
            sqlx::query_as::<_, FavoriteTrackItem>(
                r#"
                SELECT 
                    f.id,
                    f.service_item_id as service_track_id,
                    f.title,
                    COALESCE(f.artist_name, 'Unknown Artist') as artist,
                    f.album_name as album,
                    f.isrc,
                    f.image_url as cover_art_url,
                    s.name as service,
                    f.favorited_at
                FROM favorites f
                JOIN services s ON s.id = f.service_id
                WHERE s.name = ? AND f.item_type = 'track'
                ORDER BY f.favorited_at DESC NULLS LAST, f.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(svc)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
    } else {
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
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
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
) -> Result<Vec<FavoriteAlbumItem>, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!("get_favorites_albums called: service={:?}, offset={}, limit={}", service, offset, limit);

    let items = if let Some(ref svc) = service {
        if svc == "all" || svc == "local" {
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
                ORDER BY al.favorite_at DESC NULLS LAST, al.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        } else {
            sqlx::query_as::<_, FavoriteAlbumItem>(
                r#"
                SELECT 
                    f.id,
                    f.service_item_id as service_album_id,
                    f.title,
                    COALESCE(f.artist_name, 'Unknown Artist') as artist,
                    f.upc,
                    f.image_url as cover_art_url,
                    s.name as service,
                    NULL as total_tracks,
                    NULL as release_date,
                    f.favorited_at
                FROM favorites f
                JOIN services s ON s.id = f.service_id
                WHERE s.name = ? AND f.item_type = 'album'
                ORDER BY f.favorited_at DESC NULLS LAST, f.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(svc)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
    } else {
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
            ORDER BY al.favorite_at DESC NULLS LAST, al.id DESC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
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
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!("get_favorites_artists called: service={:?}, offset={}, limit={}", service, offset, limit);

    let items = if let Some(ref svc) = service {
        if svc == "all" || svc == "local" {
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
                ORDER BY a.favorite_at DESC NULLS LAST, a.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        } else {
            sqlx::query_as::<_, FavoriteArtistItem>(
                r#"
                SELECT 
                    f.id,
                    f.service_item_id as service_artist_id,
                    f.title as name,
                    f.image_url,
                    s.name as service,
                    f.favorited_at
                FROM favorites f
                JOIN services s ON s.id = f.service_id
                WHERE s.name = ? AND f.item_type = 'artist'
                ORDER BY f.favorited_at DESC NULLS LAST, f.id DESC
                LIMIT ? OFFSET ?
                "#
            )
            .bind(svc)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        }
    } else {
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
            ORDER BY a.favorite_at DESC NULLS LAST, a.id DESC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
    };

    Ok(items)
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
        .bind(&service_item_id)
        .execute(&state.db)
        .await;
    } else {
        let _ = sqlx::query(
            "DELETE FROM favorites WHERE account_id = ? AND item_type = ? AND service_item_id = ?"
        )
        .bind(account_id)
        .bind(&item_type_lower)
        .bind(&service_item_id)
        .execute(&state.db)
        .await;
    }

    Ok(PushFavoriteResponse {
        service: service_lower,
        item_type: item_type_lower,
        service_item_id,
        is_favorite,
        status: "success".to_string(),
        message: format!("Successfully propagated favorite state to {}", service),
    })
}

async fn upsert_canonical_favorite_track(
    db: &sqlx::Pool<sqlx::Sqlite>,
    service_id: i64,
    service_track_id: &str,
    title: &str,
    artist_name: &str,
    album_name: Option<&str>,
    isrc: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let artist_id: i64 = match sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
        .bind(artist_name)
        .fetch_optional(db)
        .await?
    {
        Some(id) => id,
        None => {
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                .bind(artist_name)
                .fetch_one(db)
                .await?
        }
    };

    let album_id: Option<i64> = if let Some(alb) = album_name {
        let alb_id = match sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ?")
            .bind(alb)
            .fetch_optional(db)
            .await?
        {
            Some(id) => id,
            None => {
                sqlx::query_scalar("INSERT INTO albums (title) VALUES (?) RETURNING id")
                    .bind(alb)
                    .fetch_one(db)
                    .await?
            }
        };
        Some(alb_id)
    } else {
        None
    };

    let track_id: i64 = if let Some(isrc_val) = isrc.filter(|s| !s.trim().is_empty()) {
        let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE isrc = ?")
            .bind(isrc_val)
            .fetch_optional(db)
            .await?;

        if let Some(tid) = existing {
            sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')) WHERE id = ?")
                .bind(tid)
                .execute(db)
                .await?;
            tid
        } else {
            sqlx::query_scalar(
                "INSERT INTO tracks (title, album_id, isrc, is_favorite, favorite_at) VALUES (?, ?, ?, 1, datetime('now')) RETURNING id"
            )
            .bind(title)
            .bind(album_id)
            .bind(isrc_val)
            .fetch_one(db)
            .await?
        }
    } else {
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT t.id FROM tracks t JOIN track_artists ta ON ta.track_id = t.id WHERE t.title = ? AND ta.artist_id = ? LIMIT 1"
        )
        .bind(title)
        .bind(artist_id)
        .fetch_optional(db)
        .await?;

        if let Some(tid) = existing {
            sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')) WHERE id = ?")
                .bind(tid)
                .execute(db)
                .await?;
            tid
        } else {
            sqlx::query_scalar(
                "INSERT INTO tracks (title, album_id, is_favorite, favorite_at) VALUES (?, ?, 1, datetime('now')) RETURNING id"
            )
            .bind(title)
            .bind(album_id)
            .fetch_one(db)
            .await?
        }
    };

    let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(db)
        .await;

    let _ = sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?) \
         ON CONFLICT(track_id, service_id) DO UPDATE SET service_track_id = excluded.service_track_id"
    )
    .bind(track_id)
    .bind(service_id)
    .bind(service_track_id)
    .execute(db)
    .await;

    Ok(track_id)
}

async fn upsert_canonical_favorite_album(
    db: &sqlx::Pool<sqlx::Sqlite>,
    _service_id: i64,
    _service_album_id: &str,
    title: &str,
    artist_name: &str,
    upc: Option<&str>,
    image_url: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let artist_id: i64 = match sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
        .bind(artist_name)
        .fetch_optional(db)
        .await?
    {
        Some(id) => id,
        None => {
            sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                .bind(artist_name)
                .fetch_one(db)
                .await?
        }
    };

    let album_id: i64 = if let Some(upc_val) = upc.filter(|s| !s.trim().is_empty()) {
        let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE upc = ?")
            .bind(upc_val)
            .fetch_optional(db)
            .await?;

        if let Some(aid) = existing {
            sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')), cover_art_url = COALESCE(cover_art_url, ?) WHERE id = ?")
                .bind(image_url)
                .bind(aid)
                .execute(db)
                .await?;
            aid
        } else {
            sqlx::query_scalar(
                "INSERT INTO albums (title, upc, cover_art_url, is_favorite, favorite_at) VALUES (?, ?, ?, 1, datetime('now')) RETURNING id"
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
            sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')), cover_art_url = COALESCE(cover_art_url, ?) WHERE id = ?")
                .bind(image_url)
                .bind(aid)
                .execute(db)
                .await?;
            aid
        } else {
            sqlx::query_scalar(
                "INSERT INTO albums (title, cover_art_url, is_favorite, favorite_at) VALUES (?, ?, 1, datetime('now')) RETURNING id"
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

    Ok(album_id)
}

async fn upsert_canonical_favorite_artist(
    db: &sqlx::Pool<sqlx::Sqlite>,
    _service_id: i64,
    _service_artist_id: &str,
    name: &str,
) -> Result<i64, sqlx::Error> {
    let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
        .bind(name)
        .fetch_optional(db)
        .await?;

    let artist_id = if let Some(aid) = existing {
        sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')) WHERE id = ?")
            .bind(aid)
            .execute(db)
            .await?;
        aid
    } else {
        sqlx::query_scalar(
            "INSERT INTO artists (name, is_favorite, favorite_at) VALUES (?, 1, datetime('now')) RETURNING id"
        )
        .bind(name)
        .fetch_one(db)
        .await?
    };

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
                let page = client.get_favorites(0, 100).await?;
                total_found += page.total as i64;
                for item in page.items {
                    let track = item.item;
                    let track_id_str = track.id.to_string();
                    let title = track.title.clone();
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

                    // Canonical library synchronization with ISRC deduplication
                    let _ = upsert_canonical_favorite_track(
                        &state.db,
                        service_id,
                        &track_id_str,
                        &title,
                        &artist_name,
                        album_name.as_deref(),
                        isrc.as_deref(),
                    ).await;
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
                let page = client.get_favorites(0, 100).await?;
                total_found += page.tracks.total as i64;
                for track in page.tracks.items {
                    let track_id_str = track.id.to_string();
                    let title = track.title.unwrap_or_else(|| "Unknown Track".to_string());
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

                    // Canonical library synchronization with ISRC deduplication
                    let _ = upsert_canonical_favorite_track(
                        &state.db,
                        service_id,
                        &track_id_str,
                        &title,
                        &artist_name,
                        album_name.as_deref(),
                        isrc.as_deref(),
                    ).await;
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
                let page = client.get_saved_tracks(0, 50).await?;
                total_found += page.total as i64;
                for saved in page.items {
                    let track = saved.track;
                    let track_id_str = track.id.clone();
                    let title = track.name.clone();
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

                    // Canonical library synchronization with ISRC deduplication
                    let _ = upsert_canonical_favorite_track(
                        &state.db,
                        service_id,
                        &track_id_str,
                        &title,
                        &artist_name,
                        album_name.as_deref(),
                        isrc.as_deref(),
                    ).await;
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
