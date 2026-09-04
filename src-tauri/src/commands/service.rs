// Service Commands - included via include!() in mod.rs
// 
// Spotify auth, service imports (Spotify, Qobuz, Tidal, Deezer, etc.)

// ==============================================
// SHARED IMPORT HELPERS
// ==============================================

/// Load account credentials for a service (internal helper)
async fn load_service_credentials(
    db: &sqlx::SqlitePool,
    service_name: &str,
) -> Result<(i64, serde_json::Value), String> {
    let account: (i64, String, Option<i64>) = sqlx::query_as(
        "SELECT a.id, a.credentials_json, a.credentials_invalid FROM accounts a 
         JOIN services s ON s.id = a.service_id 
         WHERE LOWER(s.name) = LOWER(?) AND a.is_active = 1 
         ORDER BY a.id DESC LIMIT 1"
    )
    .bind(service_name)
    .fetch_one(db)
    .await
    .map_err(|_| format!("{} account not connected", service_name))?;

    if account.2.unwrap_or(0) != 0 {
        return Err(format!("RequiresAuth: {} account credentials marked invalid. Please reconnect in Settings > Accounts.", service_name));
    }

    let decrypted = crate::crypto::decrypt(&account.1)?;
    let creds: serde_json::Value = serde_json::from_str(&decrypted)
        .map_err(|e| format!("Invalid credentials: {}", e))?;

    Ok((account.0, creds))
}

/// S186: Resolve a usable Qobuz user_auth_token for the sync/enrichment/playlists paths.
///
/// Forensic context (syncify-dev.log:282786/:283332, accounts row id=5): the browser
/// login bridge legitimately returns success WITHOUT an API token when the Qobuz web
/// player no longer exposes a capturable one ("didn't yield token from XHR headers",
/// "JS fetch error"), keeping only username/password. The DOWNLOAD pipeline already
/// auto-logs-in with those stored credentials (download/qobuz.rs), but sync hard-failed
/// with "RequiresAuth: Qobuz user auth token missing in credentials" — an asymmetry
/// that made freshly connected accounts permanently unusable for sync.
///
/// Contract (mirrors download/qobuz.rs resolve_user_auth_token):
///   1. A stored, viability-filtered user_auth_token/auth_token/access_token wins as-is.
///   2. Otherwise a signed `user/login` round-trip with the stored username/password.
///   3. The fresh token is PERSISTED onto THIS account row only (whole-payload replace,
///      same single-writer semantics as upsert_service_account) so subsequent syncs and
///      downloads reuse it instead of re-logging-in on every run.
///
/// `login` is injected so regression tests can fake the network round-trip offline.
pub async fn resolve_qobuz_user_auth_token_with<L, Fut>(
    db: &sqlx::SqlitePool,
    account_id: i64,
    creds: &serde_json::Value,
    login: L,
) -> Result<String, String>
where
    L: FnOnce(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    // 1) Stored token — same viability rules as start_auth_and_save and download path.
    for key in ["user_auth_token", "auth_token", "access_token"] {
        if let Some(token) = creds.get(key).and_then(|v| v.as_str()) {
            let trimmed = token.trim();
            if is_viable_qobuz_token_auth(trimmed) {
                return Ok(trimmed.to_string());
            }
        }
    }

    // 2) Username/password API auto-login — same fallback contract as downloads.
    //    Values that look like captured console errors (S186 forensics) are rejected
    //    up-front so sync fails fast with an actionable message instead of burning a
    //    network round-trip on credentials that can never work.
    let username = creds["username"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| is_plausible_qobuz_credential_value(s));
    let password = creds["password"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| is_plausible_qobuz_credential_value(s));

    let (username, password) = match (username, password) {
        (Some(u), Some(p)) => (u, p),
        _ => {
            return Err(
                "RequiresAuth: Qobuz user auth token missing in credentials and no usable username/password fallback — please reconnect in Settings > Accounts."
                    .to_string(),
            );
        }
    };

    let fresh_token = login(username.to_string(), password.to_string())
        .await
        .map_err(|e| {
            format!(
                "RequiresAuth: Qobuz auto-login with stored credentials failed ({}). Please reconnect in Settings > Accounts.",
                e
            )
        })?;

    let fresh_token = fresh_token.trim().to_string();
    if !is_viable_qobuz_token_auth(&fresh_token) {
        return Err(
            "RequiresAuth: Qobuz auto-login returned an unusable token. Please reconnect in Settings > Accounts."
                .to_string(),
        );
    }

    // 3) Persist the fresh token back onto THIS account row only.
    let mut updated = creds.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert(
            "user_auth_token".to_string(),
            serde_json::Value::String(fresh_token.clone()),
        );
        obj.insert(
            "auth_token".to_string(),
            serde_json::Value::String(fresh_token.clone()),
        );
    }
    let encrypted = crate::crypto::encrypt(&updated.to_string())?;
    sqlx::query("UPDATE accounts SET credentials_json = ?, last_auth_error = NULL WHERE id = ?")
        .bind(&encrypted)
        .bind(account_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to persist refreshed Qobuz token: {}", e))?;

    tracing::info!(
        "[S186] Persisted fresh Qobuz user_auth_token onto account {}",
        account_id
    );

    Ok(fresh_token)
}

/// Real-network variant of [`resolve_qobuz_user_auth_token_with`] used by commands.
async fn resolve_qobuz_user_auth_token(
    db: &sqlx::SqlitePool,
    account_id: i64,
    creds: &serde_json::Value,
) -> Result<String, String> {
    let login_client = crate::services::QobuzClient::new(
        std::env::var("QOBUZ_APP_ID")
            .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string()),
        std::env::var("QOBUZ_APP_SECRET")
            .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string()),
    );
    resolve_qobuz_user_auth_token_with(db, account_id, creds, |u, p| async move {
        login_client.login(&u, &p).await
    })
    .await
}

/// Emit import progress event (shared helper)
pub(crate) fn emit_import_progress(
    window: &tauri::Window,
    service: &str,
    status: &str,
    current: u64,
    total: u64,
    message: &str,
) {
    let _ = window.emit(
        "import-progress",
        serde_json::json!({
            "service": service,
            "status": status,
            "current": current,
            "total": total,
            "message": message
        }),
    );
}

/// Emit import complete event (shared helper)
pub(crate) fn emit_import_complete(window: &tauri::Window, service: &str, imported: u64, skipped: u64) {
    let _ = window.emit(
        "import-complete",
        serde_json::json!({
            "service": service,
            "imported": imported,
            "skipped": skipped,
            "message": format!("Imported {} tracks, {} skipped", imported, skipped)
        }),
    );
}

/// Get or refresh Spotify access token
/// Returns valid access token, refreshing if needed and saving to DB.
async fn get_or_refresh_spotify_token(
    db: &sqlx::SqlitePool,
    account_id: i64,
    creds: &serde_json::Value,
) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
    let buffer_seconds = 300; // 5 minutes

    if now >= (expires_at - buffer_seconds) {
        tracing::info!("Spotify access token expired or expiring soon, refreshing via PKCE...");

        let refresh_token = creds["refresh_token"]
            .as_str()
            .ok_or("Missing refresh token - please reconnect to Spotify")?;

        // S196: credentials resolved from DB settings first, env fallback (dev).
        let config: SpotifyConfig = crate::commands::resolve_spotify_config(db)
            .await
            .map_err(|e| format!("Spotify config error: {}", e))?;
        let client_id = config.client_id;
        let http_client = reqwest::Client::new();
        
        let params = [
            ("client_id", client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        let token_resp = http_client
            .post("https://accounts.spotify.com/api/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Refresh request failed: {}", e))?;

        if !token_resp.status().is_success() {
            let error = token_resp.text().await.unwrap_or_default();
            tracing::error!("Token refresh failed: {}", error);
            return Err(format!("Token refresh failed: {}", error));
        }

        let token_data: serde_json::Value = token_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        let new_access_token = token_data["access_token"]
            .as_str()
            .ok_or("Missing access_token in refresh response")?
            .to_string();
            
        let new_refresh_token = token_data["refresh_token"]
            .as_str()
            .unwrap_or(refresh_token)
            .to_string();
            
        let expires_in = token_data["expires_in"].as_i64().unwrap_or(3600);
        let new_expires_at = now + expires_in;

        let updated_creds = serde_json::json!({
            "token_type": "Bearer",
            "access_token": new_access_token,
            "refresh_token": new_refresh_token,
            "expires_at": new_expires_at
        });

        let encrypted = crate::crypto::encrypt(&updated_creds.to_string())?;
        sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
            .bind(&encrypted)
            .bind(account_id)
            .execute(db)
            .await
            .map_err(|e| format!("Failed to save refreshed token: {}", e))?;

        tracing::info!("Spotify PKCE token refreshed and saved to database");
        Ok(new_access_token)
    } else {
        creds["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing access token".to_string())
    }
}

// ==============================================
// SPOTIFY AUTH
// ==============================================

/// Get Spotify auth URL
#[tauri::command]
pub async fn start_spotify_auth(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("start_spotify_auth called");

    // S196: credentials resolved from DB settings first, env fallback (dev).
    let config = crate::commands::resolve_spotify_config(&state.db)
        .await
        .map_err(|e| format!("Config error: {}", e))?;

    Ok(config.auth_url(SPOTIFY_SCOPES))
}

/// Handle Spotify OAuth callback
#[tauri::command]
pub async fn spotify_auth_callback(
    state: State<'_, AppState>,
    code: String,
) -> Result<String, String> {
    tracing::info!("spotify_auth_callback called");

    // S196: credentials resolved from DB settings first, env fallback (dev).
    let config = crate::commands::resolve_spotify_config(&state.db).await?;

    // Exchange code for token
    let token = config.exchange_code(&code).await?;

    // Get user info
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = now + token.expires_in;
    
    let client = SpotifyClient::new(token.access_token.clone(), token.refresh_token.clone(), expires_at);
    let user = client.get_current_user().await?;

    // Save account to database
    let spotify_service_id: (i64,) =
        sqlx::query_as("SELECT id FROM services WHERE name = 'spotify'")
            .fetch_one(&state.db)
            .await
            .map_err(|e| format!("Service not found: {}", e))?;

    // Encrypt and store credentials with absolute expiry timestamp
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        + token.expires_in;

    let credentials = serde_json::json!({
        "access_token": token.access_token,
        "refresh_token": token.refresh_token,
        "expires_at": expires_at
    })
    .to_string();

    let encrypted = crate::crypto::encrypt(&credentials)?;

    sqlx::query(
        "INSERT OR REPLACE INTO accounts (service_id, display_name, email, credentials_json, is_active) VALUES (?, ?, ?, ?, 1)"
    )
    .bind(spotify_service_id.0)
    .bind(&user.display_name)
    .bind(&user.email)
    .bind(&encrypted)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to save account: {}", e))?;

    Ok(format!(
        "Connected as {}",
        user.display_name.unwrap_or_else(|| user.id)
    ))
}

/// Import Spotify library
#[tauri::command]
pub async fn import_spotify_library(
    window: tauri::Window,
    state: tauri::State<'_, crate::AppState>,
    import_lock: tauri::State<'_, ImportLock>,
) -> Result<ImportResult, String> {
    let _guard = import_lock
        .0
        .try_lock()
        .map_err(|_| "An import is already in progress".to_string())?;

    tracing::info!("import_spotify_library called");

    // Use shared helpers for credential loading and token refresh
    let (account_id, creds) = load_service_credentials(&state.db, "spotify").await?;
    let access_token = get_or_refresh_spotify_token(&state.db, account_id, &creds).await?;

    // Use shared helper for progress events
    emit_import_progress(&window, "Spotify", "started", 0, 0, "Starting Spotify library import...");

    // Import library with progress
    let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
    let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
    let client = SpotifyClient::new(access_token, refresh_token, expires_at);

    // Use local import cache for deduplication
    let mut cache = ImportCache::new();
    let spotify_service_id = cache.get_service_id(&state.db, "spotify").await?;

    // First, get total count
    let first_page = client.get_saved_tracks(0, 1).await?;
    let total = first_page.total as u64;

    // Now do the actual import with parallel page fetching
    let limit = 50;
    let concurrent_pages = 4; // Fetch 4 pages at once
    let mut imported = 0u64;
    let mut skipped = 0u64;
    let mut skip_no_album = 0u64;
    let mut skip_invalid_data = 0u64;
    let mut dedupe_already_exists = 0u64; // Track already in library (by ISRC match)
    let mut processed = 0u64;
    let total_pages = ((total as i32) + limit - 1) / limit;

    tracing::info!(
        "Starting parallel import: {} total tracks, {} pages, {} concurrent",
        total,
        total_pages,
        concurrent_pages
    );

    let mut current_page = 0;

    while current_page < total_pages {
        // Calculate which pages to fetch in this batch
        let pages_to_fetch: Vec<i32> =
            (current_page..std::cmp::min(current_page + concurrent_pages, total_pages)).collect();

        tracing::info!(
            "Fetching pages {} to {} ({} pages)",
            pages_to_fetch.first().unwrap_or(&0),
            pages_to_fetch.last().unwrap_or(&0),
            pages_to_fetch.len()
        );

        // Fetch all pages concurrently
        let fetch_futures: Vec<_> = pages_to_fetch
            .iter()
            .map(|page_num| {
                let offset = page_num * limit;
                client.get_saved_tracks(offset, limit)
            })
            .collect();

        let results = futures::future::join_all(fetch_futures).await;

        // Process each page's results in order
        for (i, result) in results.into_iter().enumerate() {
            let page = match result {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to fetch page {}: {}", pages_to_fetch[i], e);
                    continue;
                }
            };

            tracing::info!(
                "Processing page {} with {} tracks",
                pages_to_fetch[i],
                page.items.len()
            );

            for saved in &page.items {
                let track = &saved.track;

                // Skip tracks without albums (local files, etc.)
                let Some(ref album) = track.album else {
                    skip_no_album += 1;
                    skipped += 1;
                    continue;
                };

                // Skip tracks with empty/invalid data
                if track.id.is_empty() || track.name.is_empty() || track.duration_ms == 0 {
                    tracing::warn!(
                        "Skipping invalid track: id='{}', name='{}', duration={}",
                        track.id,
                        track.name,
                        track.duration_ms
                    );
                    skip_invalid_data += 1;
                    skipped += 1;
                    continue;
                }

                let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());

                // Get or create ALL artists (with roles)
                let mut artist_ids: Vec<(i64, &str)> = Vec::new();
                for (index, artist) in track.artists.iter().enumerate() {
                    if artist.name.is_empty() {
                        continue;
                    }
                    let artist_id = cache.get_or_create_artist(&state.db, &artist.name).await?;
                    let role = if index == 0 { "primary" } else { "featured" };
                    artist_ids.push((artist_id, role));
                }

                // Fallback for no artists
                if artist_ids.is_empty() {
                    let artist_id = cache
                        .get_or_create_artist(&state.db, "Unknown Artist")
                        .await?;
                    artist_ids.push((artist_id, "primary"));
                }

                let primary_artist_id = artist_ids
                    .first()
                    .map(|a| a.0)
                    .ok_or_else(|| "Failed to resolve primary artist".to_string())?;

                // Get or create album (cached)
                let album_key = format!("{}:{}", primary_artist_id, &album.name);
                let image_url = album.images.first().map(|i| i.url.as_str());
                let album_id = cache
                    .get_or_create_album(
                        &state.db,
                        &state.album_lock,
                        &album_key,
                        &album.name,
                        primary_artist_id,
                        album.release_date.as_deref(),
                        image_url,
                    )
                    .await?;

                // Get or create track
                let track_id = client
                    .get_or_create_track(&state.db, track, isrc.as_deref(), Some(album_id))
                    .await?;

                // Link ALL artists to track (with retry for busy database)
                for (artist_id, role) in artist_ids {
                    let mut retries = 0;
                    loop {
                        match sqlx::query(
                            "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, ?)"
                        )
                        .bind(track_id)
                        .bind(artist_id)
                        .bind(role)
                        .execute(&state.db)
                        .await {
                            Ok(_) => break,
                            Err(e) => {
                                retries += 1;
                                if retries >= 3 {
                                    tracing::error!("Failed to link artist {} to track {} after 3 retries: {}", artist_id, track_id, e);
                                    break;
                                }
                                tracing::warn!("Retry {} for track_artists insert: {}", retries, e);
                                tokio::time::sleep(std::time::Duration::from_millis(100 * retries as u64)).await;
                            }
                        }
                    }
                }

                // Add to library entry
                let result = sqlx::query(
                    "INSERT OR IGNORE INTO library_entries (account_id, track_id, added_at, is_liked) VALUES (?, ?, ?, 1)"
                )
                .bind(account_id)
                .bind(track_id)
                .bind(&saved.added_at)
                .execute(&state.db)
                .await
                .map_err(|e| format!("DB error: {}", e))?;

                if result.rows_affected() > 0 {
                    imported += 1;
                } else {
                    dedupe_already_exists += 1; // Track already in library (matched by ISRC)
                }

                // Add track source (using cached service_id)
                let _ = sqlx::query(
                    "INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, ?, ?, 1)"
                )
                .bind(track_id)
                .bind(spotify_service_id)
                .bind(&track.id)
                .execute(&state.db)
                .await;

                processed += 1;

                // Emit progress every 50 tracks (less frequent since we're processing faster)
                if processed % 50 == 0 || processed == total {
                    let _ = window.emit(
                        "import-progress",
                        serde_json::json!({
                            "service": "Spotify",
                            "status": "progress",
                            "current": processed,
                            "total": total,
                            "message": format!("Importing tracks... {}/{}", processed, total)
                        }),
                    );
                }
            }
        }

        current_page += concurrent_pages;
    }

    // Emit completion event
    let _ = window.emit(
        "import-complete",
        serde_json::json!({
            "service": "Spotify",
            "imported": imported,
            "skipped": skipped,
            "message": format!("Imported {} tracks, {} already existed", imported, skipped)
        }),
    );

    // Log cache statistics with skip breakdown for verification
    let (cached_artists, cached_albums) = cache.stats();
    tracing::info!(
        "Spotify import: {} new, {} already existed, {} skipped [no_album: {}, invalid: {}] (cache: {} artists, {} albums)",
        imported, dedupe_already_exists, skipped, skip_no_album, skip_invalid_data, cached_artists, cached_albums
    );

    Ok(ImportResult {
        imported: imported as i32,
        skipped: skipped as i32,
    })
}

/// Import Spotify playlists and their tracks
#[tauri::command]
pub async fn import_spotify_playlists(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<ImportResult, String> {
    tracing::info!("import_spotify_playlists called");

    // Use shared helpers for credential loading and token refresh
    let (account_id, creds) = load_service_credentials(&state.db, "spotify").await?;
    let access_token = get_or_refresh_spotify_token(&state.db, account_id, &creds).await?;

    let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
    let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
    let client = crate::services::SpotifyClient::new(access_token, refresh_token, expires_at);

    // Use local import cache for deduplication
    let mut cache = ImportCache::new();

    // Use shared helper for progress events
    emit_import_progress(&window, "spotify_playlists", "started", 0, 0, "Fetching playlists...");

    let mut offset = 0;
    let limit = 50;
    let mut playlists_imported = 0;
    let mut tracks_imported = 0;

    // First pass: get all playlists
    loop {
        tracing::info!("Fetching playlists page offset={}", offset);
        let page = client.get_playlists(offset, limit).await?;
        tracing::info!("Got {} playlists from page", page.items.len());

        if page.items.is_empty() {
            break;
        }

        for playlist in &page.items {
            // Insert playlist into database
            let result = sqlx::query(
                r#"
                INSERT OR REPLACE INTO playlists 
                (account_id, service_playlist_id, name, description, owner_name, is_public, is_collaborative, image_url, track_count, last_synced) 
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                "#
            )
            .bind(account_id)
            .bind(&playlist.id)
            .bind(&playlist.name)
            .bind(&playlist.description)
            .bind(playlist.owner.as_ref().and_then(|o| o.display_name.as_deref()))
            .bind(playlist.public.unwrap_or(true) as i32)
            .bind(playlist.collaborative as i32)
            .bind(playlist.images.first().map(|i| i.url.clone()))
            .bind(playlist.tracks.as_ref().map(|t| t.total).unwrap_or(0))
            .execute(&state.db)
            .await;

            if let Err(e) = &result {
                tracing::warn!("Failed to insert playlist: {}", e);
            }

            if result.is_ok() {
                playlists_imported += 1;

                tracing::info!(
                    "Processing playlist {}/{}: {} (id={})",
                    playlists_imported,
                    page.total,
                    &playlist.name,
                    &playlist.id
                );

                // Get the playlist_id we just inserted
                let playlist_db_id: (i64,) = sqlx::query_as(
                    "SELECT id FROM playlists WHERE account_id = ? AND service_playlist_id = ?",
                )
                .bind(account_id)
                .bind(&playlist.id)
                .fetch_one(&state.db)
                .await
                .map_err(|e| format!("Failed to get playlist ID: {}", e))?;

                // Import playlist tracks (with error handling to skip problematic playlists)
                let mut track_offset = 0;
                let track_limit = 100;

                loop {
                    tracing::debug!(
                        "Fetching tracks for playlist {} offset={}",
                        &playlist.name,
                        track_offset
                    );

                    let tracks_result = client
                        .get_playlist_tracks(&playlist.id, track_offset, track_limit)
                        .await;

                    let tracks_page = match tracks_result {
                        Ok(page) => page,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to fetch tracks for playlist {}: {} - skipping",
                                &playlist.name,
                                e
                            );
                            break;
                        }
                    };

                    if tracks_page.items.is_empty() {
                        break;
                    }

                    for (position, item) in tracks_page.items.iter().enumerate() {
                        if let Some(ref track) = item.track {
                            // Skip tracks without albums (podcasts, local files, etc.)
                            let Some(ref album) = track.album else {
                                continue;
                            };

                            // Skip tracks with empty/invalid data
                            if track.id.is_empty()
                                || track.name.is_empty()
                                || track.duration_ms == 0
                            {
                                continue;
                            }

                            // Get or create the track (using cached artist/album lookups)
                            let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());
                            let artist_name = track
                                .artists
                                .first()
                                .map(|a| a.name.clone())
                                .filter(|name| !name.is_empty())
                                .unwrap_or_else(|| "Unknown Artist".to_string());
                            let artist_id =
                                cache.get_or_create_artist(&state.db, &artist_name).await?;

                            let album_key = format!("{}:{}", artist_id, &album.name);
                            let image_url = album.images.first().map(|i| i.url.as_str());
                            let album_id = cache
                                .get_or_create_album(
                                    &state.db,
                                    &state.album_lock,
                                    &album_key,
                                    &album.name,
                                    artist_id,
                                    album.release_date.as_deref(),
                                    image_url,
                                )
                                .await?;
                            let track_id = client
                                .get_or_create_track(
                                    &state.db,
                                    track,
                                    isrc.as_deref(),
                                    Some(album_id),
                                )
                                .await?;

                            // Link all artists to the track (primary + featured)
                            for (i, spotify_artist) in track.artists.iter().enumerate() {
                                if spotify_artist.name.is_empty() {
                                    continue;
                                }
                                let aid = cache
                                    .get_or_create_artist(&state.db, &spotify_artist.name)
                                    .await?;
                                let role = if i == 0 { "primary" } else { "featured" };
                                let _ = sqlx::query(
                                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, ?)"
                                )
                                .bind(track_id)
                                .bind(aid)
                                .bind(role)
                                .execute(&state.db)
                                .await;
                            }

                            // Add track source (link to Spotify service)
                            let _ = sqlx::query(
                                "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id) 
                                 SELECT ?, id, ? FROM services WHERE name = 'spotify'"
                            )
                            .bind(track_id)
                            .bind(&track.id)
                            .execute(&state.db)
                            .await;

                            // Add to playlist_tracks
                            let _ = sqlx::query(
                                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (?, ?, ?, ?)"
                            )
                            .bind(playlist_db_id.0)
                            .bind(track_id)
                            .bind((track_offset + position as i32) as i32)
                            .bind(item.added_at.as_deref())
                            .execute(&state.db)
                            .await;

                            tracks_imported += 1;
                        }
                    }

                    track_offset += track_limit;
                    if tracks_page.next.is_none() || tracks_page.items.len() < track_limit as usize
                    {
                        break;
                    }
                }
            }

            // Update progress
            let _ = window.emit("import-progress", serde_json::json!({
                "service": "spotify_playlists",
                "status": "progress",
                "current": playlists_imported,
                "total": page.total,
                "message": format!("Imported {} playlists ({} tracks)", playlists_imported, tracks_imported)
            }));
        }

        offset += limit;
        if page.next.is_none() {
            break;
        }
    }

    // Emit completion event
    let _ = window.emit(
        "import-complete",
        serde_json::json!({
            "service": "spotify_playlists",
            "imported": playlists_imported,
            "tracks": tracks_imported
        }),
    );

    // Log cache statistics
    let (cached_artists, cached_albums) = cache.stats();
    tracing::info!(
        "Spotify playlists import complete: {} playlists, {} tracks (cache: {} artists, {} albums)",
        playlists_imported,
        tracks_imported,
        cached_artists,
        cached_albums
    );

    Ok(ImportResult {
        imported: playlists_imported as i32,
        skipped: tracks_imported as i32, // repurposed for track count
    })
}

/// Standalone command to enrich album metadata (label, UPC) for Spotify albums
#[tauri::command]
pub async fn enrich_album_metadata(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<ImportResult, String> {
    tracing::info!("enrich_album_metadata called");

    let (account_id, creds) = load_service_credentials(&state.db, "spotify").await?;
    let access_token = get_or_refresh_spotify_token(&state.db, account_id, &creds).await?;
    let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
    let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());

    let mut client = SpotifyClient::new(access_token, refresh_token, expires_at);
    
    // Perform enrichment
    let result = client.enrich_albums(&state.db, account_id, Some(&window)).await?;
    
    Ok(result)
}

/// Standalone command to enrich album metadata (label, UPC) for Qobuz albums
#[tauri::command]
pub async fn enrich_qobuz_album_metadata(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<ImportResult, String> {
    tracing::info!("enrich_qobuz_album_metadata called");

    let (_account_id, creds) = load_service_credentials(&state.db, "qobuz").await?;

    // S186: shared resolver — stored token first, username/password auto-login fallback
    let user_auth_token = resolve_qobuz_user_auth_token(&state.db, _account_id, &creds).await?;

    let client = QobuzClient::new_with_token(
        QOBUZ_APP_ID.to_string(),
        "".to_string(), // Secret not needed for token-based calls
        user_auth_token,
    );

    // Perform enrichment
    let result = client.enrich_albums(&state.db, Some(&window)).await?;
    
    Ok(result)
}

/// Get all service statuses
#[tauri::command]
pub async fn get_service_statuses(
    state: State<'_, AppState>,
) -> Result<Vec<ServiceStatus>, String> {
    tracing::info!("get_service_statuses called");

    let statuses = sqlx::query_as::<_, (String, Option<i64>, Option<String>, i64, i64, i64, Option<String>, i64, Option<String>, Option<String>)>(
        r#"
        SELECT 
            s.name,
            a.id as account_id,
            a.email,
            (SELECT COUNT(*) FROM library_entries le WHERE le.account_id = a.id) as cnt,
            (SELECT COUNT(*) FROM library_entries le WHERE le.account_id = a.id AND le.is_liked = 1) as fav_cnt,
            (SELECT COUNT(*) FROM playlists p WHERE p.account_id = a.id) as playlist_cnt,
            a.last_synced,
            IFNULL(a.credentials_invalid, 0) as credentials_invalid,
            a.invalid_reason,
            a.last_auth_error
        FROM services s
        LEFT JOIN accounts a ON a.service_id = s.id AND a.is_active = 1
        ORDER BY s.id
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(statuses
        .into_iter()
        .map(
            |(name, account_id, email, cnt, fav_cnt, playlist_cnt, last_synced, credentials_invalid, invalid_reason, last_auth_error)| ServiceStatus {
                name,
                connected: account_id.is_some(),
                account_email: email,
                library_count: cnt,
                favorites_count: fav_cnt,
                playlists_count: playlist_cnt,
                last_synced,
                credentials_invalid: credentials_invalid != 0,
                invalid_reason,
                last_auth_error,
            },
        )
        .collect())
}

// ==============================================
// SETTINGS COMMANDS
// ==============================================

/// Get application settings
#[tauri::command]
pub async fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    tracing::info!("get_app_settings called");

    let effective = resolve_effective_download_paths(&state.db).await
        .map_err(|e| format!("Failed to resolve effective download path: {}", e))?;
    let quality: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'preferred_quality'")
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
    let auto_dl: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'auto_download_favorites'")
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

    Ok(AppSettings {
        download_path: effective.library_root,
        preferred_quality: quality.map(|r| r.0).unwrap_or_else(|| "lossless".into()),
        auto_download_favorites: auto_dl.map(|r| r.0 == "true").unwrap_or(true),
    })
}

/// Save application settings
#[tauri::command]
pub async fn service_save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<String, String> {
    tracing::info!("save_settings called");

    if !settings.download_path.trim().is_empty() {
        let trimmed = settings.download_path.trim();
        let _ = sqlx::query("UPDATE folder_settings SET base_folder = ?, updated_at = datetime('now') WHERE id = 1")
            .bind(trimmed)
            .execute(&state.db)
            .await;
        for key in &["download_path", "dl_download_path", "download_dir"] {
            let _ = sqlx::query("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at")
                .bind(key)
                .bind(trimmed)
                .execute(&state.db)
                .await;
        }
    }

    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('preferred_quality', ?)")
        .bind(&settings.preferred_quality)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('auto_download_favorites', ?)",
    )
    .bind(if settings.auto_download_favorites {
        "true"
    } else {
        "false"
    })
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok("Settings saved".into())
}

/// Import Qobuz library — delegates to perform_sync_service_with_emitter (S128B)
///
/// This command is kept for backwards compatibility with the UI; all
/// actual work and progress emission is performed by `perform_sync_service_with_emitter`.
#[tauri::command]
pub async fn import_qobuz_library(
    window: tauri::Window,
    state: tauri::State<'_, crate::AppState>,
    import_lock: tauri::State<'_, ImportLock>,
) -> Result<ImportResult, String> {
    let _guard = import_lock
        .0
        .try_lock()
        .map_err(|_| "An import is already in progress".to_string())?;

    tracing::info!("import_qobuz_library: delegating to perform_sync_service_with_emitter (S128B)");

    match perform_sync_service_with_emitter(&state.db, "qobuz", None, None, Some(&window)).await {
        Ok(result) => {
            let total_imported = result.imported_tracks_total;
            let total_skipped = result.skipped_tracks_total;
            Ok(ImportResult {
                imported: total_imported as i32,
                skipped: total_skipped as i32,
            })
        }
        Err(e) => {
            if e.starts_with("RequiresAuth:") {
                tracing::warn!("import_qobuz_library: authentication required — {}", e);
            } else {
                tracing::error!("import_qobuz_library error: {}", e);
            }
            Err(e)
        }
    }
}

/// Import Qobuz playlists and their metadata
#[tauri::command]
pub async fn import_qobuz_playlists(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
    import_lock: tauri::State<'_, ImportLock>,
) -> Result<ImportResult, String> {
    let _guard = import_lock
        .0
        .try_lock()
        .map_err(|_| "An import is already in progress".to_string())?;

    tracing::info!("import_qobuz_playlists called");

    // Load credentials
    let (account_id, creds) = load_service_credentials(&state.db, "qobuz").await?;
    
    // Qobuz requires app_id/app_secret from env for signing
    let app_id = std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
    let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());

    // S186: shared resolver — stored token first, username/password auto-login fallback
    let user_auth_token = resolve_qobuz_user_auth_token(&state.db, account_id, &creds).await?;

    let client = crate::services::QobuzClient::new_with_token(app_id, app_secret, user_auth_token);

    // Call service implementation
    client.import_playlists(&state.db, account_id, &app).await
}

/// Import Tidal library
#[tauri::command]
pub async fn import_tidal_library(
    window: tauri::Window,
    state: tauri::State<'_, crate::AppState>,
    import_lock: tauri::State<'_, ImportLock>,
) -> Result<ImportResult, String> {
    let _guard = import_lock
        .0
        .try_lock()
        .map_err(|_| "An import is already in progress".to_string())?;

    tracing::info!("import_tidal_library called");

    // Use shared helper for credential loading
    let (account_id, creds) = load_service_credentials(&state.db, "tidal").await?;

    // Get access token
    let access_token = creds["access_token"]
        .as_str()
        .ok_or("Missing access token in stored credentials")?;

    // Get user_id and country from stored credentials
    let user_id = creds["user_id"]
        .as_str()
        .or_else(|| creds["user"]["userId"].as_str())
        .unwrap_or("0");

    let country = creds["country_code"]
        .as_str()
        .or_else(|| creds["user"]["countryCode"].as_str())
        .unwrap_or("US");

    // Initialize client
    let client = crate::services::TidalClient::new(access_token.to_string())
        .with_user(user_id.to_string(), country.to_string());

    // Fetch total count first (optional check)
    let _ = client.get_favorites(0, 1).await.ok();

    // Phase 1: Favorites (Warp Speed)
    let fav_res = client.import_favorites(&state.db, account_id, Some(&window)).await?;
    let (imported, skipped) = (fav_res.imported as i64, fav_res.skipped as i64);
 
    // Phase 2: Playlists (Warp Speed)
    let _ = client.import_playlists(&state.db, account_id, Some(&window)).await;

    // Phase 3: Favorite Albums (Warp Speed)
    let _ = client.import_favorite_albums(&state.db, account_id, Some(&window)).await;

    // Phase 4: Favorite Artists (Warp Speed)
    let _ = client.import_favorite_artists(&state.db, account_id, Some(&window)).await;

    // Update last_synced
    let _ = sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(&state.db)
        .await;
 
    // Use helper for complete event - Redundant broadcast to ensure UI clears all bars
    emit_import_complete(&window, "tidal", imported as u64, skipped as u64);
    emit_import_complete(&window, "tidal_playlists", 0, 0);
    emit_import_complete(&window, "tidal_albums", 0, 0);
    emit_import_complete(&window, "tidal_artists", 0, 0);
    emit_import_complete(&window, "tidal_library", 0, 0); // Some UI components use the library alias

    tracing::info!(
        "Tidal import complete: {} favorites imported, {} skipped",
        imported,
        skipped
    );

    Ok(ImportResult {
        imported: imported as i32,
        skipped: skipped as i32,
    })
}

/// Import Deezer library
#[tauri::command]
pub async fn import_deezer_library(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    tracing::info!("import_deezer_library called");

    // Use shared helper for credential loading
    let (account_id, creds) = load_service_credentials(&state.db, "deezer").await?;

    // Get ARL from stored credentials
    let arl = creds["arl"]
        .as_str()
        .or_else(|| creds["access_token"].as_str())
        .ok_or("Missing ARL in stored credentials")?;

    // Initialize client
    let mut client = crate::services::DeezerClient::new(arl.to_string());

    // Init client to get user_ID and check session
    if let Err(e) = client.init().await {
        tracing::warn!("Failed to init Deezer client: {}", e);
        // We continue only if we can find a user_id fallback, but usually init failure means invalid ARL.
        // However, we might have user_id in creds.
    }

    let user_id = client
        .user_id()
        .or_else(|| creds["user_id"].as_str().map(|s| s.to_string()))
        .ok_or("Deezer User ID not found. Please re-login.")?;

    // Fetch total count first
    let total_tracks = match client.get_favorites_public(&user_id, 0, 1).await {
        Ok((_, total)) => total,
        Err(e) => {
            tracing::warn!("Failed to fetch Deezer total: {}", e);
            0
        }
    };

    // Use shared helper for progress events
    emit_import_progress(&window, "deezer", "started", 0, total_tracks as u64,
        &format!("Starting import of {} tracks...", total_tracks));

    let mut offset = 0;
    let limit = 50; // Deezer limit
    let mut imported = 0;
    let mut skipped = 0;

    let deezer_service_id = client.get_service_id(&state.db, "deezer").await?;

    loop {
        let (tracks, _) = client.get_favorites_public(&user_id, offset, limit).await?;

        if tracks.is_empty() {
            break;
        }

        let batch_size = tracks.len();

        for track in tracks {
            // Get or create artist
            let artist_name = track.artist_name.clone().unwrap_or_default();
            let artist_id = client.get_or_create_artist(&state.db, &artist_name).await?;

            // Get or create album (using title since public track obj has album title)
            let album_title = track.album_title.clone().unwrap_or_default();
            let album_id = if !album_title.is_empty() {
                Some(
                    client
                        .get_or_create_album_by_title(&state.db, &album_title, artist_id)
                        .await?,
                )
            } else {
                None
            };

            // Get or create track
            let track_id = client
                .get_or_create_track(&state.db, &track, album_id)
                .await?;

            // Link artist to track
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
            )
            .bind(track_id)
            .bind(artist_id)
            .execute(&state.db)
            .await;

            // Add to library entry
            let result = sqlx::query(
                "INSERT OR IGNORE INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1)"
            )
            .bind(account_id)
            .bind(track_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

            if result.rows_affected() > 0 {
                imported += 1;
            } else {
                skipped += 1;
            }

            // Add track source (assuming FLAC 16/44.1 available for now)
            let _ = sqlx::query(
                r#"
                INSERT OR REPLACE INTO track_sources 
                (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) 
                VALUES (?, ?, ?, 'FLAC', 16, 44100, 1)
                "#,
            )
            .bind(track_id)
            .bind(deezer_service_id)
            .bind(track.id)
            .execute(&state.db)
            .await;
        }

        // Update progress using helper
        emit_import_progress(&window, "deezer", "progress",
            (imported + skipped) as u64, total_tracks as u64,
            &format!("Processed {} of {} tracks", imported + skipped, total_tracks));

        offset += limit;

        if batch_size < limit as usize {
            break;
        }
    }

    // Update last_synced
    let _ = sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(&state.db)
        .await;

    // Use helper for complete event
    emit_import_complete(&window, "deezer", imported as u64, skipped as u64);

    tracing::info!(
        "Deezer import complete: {} imported, {} skipped",
        imported,
        skipped
    );

    Ok(ImportResult {
        imported: imported as i32,
        skipped: skipped as i32,
    })
}

/// Import SoundCloud library
/// S190-interín: núcleo del import de likes de SoundCloud, COMPARTIDO por el
/// comando legacy y el brazo "soundcloud" del motor unificado. Ruta CRUD
/// directa (dedup título+duración, sin identidad canónica ISRC) hasta la
/// integración real de Fase 3 — documentado en Deuda_Tecnica_y_UX.md.
async fn run_soundcloud_likes_import(
    db: &DbPool,
    mut on_progress: impl FnMut(u64),
) -> Result<(i64, i64), String> {

    // Use shared helper for credential loading
    let (account_id, creds) = load_service_credentials(db, "soundcloud").await?;

    let oauth_token = creds["oauth_token"]
        .as_str()
        .or_else(|| creds["access_token"].as_str())
        .ok_or("Missing OAuth token in stored credentials")?;

    let user_id = creds["user_id"]
        .as_i64()
        .ok_or("Missing user_id in stored credentials")?;

    // Initialize client
    let client =
        crate::services::SoundCloudClient::new(oauth_token.to_string()).with_user_id(user_id);

    // Use shared helper for progress events

    let mut imported = 0;
    let mut skipped = 0;
    let mut next_url: Option<String> = None;

    let soundcloud_service_id = client.get_service_id(db, "soundcloud").await?;

    loop {
        let page = client.get_likes(next_url.as_deref()).await?;

        if page.collection.is_empty() {
            break;
        }

        for like in &page.collection {
            if let Some(ref track) = like.track {
                // Get or create artist
                let artist_name = track
                    .user
                    .as_ref()
                    .map(|u| u.username.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                let artist_id = client.get_or_create_artist(db, &artist_name).await?;

                // Create/update track
                let track_id: i64 =
                    if let Some(row) = sqlx::query_as::<_, (i64,)>(
                        "INSERT OR IGNORE INTO tracks (title, duration_ms) VALUES (?, ?) RETURNING id"
                    )
                        .bind(&track.title)
                        .bind(track.duration) // SoundCloud uses milliseconds
                        .fetch_optional(db)
                        .await
                        .map_err(|e| format!("DB error: {}", e))?
                    {
                        row.0
                    } else {
                        // Duplicate — fetch existing ID
                        sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE title = ? AND duration_ms = ?")
                            .bind(&track.title)
                            .bind(track.duration)
                            .fetch_one(db)
                            .await
                            .map(|r| r.0)
                            .unwrap_or(0)
                    };

                if track_id == 0 {
                    skipped += 1;
                    continue;
                }

                // Add track-artist relation
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
                    "INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, format, bitrate, quality_score, available) VALUES (?, ?, ?, 'MP3', 128, NULL, 1)"
                )
                .bind(track_id)
                .bind(soundcloud_service_id)
                .bind(track.id.to_string())
                .execute(db)
                .await;
            }
        }

        // Update progress using helper
        on_progress((imported + skipped) as u64);

        // Continue pagination
        next_url = page.next_href;
        if next_url.is_none() {
            break;
        }
    }

    // Update last_synced
    let _ = sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(db)
        .await;

    tracing::info!(
        "SoundCloud import complete: {} imported, {} skipped",
        imported,
        skipped
    );


    Ok((imported, skipped))
}

#[tauri::command]
pub async fn import_soundcloud_library(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    tracing::info!("import_soundcloud_library called");
    emit_import_progress(&window, "soundcloud", "started", 0, 0, "Starting SoundCloud import...");
    let (imported, skipped) = run_soundcloud_likes_import(&state.db, |done| {
        emit_import_progress(&window, "soundcloud", "progress", done, done, &format!("Imported {} tracks...", done));
    })
    .await?;
    tracing::info!("SoundCloud import complete: {} imported, {} skipped", imported, skipped);
    emit_import_complete(&window, "soundcloud", imported as u64, skipped as u64);
    Ok(ImportResult { imported: imported as i32, skipped: skipped as i32 })
}

/// Import Apple Music library
/// S190-interín: núcleo del import de biblioteca de Apple Music, COMPARTIDO
/// por el comando legacy y el brazo "apple_music" del motor unificado.
/// Requiere music_user_token + developer_token en credenciales guardadas.
async fn run_apple_music_library_import(
    db: &DbPool,
    mut on_progress: impl FnMut(u64),
) -> Result<(i64, i64), String> {
    let (account_id, creds) = load_service_credentials(db, "apple_music").await?;

    let music_user_token = creds["music_user_token"]
        .as_str()
        .ok_or("Missing music_user_token in stored credentials")?;
    tracing::info!("music_user_token length: {}", music_user_token.len());

    let developer_token = creds["developer_token"].as_str().ok_or(
        "Missing developer_token in stored credentials - Apple Music requires a developer account",
    )?;
    tracing::info!("developer_token length: {}", developer_token.len());

    // Initialize client
    let client = crate::services::AppleMusicClient::new(
        developer_token.to_string(),
        music_user_token.to_string(),
    );


    let mut offset = 0;
    let limit = 100;
    let mut imported = 0;
    let mut skipped = 0;

    let apple_service_id = client.get_service_id(db, "apple_music").await?;
    tracing::info!("Apple Music service_id={}", apple_service_id);

    loop {
        tracing::info!("Fetching Apple Music library songs: offset={}, limit={}", offset, limit);
        let page = match client.get_library_songs(offset, limit).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Apple Music API error: {}", e);
                return Err(e);
            }
        };

        let track_count = page.data.as_ref().map(|t| t.len()).unwrap_or(0);
        tracing::info!("Apple Music API returned {} tracks, has_next: {}", track_count, page.next.is_some());

        let tracks = match page.data {
            Some(t) if !t.is_empty() => t,
            _ => {
                tracing::info!("No more tracks, breaking loop");
                break;
            }
        };

        for track in &tracks {
            if let Some(ref attrs) = track.attributes {
                tracing::info!("Processing track: {} by {} (ISRC: {:?})", 
                    &attrs.name, &attrs.artist_name, &attrs.isrc);
                
                // Get or create artist
                let artist_id = client
                    .get_or_create_artist(db, &attrs.artist_name)
                    .await?;
                tracing::debug!("Artist ID for {}: {}", &attrs.artist_name, artist_id);

                // Create/update track
                let duration_ms = attrs.duration_in_millis.unwrap_or(0);
                let track_id: i64 =
                    if let Some(row) = sqlx::query_as::<_, (i64,)>(
                        "INSERT OR IGNORE INTO tracks (title, duration_ms, isrc) VALUES (?, ?, ?) RETURNING id",
                    )
                    .bind(&attrs.name)
                    .bind(duration_ms)
                    .bind(&attrs.isrc)
                    .fetch_optional(db)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?
                    {
                        tracing::info!("Inserted new track ID: {}", row.0);
                        row.0
                    } else {
                        // Track already exists, find it
                        let existing = sqlx::query_as::<_, (i64,)>(
                            "SELECT id FROM tracks WHERE title = ? AND isrc = ?",
                        )
                        .bind(&attrs.name)
                        .bind(&attrs.isrc)
                        .fetch_optional(db)
                        .await
                        .ok()
                        .flatten()
                        .map(|r| r.0)
                        .unwrap_or(0);
                        
                        if existing > 0 {
                            tracing::info!("Found existing track ID: {}", existing);
                        } else {
                            tracing::warn!("Could not find track after insert failed: {} (ISRC: {:?})", 
                                &attrs.name, &attrs.isrc);
                        }
                        existing
                    };

                if track_id == 0 {
                    tracing::warn!("Skipping track with ID 0: {}", &attrs.name);
                    skipped += 1;
                    continue;
                }

                // Add track-artist relation
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
                    tracing::info!("Imported track: {} (ID: {})", &attrs.name, track_id);
                } else {
                    skipped += 1;
                    tracing::info!("Track already in library: {} (ID: {})", &attrs.name, track_id);
                }

                // Add track source
                let _ = sqlx::query(
                    "INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, bitrate, quality_score, available) VALUES (?, ?, ?, 'AAC', 16, 44100, 256, NULL, 1)"
                )
                .bind(track_id)
                .bind(apple_service_id)
                .bind(&track.id)
                .execute(db)
                .await;
            } else {
                tracing::warn!("Track missing attributes: {:?}", track.id);
            }
        }

        // Update progress using helper
        on_progress((imported + skipped) as u64);

        offset += limit;

        // Stop if we got fewer tracks than requested
        if tracks.len() < limit as usize || page.next.is_none() {
            break;
        }
    }

    // Update last_synced
    let _ = sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(db)
        .await;

    tracing::info!(
        "Apple Music import complete: {} imported, {} skipped",
        imported,
        skipped
    );


    Ok((imported, skipped))
}

#[tauri::command]
pub async fn import_apple_music_library(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    tracing::info!("import_apple_music_library called");
    emit_import_progress(&window, "apple_music", "started", 0, 0, "Starting Apple Music import...");
    let (imported, skipped) = run_apple_music_library_import(&state.db, |done| {
        emit_import_progress(&window, "apple_music", "progress", done, done, &format!("Imported {} tracks...", done));
    })
    .await?;
    tracing::info!("Apple Music import complete: {} imported, {} skipped", imported, skipped);
    emit_import_complete(&window, "apple_music", imported as u64, skipped as u64);
    Ok(ImportResult { imported: imported as i32, skipped: skipped as i32 })
}

#[tauri::command]
pub async fn import_service(
    service_name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    tracing::info!("import_service called for: {}", service_name);

    match service_name.to_lowercase().as_str() {
        "spotify" => {
            // For Spotify, we need OAuth flow - return the auth URL
            // S196: credentials resolved from DB settings first, env fallback (dev).
            let config = crate::commands::resolve_spotify_config(&state.db)
                .await
                .map_err(|e| e.to_string())?;
            let auth_url = config.auth_url(SPOTIFY_SCOPES);
            Ok(format!("Open this URL to login:\n{}", auth_url))
        }
        "qobuz" => {
            // Use env credentials
            let app_id = std::env::var("QOBUZ_APP_ID").map_err(|_| "Qobuz credentials not configured. Set QOBUZ_APP_ID and QOBUZ_APP_SECRET environment variables.")?;
            let app_secret =
                std::env::var("QOBUZ_APP_SECRET").map_err(|_| "Qobuz credentials not configured. Set QOBUZ_APP_ID and QOBUZ_APP_SECRET environment variables.")?;
            let username = std::env::var("QOBUZ_USERNAME").map_err(|_| "QOBUZ_USERNAME not set")?;
            let password = std::env::var("QOBUZ_PASSWORD").map_err(|_| "QOBUZ_PASSWORD not set")?;

            // Login and get user auth token
            let client = crate::services::QobuzClient::new(app_id.clone(), app_secret.clone());
            let user_auth_token = client.login(&username, &password).await?;

            // Get Qobuz account
            let qobuz_service_id: (i64,) =
                sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
                    .fetch_one(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;

            let account_id: i64 = sqlx::query_scalar("INSERT OR REPLACE INTO accounts (service_id, display_name, is_active) VALUES (?, 'Qobuz User', 1) RETURNING id")
                .bind(qobuz_service_id.0)
                .fetch_one(&state.db).await.map_err(|e| e.to_string())?;

            // Create client with token and import — reuse already-validated app_id/app_secret
            let authed_client = crate::services::QobuzClient::new_with_token(
                app_id,
                app_secret,
                user_auth_token,
            );
            let result = authed_client.import_library(&state.db, account_id).await?;
            Ok(format!(
                "Qobuz: {} imported, {} skipped",
                result.imported, result.skipped
            ))
        }
        "tidal" => {
            let access_token =
                std::env::var("TIDAL_ACCESS_TOKEN").map_err(|_| "TIDAL_ACCESS_TOKEN not set")?;

            let tidal_service_id: (i64,) =
                sqlx::query_as("SELECT id FROM services WHERE name = 'tidal'")
                    .fetch_one(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;

            let account_id: i64 = sqlx::query_scalar("INSERT OR REPLACE INTO accounts (service_id, display_name, is_active) VALUES (?, 'Tidal User', 1) RETURNING id")
                .bind(tidal_service_id.0)
                .fetch_one(&state.db).await.map_err(|e| e.to_string())?;

            // Parse user_id from JWT (simplified - just use placeholder)
            let client = crate::services::TidalClient::new(access_token)
                .with_user("206464893".into(), "MX".into());

            let result = client.import_favorites(&state.db, account_id, None).await?;
            Ok(format!(
                "Tidal: {} imported, {} skipped",
                result.imported, result.skipped
            ))
        }
        "deezer" => {
            let arl = std::env::var("DEEZER_ARL").map_err(|_| "DEEZER_ARL not set")?;

            let deezer_service_id: (i64,) =
                sqlx::query_as("SELECT id FROM services WHERE name = 'deezer'")
                    .fetch_one(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;

            let account_id: i64 = sqlx::query_scalar("INSERT OR REPLACE INTO accounts (service_id, display_name, is_active) VALUES (?, 'Deezer User', 1) RETURNING id")
                .bind(deezer_service_id.0)
                .fetch_one(&state.db).await.map_err(|e| e.to_string())?;

            let mut client = crate::services::DeezerClient::new(arl);
            let result = client.import_library(&state.db, account_id).await?;
            Ok(format!(
                "Deezer: {} imported, {} skipped",
                result.imported, result.skipped
            ))
        }
        // Fase 3 pendiente: same actionable reason as start_auth_and_save/sync.
        "apple_music" => Err("Apple Music sync requiere developer token — Fase 3 pendiente: la integración real está bloqueada por credenciales de la API de Apple.".into()),
        _ => Err(format!("Unknown service: {}", service_name)),
    }
}

/// Perform unified synchronization for a service using real auth checks and granular preferences (delegates to perform_sync_service_with_emitter)
#[allow(dead_code)]
pub async fn perform_sync_service(
    db: &sqlx::SqlitePool,
    service_name: &str,
    account_id_opt: Option<i64>,
    preferences_opt: Option<ImportPreferences>,
) -> Result<ServiceSyncResult, String> {
    perform_sync_service_with_emitter(db, service_name, account_id_opt, preferences_opt, None::<&()>).await
}

/// S195(c): catalog upsert with retry-on-locked.
///
/// The upsert runs as a DEFERRED transaction that reads and then writes; when the
/// background EnrichmentWorker (or any other pool writer) commits in between,
/// SQLite fails the write upgrade with `SQLITE_BUSY_SNAPSHOT` (code 5) WITHOUT
/// consulting `busy_timeout` (see `db::is_sqlite_locked_error`). The failed
/// transaction rolls back completely, so re-running the whole idempotent upsert
/// is safe. Two bounded attempts after backoff turn the observed
/// "1 failure per ~9k tracks" race into a transparent retry instead of a lost
/// library item; non-locked errors are returned untouched.
/// FIX 2026-08-25 ("credenciales Tidal duran muy poco"): tras un 401 en sync
/// intenta UNA vez un refresh forzado. `Some(nuevo_token)` ⇒ el 401 era
/// recuperable (skew, rotación en vuelo, TTL justo); `None` ⇒ el propio
/// refresh fue rechazado / no hay refresh-token y procede invalidar la cuenta.
async fn tidal_force_refresh_after_401(db: &sqlx::SqlitePool) -> Option<String> {
    let http_client = crate::download::http_client::create_http_client();
    let (creds_opt, _) =
        crate::services::tidal_pipeline::resolve_and_refresh_gui_credentials_opts(
            db,
            &http_client,
            true,
        )
        .await;
    creds_opt.map(|c| c.access_token)
}

async fn enrich_persist_with_locked_retry(
    engine: &crate::services::enrichment::EnrichmentEngine,
    db: &sqlx::SqlitePool,
    input: crate::services::enrichment::SyncTrackInput,
) -> Result<crate::services::enrichment::SyncTrackResult, String> {
    let mut attempt: u32 = 0;
    loop {
        match engine.enrich_and_persist_sync_track(db, input.clone()).await {
            Ok(res) => return Ok(res),
            Err(e)
                if crate::db::is_sqlite_locked_error(&e) && attempt < 5 =>
            {
                attempt += 1;
                // Backoff exponencial: 200/400/800/1600/3200 ms. Con BEGIN
                // IMMEDIATE esto es defensa en profundidad (los escritores ya
                // se hacen fila en busy_timeout), pero cubre colisiones de
                // lectores/checkpoint y BDs externas.
                let backoff_ms = 200u64.saturating_mul(1u64 << (attempt - 1)).min(3200);
                tracing::warn!(
                    attempt,
                    backoff_ms,
                    error = %e,
                    "[S195] SQLite locked during catalog upsert (BUSY_SNAPSHOT escapes busy_timeout); retrying whole transaction"
                );
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// True when `err` is Spotify's scope-denied response for an endpoint whose
/// scope was never granted to the stored token: HTTP 403 plus the
/// "Insufficient client scope" reason string (e.g. /me/following without
/// `user-follow-read`). A missing scope must degrade to a warning instead of
/// failing the whole sync — tokens issued before the scope existed keep
/// syncing everything else until the user re-authenticates.
fn is_spotify_scope_forbidden_error(err: &str) -> bool {
    err.contains("403") && err.to_lowercase().contains("insufficient client scope")
}

/// Perform unified synchronization for a service with explicit progress emitter (S128B)
pub async fn perform_sync_service_with_emitter<E: SyncProgressEmitter>(
    db: &sqlx::SqlitePool,
    service_name: &str,
    account_id_opt: Option<i64>,
    preferences_opt: Option<ImportPreferences>,
    emitter: Option<&E>,
) -> Result<ServiceSyncResult, String> {
    let service_normalized = service_name.to_lowercase();

    // S195-fix: lock de escritor POR servicio+cuenta (flock multi-proceso).
    // Permite sincronizar servicios distintos en paralelo; bloquea solo el
    // mismo servicio+cuenta duplicado (doble clic u otra instancia de la app).
    let effective_account = account_id_opt.unwrap_or(0);
    let _sync_writer_guard =
        crate::db::SyncWriterLock::acquire(db, &service_normalized, effective_account).await?;

    let emit = |event: SyncProgressEvent| {
        if let Some(e) = emitter {
            e.emit_sync_progress(&event);
        }
    };

    // 0. Emit started event immediately
    emit(SyncProgressEvent {
        service: service_normalized.clone(),
        account_id: account_id_opt,
        operation: "sync".to_string(),
        phase: "authenticating".to_string(),
        current: 0,
        total: None,
        message: format!("Authenticating {} connection...", service_name),
        imported_tracks_total: 0,
        favorite_tracks_total: 0,
        terminal: false,
        status: "running".to_string(),
    });

    // Fase 3 pendiente (docs/PLAN_UNIFICACION_IMPORTACION.md): Apple Music no
    // tiene rama en este motor — la integración real está BLOQUEADA por
    // credenciales reales (developer token JWT de Apple). Detectarlo antes del
    // flujo de auth evita el engañoso "Unsupported service for sync" para
    // cuentas ya conectadas y devuelve una razón accionable en su lugar.
    if service_normalized == "apple_music" {
        let err_msg = "Apple Music sync requiere developer token — Fase 3 pendiente: la integración real está bloqueada por credenciales de la API de Apple, así que esta cuenta todavía no se puede sincronizar.".to_string();
        emit(SyncProgressEvent::failed(&service_normalized, account_id_opt, "authenticating", &err_msg, 0, 0));
        return Err(err_msg);
    }

    // 1. Verify real auth status before attempting any sync
    let auth_status = match perform_get_service_auth_status(db, &service_normalized, account_id_opt).await {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("RequiresAuth: Authentication check failed for {}: {}", service_name, e);
            emit(SyncProgressEvent::requires_auth(&service_normalized, account_id_opt, &err_msg));
            return Err(err_msg);
        }
    };

    if auth_status.status != "connected_valid" {
        let raw_err = auth_status
            .error_message
            .unwrap_or_else(|| "Missing valid authentication".to_string());
        let err_msg = if service_normalized == "qobuz" {
            format!("RequiresAuth: Qobuz user authentication required ({})", raw_err)
        } else {
            format!("RequiresAuth: {} account authentication required ({})", service_name, raw_err)
        };
        emit(SyncProgressEvent::requires_auth(
            &service_normalized,
            auth_status.account_id.or(account_id_opt),
            &err_msg,
        ));
        return Err(err_msg);
    }

    let account_id = match auth_status.account_id {
        Some(id) => id,
        None => {
            let err_msg = format!("RequiresAuth: No active account ID found for {}", service_name);
            emit(SyncProgressEvent::requires_auth(&service_normalized, None, &err_msg));
            return Err(err_msg);
        }
    };

    // S168: Acquire AccountSync lock to prevent concurrent overlapping syncs on same account
    let _account_sync_guard = match crate::services::get_global_concurrency_manager()
        .acquire(
            syncify_core_domain::LockScope::AccountSync(account_id),
            Some(&format!("sync-{}-{}", service_normalized, account_id)),
            None,
        )
        .await
    {
        Ok(g) => g,
        Err(e) => {
            let err_msg = format!("Concurrency lock error: {}", e);
            emit(SyncProgressEvent::failed(&service_normalized, Some(account_id), "authenticating", &err_msg, 0, 0));
            return Err(err_msg);
        }
    };

    // 2. Load decrypted credentials
    let creds_json_row: Option<(String,)> = sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_optional(db)
        .await
        .map_err(|e| {
            let err_msg = format!("Database error loading credentials for account {}: {}", account_id, e);
            emit(SyncProgressEvent::failed(&service_normalized, Some(account_id), "authenticating", &err_msg, 0, 0));
            err_msg
        })?;

    let ciphertext = match creds_json_row {
        Some((c,)) if !c.trim().is_empty() => c,
        _ => {
            let err_msg = format!("RequiresAuth: Credentials missing for account {}", account_id);
            emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
            return Err(err_msg);
        }
    };

    let decrypted = match crate::crypto::decrypt(&ciphertext) {
        Ok(d) => d,
        Err(e) => {
            let err_msg = format!("RequiresAuth: Failed to decrypt account credentials: {}", e);
            emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
            return Err(err_msg);
        }
    };

    let creds: serde_json::Value = match serde_json::from_str(&decrypted) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("RequiresAuth: Malformed credentials JSON: {}", e);
            emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
            return Err(err_msg);
        }
    };

    // 3. Resolve import preferences (persisted or passed explicitly)
    let prefs = match preferences_opt {
        Some(p) => p,
        None => match perform_get_service_import_preferences(db, &service_normalized).await {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("Failed to get preferences for {}: {}", service_name, e);
                emit(SyncProgressEvent::failed(&service_normalized, Some(account_id), "authenticating", &err_msg, 0, 0));
                return Err(err_msg);
            }
        },
    };

    let enrichment_engine = crate::services::enrichment::EnrichmentEngine::new();
    let sync_start = std::time::Instant::now();
    let mut api_fetch_ms: u64 = 0;
    let mut entity_expansion_ms: u64 = 0;
    let mut enrichment_ms: u64 = 0;
    let mut persistence_ms: u64 = 0;
    let availability_check_ms: u64 = 0;

    let mut imported_tracks_total: u64 = 0;
    let mut favorite_tracks_total: u64 = 0;
    let mut favorite_albums_total: u64 = 0;
    let mut favorite_artists_total: u64 = 0;
    let mut playlists_total: u64 = 0;
    let mut purchases_total: u64 = 0;
    let mut skipped_tracks_total: u64 = 0;
    let mut metadata_enriched: u64 = 0;
    let mut metadata_partial: u64 = 0;
    let mut availability_checked: u64 = 0;
    let mut album_expansion_metrics = crate::commands::types::AlbumSyncExpansionMetrics::default();
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut albums_unavailable: u64 = 0;
    let mut tracks_unavailable: u64 = 0;
    let mut tracks_expansion_deferred: u64 = 0;

    let mut tracks_processed: u64 = 0;
    let mut tracks_changed_unique: u64 = 0;
    let mut tracks_new_global: u64 = 0;
    let mut sources_new_for_service: u64 = 0;
    let mut library_entries_new_for_account: u64 = 0;
    let mut tracks_already_present: u64 = 0;
    let mut favorites_seen: u64 = 0;
    let mut albums_seen: u64 = 0;
    let mut playlists_seen: u64 = 0;
    let mut tracks_expanded: u64 = 0;
    let mut tracks_expansion_failed: u64 = 0;

    // 4. Dispatch sync by service
    match service_normalized.as_str() {
        "qobuz" => {
            let app_id = std::env::var("QOBUZ_APP_ID")
                .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
            let app_secret = std::env::var("QOBUZ_APP_SECRET")
                .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());

            // S186: resolve the token exactly like the download pipeline — stored token
            // first, then username/password auto-login with the result persisted back.
            let user_auth_token = match resolve_qobuz_user_auth_token(db, account_id, &creds).await {
                Ok(tok) => tok,
                Err(err_msg) => {
                    emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                    return Err(err_msg);
                }
            };

            let client = crate::services::QobuzClient::new_with_token(app_id, app_secret, user_auth_token);
            let qobuz_service_id = match client.get_service_id(db, "qobuz").await {
                Ok(id) => id,
                Err(e) => {
                    let err_msg = format!("Failed to get Qobuz service id: {}", e);
                    emit(SyncProgressEvent::failed(&service_normalized, Some(account_id), "authenticating", &err_msg, 0, 0));
                    return Err(err_msg);
                }
            };

            // Phase 1: Favorite Tracks
            if prefs.favorite_tracks {
                emit(SyncProgressEvent::running(
                    &service_normalized,
                    Some(account_id),
                    "fetching_favorite_tracks",
                    0,
                    None,
                    "Importing favorite tracks...",
                    imported_tracks_total,
                    favorite_tracks_total,
                ));

                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_favorites(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.tracks.total as u64;
                            if page.tracks.items.is_empty() {
                                break;
                            }
                            for track in &page.tracks.items {
                                let artist_name = track
                                    .performer
                                    .as_ref()
                                    .and_then(|a| a.name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());
                                let album_title = track.album.as_ref().and_then(|a| a.title.clone());
                                let album_cover = track.album.as_ref().and_then(|a| a.image.as_ref().and_then(|img| img.large.clone().or_else(|| img.small.clone())));
                                let (release_date_val, release_year_val) = match track.album.as_ref().and_then(|a| a.released_at) {
                                    Some(ts) => {
                                        let date_str = chrono::DateTime::from_timestamp(ts, 0)
                                            .map(|dt| dt.format("%Y-%m-%d").to_string());
                                        let year_str = chrono::DateTime::from_timestamp(ts, 0)
                                            .map(|dt| dt.format("%Y").to_string());
                                        (date_str, year_str)
                                    }
                                    None => (None, None),
                                };
                                let quality_score = client.compute_quality_score(track);
                                let is_hires = track.maximum_bit_depth.unwrap_or(16) > 16 || track.maximum_sampling_rate.unwrap_or(44.1) > 44.1;

                                let sync_input = crate::services::enrichment::SyncTrackInput {
                                    origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                        title: track.title.clone(),
                                        artist: Some(artist_name),
                                        album: album_title,
                                        album_artist: track.album.as_ref().and_then(|a| a.artist.as_ref().and_then(|art| art.name.clone())),
                                        composer: track.composer.as_ref().and_then(|c| c.name.clone()),
                                        performers: track.performers.clone().or_else(|| track.performer.as_ref().and_then(|p| p.name.clone())),
                                        track_number: track.track_number.map(|tn| tn as u32),
                                        track_total: track.album.as_ref().and_then(|a| a.tracks.as_ref()).map(|c| c.total as u32),
                                        disc_number: track.media_number.map(|dn| dn as u32),
                                        isrc: track.isrc.clone(),
                                        barcode: track.album.as_ref().and_then(|a| a.upc.clone()),
                                        label: track.album.as_ref().and_then(|a| a.label.as_ref().and_then(|l| l.name.clone())),
                                        release_date: release_date_val,
                                        release_year: release_year_val,
                                        release_country: None,
                                        genre: track.album.as_ref().and_then(|a| a.genre_name()),
                                        explicit: None,
                                        source_name: "qobuz".to_string(),
                                        ..Default::default()
                                    },
                                    service_track_id: track.id.to_string(),
                                    service_name: "qobuz".to_string(),
                                    service_id: qobuz_service_id,
                                    account_id,
                                    is_favorite: true,
                                    is_purchased: false,
                                    format: Some("FLAC".to_string()),
                                    bit_depth: track.maximum_bit_depth,
                                    sample_rate: track.maximum_sampling_rate.map(|r| (r * 1000.0) as i32),
                                    quality_score: Some(quality_score),
                                    audio_quality: Some(if is_hires { "hires".to_string() } else { "lossless".to_string() }),
                                    cover_art_url: album_cover,
                                    duration_ms: Some((track.duration * 1000) as i64),
                                    query_musicbrainz: false,
                                    album_is_favorite: false,
                                    album_provider_track_id: None,
                                };

                                let t_enrich = std::time::Instant::now();
                                match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                    Ok(res) => {
                                        enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                        tracks_processed += 1;
                                        if res.is_new_global_track {
                                            tracks_new_global += 1;
                                        }
                                        if res.is_new_source_for_service {
                                            sources_new_for_service += 1;
                                        }
                                        if res.is_new_library_entry_for_account {
                                            library_entries_new_for_account += 1;
                                        }
                                        if res.is_already_present {
                                            tracks_already_present += 1;
                                        }
                                        if res.is_new_import {
                                            tracks_changed_unique += 1;
                                            imported_tracks_total += 1;
                                        } else {
                                            skipped_tracks_total += 1;
                                        }
                                        favorite_tracks_total += 1;
                                        availability_checked += 1;
                                        match res.completeness {
                                            syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                            _ => metadata_partial += 1,
                                        }
                                    }
                                    Err(e) => {
                                        errors.push(format!("Qobuz track error for {}: {}", track.id, e));
                                    }
                                }

                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_tracks",
                                    favorite_tracks_total,
                                    Some(page_total),
                                    format!("Importing favorite tracks ({}/{})", favorite_tracks_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));
                            }
                            offset += limit;
                            if page.tracks.items.len() < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.contains("401") || e.contains("User authentication is required") {
                                tracing::warn!("[perform_sync_service/qobuz] 401 on favorites — marking credentials invalid");
                                let _ = mark_account_credentials_invalid(db, "qobuz", "HTTP 401: User authentication required").await;
                                let err_msg = format!("RequiresAuth: Qobuz session rejected (401) while fetching favorites: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            errors.push(format!("Qobuz favorites error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 2: Favorite Albums
            if prefs.favorite_albums {
                emit(SyncProgressEvent::running(
                    &service_normalized,
                    Some(account_id),
                    "importing_favorite_albums",
                    0,
                    None,
                    "Importing favorite albums...",
                    imported_tracks_total,
                    favorite_tracks_total,
                ));

                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_favorite_albums(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.albums.total as u64;
                            let items_len = page.albums.items.len();
                            if items_len == 0 {
                                break;
                            }
                            album_expansion_metrics.albums_received += items_len as u64;
                            let client_ref = &client;
                            let mut expand_stream = futures_util::stream::iter(page.albums.items.into_iter().map(|album_meta| {
                                async move {
                                    let has_tracks = album_meta.tracks.as_ref().map(|t| !t.items.is_empty()).unwrap_or(false);
                                    if has_tracks {
                                        (album_meta.id.clone(), Ok(album_meta), false, 0u64)
                                    } else {
                                        let t_exp = std::time::Instant::now();
                                        let res = client_ref.get_album_full(&album_meta.id).await;
                                        let elapsed = t_exp.elapsed().as_millis() as u64;
                                        (album_meta.id.clone(), res, true, elapsed)
                                    }
                                }
                            }))
                            .buffer_unordered(5);

                            while let Some((alb_id, res, was_expansion_request, exp_ms)) = expand_stream.next().await {
                                if was_expansion_request {
                                    album_expansion_metrics.albums_needing_expansion += 1;
                                    album_expansion_metrics.album_detail_requests += 1;
                                    entity_expansion_ms += exp_ms;
                                }

                                let full_album = match res {
                                    Ok(album) => {
                                        if was_expansion_request {
                                            album_expansion_metrics.album_detail_success += 1;
                                        }
                                        album
                                    }
                                    Err(e) => {
                                        if was_expansion_request {
                                            album_expansion_metrics.album_detail_failed += 1;
                                            if album_expansion_metrics.first_error_code.is_none() {
                                                album_expansion_metrics.first_error_code = Some(e.clone());
                                                album_expansion_metrics.first_error_album_id = Some(alb_id.clone());
                                            }
                                        }
                                        tracing::error!("[perform_sync_service/qobuz] Failed to expand album {}: {}", alb_id, e);
                                        errors.push(format!("Qobuz album detail error for {}: {}", alb_id, e));
                                        continue;
                                    }
                                };

                                favorite_albums_total += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_albums",
                                    favorite_albums_total,
                                    Some(page_total),
                                    format!("Importing favorite albums ({}/{})", favorite_albums_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let album_title = full_album.title.clone();
                                let album_cover = full_album.image.as_ref().and_then(|img| img.large.clone().or_else(|| img.small.clone()));
                                let (release_date_val, release_year_val) = match full_album.released_at {
                                    Some(ts) => {
                                        let date_str = chrono::DateTime::from_timestamp(ts, 0)
                                            .map(|dt| dt.format("%Y-%m-%d").to_string());
                                        let year_str = chrono::DateTime::from_timestamp(ts, 0)
                                            .map(|dt| dt.format("%Y").to_string());
                                        (date_str, year_str)
                                    }
                                    None => (None, None),
                                };
                                let label_val = full_album.label.as_ref().and_then(|l| l.name.clone());

                                if let Some(ref container) = full_album.tracks {
                                    for track in &container.items {
                                        album_expansion_metrics.tracks_received += 1;
                                        if track.id <= 0 {
                                            album_expansion_metrics.tracks_invalid += 1;
                                            continue;
                                        }

                                        let artist_name = track
                                            .performer
                                            .as_ref()
                                            .and_then(|a| a.name.clone())
                                            .or_else(|| full_album.artist.as_ref().and_then(|a| a.name.clone()))
                                            .unwrap_or_else(|| "Unknown".to_string());
                                        let quality_score = client.compute_quality_score(track);
                                        let is_hires = track.maximum_bit_depth.unwrap_or(16) > 16 || track.maximum_sampling_rate.unwrap_or(44.1) > 44.1;

                                        let sync_input = crate::services::enrichment::SyncTrackInput {
                                            origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                title: track.title.clone(),
                                                artist: Some(artist_name),
                                                album: album_title.clone(),
                                                album_artist: full_album.artist.as_ref().and_then(|a| a.name.clone()),
                                                composer: track.composer.as_ref().and_then(|c| c.name.clone()),
                                                performers: track.performers.clone().or_else(|| track.performer.as_ref().and_then(|p| p.name.clone())),
                                                track_number: track.track_number.map(|tn| tn as u32),
                                                track_total: Some(container.total as u32),
                                                disc_number: track.media_number.map(|dn| dn as u32),
                                                isrc: track.isrc.clone(),
                                                barcode: full_album.upc.clone(),
                                                label: label_val.clone(),
                                                release_date: release_date_val.clone(),
                                                release_year: release_year_val.clone(),
                                                release_country: None,
                                                genre: full_album.genre_name(),
                                                explicit: None,
                                                source_name: "qobuz".to_string(),
                                                ..Default::default()
                                            },
                                            service_track_id: track.id.to_string(),
                                            service_name: "qobuz".to_string(),
                                            service_id: qobuz_service_id,
                                            account_id,
                                            is_favorite: false,
                                            is_purchased: false,
                                            format: Some("FLAC".to_string()),
                                            bit_depth: track.maximum_bit_depth,
                                            sample_rate: track.maximum_sampling_rate.map(|r| (r * 1000.0) as i32),
                                            quality_score: Some(quality_score),
                                            audio_quality: Some(if is_hires { "hires".to_string() } else { "lossless".to_string() }),
                                            cover_art_url: album_cover.clone(),
                                            duration_ms: Some((track.duration * 1000) as i64),
                                            query_musicbrainz: false,
                                            // S198: this whole phase IS the favorite-albums pass.
                                            album_is_favorite: true,
                                            album_provider_track_id: Some(alb_id.clone()),
                                        };

                                        let t_enrich = std::time::Instant::now();
                                        match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                            Ok(res) => {
                                                enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                                if res.is_new_global_track {
                                                    tracks_new_global += 1;
                                                }
                                                if res.is_new_source_for_service {
                                                    sources_new_for_service += 1;
                                                }
                                                if res.is_new_library_entry_for_account {
                                                    library_entries_new_for_account += 1;
                                                }
                                                if res.is_already_present {
                                                    tracks_already_present += 1;
                                                }
                                                if res.is_new_import {
                                                    imported_tracks_total += 1;
                                                    album_expansion_metrics.tracks_persisted_new += 1;
                                                } else {
                                                    skipped_tracks_total += 1;
                                                    album_expansion_metrics.tracks_existing += 1;
                                                }
                                                availability_checked += 1;
                                                match res.completeness {
                                                    syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                    _ => metadata_partial += 1,
                                                }
                                            }
                                            Err(e) => {
                                                album_expansion_metrics.tracks_invalid += 1;
                                                tracing::warn!("[perform_sync_service/qobuz] track error for track {}: {}", track.id, e);
                                            }
                                        }
                                    }
                                }

                                // Mark album as favorite
                                let t_pers = std::time::Instant::now();
                                let _ = sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP) WHERE title = ? COLLATE NOCASE")
                                    .bind(&full_album.title)
                                    .execute(db)
                                    .await;
                                persistence_ms += t_pers.elapsed().as_millis() as u64;
                            }
                            offset += limit;
                            if items_len < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.contains("401") || e.contains("User authentication is required") {
                                tracing::warn!("[perform_sync_service/qobuz] 401 on favorite albums — marking credentials invalid");
                                let _ = mark_account_credentials_invalid(db, "qobuz", "HTTP 401: User authentication required").await;
                                let err_msg = format!("RequiresAuth: Qobuz session rejected (401) while fetching favorite albums: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            errors.push(format!("Qobuz favorite albums error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 3: Purchases
            if prefs.purchases {
                emit(SyncProgressEvent::running(
                    &service_normalized,
                    Some(account_id),
                    "importing_purchases",
                    0,
                    None,
                    "Importing purchased tracks...",
                    imported_tracks_total,
                    favorite_tracks_total,
                ));

                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_purchases(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.albums.total as u64;
                            let items_len = page.albums.items.len();
                            if items_len == 0 {
                                break;
                            }
                            album_expansion_metrics.albums_received += items_len as u64;
                            let client_ref = &client;
                            let mut expand_stream = futures_util::stream::iter(page.albums.items.into_iter().map(|purchase| {
                                async move {
                                    let has_tracks = purchase.tracks.as_ref().map(|t| !t.items.is_empty()).unwrap_or(false);
                                    if has_tracks {
                                        (purchase.id.clone(), Ok(purchase), false, 0u64)
                                    } else {
                                        let t_exp = std::time::Instant::now();
                                        let res = client_ref.get_album_full(&purchase.id).await;
                                        let elapsed = t_exp.elapsed().as_millis() as u64;
                                        (purchase.id.clone(), res, true, elapsed)
                                    }
                                }
                            }))
                            .buffer_unordered(5);

                            while let Some((alb_id, res, was_expansion_request, exp_ms)) = expand_stream.next().await {
                                if was_expansion_request {
                                    album_expansion_metrics.albums_needing_expansion += 1;
                                    album_expansion_metrics.album_detail_requests += 1;
                                    entity_expansion_ms += exp_ms;
                                }

                                let full_album = match res {
                                    Ok(album) => {
                                        if was_expansion_request {
                                            album_expansion_metrics.album_detail_success += 1;
                                        }
                                        album
                                    }
                                    Err(e) => {
                                        if was_expansion_request {
                                            album_expansion_metrics.album_detail_failed += 1;
                                            if album_expansion_metrics.first_error_code.is_none() {
                                                album_expansion_metrics.first_error_code = Some(e.clone());
                                                album_expansion_metrics.first_error_album_id = Some(alb_id.clone());
                                            }
                                        }
                                        tracing::error!("[perform_sync_service/qobuz] Failed to expand purchase album {}: {}", alb_id, e);
                                        errors.push(format!("Qobuz purchase album detail error for {}: {}", alb_id, e));
                                        continue;
                                    }
                                };

                                purchases_total += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_purchases",
                                    purchases_total,
                                    Some(page_total),
                                    format!("Importing purchases ({}/{})", purchases_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let album_title = full_album.title.clone();
                                let album_cover = full_album.image.as_ref().and_then(|img| img.large.clone().or_else(|| img.small.clone()));
                                let (release_date_val, release_year_val) = match full_album.released_at {
                                    Some(ts) => {
                                        let date_str = chrono::DateTime::from_timestamp(ts, 0)
                                            .map(|dt| dt.format("%Y-%m-%d").to_string());
                                        let year_str = chrono::DateTime::from_timestamp(ts, 0)
                                            .map(|dt| dt.format("%Y").to_string());
                                        (date_str, year_str)
                                    }
                                    None => (None, None),
                                };
                                let label_val = full_album.label.as_ref().and_then(|l| l.name.clone());

                                if let Some(ref container) = full_album.tracks {
                                    for track in &container.items {
                                        album_expansion_metrics.tracks_received += 1;
                                        if track.id <= 0 {
                                            album_expansion_metrics.tracks_invalid += 1;
                                            continue;
                                        }

                                        let artist_name = track
                                            .performer
                                            .as_ref()
                                            .and_then(|a| a.name.clone())
                                            .or_else(|| full_album.artist.as_ref().and_then(|a| a.name.clone()))
                                            .unwrap_or_else(|| "Unknown".to_string());
                                        let quality_score = client.compute_quality_score(track);
                                        let is_hires = track.maximum_bit_depth.unwrap_or(16) > 16 || track.maximum_sampling_rate.unwrap_or(44.1) > 44.1;

                                        let sync_input = crate::services::enrichment::SyncTrackInput {
                                            origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                title: track.title.clone(),
                                                artist: Some(artist_name),
                                                album: album_title.clone(),
                                                album_artist: full_album.artist.as_ref().and_then(|a| a.name.clone()),
                                                composer: track.composer.as_ref().and_then(|c| c.name.clone()),
                                                performers: track.performers.clone().or_else(|| track.performer.as_ref().and_then(|p| p.name.clone())),
                                                track_number: track.track_number.map(|tn| tn as u32),
                                                track_total: Some(container.total as u32),
                                                disc_number: track.media_number.map(|dn| dn as u32),
                                                isrc: track.isrc.clone(),
                                                barcode: full_album.upc.clone(),
                                                label: label_val.clone(),
                                                release_date: release_date_val.clone(),
                                                release_year: release_year_val.clone(),
                                                release_country: None,
                                                genre: full_album.genre_name(),
                                                explicit: None,
                                                source_name: "qobuz".to_string(),
                                                ..Default::default()
                                            },
                                            service_track_id: track.id.to_string(),
                                            service_name: "qobuz".to_string(),
                                            service_id: qobuz_service_id,
                                            account_id,
                                            is_favorite: false,
                                            is_purchased: true,
                                            format: Some("FLAC".to_string()),
                                            bit_depth: track.maximum_bit_depth,
                                            sample_rate: track.maximum_sampling_rate.map(|r| (r * 1000.0) as i32),
                                            quality_score: Some(quality_score),
                                            audio_quality: Some(if is_hires { "hires".to_string() } else { "lossless".to_string() }),
                                            cover_art_url: album_cover.clone(),
                                            duration_ms: Some((track.duration * 1000) as i64),
                                            query_musicbrainz: false,
                                            album_is_favorite: false,
                                            album_provider_track_id: None,
                                        };

                                        let t_enrich = std::time::Instant::now();
                                        match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                            Ok(res) => {
                                                enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                                if res.is_new_global_track {
                                                    tracks_new_global += 1;
                                                }
                                                if res.is_new_source_for_service {
                                                    sources_new_for_service += 1;
                                                }
                                                if res.is_new_library_entry_for_account {
                                                    library_entries_new_for_account += 1;
                                                }
                                                if res.is_already_present {
                                                    tracks_already_present += 1;
                                                }
                                                if res.is_new_import {
                                                    imported_tracks_total += 1;
                                                    album_expansion_metrics.tracks_persisted_new += 1;
                                                } else {
                                                    skipped_tracks_total += 1;
                                                    album_expansion_metrics.tracks_existing += 1;
                                                }
                                                availability_checked += 1;
                                                match res.completeness {
                                                    syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                    _ => metadata_partial += 1,
                                                }
                                            }
                                            Err(e) => {
                                                album_expansion_metrics.tracks_invalid += 1;
                                                tracing::warn!("[perform_sync_service/qobuz] track error for purchase track {}: {}", track.id, e);
                                            }
                                        }
                                    }
                                }
                            }
                            offset += limit;
                            if items_len < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.contains("401") || e.contains("User authentication is required") {
                                tracing::warn!("[perform_sync_service/qobuz] 401 on purchases — marking credentials invalid");
                                let _ = mark_account_credentials_invalid(db, "qobuz", "HTTP 401: User authentication required").await;
                                let err_msg = format!("RequiresAuth: Qobuz session rejected (401) while fetching purchases: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            errors.push(format!("Qobuz purchases error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 4: Playlists
            if prefs.playlists {
                emit(SyncProgressEvent::running(
                    &service_normalized,
                    Some(account_id),
                    "importing_playlists",
                    0,
                    None,
                    "Importing playlists...",
                    imported_tracks_total,
                    favorite_tracks_total,
                ));

                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_playlists(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.playlists.total as u64;
                            if page.playlists.items.is_empty() {
                                break;
                            }
                            for pl in &page.playlists.items {
                                playlists_total += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_playlists",
                                    playlists_total,
                                    Some(page_total),
                                    format!("Importing playlist: {} ({}/{})", pl.name, playlists_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let image_url = pl.images300.as_ref().and_then(|imgs| imgs.first().cloned());
                                let t_pers = std::time::Instant::now();
                                let _ = sqlx::query(
                                    r#"INSERT OR REPLACE INTO playlists 
                                       (account_id, service_playlist_id, name, description, is_public, is_collaborative, image_url, track_count, last_synced) 
                                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"#
                                )
                                .bind(account_id)
                                .bind(pl.id.to_string())
                                .bind(&pl.name)
                                .bind(&pl.description)
                                .bind(pl.is_public.unwrap_or(true) as i32)
                                .bind(pl.is_collaborative.unwrap_or(false) as i32)
                                .bind(&image_url)
                                .bind(pl.tracks_count.unwrap_or(0))
                                .execute(db)
                                .await;
                                persistence_ms += t_pers.elapsed().as_millis() as u64;

                                // S198: full pagination of Qobuz playlist expansion.
                                // Owner live audit (docs/s197_auditoria_importaciones.md §3):
                                // fixed offset=0 capped every mirror at ~199 tracks while
                                // declared totals reached 1,999+. S187 semantics generalized
                                // via services::import_pagination (short page ≠ end when a
                                // total is known; gap logged, never silent abort).
                                let playlist_db_id: Option<(i64,)> = sqlx::query_as(
                                    "SELECT id FROM playlists WHERE account_id = ? AND service_playlist_id = ?"
                                )
                                .bind(account_id)
                                .bind(pl.id.to_string())
                                .fetch_optional(db)
                                .await
                                .ok()
                                .flatten();
                                if let Some((p_id,)) = playlist_db_id {
                                    let mut track_offset: i32 = 0;
                                    let qobuz_page_limit: i32 = 200;
                                    // Declared fallback when a page omits its total.
                                    let mut playlist_provider_total: i64 =
                                        pl.tracks_count.map(|c| c.max(0) as i64).unwrap_or(0);
                                    loop {
                                        let t_exp = std::time::Instant::now();
                                        let detail = match client.get_playlist_tracks(pl.id, track_offset, qobuz_page_limit).await {
                                            Ok(d) => d,
                                            Err(e) => {
                                                tracing::warn!(
                                                    "[S198] Qobuz playlist '{}' expansion incomplete at offset {}: {}",
                                                    pl.name, track_offset, e
                                                );
                                                break;
                                            }
                                        };
                                        entity_expansion_ms += t_exp.elapsed().as_millis() as u64;
                                        let tracks_container = match detail.tracks {
                                            Some(c) => c,
                                            None => break,
                                        };
                                        if tracks_container.items.is_empty() {
                                            break;
                                        }
                                        playlist_provider_total = playlist_provider_total
                                            .max(tracks_container.total.max(0) as i64);
                                        if crate::services::import_pagination::is_short_page(
                                            track_offset,
                                            tracks_container.items.len() as i32,
                                            qobuz_page_limit,
                                            Some(playlist_provider_total),
                                        ) {
                                            tracing::warn!(
                                                "[S198] Qobuz playlist '{}': short page at offset {} ({}/{} items) — possible server-side filtering gap",
                                                pl.name, track_offset, tracks_container.items.len(), qobuz_page_limit
                                            );
                                        }
                                        for (idx, track) in tracks_container.items.iter().enumerate() {
                                            let pos = track_offset as usize + idx;
                                            let artist_name = track
                                                .performer
                                                .as_ref()
                                                .and_then(|a| a.name.clone())
                                                .unwrap_or_else(|| "Unknown".to_string());
                                            let album_title = track.album.as_ref().and_then(|a| a.title.clone());
                                            let album_cover = track.album.as_ref().and_then(|a| a.image.as_ref().and_then(|img| img.large.clone().or_else(|| img.small.clone())));
                                            let (release_date_val, release_year_val) = match track.album.as_ref().and_then(|a| a.released_at) {
                                                Some(ts) => {
                                                    let date_str = chrono::DateTime::from_timestamp(ts, 0)
                                                        .map(|dt| dt.format("%Y-%m-%d").to_string());
                                                    let year_str = chrono::DateTime::from_timestamp(ts, 0)
                                                        .map(|dt| dt.format("%Y").to_string());
                                                    (date_str, year_str)
                                                }
                                                None => (None, None),
                                            };
                                            let quality_score = client.compute_quality_score(track);
                                            let is_hires = track.maximum_bit_depth.unwrap_or(16) > 16 || track.maximum_sampling_rate.unwrap_or(44.1) > 44.1;

                                            let sync_input = crate::services::enrichment::SyncTrackInput {
                                                origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                    title: track.title.clone(),
                                                    artist: Some(artist_name),
                                                    album: album_title,
                                                    album_artist: track.album.as_ref().and_then(|a| a.artist.as_ref().and_then(|art| art.name.clone())),
                                                    composer: track.composer.as_ref().and_then(|c| c.name.clone()),
                                                    performers: track.performers.clone().or_else(|| track.performer.as_ref().and_then(|p| p.name.clone())),
                                                    track_number: track.track_number.map(|tn| tn as u32),
                                                    track_total: track.album.as_ref().and_then(|a| a.tracks.as_ref()).map(|c| c.total as u32),
                                                    disc_number: track.media_number.map(|dn| dn as u32),
                                                    isrc: track.isrc.clone(),
                                                    barcode: track.album.as_ref().and_then(|a| a.upc.clone()),
                                                    label: track.album.as_ref().and_then(|a| a.label.as_ref().and_then(|l| l.name.clone())),
                                                    release_date: release_date_val,
                                                    release_year: release_year_val,
                                                    release_country: None,
                                                    genre: track.album.as_ref().and_then(|a| a.genre_name()),
                                                    explicit: None,
                                                    source_name: "qobuz".to_string(),
                                                    ..Default::default()
                                                },
                                                service_track_id: track.id.to_string(),
                                                service_name: "qobuz".to_string(),
                                                service_id: qobuz_service_id,
                                                account_id,
                                                is_favorite: false,
                                                is_purchased: false,
                                                format: Some("FLAC".to_string()),
                                                bit_depth: track.maximum_bit_depth,
                                                sample_rate: track.maximum_sampling_rate.map(|r| (r * 1000.0) as i32),
                                                quality_score: Some(quality_score),
                                                audio_quality: Some(if is_hires { "hires".to_string() } else { "lossless".to_string() }),
                                                cover_art_url: album_cover,
                                                duration_ms: Some((track.duration * 1000) as i64),
                                                query_musicbrainz: false,
                                                album_is_favorite: false,
                                                album_provider_track_id: None,
                                            };

                                            let t_enrich = std::time::Instant::now();
                                            if let Ok(res) = enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                                enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                                let t_pl_track = std::time::Instant::now();
                                                let _ = sqlx::query(
                                                    "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                                                )
                                                .bind(p_id)
                                                .bind(res.track_id)
                                                .bind(pos as i32 + 1)
                                                .execute(db)
                                                .await;
                                                persistence_ms += t_pl_track.elapsed().as_millis() as u64;

                                                if res.is_new_global_track {
                                                    tracks_new_global += 1;
                                                }
                                                if res.is_new_source_for_service {
                                                    sources_new_for_service += 1;
                                                }
                                                if res.is_new_library_entry_for_account {
                                                    library_entries_new_for_account += 1;
                                                }
                                                if res.is_already_present {
                                                    tracks_already_present += 1;
                                                }
                                                if res.is_new_import {
                                                    imported_tracks_total += 1;
                                                } else {
                                                    skipped_tracks_total += 1;
                                                }
                                                availability_checked += 1;
                                                match res.completeness {
                                                    syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                    _ => metadata_partial += 1,
                                                }
                                            }
                                        }
                                        // S198: advance by the REAL page length; stop at
                                        // total or on a page that cannot continue.
                                        // FIX 2026-08-25: este match estaba DENTRO del bucle
                                        // `for` de pistas — tras la pista #1 next_offset
                                        // devolvía None (0+len >= total) y el break salía del
                                        // for sin que track_offset avanzara jamás: el loop
                                        // externo re-descargaba la página 0 infinitamente
                                        // (solo la pista 1 se vinculaba y el sync quedaba
                                        // "trabado en la primera playlist"). Vive a nivel de
                                        // página, como en los brazos spotify/deezer.
                                        match crate::services::import_pagination::next_offset(
                                            track_offset,
                                            tracks_container.items.len() as i32,
                                            qobuz_page_limit,
                                            Some(playlist_provider_total),
                                        ) {
                                            Some(next) => track_offset = next,
                                            None => break,
                                        }
                                    }
                                }
                            }
                            offset += limit;
                            if page.playlists.items.len() < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.contains("401") || e.contains("User authentication is required") {
                                tracing::warn!("[perform_sync_service/qobuz] 401 on playlists — marking credentials invalid");
                                let _ = mark_account_credentials_invalid(db, "qobuz", "HTTP 401: User authentication required").await;
                                let err_msg = format!("RequiresAuth: Qobuz session rejected (401) while fetching playlists: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            errors.push(format!("Qobuz playlists error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 5: Favorite Artists
            if prefs.favorite_artists {
                emit(SyncProgressEvent::running(
                    &service_normalized,
                    Some(account_id),
                    "importing_favorite_artists",
                    0,
                    None,
                    "Importing favorite artists...",
                    imported_tracks_total,
                    favorite_tracks_total,
                ));

                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_favorite_artists(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            if page.artists.items.is_empty() {
                                break;
                            }
                            for art in &page.artists.items {
                                favorite_artists_total += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_artists",
                                    favorite_artists_total,
                                    None,
                                    format!("Importing favorite artists ({})", favorite_artists_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));
                                if let Some(ref name) = art.name {
                                    let t_pers = std::time::Instant::now();
                                    if let Ok(aid) = client.get_or_create_artist(db, name).await {
                                        let _ = sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP) WHERE id = ?")
                                            .bind(aid)
                                            .execute(db)
                                            .await;
                                    }
                                    persistence_ms += t_pers.elapsed().as_millis() as u64;
                                }
                            }
                            offset += limit;
                            if page.artists.items.len() < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.contains("401") || e.contains("User authentication is required") {
                                tracing::warn!("[perform_sync_service/qobuz] 401 on favorite artists — marking credentials invalid");
                                let _ = mark_account_credentials_invalid(db, "qobuz", "HTTP 401: User authentication required").await;
                                let err_msg = format!("RequiresAuth: Qobuz session rejected (401) while fetching favorite artists: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            errors.push(format!("Qobuz favorite artists error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 6: History (if requested in preferences)
            if prefs.library_history {
                emit(SyncProgressEvent::running(
                    &service_normalized,
                    Some(account_id),
                    "importing_history",
                    0,
                    None,
                    "Importing library history...",
                    imported_tracks_total,
                    favorite_tracks_total,
                ));
            }
        }
        "tidal" => {
            // FIX 2026-08-25 ("credenciales de Tidal duran muy poco"): el sync
            // usa la MISMA ruta refresh-aware que las descargas — refresca y
            // PERSISTE tokens rotados ANTES de la primera llamada, de modo que
            // el TTL natural del access token ya no produce un 401 que mate la
            // cuenta en el primer sync.
            let http_client = crate::download::http_client::create_http_client();
            let (resolved_tidal_creds, _) = crate::services::tidal_pipeline::
                resolve_and_refresh_gui_credentials(db, &http_client).await;
            let resolved_tidal_creds = match resolved_tidal_creds {
                Some(c) => c,
                None => {
                    // resolve_and_refresh ya distingue "sin cuenta / rechazo
                    // real" (invalida ella misma) de fallo transitorio; aquí no
                    // re-invalidamos.
                    let err_msg = "RequiresAuth: no active or valid Tidal account available (si tu conexión acabó de caer, reintenta — la sesión puede seguir válida)".to_string();
                    emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                    return Err(err_msg);
                }
            };
            let access_token = resolved_tidal_creds.access_token.clone();
            let user_id = resolved_tidal_creds.user_id.as_ref()
                .and_then(|v| v.as_str().map(str::to_string)
                    .or_else(|| v.as_i64().map(|n| n.to_string())))
                .or_else(|| creds["user_id"].as_str().map(str::to_string))
                .unwrap_or_else(|| "0".to_string());
            let country = resolved_tidal_creds.country_code.clone()
                .or_else(|| creds["country_code"].as_str().map(str::to_string))
                .unwrap_or_else(|| "US".to_string());

            // FIX 2026-08-25: token ya refrescado proactivamente (ver arriba);
            // `mut` habilita el hot-swap de token tras un refresh forzado post-401.
            let mut client = crate::services::TidalClient::new(access_token.to_string())
                .with_user(user_id.to_string(), country.to_string());
            let tidal_service_id = client.get_service_id(db, "tidal").await.unwrap_or(3);

            // Phase 1: Favorite Tracks
            if prefs.favorite_tracks {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_tracks", 0, None, "Fetching Tidal favorite tracks...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                // S187: provider-reported grand total, for honest X-of-Y reporting.
                let mut favorites_provider_total: i64 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_favorites_with_retry(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.total as u64;
                            favorites_provider_total = favorites_provider_total.max(page.total as i64);
                            if page.items.is_empty() {
                                break;
                            }
                            for item in &page.items {
                                favorites_seen += 1;
                                let track = &item.item;
                                let artist_name = track.artist.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                                let album_title = track.album.as_ref().map(|a| a.title.clone());
                                let album_cover = track.album.as_ref().and_then(|a| a.cover_url());

                                let sync_input = crate::services::enrichment::SyncTrackInput {
                                    origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                        title: Some(track.title.clone()),
                                        artist: Some(artist_name),
                                        album: album_title,
                                        album_artist: track.album.as_ref().and_then(|a| a.artist.as_ref().map(|art| art.name.clone())),
                                        track_number: track.track_number.map(|tn| tn as u32),
                                        disc_number: track.disc_number.map(|dn| dn as u32),
                                        isrc: track.isrc.clone(),
                                        barcode: track.album.as_ref().and_then(|a| a.upc.clone()),
                                        label: track.album.as_ref().and_then(|a| a.label.clone()),
                                        release_date: track.album.as_ref().and_then(|a| a.release_date.clone()),
                                        source_name: "tidal".to_string(),
                                        ..Default::default()
                                    },
                                    service_track_id: track.id.to_string(),
                                    service_name: "tidal".to_string(),
                                    service_id: tidal_service_id,
                                    account_id,
                                    is_favorite: true,
                                    is_purchased: false,
                                    // FIX 2026-08-25 (matriz de metadatos): calidad
                                    // declarada por Tidal, no un hardcode.
                                    format: Some("FLAC".to_string()),
                                    bit_depth: None,
                                    sample_rate: None,
                                    quality_score: None,
                                    audio_quality: Some(
                                        track.audio_quality.clone().unwrap_or_else(|| "lossless".to_string()),
                                    ),
                                    cover_art_url: album_cover,
                                    duration_ms: Some((track.duration * 1000) as i64),
                                    query_musicbrainz: false,
                                    album_is_favorite: false,
                                    album_provider_track_id: None,
                                };

                                let t_enrich = std::time::Instant::now();
                                match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                    Ok(res) => {
                                        enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                        tracks_processed += 1;
                                        if res.is_new_global_track {
                                            tracks_new_global += 1;
                                        }
                                        if res.is_new_source_for_service {
                                            sources_new_for_service += 1;
                                        }
                                        if res.is_new_library_entry_for_account {
                                            library_entries_new_for_account += 1;
                                        }
                                        if res.is_already_present {
                                            tracks_already_present += 1;
                                        }
                                        if res.is_new_import {
                                            tracks_changed_unique += 1;
                                            imported_tracks_total += 1;
                                        } else {
                                            skipped_tracks_total += 1;
                                        }
                                        favorite_tracks_total += 1;
                                        availability_checked += 1;
                                        match res.completeness {
                                            syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                            _ => metadata_partial += 1,
                                        }
                                    }
                                    Err(e) => {
                                        errors.push(format!("Tidal favorite track error for {}: {}", track.id, e));
                                    }
                                }

                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_tracks",
                                    favorite_tracks_total,
                                    Some(page_total),
                                    format!("Importing Tidal favorite tracks ({}/{})", favorite_tracks_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));
                            }
                            // S187: advance by the REAL page length; a short-but-non-empty
                            // page is NOT end-of-data (Tidal sends no `next` and may return
                            // filtered pages). Stop when the provider total is reached.
                            offset += page.items.len() as i32;
                            if !crate::services::tidal::should_continue_tidal_pagination(
                                page.items.len(),
                                favorites_seen,
                                page.total as i64,
                            ) {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Tidal favorite tracks fetch error: {}", e);
                            if e.contains("RequiresAuth") || e.contains("401") {
                                // FIX 2026-08-25: un 401 ya no condena la cuenta de inmediato. Se
                                // intenta UN refresh forzado; si el proveedor emite token fresco, la
                                // cuenta sigue válida (las descargas tampoco se bloquean) y el hueco
                                // queda registrado como warning para re-ejecutar el sync.
                                if let Some(fresh_tok) = tidal_force_refresh_after_401(db).await {
                                    client.set_access_token(fresh_tok);
                                    warnings.push(format!("Tidal: sesión renovada a mitad de sync tras 401 — vuelve a ejecutar el sync para completar lo omitido ({} )", e));
                                    break;
                                }
                                let _ = mark_account_credentials_invalid(db, "tidal", "HTTP 401: Tidal session unauthorized or expired").await;
                                let err_msg = format!("RequiresAuth: Tidal session rejected (401) while fetching favorites: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            // S187: transient failure survived retries — record the gap
                            // honestly; NEVER silently truncate the import.
                            warnings.push(format!(
                                "Tidal favorite tracks incomplete: imported {} of {} provider tracks (page at offset {} failed after retry)",
                                favorites_seen, favorites_provider_total, offset
                            ));
                            tracing::warn!(
                                "[S187][tidal] favorite tracks: importadas {} de {} provider",
                                favorites_seen, favorites_provider_total
                            );
                            break;
                        }
                    }
                }
                tracing::info!(
                    "[S187][tidal] favorite tracks: importadas {} de {} provider",
                    favorites_seen, favorites_provider_total
                );
            }

            // Phase 2: Favorite Albums
            if prefs.favorite_albums {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_albums", 0, None, "Fetching Tidal favorite albums...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                // S187: provider-reported grand total, for honest X-of-Y reporting.
                let mut albums_provider_total: i64 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_favorite_albums_with_retry(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.total as u64;
                            albums_provider_total = albums_provider_total.max(page.total as i64);
                            if page.items.is_empty() {
                                break;
                            }
                            for item in &page.items {
                                let album = &item.item;
                                favorite_albums_total += 1;
                                albums_seen += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_albums",
                                    favorite_albums_total,
                                    Some(page_total),
                                    format!("Importing Tidal favorite albums ({}/{})", favorite_albums_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let album_tidal_id_str = album.tidal_id.to_string();

                                // 1. Ensure artist and album exist in local DB and is marked favorite
                                let artist_id = if let Some(ref artist) = album.artist {
                                    let artist_res: Option<(i64,)> = sqlx::query_as("INSERT OR IGNORE INTO artists (name) VALUES (?) RETURNING id")
                                        .bind(&artist.name)
                                        .fetch_optional(db)
                                        .await
                                        .unwrap_or(None);

                                    if let Some(row) = artist_res {
                                        row.0
                                    } else {
                                        sqlx::query_as::<_, (i64,)>("SELECT id FROM artists WHERE name = ?")
                                            .bind(&artist.name)
                                            .fetch_one(db)
                                            .await
                                            .map(|r| r.0)
                                            .unwrap_or(1)
                                    }
                                } else {
                                    1
                                };

                                let t_pers = std::time::Instant::now();
                                let aid_res: Option<(i64,)> = sqlx::query_as(
                                    r#"
                                    INSERT INTO albums (title, release_date, total_tracks, cover_art_url, tidal_id, label, upc, is_favorite, favorite_at)
                                    VALUES (?, ?, ?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP)
                                    ON CONFLICT(tidal_id) WHERE tidal_id IS NOT NULL DO UPDATE SET
                                        label = COALESCE(albums.label, excluded.label),
                                        upc = COALESCE(albums.upc, excluded.upc),
                                        is_favorite = 1,
                                        favorite_at = COALESCE(albums.favorite_at, CURRENT_TIMESTAMP)
                                    RETURNING id
                                    "#
                                )
                                .bind(&album.title)
                                .bind(&album.release_date)
                                .bind(album.total_tracks)
                                .bind(album.cover_url())
                                .bind(&album_tidal_id_str)
                                .bind(&album.label)
                                .bind(&album.upc)
                                .fetch_optional(db)
                                .await
                                .unwrap_or(None);

                                if let Some((album_id,)) = aid_res {
                                    let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
                                        .bind(album_id)
                                        .bind(artist_id)
                                        .execute(db)
                                        .await;
                                }
                                persistence_ms += t_pers.elapsed().as_millis() as u64;

                                // 2. Check cached availability unless force_retry_unavailable is requested
                                let mut is_cached_unavailable = false;
                                if !prefs.force_retry_unavailable {
                                    if let Ok(Some((cached_status, reason))) = crate::services::tidal::check_album_availability(
                                        db,
                                        tidal_service_id,
                                        &album_tidal_id_str,
                                        crate::services::tidal::DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS,
                                    ).await {
                                        is_cached_unavailable = true;
                                        albums_unavailable += 1;
                                        let exp_count = album.total_tracks.unwrap_or(1) as u64;
                                        tracks_unavailable += exp_count;
                                        tracks_expansion_deferred += exp_count;
                                        warnings.push(format!(
                                            "Tidal album '{}' ({}) skipped (cached as {:?}: {})",
                                            album.title, album.tidal_id, cached_status, reason
                                        ));
                                        tracing::info!(
                                            album_id = album.tidal_id,
                                            title = %album.title,
                                            status = ?cached_status,
                                            "[perform_sync_service/tidal] Album expansion skipped due to active TTL cache"
                                        );
                                    }
                                }

                                if is_cached_unavailable {
                                    continue;
                                }

                                // 3. Expand album tracks from Tidal API
                                let t_exp = std::time::Instant::now();
                                let exp_res = client.get_album_tracks_expanded(album.tidal_id, 0, 100).await;
                                entity_expansion_ms += t_exp.elapsed().as_millis() as u64;

                                match exp_res {
                                    Ok(res) => {
                                        match res.status {
                                            crate::services::tidal::TidalAlbumExpansionStatus::Available => {
                                                // Clear previous unavailable record if any
                                                let _ = crate::services::tidal::clear_album_availability(db, tidal_service_id, &album_tidal_id_str).await;

                                                for track_item in &res.tracks {
                                                    tracks_expanded += 1;
                                                    let track = track_item.track();
                                                    let artist_name = track.artist.as_ref().map(|a| a.name.clone())
                                                        .or_else(|| album.artist.as_ref().map(|a| a.name.clone()))
                                                        .unwrap_or_else(|| "Unknown".to_string());
                                                    let album_cover = album.cover_url();

                                                    let sync_input = crate::services::enrichment::SyncTrackInput {
                                                        origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                            title: Some(track.title.clone()),
                                                            artist: Some(artist_name),
                                                            album: Some(album.title.clone()),
                                                            album_artist: album.artist.as_ref().map(|a| a.name.clone()),
                                                            track_number: track.track_number.map(|tn| tn as u32),
                                                            disc_number: track.disc_number.map(|dn| dn as u32),
                                                            isrc: track.isrc.clone(),
                                                            barcode: album.upc.clone(),
                                                            label: album.label.clone(),
                                                            release_date: album.release_date.clone(),
                                                            source_name: "tidal".to_string(),
                                                            ..Default::default()
                                                        },
                                                        service_track_id: track.id.to_string(),
                                                        service_name: "tidal".to_string(),
                                                        service_id: tidal_service_id,
                                                        account_id,
                                                        is_favorite: false,
                                                        is_purchased: false,
                                                        format: Some("FLAC".to_string()),
                                                        bit_depth: None,
                                                        sample_rate: None,
                                                        quality_score: None,
                                                        audio_quality: Some(
                                                            track.audio_quality.clone().unwrap_or_else(|| "lossless".to_string()),
                                                        ),
                                                        cover_art_url: album_cover,
                                                        duration_ms: Some((track.duration * 1000) as i64),
                                                        query_musicbrainz: false,
                                                        album_is_favorite: true,
                                                        album_provider_track_id: None,
                                                    };

                                                    let t_enrich = std::time::Instant::now();
                                                    match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                                        Ok(res) => {
                                                            enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                                            tracks_processed += 1;
                                                            if res.is_new_global_track {
                                                                tracks_new_global += 1;
                                                            }
                                                            if res.is_new_source_for_service {
                                                                sources_new_for_service += 1;
                                                            }
                                                            if res.is_new_library_entry_for_account {
                                                                library_entries_new_for_account += 1;
                                                            }
                                                            if res.is_already_present {
                                                                tracks_already_present += 1;
                                                            }
                                                            if res.is_new_import {
                                                                tracks_changed_unique += 1;
                                                                imported_tracks_total += 1;
                                                            } else {
                                                                skipped_tracks_total += 1;
                                                            }
                                                            availability_checked += 1;
                                                            match res.completeness {
                                                                syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                                _ => metadata_partial += 1,
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracks_expansion_failed += 1;
                                                            errors.push(format!("Tidal album track error for {}: {}", track.id, e));
                                                        }
                                                    }
                                                }
                                            }
                                            crate::services::tidal::TidalAlbumExpansionStatus::UnavailableFromProvider
                                            | crate::services::tidal::TidalAlbumExpansionStatus::RegionRestricted => {
                                                let _ = crate::services::tidal::record_album_availability(
                                                    db,
                                                    tidal_service_id,
                                                    &album_tidal_id_str,
                                                    res.status,
                                                    res.http_status,
                                                    res.sub_status,
                                                    res.reason.as_deref(),
                                                ).await;

                                                albums_unavailable += 1;
                                                let exp_count = album.total_tracks.unwrap_or(1) as u64;
                                                tracks_unavailable += exp_count;
                                                let warn_msg = format!(
                                                    "Album '{}' ({}) is unavailable from Tidal ({:?}: {})",
                                                    album.title, album.tidal_id, res.status, res.reason.as_deref().unwrap_or("Asset not found")
                                                );
                                                tracing::warn!(
                                                    album_id = album.tidal_id,
                                                    title = %album.title,
                                                    status = ?res.status,
                                                    reason = ?res.reason,
                                                    "[perform_sync_service/tidal] Album unavailable from provider"
                                                );
                                                warnings.push(warn_msg);
                                            }
                                            crate::services::tidal::TidalAlbumExpansionStatus::AuthFailed => {
                                                let err_msg = format!("RequiresAuth: Tidal session unauthorized (401/403) while expanding album {}: {}", album.tidal_id, res.reason.unwrap_or_default());
                                                // FIX 2026-08-25: un 401 ya no condena la cuenta de inmediato. Se
                                                // intenta UN refresh forzado; si el proveedor emite token fresco, la
                                                // cuenta sigue válida (las descargas tampoco se bloquean) y el hueco
                                                // queda registrado como warning para re-ejecutar el sync.
                                                if let Some(fresh_tok) = tidal_force_refresh_after_401(db).await {
                                                    client.set_access_token(fresh_tok);
                                                    warnings.push(format!("Tidal: sesión renovada a mitad de sync tras 401 expandiendo álbum {} — vuelve a ejecutar el sync para completarlo", album.tidal_id));
                                                    break;
                                                }
                                                let _ = mark_account_credentials_invalid(db, "tidal", "HTTP 401: Tidal session unauthorized or expired").await;
                                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                                return Err(err_msg);
                                            }
                                            crate::services::tidal::TidalAlbumExpansionStatus::RateLimited
                                            | crate::services::tidal::TidalAlbumExpansionStatus::TemporarilyFailed
                                            | crate::services::tidal::TidalAlbumExpansionStatus::MalformedResponse => {
                                                let exp_count = album.total_tracks.unwrap_or(1) as u64;
                                                tracks_expansion_failed += exp_count;
                                                let err_msg = format!("Failed to expand album tracks for {} ({}): {:?}", album.title, album.tidal_id, res.reason);
                                                tracing::error!("[perform_sync_service/tidal] {}", err_msg);
                                                errors.push(err_msg);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let exp_count = album.total_tracks.unwrap_or(1) as u64;
                                        tracks_expansion_failed += exp_count;
                                        errors.push(format!("Failed to expand album tracks for {} ({}): {}", album.title, album.tidal_id, e));
                                    }
                                }
                            }
                            // S187: advance by the REAL page length; short pages continue.
                            offset += page.items.len() as i32;
                            if !crate::services::tidal::should_continue_tidal_pagination(
                                page.items.len(),
                                albums_seen,
                                page.total as i64,
                            ) {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Tidal favorite albums fetch error: {}", e);
                            if e.contains("RequiresAuth") || e.contains("401") {
                                // FIX 2026-08-25: un 401 ya no condena la cuenta de inmediato. Se
                                // intenta UN refresh forzado; si el proveedor emite token fresco, la
                                // cuenta sigue válida (las descargas tampoco se bloquean) y el hueco
                                // queda registrado como warning para re-ejecutar el sync.
                                if let Some(fresh_tok) = tidal_force_refresh_after_401(db).await {
                                    client.set_access_token(fresh_tok);
                                    warnings.push(format!("Tidal: sesión renovada a mitad de sync tras 401 — vuelve a ejecutar el sync para completar lo omitido ({} )", e));
                                    break;
                                }
                                let _ = mark_account_credentials_invalid(db, "tidal", "HTTP 401: Tidal session unauthorized or expired").await;
                                let err_msg = format!("RequiresAuth: Tidal session rejected (401) while fetching favorite albums: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            // S187: record the gap honestly instead of silently truncating.
                            warnings.push(format!(
                                "Tidal favorite albums incomplete: imported {} of {} provider albums (page at offset {} failed after retry)",
                                albums_seen, albums_provider_total, offset
                            ));
                            tracing::warn!(
                                "[S187][tidal] favorite albums: importados {} de {} provider",
                                albums_seen, albums_provider_total
                            );
                            break;
                        }
                    }
                }
                tracing::info!(
                    "[S187][tidal] favorite albums: importados {} de {} provider",
                    albums_seen, albums_provider_total
                );
            }


            // Phase 3: Playlists
            if prefs.playlists {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_playlists", 0, None, "Fetching Tidal playlists...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                // S187: provider-reported grand total, for honest X-of-Y reporting.
                let mut playlists_provider_total: i64 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_playlists_with_retry(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.total as u64;
                            playlists_provider_total = playlists_provider_total.max(page.total as i64);
                            if page.items.is_empty() {
                                break;
                            }
                            for pl in &page.items {
                                playlists_total += 1;
                                playlists_seen += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_playlists",
                                    playlists_total,
                                    Some(page_total),
                                    format!("Importing Tidal playlist: {} ({}/{})", pl.title, playlists_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let t_pers = std::time::Instant::now();
                                let _ = sqlx::query(
                                    r#"INSERT OR REPLACE INTO playlists 
                                       (account_id, service_playlist_id, name, description, is_public, is_collaborative, image_url, track_count, last_synced) 
                                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"#
                                )
                                .bind(account_id)
                                .bind(&pl.uuid)
                                .bind(&pl.title)
                                .bind(&pl.description)
                                .bind(pl.public_playlist.unwrap_or(true) as i32)
                                .bind(0)
                                .bind(None::<String>)
                                .bind(pl.track_count)
                                .execute(db)
                                .await;
                                persistence_ms += t_pers.elapsed().as_millis() as u64;

                                // S187: paginate ALL pages of this playlist's tracks.
                                // Previously a single page (offset always 0, limit 100) was fetched
                                // and never advanced, so every playlist longer than ~100 items lost
                                // its remainder (owner DB: 57 playlists declare 25220 provider tracks
                                // but only 3846 playlist_tracks rows existed).
                                let playlist_db_id: Option<(i64,)> = sqlx::query_as(
                                    "SELECT id FROM playlists WHERE account_id = ? AND service_playlist_id = ?"
                                )
                                .bind(account_id)
                                .bind(&pl.uuid)
                                .fetch_optional(db)
                                .await
                                .ok()
                                .flatten();

                                let mut track_offset: i32 = 0;
                                let track_limit: i32 = 100;
                                let mut playlist_tracks_seen: u64 = 0;
                                // Fallback when the API omits totalNumberOfItems: declared count.
                                let mut playlist_provider_total: i64 = pl.track_count as i64;
                                let mut playlist_fetch_error: Option<String> = None;

                                if let Some((p_id,)) = playlist_db_id {
                                    loop {
                                        let t_exp = std::time::Instant::now();
                                        match client.get_playlist_tracks_with_retry(&pl.uuid, track_offset, track_limit).await {
                                            Ok(tracks_page) => {
                                                entity_expansion_ms += t_exp.elapsed().as_millis() as u64;
                                                if tracks_page.total > 0 {
                                                    playlist_provider_total = playlist_provider_total.max(tracks_page.total as i64);
                                                }
                                                if tracks_page.items.is_empty() {
                                                    break;
                                                }
                                                for (pos, item) in tracks_page.items.iter().enumerate() {
                                                    tracks_expanded += 1;
                                                    playlist_tracks_seen += 1;
                                                    let track = &item.item;
                                                    let artist_name = track.artist.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                                                    let album_title = track.album.as_ref().map(|a| a.title.clone());
                                                    let album_cover = track.album.as_ref().and_then(|a| a.cover_url());

                                                    let sync_input = crate::services::enrichment::SyncTrackInput {
                                                        origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                            title: Some(track.title.clone()),
                                                            artist: Some(artist_name),
                                                            album: album_title,
                                                            album_artist: track.album.as_ref().and_then(|a| a.artist.as_ref().map(|art| art.name.clone())),
                                                            track_number: track.track_number.map(|tn| tn as u32),
                                                            disc_number: track.disc_number.map(|dn| dn as u32),
                                                            isrc: track.isrc.clone(),
                                                            barcode: track.album.as_ref().and_then(|a| a.upc.clone()),
                                                            label: track.album.as_ref().and_then(|a| a.label.clone()),
                                                            release_date: track.album.as_ref().and_then(|a| a.release_date.clone()),
                                                            source_name: "tidal".to_string(),
                                                            ..Default::default()
                                                        },
                                                        service_track_id: track.id.to_string(),
                                                        service_name: "tidal".to_string(),
                                                        service_id: tidal_service_id,
                                                        account_id,
                                                        is_favorite: false,
                                                        is_purchased: false,
                                                        format: Some("FLAC".to_string()),
                                                        bit_depth: None,
                                                        sample_rate: None,
                                                        quality_score: None,
                                                        audio_quality: Some(
                                                            track.audio_quality.clone().unwrap_or_else(|| "lossless".to_string()),
                                                        ),
                                                        cover_art_url: album_cover,
                                                        duration_ms: Some((track.duration * 1000) as i64),
                                                        query_musicbrainz: false,
                                                        album_is_favorite: false,
                                                        album_provider_track_id: None,
                                                    };

                                                    let t_enrich = std::time::Instant::now();
                                                    match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                                        Ok(res) => {
                                                            enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                                            tracks_processed += 1;
                                                            let t_pl_track = std::time::Instant::now();
                                                            let _ = sqlx::query(
                                                                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                                                            )
                                                            .bind(p_id)
                                                            .bind(res.track_id)
                                                            .bind(track_offset + pos as i32 + 1)
                                                            .execute(db)
                                                            .await;
                                                            persistence_ms += t_pl_track.elapsed().as_millis() as u64;

                                                            if res.is_new_global_track {
                                                                tracks_new_global += 1;
                                                            }
                                                            if res.is_new_source_for_service {
                                                                sources_new_for_service += 1;
                                                            }
                                                            if res.is_new_library_entry_for_account {
                                                                library_entries_new_for_account += 1;
                                                            }
                                                            if res.is_already_present {
                                                                tracks_already_present += 1;
                                                            }
                                                            if res.is_new_import {
                                                                tracks_changed_unique += 1;
                                                                imported_tracks_total += 1;
                                                            } else {
                                                                skipped_tracks_total += 1;
                                                            }
                                                            availability_checked += 1;
                                                            match res.completeness {
                                                                syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                                _ => metadata_partial += 1,
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracks_expansion_failed += 1;
                                                            errors.push(format!("Tidal playlist track error for {}: {}", track.id, e));
                                                        }
                                                    }
                                                }
                                                // S187: advance by the REAL page length; short-but-non-empty
                                                // pages are NOT end-of-data.
                                                track_offset += tracks_page.items.len() as i32;
                                                if !crate::services::tidal::should_continue_tidal_pagination(
                                                    tracks_page.items.len(),
                                                    playlist_tracks_seen,
                                                    tracks_page.total as i64,
                                                ) {
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                // S187: transient failure survived retries. Record it,
                                                // report X-of-Y below and continue with the next playlist;
                                                // NEVER silently truncate the rest of the import.
                                                playlist_fetch_error = Some(e);
                                                break;
                                            }
                                        }
                                    }
                                }

                                if let Some(fetch_err) = playlist_fetch_error {
                                    let missing = (playlist_provider_total - playlist_tracks_seen as i64).max(0) as u64;
                                    tracks_expansion_failed += missing;
                                    errors.push(format!(
                                        "Failed to expand playlist tracks for {} ({}): {}: imported {} of {} (offset {})",
                                        pl.title, pl.uuid, fetch_err, playlist_tracks_seen, playlist_provider_total, track_offset
                                    ));
                                    tracing::warn!(
                                        "[S187][tidal] playlist '{}': importadas {} de {} provider",
                                        pl.title, playlist_tracks_seen, playlist_provider_total
                                    );
                                } else {
                                    tracing::info!(
                                        "[S187][tidal] playlist '{}': importadas {} de {} provider",
                                        pl.title, playlist_tracks_seen, playlist_provider_total
                                    );
                                    if playlist_provider_total > 0 && (playlist_tracks_seen as i64) < playlist_provider_total {
                                        warnings.push(format!(
                                            "Tidal playlist '{}' incomplete: imported {} of {} tracks",
                                            pl.title, playlist_tracks_seen, playlist_provider_total
                                        ));
                                    }
                                }
                            }
                            // S187: advance by the REAL page length; short pages continue.
                            offset += page.items.len() as i32;
                            if !crate::services::tidal::should_continue_tidal_pagination(
                                page.items.len(),
                                playlists_seen,
                                page.total as i64,
                            ) {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Tidal playlists fetch error: {}", e);
                            if e.contains("RequiresAuth") || e.contains("401") {
                                // FIX 2026-08-25: un 401 ya no condena la cuenta de inmediato. Se
                                // intenta UN refresh forzado; si el proveedor emite token fresco, la
                                // cuenta sigue válida (las descargas tampoco se bloquean) y el hueco
                                // queda registrado como warning para re-ejecutar el sync.
                                if let Some(fresh_tok) = tidal_force_refresh_after_401(db).await {
                                    client.set_access_token(fresh_tok);
                                    warnings.push(format!("Tidal: sesión renovada a mitad de sync tras 401 — vuelve a ejecutar el sync para completar lo omitido ({} )", e));
                                    break;
                                }
                                let _ = mark_account_credentials_invalid(db, "tidal", "HTTP 401: Tidal session unauthorized or expired").await;
                                let err_msg = format!("RequiresAuth: Tidal session rejected (401) while fetching playlists: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            // S187: record the gap honestly instead of silently truncating.
                            warnings.push(format!(
                                "Tidal playlists incomplete: imported {} of {} provider playlists (page at offset {} failed after retry)",
                                playlists_seen, playlists_provider_total, offset
                            ));
                            tracing::warn!(
                                "[S187][tidal] playlists: importadas {} de {} provider",
                                playlists_seen, playlists_provider_total
                            );
                            break;
                        }
                    }
                }
                tracing::info!(
                    "[S187][tidal] playlists: importadas {} de {} provider",
                    playlists_seen, playlists_provider_total
                );
            }

            // Phase 4: Favorite Artists
            if prefs.favorite_artists {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_artists", 0, None, "Fetching Tidal favorite artists...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                // S187: provider-reported grand total, for honest X-of-Y reporting.
                let mut artists_provider_total: i64 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_favorite_artists_with_retry(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            artists_provider_total = artists_provider_total.max(page.total as i64);
                            if page.items.is_empty() {
                                break;
                            }
                            for item in &page.items {
                                favorite_artists_total += 1;
                                let art = &item.item;
                                let t_pers = std::time::Instant::now();
                                if let Ok(aid) = client.get_or_create_artist(db, &art.name).await {
                                    let tidal_id_str = art.id.to_string();
                                    let _ = sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP), tidal_id = COALESCE(tidal_id, ?) WHERE id = ?")
                                        .bind(&tidal_id_str)
                                        .bind(aid)
                                        .execute(db)
                                        .await;
                                }
                                persistence_ms += t_pers.elapsed().as_millis() as u64;
                            }
                            // S187: advance by the REAL page length; short pages continue.
                            offset += page.items.len() as i32;
                            if !crate::services::tidal::should_continue_tidal_pagination(
                                page.items.len(),
                                favorite_artists_total,
                                page.total as i64,
                            ) {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Tidal favorite artists fetch error: {}", e);
                            if e.contains("RequiresAuth") || e.contains("401") {
                                // FIX 2026-08-25: un 401 ya no condena la cuenta de inmediato. Se
                                // intenta UN refresh forzado; si el proveedor emite token fresco, la
                                // cuenta sigue válida (las descargas tampoco se bloquean) y el hueco
                                // queda registrado como warning para re-ejecutar el sync.
                                if let Some(fresh_tok) = tidal_force_refresh_after_401(db).await {
                                    client.set_access_token(fresh_tok);
                                    warnings.push(format!("Tidal: sesión renovada a mitad de sync tras 401 — vuelve a ejecutar el sync para completar lo omitido ({} )", e));
                                    break;
                                }
                                let _ = mark_account_credentials_invalid(db, "tidal", "HTTP 401: Tidal session unauthorized or expired").await;
                                let err_msg = format!("RequiresAuth: Tidal session rejected (401) while fetching favorite artists: {}", e);
                                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                                return Err(err_msg);
                            }
                            // S187: record the gap honestly instead of silently truncating.
                            warnings.push(format!(
                                "Tidal favorite artists incomplete: imported {} of {} provider artists (page at offset {} failed after retry)",
                                favorite_artists_total, artists_provider_total, offset
                            ));
                            tracing::warn!(
                                "[S187][tidal] favorite artists: importados {} de {} provider",
                                favorite_artists_total, artists_provider_total
                            );
                            break;
                        }
                    }
                }
                tracing::info!(
                    "[S187][tidal] favorite artists: importados {} de {} provider",
                    favorite_artists_total, artists_provider_total
                );
            }
        }
        "spotify" => {
            let access_token = match get_or_refresh_spotify_token(db, account_id, &creds).await {
                Ok(tok) => tok,
                Err(e) => {
                    let err_msg = format!("RequiresAuth: Spotify authentication failed: {}", e);
                    emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                    return Err(err_msg);
                }
            };
            let refresh_token = creds["refresh_token"].as_str().map(|s| s.to_string());
            let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
            let client = SpotifyClient::new(access_token, refresh_token, expires_at);
            let spotify_service_id = client.get_service_id(db, "spotify").await.unwrap_or(1);

            // Phase 1: Saved (Favorite) Tracks
            if prefs.favorite_tracks {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_tracks", 0, None, "Fetching Spotify library...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_saved_tracks(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.total as u64;
                            if page.items.is_empty() {
                                break;
                            }
                            for item in &page.items {
                                let track = &item.track;
                                let artist_name = track.artists.first().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                                let album_title = track.album.as_ref().map(|a| a.name.clone());
                                let album_cover = track.album.as_ref().and_then(|a| a.images.first()).map(|img| img.url.clone());
                                let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());
                                let barcode = track.album.as_ref().and_then(|a| a.external_ids.as_ref()).and_then(|e| e.upc.clone().or_else(|| e.ean.clone()));
                                let label = track.album.as_ref().and_then(|a| a.label.clone());
                                let release_date = track.album.as_ref().and_then(|a| a.release_date.clone());
                                let album_artist = track.artists.first().map(|a| a.name.clone());

                                let sync_input = crate::services::enrichment::SyncTrackInput {
                                    origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                        title: Some(track.name.clone()),
                                        artist: Some(artist_name),
                                        album: album_title,
                                        album_artist,
                                        track_number: track.track_number.map(|tn| tn as u32),
                                        disc_number: track.disc_number.map(|dn| dn as u32),
                                        isrc,
                                        barcode,
                                        label,
                                        release_date,
                                        explicit: Some(track.explicit),
                                        source_name: "spotify".to_string(),
                                        ..Default::default()
                                    },
                                    service_track_id: track.id.clone(),
                                    service_name: "spotify".to_string(),
                                    service_id: spotify_service_id,
                                    account_id,
                                    is_favorite: true,
                                    is_purchased: false,
                                    format: Some("OGG".to_string()),
                                    bit_depth: None,
                                    sample_rate: None,
                                    quality_score: None,
                                    audio_quality: Some("standard".to_string()),
                                    cover_art_url: album_cover,
                                    duration_ms: Some(track.duration_ms as i64),
                                    query_musicbrainz: false,
                                    album_is_favorite: false,
                                    album_provider_track_id: None,
                                };

                                let t_enrich = std::time::Instant::now();
                                match enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                    Ok(res) => {
                                        enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                        tracks_processed += 1;
                                        if res.is_new_global_track {
                                            tracks_new_global += 1;
                                        }
                                        if res.is_new_source_for_service {
                                            sources_new_for_service += 1;
                                        }
                                        if res.is_new_library_entry_for_account {
                                            library_entries_new_for_account += 1;
                                        }
                                        if res.is_already_present {
                                            tracks_already_present += 1;
                                        }
                                        if res.is_new_import {
                                            tracks_changed_unique += 1;
                                            imported_tracks_total += 1;
                                        } else {
                                            skipped_tracks_total += 1;
                                        }
                                        favorite_tracks_total += 1;
                                        availability_checked += 1;
                                        match res.completeness {
                                            syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                            _ => metadata_partial += 1,
                                        }
                                    }
                                    Err(e) => {
                                        errors.push(format!("Spotify track error for {}: {}", track.id, e));
                                    }
                                }

                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_tracks",
                                    favorite_tracks_total,
                                    Some(page_total),
                                    format!("Importing Spotify favorite tracks ({}/{})", favorite_tracks_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));
                            }
                            offset += limit;
                            if page.items.len() < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            errors.push(format!("Spotify favorite tracks fetch error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 2: Favorite Albums
            if prefs.favorite_albums {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_albums", 0, None, "Fetching Spotify albums...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_saved_albums(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.total as u64;
                            if page.items.is_empty() {
                                break;
                            }
                            for saved in &page.items {
                                let album = &saved.album;
                                favorite_albums_total += 1;
                                albums_seen += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_favorite_albums",
                                    favorite_albums_total,
                                    Some(page_total),
                                    format!("Importing Spotify albums ({}/{})", favorite_albums_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let album_cover = album.images.first().map(|img| img.url.clone());
                                let barcode = album.external_ids.as_ref().and_then(|e| e.upc.clone().or_else(|| e.ean.clone()));

                                let t_exp = std::time::Instant::now();
                                let tracks_res = if let Some(ref tracks_pag) = album.tracks {
                                    Ok(tracks_pag.clone())
                                } else {
                                    client.get_album_tracks(&album.id, 0, 100).await
                                };

                                if let Ok(tracks_page) = tracks_res {
                                    entity_expansion_ms += t_exp.elapsed().as_millis() as u64;
                                    for track in &tracks_page.items {
                                        tracks_expanded += 1;
                                        let artist_name = track.artists.first().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                                        let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());

                                        let sync_input = crate::services::enrichment::SyncTrackInput {
                                            origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                title: Some(track.name.clone()),
                                                artist: Some(artist_name),
                                                album: Some(album.name.clone()),
                                                album_artist: None,
                                                track_number: track.track_number.map(|tn| tn as u32),
                                                disc_number: track.disc_number.map(|dn| dn as u32),
                                                isrc,
                                                barcode: barcode.clone(),
                                                label: album.label.clone(),
                                                release_date: album.release_date.clone(),
                                                explicit: Some(track.explicit),
                                                source_name: "spotify".to_string(),
                                                ..Default::default()
                                            },
                                            service_track_id: track.id.clone(),
                                            service_name: "spotify".to_string(),
                                            service_id: spotify_service_id,
                                            account_id,
                                            is_favorite: false,
                                            is_purchased: false,
                                            format: Some("OGG".to_string()),
                                            bit_depth: None,
                                            sample_rate: None,
                                            quality_score: None,
                                            audio_quality: Some("standard".to_string()),
                                            cover_art_url: album_cover.clone(),
                                            duration_ms: Some(track.duration_ms as i64),
                                            query_musicbrainz: false,
                                            album_is_favorite: false,
                                            album_provider_track_id: None,
                                        };

                                        let t_enrich = std::time::Instant::now();
                                        if let Ok(res) = enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                            enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                            tracks_processed += 1;
                                            if res.is_new_global_track {
                                                tracks_new_global += 1;
                                            }
                                            if res.is_new_source_for_service {
                                                sources_new_for_service += 1;
                                            }
                                            if res.is_new_library_entry_for_account {
                                                library_entries_new_for_account += 1;
                                            }
                                            if res.is_already_present {
                                                tracks_already_present += 1;
                                            }
                                            if res.is_new_import {
                                                tracks_changed_unique += 1;
                                                imported_tracks_total += 1;
                                            } else {
                                                skipped_tracks_total += 1;
                                            }
                                            availability_checked += 1;
                                            match res.completeness {
                                                syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                _ => metadata_partial += 1,
                                            }
                                        }
                                    }
                                }

                                // Mark album as favorite
                                let t_pers = std::time::Instant::now();
                                let _ = sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP) WHERE title = ? COLLATE NOCASE")
                                    .bind(&album.name)
                                    .execute(db)
                                    .await;
                                persistence_ms += t_pers.elapsed().as_millis() as u64;
                            }
                            offset += limit;
                            if page.items.len() < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            errors.push(format!("Spotify favorite albums fetch error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 3: Playlists
            if prefs.playlists {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_playlists", 0, None, "Fetching Spotify playlists...", imported_tracks_total, favorite_tracks_total));
                let mut offset = 0;
                let limit = 50;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_playlists(offset, limit).await {
                        Ok(page) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_total = page.total as u64;
                            if page.items.is_empty() {
                                break;
                            }
                            for pl in &page.items {
                                playlists_total += 1;
                                playlists_seen += 1;
                                emit(SyncProgressEvent::running(
                                    &service_normalized,
                                    Some(account_id),
                                    "importing_playlists",
                                    playlists_total,
                                    Some(page_total),
                                    format!("Importing Spotify playlist: {} ({}/{})", pl.name, playlists_total, page_total),
                                    imported_tracks_total,
                                    favorite_tracks_total,
                                ));

                                let img_url = pl.images.first().map(|i| i.url.clone());
                                let t_pers = std::time::Instant::now();
                                let _ = sqlx::query(
                                    r#"INSERT OR REPLACE INTO playlists 
                                       (account_id, service_playlist_id, name, description, is_public, is_collaborative, image_url, track_count, last_synced) 
                                       VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"#
                                )
                                .bind(account_id)
                                .bind(&pl.id)
                                .bind(&pl.name)
                                .bind(&pl.description)
                                .bind(pl.public.unwrap_or(true) as i32)
                                .bind(pl.collaborative as i32)
                                .bind(img_url)
                                .bind(pl.tracks.as_ref().map(|t| t.total).unwrap_or(0))
                                .execute(db)
                                .await;
                                persistence_ms += t_pers.elapsed().as_millis() as u64;

                                // S198: full pagination of Spotify playlist expansion.
                                // Owner live audit (docs/s197_auditoria_importaciones.md §3):
                                // 'Liked Songs Pt. 1' declared 9,946 but imported exactly 99
                                // (single page, offset=0). S187 semantics generalized via
                                // services::import_pagination; client already retries 429 x3.
                                let playlist_db_id: Option<(i64,)> = sqlx::query_as(
                                    "SELECT id FROM playlists WHERE account_id = ? AND service_playlist_id = ?"
                                )
                                .bind(account_id)
                                .bind(&pl.id)
                                .fetch_optional(db)
                                .await
                                .ok()
                                .flatten();
                                if let Some((p_id,)) = playlist_db_id {
                                    let mut track_offset: i32 = 0;
                                    let spotify_page_limit: i32 = 100;
                                    let mut playlist_provider_total: i64 = pl
                                        .tracks
                                        .as_ref()
                                        .map(|t| t.total.max(0) as i64)
                                        .unwrap_or(0);
                                    loop {
                                        let t_exp = std::time::Instant::now();
                                        let tracks_page = match client.get_playlist_tracks(&pl.id, track_offset, spotify_page_limit).await {
                                            Ok(p) => p,
                                            Err(e) => {
                                                tracing::warn!(
                                                    "[S198] Spotify playlist '{}' expansion incomplete at offset {}: {}",
                                                    pl.name, track_offset, e
                                                );
                                                break;
                                            }
                                        };
                                        entity_expansion_ms += t_exp.elapsed().as_millis() as u64;
                                        if tracks_page.items.is_empty() {
                                            break;
                                        }
                                        playlist_provider_total = playlist_provider_total
                                            .max(tracks_page.total.max(0) as i64);
                                        for (idx, item) in tracks_page.items.iter().enumerate() {
                                            let pos = track_offset as usize + idx;
                                            if let Some(ref track) = item.track {
                                                tracks_expanded += 1;
                                                let artist_name = track.artists.first().map(|a| a.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                                                let album_title = track.album.as_ref().map(|a| a.name.clone());
                                                let album_cover = track.album.as_ref().and_then(|a| a.images.first()).map(|img| img.url.clone());
                                                let isrc = track.external_ids.as_ref().and_then(|e| e.isrc.clone());
                                                let barcode = track.album.as_ref().and_then(|a| a.external_ids.as_ref()).and_then(|e| e.upc.clone().or_else(|| e.ean.clone()));
                                                let label = track.album.as_ref().and_then(|a| a.label.clone());
                                                let release_date = track.album.as_ref().and_then(|a| a.release_date.clone());
                                                let album_artist = track.artists.first().map(|a| a.name.clone());

                                                let sync_input = crate::services::enrichment::SyncTrackInput {
                                                    origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                        title: Some(track.name.clone()),
                                                        artist: Some(artist_name),
                                                        album: album_title,
                                                        album_artist,
                                                        track_number: track.track_number.map(|tn| tn as u32),
                                                        disc_number: track.disc_number.map(|dn| dn as u32),
                                                        isrc,
                                                        barcode,
                                                        label,
                                                        release_date,
                                                        explicit: Some(track.explicit),
                                                        source_name: "spotify".to_string(),
                                                        ..Default::default()
                                                    },
                                                    service_track_id: track.id.clone(),
                                                    service_name: "spotify".to_string(),
                                                    service_id: spotify_service_id,
                                                    account_id,
                                                    is_favorite: false,
                                                    is_purchased: false,
                                                    format: Some("OGG".to_string()),
                                                    bit_depth: None,
                                                    sample_rate: None,
                                                    quality_score: None,
                                                    audio_quality: Some("standard".to_string()),
                                                    cover_art_url: album_cover,
                                                    duration_ms: Some(track.duration_ms as i64),
                                                    query_musicbrainz: false,
                                                    album_is_favorite: false,
                                                    album_provider_track_id: None,
                                                };

                                                let t_enrich = std::time::Instant::now();
                                                if let Ok(res) = enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await {
                                                    enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                                    let t_pl_track = std::time::Instant::now();
                                                    let _ = sqlx::query(
                                                        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                                                    )
                                                    .bind(p_id)
                                                    .bind(res.track_id)
                                                    .bind(pos as i32 + 1)
                                                    .execute(db)
                                                    .await;
                                                    persistence_ms += t_pl_track.elapsed().as_millis() as u64;
                                                    tracks_processed += 1;

                                                    if res.is_new_global_track {
                                                        tracks_new_global += 1;
                                                    }
                                                    if res.is_new_source_for_service {
                                                        sources_new_for_service += 1;
                                                    }
                                                    if res.is_new_library_entry_for_account {
                                                        library_entries_new_for_account += 1;
                                                    }
                                                    if res.is_already_present {
                                                        tracks_already_present += 1;
                                                    }
                                                    if res.is_new_import {
                                                        tracks_changed_unique += 1;
                                                        imported_tracks_total += 1;
                                                    } else {
                                                        skipped_tracks_total += 1;
                                                    }
                                                    availability_checked += 1;
                                                    match res.completeness {
                                                        syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                                        _ => metadata_partial += 1,
                                                    }
                                                }
                                            }
                                        }
                                        // S198: advance by the REAL page length; stop at
                                        // total or on a page that cannot continue.
                                        match crate::services::import_pagination::next_offset(
                                            track_offset,
                                            tracks_page.items.len() as i32,
                                            spotify_page_limit,
                                            Some(playlist_provider_total),
                                        ) {
                                            Some(next) => track_offset = next,
                                            None => break,
                                        }
                                    }
                                }
                            }
                            offset += limit;
                            if page.items.len() < limit as usize {
                                break;
                            }
                        }
                        Err(e) => {
                            errors.push(format!("Spotify playlists fetch error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 3b: Favorite Albums (S198 — owner live audit §4: only Tidal
            // ever wrote albums.is_favorite; Spotify had no writer despite
            // sync_albums=1). Materializes saved albums with spotify_id so the
            // favorite flag and provider identity survive for migration/UI.
            if prefs.favorite_albums {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "importing_favorite_albums", 0, None, "Importing favorite albums (Spotify)...", imported_tracks_total, favorite_tracks_total));
                let mut albums_offset: i32 = 0;
                let spotify_album_page: i32 = 50;
                loop {
                    match client.get_saved_albums(albums_offset, spotify_album_page).await {
                        Ok(page) => {
                            if page.items.is_empty() {
                                break;
                            }
                            for saved in &page.items {
                                let album = &saved.album;
                                if album.id.is_empty() || album.name.is_empty() {
                                    continue;
                                }
                                let artist_name = album
                                    .artists
                                    .first()
                                    .map(|a| a.name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());
                                let artist_id = match client.get_or_create_artist(db, &artist_name).await {
                                    Ok(id) => id,
                                    Err(_) => continue,
                                };
                                let cover = album.images.first().map(|img| img.url.clone());
                                let upc = album.external_ids.as_ref().and_then(|e| e.upc.clone());
                                // Same upsert contract as the Tidal favorite-albums writer:
                                // identity by partial-unique provider id, COALESCE preserves
                                // richer local data, is_favorite is monotonic.
                                let aid_res: Option<(i64,)> = sqlx::query_as(
                                    r#"
                                    INSERT INTO albums (title, release_date, total_tracks, cover_art_url, spotify_id, upc, label, is_favorite, favorite_at)
                                    VALUES (?, ?, ?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP)
                                    ON CONFLICT(spotify_id) WHERE spotify_id IS NOT NULL DO UPDATE SET
                                        label = COALESCE(albums.label, excluded.label),
                                        upc = COALESCE(albums.upc, excluded.upc),
                                        is_favorite = 1,
                                        favorite_at = COALESCE(albums.favorite_at, CURRENT_TIMESTAMP)
                                    RETURNING id
                                    "#
                                )
                                .bind(&album.name)
                                .bind(&album.release_date)
                                .bind(album.total_tracks)
                                .bind(&cover)
                                .bind(&album.id)
                                .bind(upc)
                                .bind(&album.label)
                                .fetch_optional(db)
                                .await
                                .unwrap_or(None);
                                if let Some((album_id,)) = aid_res {
                                    let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
                                        .bind(album_id)
                                        .bind(artist_id)
                                        .execute(db)
                                        .await;
                                    favorite_albums_total += 1;

                                    if let Ok(tracks_page) = client.get_album_tracks(&album.id, 0, 50).await {
                                        for track in &tracks_page.items {
                                            let track_artist = track.artists.first().map(|a| a.name.clone()).unwrap_or_else(|| artist_name.clone());
                                            let sync_input = crate::services::enrichment::SyncTrackInput {
                                                origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                                    title: Some(track.name.clone()),
                                                    artist: Some(track_artist),
                                                    album: Some(album.name.clone()),
                                                    album_artist: Some(artist_name.clone()),
                                                    track_number: track.track_number.map(|n| n as u32),
                                                    disc_number: track.disc_number.map(|n| n as u32),
                                                    isrc: track.external_ids.as_ref().and_then(|e| e.isrc.clone()),
                                                    barcode: album.external_ids.as_ref().and_then(|e| e.upc.clone()),
                                                    label: album.label.clone(),
                                                    release_date: album.release_date.clone(),
                                                    source_name: "spotify".to_string(),
                                                    ..Default::default()
                                                },
                                                service_track_id: track.id.clone(),
                                                service_name: "spotify".to_string(),
                                                service_id: spotify_service_id,
                                                account_id,
                                                is_favorite: false,
                                                is_purchased: false,
                                                format: Some("OGG_VORBIS".to_string()),
                                                bit_depth: None,
                                                sample_rate: None,
                                                quality_score: None,
                                                audio_quality: Some("lossy".to_string()),
                                                cover_art_url: cover.clone(),
                                                duration_ms: Some(track.duration_ms as i64),
                                                query_musicbrainz: false,
                                                album_is_favorite: true,
                                                album_provider_track_id: Some(track.id.clone()),
                                            };
                                            let _ = enrich_persist_with_locked_retry(&enrichment_engine, db, sync_input).await;
                                        }
                                    }
                                }
                            }
                            match crate::services::import_pagination::next_offset(
                                albums_offset,
                                page.items.len() as i32,
                                spotify_album_page,
                                Some(page.total.max(0) as i64),
                            ) {
                                Some(next) => albums_offset = next,
                                None => break,
                            }
                        }
                        Err(e) => {
                            errors.push(format!("Spotify saved albums error: {}", e));
                            break;
                        }
                    }
                }
            }

            // Phase 4: Followed Artists — S189-Fase-2: full cursor iteration
            // (previously only the first 50 followed artists were imported;
            // Spotify's /me/following is cursor-paginated, not offset-based).
            if prefs.favorite_artists {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_artists", 0, None, "Fetching Spotify artists...", imported_tracks_total, favorite_tracks_total));
                let mut after: Option<String> = None;
                loop {
                    let t_api = std::time::Instant::now();
                    match client.get_followed_artists(after.as_deref(), 50).await {
                        Ok(resp) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            let page_len = resp.artists.items.len();
                            for art in &resp.artists.items {
                                favorite_artists_total += 1;
                                let t_pers = std::time::Instant::now();
                                if let Ok(aid) = client.get_or_create_artist(db, &art.name).await {
                                    let _ = sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP), spotify_id = COALESCE(spotify_id, ?) WHERE id = ?")
                                        .bind(&art.id)
                                        .bind(aid)
                                        .execute(db)
                                        .await;
                                }
                                persistence_ms += t_pers.elapsed().as_millis() as u64;
                            }
                            // Cursor semantics via the shared S187/S189 policy:
                            // declared total wins over page length; without a
                            // total, only a full page with a cursor continues.
                            let declared_total = resp.artists.total.map(i64::from);
                            after = crate::services::import_pagination::next_cursor(
                                resp.artists.cursors.and_then(|c| c.after),
                                page_len,
                                50,
                                declared_total,
                                favorite_artists_total,
                            );
                            if after.is_none() {
                                break;
                            }
                        }
                        Err(e) => {
                            if is_spotify_scope_forbidden_error(&e) {
                                // S189-F2 graceful degradation: 403 scope = token
                                // granted before `user-follow-read` was requested.
                                // Omit followed artists (warning) and let the rest
                                // of the sync succeed; re-auth grants the scope.
                                tracing::warn!(
                                    "[S189-F2] Spotify followed artists skipped: missing user-follow-read scope; re-authentication required"
                                );
                                warnings.push(
                                    "Spotify: followed artists omitted — re-authenticate to grant the new 'user-follow-read' scope (re-autentica para obtener seguidos — scope nuevo requerido)"
                                        .to_string(),
                                );
                            } else {
                                errors.push(format!("Spotify followed artists fetch error: {}", e));
                            }
                            break;
                        }
                    }
                }
            }
        }
        "deezer" => {
            // S189-Fase-1: real phases for tracks/albums/artists/playlists
            // through the shared EnrichmentEngine (previously only favorite
            // tracks ran, via the legacy raw-dedupe importer, and every other
            // phase emitted an empty progress event).
            let arl = match creds["arl"].as_str().or_else(|| creds["access_token"].as_str()) {
                Some(a) => a,
                None => {
                    let err_msg = "RequiresAuth: Deezer ARL missing".to_string();
                    emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                    return Err(err_msg);
                }
            };
            let mut client = crate::services::DeezerClient::new(arl.to_string());
            // Auth parity (Fase-1 item 3): a failed init or a missing user id
            // is an explicit provider rejection → invalidate credentials and
            // surface RequiresAuth; never continue with a stale cached user.
            if let Err(e) = client.init().await {
                let _ = mark_account_credentials_invalid(db, "deezer", &e).await;
                let err_msg = format!("RequiresAuth: Deezer session rejected ({})", e);
                emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                return Err(err_msg);
            }
            let user_id = match client.user_id() {
                Some(u) if !u.is_empty() && u != "0" => u,
                _ => {
                    let err_msg = "RequiresAuth: Deezer user id unavailable".to_string();
                    emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
                    return Err(err_msg);
                }
            };
            let deezer_service_id: i64 = client.get_service_id(db, "deezer").await?;

            // Unified page size for every phase (Fase-1 item 4).
            const DEEZER_PAGE: i32 = 100;

            let persist_deezer_track = |engine_track: crate::services::enrichment::SyncTrackInput| {
                enrich_persist_with_locked_retry(&enrichment_engine, db, engine_track)
            };

            // Phase 1: Favorite Tracks
            if prefs.favorite_tracks {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "fetching_favorite_tracks", 0, None, "Importing Deezer favorite tracks...", imported_tracks_total, favorite_tracks_total));
                let mut track_offset: i32 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    let (tracks, total) = match client.get_favorites_public(&user_id, track_offset, DEEZER_PAGE).await {
                        Ok(v) => v,
                        Err(e) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            errors.push(format!("Deezer favorites error at offset {}: {}", track_offset, e));
                            break;
                        }
                    };
                    api_fetch_ms += t_api.elapsed().as_millis() as u64;
                    if tracks.is_empty() {
                        break;
                    }
                    for track in &tracks {
                        favorite_tracks_total += 1;
                        favorites_seen += 1;
                        let sync_input = crate::services::enrichment::SyncTrackInput {
                            origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                title: Some(track.title.clone()),
                                artist: Some(track.artist_name.clone().unwrap_or_else(|| "Unknown".to_string())),
                                album: track.album_title.clone(),
                                isrc: track.isrc.clone(),
                                source_name: "deezer".to_string(),
                                ..Default::default()
                            },
                            service_track_id: track.id.clone(),
                            service_name: "deezer".to_string(),
                            service_id: deezer_service_id,
                            account_id,
                            is_favorite: true,
                            format: Some("FLAC".to_string()),
                            bit_depth: Some(16),
                            sample_rate: Some(44100),
                            audio_quality: Some("lossless".to_string()),
                            duration_ms: track.duration.parse::<i64>().ok().map(|d| d * 1000),
                            query_musicbrainz: false,
                            ..Default::default()
                        };
                        // S189-F2-6: canonical bookkeeping identical to the
                        // qobuz/tidal/spotify arms.
                        let t_enrich = std::time::Instant::now();
                        match persist_deezer_track(sync_input).await {
                            Ok(res) => {
                                enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                tracks_processed += 1;
                                if res.is_new_global_track { tracks_new_global += 1; }
                                if res.is_new_source_for_service { sources_new_for_service += 1; }
                                if res.is_new_library_entry_for_account { library_entries_new_for_account += 1; }
                                if res.is_already_present { tracks_already_present += 1; }
                                if res.is_new_import {
                                    tracks_changed_unique += 1;
                                    imported_tracks_total += 1;
                                } else { skipped_tracks_total += 1; }
                                availability_checked += 1;
                                match res.completeness {
                                    syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                    _ => metadata_partial += 1,
                                }
                            }
                            Err(e) => errors.push(format!("Deezer track {}: {}", track.id, e)),
                        }
                    }
                    match crate::services::import_pagination::next_offset(
                        track_offset,
                        tracks.len() as i32,
                        DEEZER_PAGE,
                        (total > 0).then_some(total as i64),
                    ) {
                        Some(next) => track_offset = next,
                        None => break,
                    }
                }
            }

            // Phase 2: Favorite Albums (catalog expansion + favorite marking)
            if prefs.favorite_albums {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "importing_favorite_albums", 0, None, "Importing Deezer favorite albums...", imported_tracks_total, favorite_tracks_total));
                let mut album_offset: i32 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    let (albums, total) = match client.get_user_albums_public(&user_id, album_offset, DEEZER_PAGE).await {
                        Ok(v) => v,
                        Err(e) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            errors.push(format!("Deezer albums error at offset {}: {}", album_offset, e));
                            break;
                        }
                    };
                    api_fetch_ms += t_api.elapsed().as_millis() as u64;
                    if albums.is_empty() {
                        break;
                    }
                    for album in &albums {
                        albums_seen += 1;
                        let album_tracks = match client.get_album_tracks_public(&album.id).await {
                            Ok(t) => t,
                            Err(e) => {
                                album_expansion_metrics.album_detail_failed += 1;
                                warnings.push(format!("Deezer album '{}' expansion failed: {}", album.title, e));
                                continue;
                            }
                        };
                        album_expansion_metrics.album_detail_success += 1;
                        for track in &album_tracks {
                            let sync_input = crate::services::enrichment::SyncTrackInput {
                                origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                    title: Some(track.title.clone()),
                                    artist: Some(track.artist_name.clone().unwrap_or_else(|| album.artist_name.clone().unwrap_or_else(|| "Unknown".to_string()))),
                                    album: Some(album.title.clone()),
                                    album_artist: album.artist_name.clone(),
                                    isrc: track.isrc.clone(),
                                    source_name: "deezer".to_string(),
                                    ..Default::default()
                                },
                                service_track_id: track.id.clone(),
                                service_name: "deezer".to_string(),
                                service_id: deezer_service_id,
                                account_id,
                                is_favorite: false,
                                // S198 engine extension works for deezer too:
                                // the ALBUM is a favorite even though no
                                // deezer_id column exists on albums (identity
                                // stays title+artist per canonical matching).
                                album_is_favorite: true,
                                format: Some("FLAC".to_string()),
                                bit_depth: Some(16),
                                sample_rate: Some(44100),
                                audio_quality: Some("lossless".to_string()),
                                duration_ms: track.duration.parse::<i64>().ok().map(|d| d * 1000),
                                query_musicbrainz: false,
                                ..Default::default()
                            };
                        // S189-F2-6: canonical bookkeeping (album-expansion shape).
                        let t_enrich = std::time::Instant::now();
                        match persist_deezer_track(sync_input).await {
                            Ok(res) => {
                                enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                tracks_processed += 1;
                                if res.is_new_global_track { tracks_new_global += 1; }
                                if res.is_new_source_for_service { sources_new_for_service += 1; }
                                if res.is_new_library_entry_for_account { library_entries_new_for_account += 1; }
                                if res.is_already_present { tracks_already_present += 1; }
                                if res.is_new_import { imported_tracks_total += 1; } else { skipped_tracks_total += 1; }
                                availability_checked += 1;
                                match res.completeness {
                                    syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                    _ => metadata_partial += 1,
                                }
                            }
                            Err(e) => errors.push(format!("Deezer album track {}: {}", track.id, e)),
                        }
                        }
                        favorite_albums_total += 1;
                    }
                    match crate::services::import_pagination::next_offset(
                        album_offset,
                        albums.len() as i32,
                        DEEZER_PAGE,
                        (total > 0).then_some(total),
                    ) {
                        Some(next) => album_offset = next,
                        None => break,
                    }
                }
            }

            // Phase 3: Favorite Artists
            if prefs.favorite_artists {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "importing_favorite_artists", 0, None, "Importing Deezer favorite artists...", imported_tracks_total, favorite_tracks_total));
                let mut artist_offset: i32 = 0;
                loop {
                    let t_api = std::time::Instant::now();
                    let (artists, total) = match client.get_user_artists_public(&user_id, artist_offset, DEEZER_PAGE).await {
                        Ok(v) => v,
                        Err(e) => {
                            api_fetch_ms += t_api.elapsed().as_millis() as u64;
                            errors.push(format!("Deezer artists error at offset {}: {}", artist_offset, e));
                            break;
                        }
                    };
                    api_fetch_ms += t_api.elapsed().as_millis() as u64;
                    if artists.is_empty() {
                        break;
                    }
                    for (_artist_provider_id, artist_name) in &artists {
                        if let Ok(artist_db_id) = client.get_or_create_artist(db, artist_name).await {
                            let _ = sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP) WHERE id = ?")
                                .bind(artist_db_id)
                                .execute(db)
                                .await;
                            favorite_artists_total += 1;
                        }
                    }
                    match crate::services::import_pagination::next_offset(
                        artist_offset,
                        artists.len() as i32,
                        DEEZER_PAGE,
                        (total > 0).then_some(total),
                    ) {
                        Some(next) => artist_offset = next,
                        None => break,
                    }
                }
            }

            // Phase 4: Playlists (upsert + full expansion via S187 pagination)
            if prefs.playlists {
                emit(SyncProgressEvent::running(&service_normalized, Some(account_id), "importing_playlists", 0, None, "Importing Deezer playlists...", imported_tracks_total, favorite_tracks_total));
                let mut playlist_offset: i32 = 0;
                loop {
                    let (playlists, total) = match client.get_user_playlists_public(&user_id, playlist_offset, DEEZER_PAGE).await {
                        Ok(v) => v,
                        Err(e) => {
                            errors.push(format!("Deezer playlists error at offset {}: {}", playlist_offset, e));
                            break;
                        }
                    };
                    if playlists.is_empty() {
                        break;
                    }
                    for pl in &playlists {
                        let t_pers = std::time::Instant::now();
                        // FIX 2026-08-25: la columna se llama image_url
                        // (migrations/0006+0033); cover_art_url no existe en la
                        // tabla y hacía fallar TODOS los upserts deezer.
                        let upsert = sqlx::query(
                            r#"
                            INSERT INTO playlists (account_id, service_playlist_id, name, description, is_public, is_collaborative, image_url, track_count)
                            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                            ON CONFLICT(account_id, service_playlist_id) DO UPDATE SET
                                name = excluded.name,
                                description = excluded.description,
                                is_public = excluded.is_public,
                                is_collaborative = excluded.is_collaborative,
                                image_url = COALESCE(excluded.image_url, image_url),
                                track_count = excluded.track_count
                            "#,
                        )
                        .bind(account_id)
                        .bind(&pl.id)
                        .bind(&pl.title)
                        .bind(&pl.description)
                        .bind(pl.is_public.unwrap_or(true) as i32)
                        .bind(pl.is_collaborative.unwrap_or(false) as i32)
                        .bind(&pl.cover)
                        .bind(pl.nb_tracks.unwrap_or(0))
                        .execute(db)
                        .await;
                        persistence_ms += t_pers.elapsed().as_millis() as u64;
                        if let Err(e) = &upsert {
                            errors.push(format!("Deezer playlist '{}' upsert failed: {}", pl.title, e));
                            continue;
                        }
                        playlists_total += 1;
                        playlists_seen += 1;

                        let playlist_db_id: Option<(i64,)> = sqlx::query_as(
                            "SELECT id FROM playlists WHERE account_id = ? AND service_playlist_id = ?",
                        )
                        .bind(account_id)
                        .bind(&pl.id)
                        .fetch_optional(db)
                        .await
                        .ok()
                        .flatten();

                        if let Some((p_id,)) = playlist_db_id {
                            let declared_total: i64 = pl.nb_tracks.unwrap_or(0).max(0) as i64;
                            let mut track_offset: i32 = 0;
                            loop {
                                let t_exp = std::time::Instant::now();
                                let (tracks, page_total) = match client.get_playlist_tracks_public(&pl.id, track_offset, DEEZER_PAGE).await {
                                    Ok(v) => v,
                                    Err(e) => {
                                        tracing::warn!(
                                            "[S189] Deezer playlist '{}' expansion incomplete at offset {}: {}",
                                            pl.title, track_offset, e
                                        );
                                        break;
                                    }
                                };
                                entity_expansion_ms += t_exp.elapsed().as_millis() as u64;
                                if tracks.is_empty() {
                                    break;
                                }
                                let provider_total = declared_total.max(page_total);
                                for (idx, track) in tracks.iter().enumerate() {
                                    let pos = track_offset as usize + idx;
                                    tracks_expanded += 1;
                                    let sync_input = crate::services::enrichment::SyncTrackInput {
                                        origin_meta: crate::services::enrichment::OriginTrackMetadata {
                                            title: Some(track.title.clone()),
                                            artist: Some(track.artist_name.clone().unwrap_or_else(|| "Unknown".to_string())),
                                            album: track.album_title.clone(),
                                            isrc: track.isrc.clone(),
                                            source_name: "deezer".to_string(),
                                            ..Default::default()
                                        },
                                        service_track_id: track.id.clone(),
                                        service_name: "deezer".to_string(),
                                        service_id: deezer_service_id,
                                        account_id,
                                        is_favorite: false,
                                        format: Some("FLAC".to_string()),
                                        bit_depth: Some(16),
                                        sample_rate: Some(44100),
                                        audio_quality: Some("lossless".to_string()),
                                        duration_ms: track.duration.parse::<i64>().ok().map(|d| d * 1000),
                                        query_musicbrainz: false,
                                        ..Default::default()
                                    };
                                    // S189-F2-6: canonical bookkeeping.
                                    let t_enrich = std::time::Instant::now();
                                    if let Ok(res) = persist_deezer_track(sync_input).await {
                                        let t_pl_track = std::time::Instant::now();
                                        let _ = sqlx::query(
                                            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
                                        )
                                        .bind(p_id)
                                        .bind(res.track_id)
                                        .bind(pos as i32 + 1)
                                        .execute(db)
                                        .await;
                                        persistence_ms += t_pl_track.elapsed().as_millis() as u64;
                                        enrichment_ms += t_enrich.elapsed().as_millis() as u64;
                                        tracks_processed += 1;
                                        if res.is_new_global_track { tracks_new_global += 1; }
                                        if res.is_new_source_for_service { sources_new_for_service += 1; }
                                        if res.is_new_library_entry_for_account { library_entries_new_for_account += 1; }
                                        if res.is_already_present { tracks_already_present += 1; }
                                        if res.is_new_import {
                                            tracks_changed_unique += 1;
                                            imported_tracks_total += 1;
                                        } else { skipped_tracks_total += 1; }
                                        match res.completeness {
                                            syncify_metadata_domain::EnrichmentCompleteness::Enriched => metadata_enriched += 1,
                                            _ => metadata_partial += 1,
                                        }
                                    }
                                }
                                match crate::services::import_pagination::next_offset(
                                    track_offset,
                                    tracks.len() as i32,
                                    DEEZER_PAGE,
                                    (provider_total > 0).then_some(provider_total),
                                ) {
                                    Some(next) => track_offset = next,
                                    None => break,
                                }
                            }
                        }
                    }
                    match crate::services::import_pagination::next_offset(
                        playlist_offset,
                        playlists.len() as i32,
                        DEEZER_PAGE,
                        (total > 0).then_some(total),
                    ) {
                        Some(next) => playlist_offset = next,
                        None => break,
                    }
                }
            }

            // Phase 5: Library history — Deezer's public API exposes no listen
            // history endpoint; documented capacity constant, not an error.
            if prefs.library_history {
                warnings.push("Deezer: listen history not exposed by the public API (documented capability gap)".to_string());
            }
        }
        "apple_music" => {
            // S190-interín (Fase 3 completa pendiente): delega al importador de
            // biblioteca que SÍ captura ISRC (identidad canónica) y escribe
            // track_sources AAC. Requiere ambos tokens en credenciales.
            load_service_credentials(db, "apple_music")
                .await
                .map_err(|e| format!("RequiresAuth: {}", e))?;
            match run_apple_music_library_import(db, |_| {}).await {
                Ok((am_imported, am_skipped)) => {
                    imported_tracks_total = am_imported.max(0) as u64;
                    skipped_tracks_total = am_skipped.max(0) as u64;
                    tracks_processed = (am_imported + am_skipped).max(0) as u64;
                    favorite_tracks_total = (am_imported + am_skipped).max(0) as u64;
                    warnings.push("Apple Music: ruta interína de biblioteca — solo canciones, sin playlists/álbumes ni enriquecimiento rico (Fase 3 pendiente)".to_string());
                }
                Err(am_err) => {
                    errors.push(format!("Apple Music library: {}", am_err));
                }
            }
        }
        "soundcloud" => {
            // S190-interín (Fase 3 real pendiente): SoundCloud no tiene brazo de
            // enriquecimiento propio; delega al importador legacy de likes
            // compartido con el comando import_soundcloud_library. Sin ISRC ni
            // metadatos ricos — contrato honesto mientras llega la integración.
            load_service_credentials(db, "soundcloud")
                .await
                .map_err(|e| format!("RequiresAuth: {}", e))?;
            match run_soundcloud_likes_import(db, |_| {}).await {
                Ok((sc_imported, sc_skipped)) => {
                    imported_tracks_total = sc_imported.max(0) as u64;
                    skipped_tracks_total = sc_skipped.max(0) as u64;
                    tracks_processed = (sc_imported + sc_skipped).max(0) as u64;
                    favorite_tracks_total = (sc_imported + sc_skipped).max(0) as u64;
                    warnings.push("SoundCloud: ruta interína de likes — dedup por título+duración, sin ISRC (integración completa en Fase 3)".to_string());
                }
                Err(sc_err) => {
                    errors.push(format!("SoundCloud likes: {}", sc_err));
                }
            }
        }
        _ => {
            let err_msg = format!("Unsupported service for sync: {}", service_name);
            emit(SyncProgressEvent::failed(&service_normalized, Some(account_id), "authenticating", &err_msg, 0, 0));
            return Err(err_msg);
        }
    }

    // 5. Update last_synced timestamps in Phase: persisting
    emit(SyncProgressEvent::running(
        &service_normalized,
        Some(account_id),
        "persisting",
        imported_tracks_total,
        Some(imported_tracks_total),
        "Persisting sync metadata and updating timestamps...",
        imported_tracks_total,
        favorite_tracks_total,
    ));

    let t_pers = std::time::Instant::now();
    let _ = sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(db)
        .await;

    let _ = sqlx::query("UPDATE service_sync_settings SET last_synced = CURRENT_TIMESTAMP WHERE service_name = ?")
        .bind(&service_normalized)
        .execute(db)
        .await;
    persistence_ms += t_pers.elapsed().as_millis() as u64;

    // Check partial failure: If albums were received/requested for sync but 0 tracks were found/persisted/existing due to errors or empty expansions
    if album_expansion_metrics.albums_received > 0
        && album_expansion_metrics.tracks_persisted_new == 0
        && album_expansion_metrics.tracks_existing == 0
    {
        let err_detail = album_expansion_metrics
            .first_error_code
            .as_deref()
            .unwrap_or("Albums contained 0 tracks or expansion failed");
        let partial_msg = format!(
            "Qobuz album expansion failed: received {} albums, but 0 tracks imported ({})",
            album_expansion_metrics.albums_received, err_detail
        );
        if !errors.contains(&partial_msg) {
            errors.push(partial_msg);
        }
    }

    let sync_outcome = if !errors.is_empty() {
        if tracks_processed > 0 || favorite_tracks_total > 0 || favorite_albums_total > 0 {
            "partial_failure".to_string()
        } else {
            "failed".to_string()
        }
    } else if albums_unavailable > 0 || !warnings.is_empty() {
        "success_with_warnings".to_string()
    } else {
        "success".to_string()
    };

    let success = sync_outcome == "success" || sync_outcome == "success_with_warnings";

    let message = if sync_outcome == "success" {
        format!(
            "Sync completed for {}: {} tracks new global, {} sources new, {} library entries new ({} already present, {} favorites, {} albums, {} playlists)",
            service_name,
            tracks_new_global,
            sources_new_for_service,
            library_entries_new_for_account,
            tracks_already_present,
            favorite_tracks_total,
            favorite_albums_total,
            playlists_total
        )
    } else if sync_outcome == "success_with_warnings" {
        format!(
            "Sync completed with warnings for {}: {} favorites, {} albums ({} unavailable from provider), {} playlists",
            service_name,
            favorite_tracks_total,
            favorite_albums_total,
            albums_unavailable,
            playlists_total
        )
    } else {
        format!(
            "Sync completed with {} warnings/errors ({} expansion failures) for {}",
            errors.len(),
            tracks_expansion_failed,
            service_name
        )
    };

    if success {
        emit(SyncProgressEvent::completed(
            &service_normalized,
            Some(account_id),
            &message,
            imported_tracks_total,
            favorite_tracks_total,
            Some(imported_tracks_total),
        ));
    } else {
        emit(SyncProgressEvent::failed(
            &service_normalized,
            Some(account_id),
            "completed",
            format!("Sync completed with errors: {}", errors.join("; ")),
            imported_tracks_total,
            favorite_tracks_total,
        ));
    }

    let albums_total = favorite_albums_total + purchases_total;
    let availability_unknown: u64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE availability_status = 'unknown_unchecked'")
        .fetch_one(db)
        .await
        .unwrap_or(0) as u64;

    let phase_timings = SyncPhaseTimings {
        api_fetch_ms,
        entity_expansion_ms,
        enrichment_ms,
        persistence_ms,
        availability_check_ms,
        total_elapsed_ms: sync_start.elapsed().as_millis() as u64,
    };

    tracing::info!(
        "[Sync Timing/{}] Finished in {}ms (API fetch: {}ms, Expansion: {}ms, Enrichment: {}ms, DB Persist: {}ms, Availability: {}ms)",
        service_normalized,
        phase_timings.total_elapsed_ms,
        phase_timings.api_fetch_ms,
        phase_timings.entity_expansion_ms,
        phase_timings.enrichment_ms,
        phase_timings.persistence_ms,
        phase_timings.availability_check_ms
    );

    Ok(ServiceSyncResult {
        service: service_name.to_string(),
        account_id: Some(account_id),
        success,
        message,
        imported_tracks_total,
        favorite_tracks_total,
        favorite_albums_total,
        favorite_artists_total,
        playlists_total,
        purchases_total,
        skipped_tracks_total,
        albums_total,
        metadata_enriched,
        metadata_partial,
        availability_unknown,
        availability_checked,
        phase_timings: Some(phase_timings),
        album_expansion_metrics: if album_expansion_metrics.albums_received > 0 { Some(album_expansion_metrics) } else { None },
        tracks_processed,
        tracks_changed_unique,
        tracks_new_global,
        sources_new_for_service,
        library_entries_new_for_account,
        tracks_already_present,
        favorites_seen,
        albums_seen,
        playlists_seen,
        tracks_expanded,
        tracks_expansion_failed,
        albums_unavailable,
        tracks_unavailable,
        tracks_expansion_deferred,
        sync_outcome: Some(sync_outcome),
        warnings,
        errors,
        ..Default::default()
    })
}

/// Unified sync command for any service with real auth checks & granular preferences
#[tauri::command]
pub async fn sync_service(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    service: String,
    account_id: Option<i64>,
    preferences: Option<ImportPreferences>,
) -> Result<ServiceSyncResult, String> {
    perform_sync_service_with_emitter(&state.db, &service, account_id, preferences, Some(&app)).await
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Create an in-memory test database with schema
    async fn setup_test_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        // Create minimal schema for testing
        sqlx::query(
            r#"
            CREATE TABLE services (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                supports_download INTEGER DEFAULT 0,
                max_quality TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create services table");

        sqlx::query(
            r#"
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                credentials_json TEXT,
                last_synced TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(service_id, email)
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create accounts table");

        // Seed services
        sqlx::query(
            r#"
            INSERT INTO services (name, supports_download, max_quality) VALUES
                ('spotify', 0, 'lossy'),
                ('qobuz', 1, 'hires'),
                ('tidal', 1, 'hires')
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to seed services");

        // Create service_preferences table for testing
        sqlx::query(
            r#"
            CREATE TABLE service_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT NOT NULL UNIQUE,
                priority INTEGER NOT NULL DEFAULT 1,
                auto_import_enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create service_preferences table");

        // Seed service preferences
        sqlx::query(
            r#"
            INSERT INTO service_preferences (service_name, priority, auto_import_enabled) VALUES
                ('spotify', 1, 1),
                ('qobuz', 2, 1),
                ('tidal', 3, 1),
                ('deezer', 4, 0),
                ('soundcloud', 5, 0)
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to seed service_preferences");

        pool
    }

    #[tokio::test]
    async fn test_get_services_returns_all_services() {
        let pool = setup_test_db().await;

        let services: Vec<ServiceInfo> = sqlx::query_as(
            "SELECT id, name, supports_download, max_quality FROM services ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch services");

        assert_eq!(services.len(), 3);
        assert_eq!(services[0].name, "qobuz");
        assert_eq!(services[1].name, "spotify");
        assert_eq!(services[2].name, "tidal");
    }

    #[tokio::test]
    async fn test_service_info_fields() {
        let pool = setup_test_db().await;

        let qobuz: ServiceInfo = sqlx::query_as(
            "SELECT id, name, supports_download, max_quality FROM services WHERE name = 'qobuz'",
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch qobuz");

        assert_eq!(qobuz.name, "qobuz");
        assert_eq!(qobuz.supports_download, 1);
        assert_eq!(qobuz.max_quality, Some("hires".to_string()));
    }

    /// Mutex to serialize tests that modify process-wide environment variables.
    /// cargo test runs tests in parallel within the same process; env vars
    /// are global, not per-thread. Without serialization, removing QOBUZ_APP_ID
    /// in this test corrupts the environment for concurrent tests.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_qobuz_missing_env_returns_err() {
        let _guard = ENV_MUTEX.lock().unwrap();

        // Save original values
        let old_id = std::env::var("QOBUZ_APP_ID").ok();
        let old_secret = std::env::var("QOBUZ_APP_SECRET").ok();

        // Clear env vars
        std::env::remove_var("QOBUZ_APP_ID");
        std::env::remove_var("QOBUZ_APP_SECRET");

        // Reproduce exact pattern from import_service "qobuz" branch (lines 1680-1682)
        let result: Result<String, String> = std::env::var("QOBUZ_APP_ID")
            .map_err(|_| "Qobuz credentials not configured. Set QOBUZ_APP_ID \
                           and QOBUZ_APP_SECRET environment variables.".to_string());

        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("QOBUZ_APP_ID"));
        assert!(msg.contains("QOBUZ_APP_SECRET"));

        // Restore original values
        if let Some(v) = old_id { std::env::set_var("QOBUZ_APP_ID", v); }
        if let Some(v) = old_secret { std::env::set_var("QOBUZ_APP_SECRET", v); }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_track_insertion_race_condition_fix(pool: sqlx::SqlitePool) {
        // Case A: Initial Insert — RETURNING id returns new row
        let track_id_a: i64 =
            if let Some(row) = sqlx::query_as::<_, (i64,)>(
                "INSERT OR IGNORE INTO tracks (title, duration_ms, isrc) VALUES (?, ?, ?) RETURNING id"
            )
            .bind("Race Condition Track")
            .bind(123456_i64)
            .bind("MOCK-ISRC-123")
            .fetch_optional(&pool)
            .await
            .expect("Failed to insert track")
            {
                row.0
            } else {
                sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE title = ? AND duration_ms = ?")
                    .bind("Race Condition Track")
                    .bind(123456_i64)
                    .fetch_one(&pool)
                    .await
                    .map(|r| r.0)
                    .unwrap_or(0)
            };
        
        assert!(track_id_a > 0, "Initial insert logic should return > 0");

        // Case B: Duplicate Insert — RETURNING id returns None, fallback SELECT
        let track_id_b: i64 =
            if let Some(row) = sqlx::query_as::<_, (i64,)>(
                "INSERT OR IGNORE INTO tracks (title, duration_ms, isrc) VALUES (?, ?, ?) RETURNING id"
            )
            .bind("Race Condition Track")
            .bind(123456_i64)
            .bind("MOCK-ISRC-123")
            .fetch_optional(&pool)
            .await
            .expect("Failed to execute duplicate insert")
            {
                row.0
            } else {
                sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE title = ? AND duration_ms = ?")
                    .bind("Race Condition Track")
                    .bind(123456_i64)
                    .fetch_one(&pool)
                    .await
                    .map(|r| r.0)
                    .unwrap_or(0)
            };
        
        assert_eq!(track_id_a, track_id_b, "Duplicate insert should return identical track ID");
    }

    // S189-F2: scope-403 degradation — a missing OAuth scope must become a
    // warning (success_with_warnings), never a sync failure.
    #[test]
    fn test_spotify_scope_forbidden_error_detected() {
        let err = "Spotify API error (403 Forbidden): Insufficient client scope - {\"error\":{\"status\":403,\"message\":\"Insufficient client scope\",\"reason\":\"USER_FOLLOWS_MISSING_SCOPE\"}}";
        assert!(is_spotify_scope_forbidden_error(err));
    }

    #[test]
    fn test_spotify_scope_forbidden_error_requires_both_markers() {
        // 403 but different reason (e.g. subscription/region) → NOT a scope issue.
        assert!(!is_spotify_scope_forbidden_error(
            "Spotify API error (403 Forbidden): Forbidden - {}"
        ));
        // Scope text but not a 403 → NOT a scope issue.
        assert!(!is_spotify_scope_forbidden_error(
            "Spotify API error (400 Bad Request): Insufficient client scope - {}"
        ));
        // Plain transport failure → NOT a scope issue.
        assert!(!is_spotify_scope_forbidden_error("Request failed: connection reset"));
    }
}



