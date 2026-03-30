// Migration Commands - included via include!() in mod.rs
// 
// Service-to-service migration, templates, matching


// ==============================================
// SPRINT 6: MIGRATION COMMANDS
// ==============================================

use crate::models::{
    DestinationTrackMatch, MigrationItem, MigrationJob, MigrationOptions, MigrationPreviewResult,
    MigrationProgress, MigrationTemplate, PlaylistPreview,
};

/// Get migration history
#[tauri::command]
pub async fn get_migration_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<MigrationJob>, String> {
    let limit = limit.unwrap_or(50);
    sqlx::query_as::<_, MigrationJob>(
        r#"SELECT id, source_service, destination_service, source_playlist_ids, options, status,
            total_items, completed_items, failed_items, skipped_items, started_at, completed_at,
            error_message, created_at FROM migration_jobs ORDER BY created_at DESC LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to get migration history: {}", e))
}

/// Get migration job details
#[tauri::command]
pub async fn get_migration_details(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<MigrationJob, String> {
    sqlx::query_as::<_, MigrationJob>("SELECT * FROM migration_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| format!("Migration job {} not found", job_id))
}

/// Get migration items by status
#[tauri::command]
pub async fn get_migration_items_by_status(
    state: State<'_, AppState>,
    job_id: String,
    status: Option<String>,
) -> Result<Vec<MigrationItem>, String> {
    let query = if let Some(s) = status {
        sqlx::query_as::<_, MigrationItem>(
            "SELECT * FROM migration_items WHERE job_id = ? AND status = ? ORDER BY id",
        )
        .bind(&job_id)
        .bind(&s)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, MigrationItem>(
            "SELECT * FROM migration_items WHERE job_id = ? ORDER BY id",
        )
        .bind(&job_id)
        .fetch_all(&state.db)
        .await
    };
    query.map_err(|e| format!("Failed to get migration items: {}", e))
}

/// Preview a migration before starting
#[tauri::command]
pub async fn preview_migration(
    state: State<'_, AppState>,
    source_service: String,
    _destination_service: String,
    playlist_ids: Option<Vec<String>>,
    options: MigrationOptions,
) -> Result<MigrationPreviewResult, String> {
    // For preview, we count the tracks that would be migrated
    // In a real implementation, this would query the source service API
    let mut total_tracks = 0i64;
    let mut playlists: Vec<PlaylistPreview> = Vec::new();

    // If specific playlists selected, get their info
    if let Some(ids) = &playlist_ids {
        for id in ids {
            // Get playlist info from our database (if imported)
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM library_items li 
                 JOIN playlist_tracks pt ON pt.track_id = li.id 
                 WHERE pt.playlist_id = (SELECT id FROM playlists WHERE external_id = ?)",
            )
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap_or((0,));

            total_tracks += count.0;
            playlists.push(PlaylistPreview {
                id: id.clone(),
                name: format!("Playlist {}", id),
                track_count: count.0,
                matched_count: (count.0 as f64 * options.match_threshold) as i64,
            });
        }
    } else {
        // All favorites from source service
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM library_items WHERE source_service = ?")
                .bind(&source_service)
                .fetch_one(&state.db)
                .await
                .unwrap_or((0,));
        total_tracks = count.0;
    }

    let matched_tracks = (total_tracks as f64 * 0.85) as i64; // Estimate
    Ok(MigrationPreviewResult {
        total_tracks,
        matched_tracks,
        unmatched_tracks: total_tracks - matched_tracks,
        playlists,
    })
}

/// Start a new migration
#[tauri::command]
pub async fn start_migration(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    source_service: String,
    destination_service: String,
    playlist_ids: Option<Vec<String>>,
    options: MigrationOptions,
) -> Result<String, String> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let options_json = serde_json::to_string(&options).map_err(|e| e.to_string())?;
    let playlist_ids_json = playlist_ids
        .as_ref()
        .map(|ids| serde_json::to_string(ids).ok())
        .flatten();

    // Create migration job
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, source_playlist_ids, options, status)
           VALUES (?, ?, ?, ?, ?, 'pending')"#
    )
    .bind(&job_id)
    .bind(&source_service)
    .bind(&destination_service)
    .bind(&playlist_ids_json)
    .bind(&options_json)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to create migration job: {}", e))?;

    // Get tracks to migrate
    let tracks: Vec<(i64, String, String, String, Option<String>)> = if playlist_ids.is_some() {
        sqlx::query_as(
            "SELECT li.id, li.external_id, li.title, li.artist, li.album FROM library_items li
             JOIN playlist_tracks pt ON pt.track_id = li.id
             JOIN playlists p ON p.id = pt.playlist_id
             WHERE p.source_service = ? LIMIT 1000",
        )
        .bind(&source_service)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT id, external_id, title, artist, album FROM library_items WHERE source_service = ? LIMIT 1000"
        )
        .bind(&source_service)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let total_items = tracks.len() as i64;

    // Update job with total items
    sqlx::query("UPDATE migration_jobs SET total_items = ?, status = 'running', started_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(total_items)
        .bind(&job_id)
        .execute(&state.db)
        .await
        .ok();

    // Insert migration items
    for (_, ext_id, title, artist, album) in &tracks {
        sqlx::query(
            r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist, source_track_album, status)
               VALUES (?, ?, ?, ?, ?, 'pending')"#
        )
        .bind(&job_id)
        .bind(ext_id)
        .bind(title)
        .bind(artist)
        .bind(album)
        .execute(&state.db)
        .await
        .ok();
    }

    // Emit initial progress
    let _ = app.emit(
        "migration-progress",
        MigrationProgress {
            job_id: job_id.clone(),
            current_item: 0,
            total_items,
            current_track: "Starting migration...".to_string(),
            status: "running".to_string(),
            completed_count: 0,
            failed_count: 0,
            skipped_count: 0,
        },
    );

    // Initialize Qobuz client for matching (if destination is Qobuz)
    let qobuz_client: Option<crate::services::QobuzClient> =
        if destination_service.to_lowercase() == "qobuz" {
            // Get Qobuz credentials from database
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT credentials FROM accounts WHERE service_name = 'qobuz' AND is_active = 1",
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                    if let Some(token) = creds.get("user_auth_token").and_then(|v| v.as_str()) {
                        let app_id = std::env::var("QOBUZ_APP_ID")
                            .unwrap_or_else(|_| "950096963".to_string());
                        let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_default();
                        Some(crate::services::QobuzClient::new_with_token(
                            app_id,
                            app_secret,
                            token.to_string(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

    // Initialize Tidal client for matching (if destination is Tidal)
    let tidal_client: Option<crate::services::TidalClient> =
        if destination_service.to_lowercase() == "tidal" {
            // Get Tidal credentials from database
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT credentials FROM accounts WHERE service_name = 'tidal' AND is_active = 1",
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                    let access_token = creds.get("access_token").and_then(|v| v.as_str());
                    let user_id = creds.get("user_id").and_then(|v| v.as_str());
                    let country_code = creds
                        .get("country_code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("US");

                    if let (Some(token), Some(uid)) = (access_token, user_id) {
                        Some(
                            crate::services::TidalClient::new(token.to_string())
                                .with_user(uid.to_string(), country_code.to_string()),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

    // Initialize Spotify client for matching (if destination is Spotify)
    let spotify_client: Option<crate::services::SpotifyClient> =
        if destination_service.to_lowercase() == "spotify" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT credentials FROM accounts WHERE service_name = 'spotify' AND is_active = 1",
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                    if let Some(token) = creds.get("access_token").and_then(|v| v.as_str()) {
                        Some(crate::services::SpotifyClient::new(token.to_string(), None))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

    // Initialize Deezer client for matching (if destination is Deezer)
    let deezer_client: Option<crate::services::DeezerClient> =
        if destination_service.to_lowercase() == "deezer" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT credentials FROM accounts WHERE service_name = 'deezer' AND is_active = 1",
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                    if let Some(arl) = creds.get("arl").and_then(|v| v.as_str()) {
                        let mut client = crate::services::DeezerClient::new(arl.to_string());
                        // Initialize the client to get API token
                        if client.init().await.is_ok() {
                            Some(client)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

    // Initialize SoundCloud client for matching (if destination is SoundCloud)
    let soundcloud_client: Option<crate::services::SoundCloudClient> =
        if destination_service.to_lowercase() == "soundcloud" {
            let creds: Option<(String,)> = sqlx::query_as(
            "SELECT credentials FROM accounts WHERE service_name = 'soundcloud' AND is_active = 1"
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

            if let Some((creds_json,)) = creds {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                    let oauth_token = creds.get("oauth_token").and_then(|v| v.as_str());
                    let user_id = creds.get("user_id").and_then(|v| v.as_i64());

                    if let (Some(token), Some(uid)) = (oauth_token, user_id) {
                        Some(
                            crate::services::SoundCloudClient::new(token.to_string())
                                .with_user_id(uid),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

    // Process tracks with real matching
    let mut completed = 0i64;
    let mut failed = 0i64;
    let mut skipped = 0i64;

    for (i, (_, ext_id, title, artist, _)) in tracks.iter().enumerate() {
        // Check if cancelled
        let job: Option<(String,)> =
            sqlx::query_as("SELECT status FROM migration_jobs WHERE id = ?")
                .bind(&job_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

        if job.as_ref().map(|j| j.0.as_str()) == Some("cancelled") {
            break;
        }

        // Try to get ISRC for this track from our database
        let isrc: Option<(String,)> = sqlx::query_as(
            "SELECT t.isrc FROM tracks t 
             JOIN track_sources ts ON ts.track_id = t.id 
             WHERE ts.service_track_id = ? AND t.isrc IS NOT NULL LIMIT 1",
        )
        .bind(ext_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let (match_confidence, match_method, dest_track_id): (f64, &str, Option<String>) =
            if let Some(ref client) = qobuz_client {
                // Real Qobuz matching
                if let Some((track_isrc,)) = &isrc {
                    // Try ISRC match first (most reliable)
                    match client.search_by_isrc(track_isrc).await {
                        Ok(Some(result)) => {
                            // Try to add to favorites
                            match client.add_to_favorites(&result.track_id).await {
                                Ok(_) => (1.0, "isrc", Some(result.track_id)),
                                Err(e) => {
                                    tracing::warn!("Failed to add to favorites: {}", e);
                                    (1.0, "isrc", None) // Match found but transfer failed
                                }
                            }
                        }
                        Ok(None) => {
                            // ISRC not found, try metadata
                            match client.match_by_metadata(title, artist).await {
                                Ok(Some(result)) => {
                                    match client.add_to_favorites(&result.track_id).await {
                                        Ok(_) => (0.85, "metadata", Some(result.track_id)),
                                        Err(_) => (0.85, "metadata", None),
                                    }
                                }
                                _ => (0.0, "none", None),
                            }
                        }
                        Err(e) => {
                            tracing::warn!("ISRC search failed: {}", e);
                            (0.0, "none", None)
                        }
                    }
                } else {
                    // No ISRC, try metadata matching
                    match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (0.80, "metadata", Some(result.track_id)),
                            Err(_) => (0.80, "metadata", None),
                        },
                        _ => (0.0, "none", None),
                    }
                }
            } else if let Some(ref client) = tidal_client {
                // Real Tidal matching
                if let Some((track_isrc,)) = &isrc {
                    match client.search_by_isrc(track_isrc).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (1.0, "isrc", Some(result.track_id)),
                            Err(e) => {
                                tracing::warn!("Tidal add to favorites failed: {}", e);
                                (1.0, "isrc", None)
                            }
                        },
                        Ok(None) => match client.match_by_metadata(title, artist).await {
                            Ok(Some(result)) => {
                                match client.add_to_favorites(&result.track_id).await {
                                    Ok(_) => (0.85, "metadata", Some(result.track_id)),
                                    Err(_) => (0.85, "metadata", None),
                                }
                            }
                            _ => (0.0, "none", None),
                        },
                        Err(e) => {
                            tracing::warn!("Tidal ISRC search failed: {}", e);
                            (0.0, "none", None)
                        }
                    }
                } else {
                    match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (0.80, "metadata", Some(result.track_id)),
                            Err(_) => (0.80, "metadata", None),
                        },
                        _ => (0.0, "none", None),
                    }
                }
            } else if let Some(ref client) = spotify_client {
                // Real Spotify matching
                if let Some((track_isrc,)) = &isrc {
                    match client.search_by_isrc(track_isrc).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (1.0, "isrc", Some(result.track_id)),
                            Err(e) => {
                                tracing::warn!("Spotify add to favorites failed: {}", e);
                                (1.0, "isrc", None)
                            }
                        },
                        Ok(None) => match client.match_by_metadata(title, artist).await {
                            Ok(Some(result)) => {
                                match client.add_to_favorites(&result.track_id).await {
                                    Ok(_) => (0.85, "metadata", Some(result.track_id)),
                                    Err(_) => (0.85, "metadata", None),
                                }
                            }
                            _ => (0.0, "none", None),
                        },
                        Err(e) => {
                            tracing::warn!("Spotify ISRC search failed: {}", e);
                            (0.0, "none", None)
                        }
                    }
                } else {
                    match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (0.80, "metadata", Some(result.track_id)),
                            Err(_) => (0.80, "metadata", None),
                        },
                        _ => (0.0, "none", None),
                    }
                }
            } else if let Some(ref client) = deezer_client {
                // Real Deezer matching
                if let Some((track_isrc,)) = &isrc {
                    match client.search_by_isrc(track_isrc).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (1.0, "isrc", Some(result.track_id)),
                            Err(e) => {
                                tracing::warn!("Deezer add to favorites failed: {}", e);
                                (1.0, "isrc", None)
                            }
                        },
                        Ok(None) => match client.match_by_metadata(title, artist).await {
                            Ok(Some(result)) => {
                                match client.add_to_favorites(&result.track_id).await {
                                    Ok(_) => (0.85, "metadata", Some(result.track_id)),
                                    Err(_) => (0.85, "metadata", None),
                                }
                            }
                            _ => (0.0, "none", None),
                        },
                        Err(e) => {
                            tracing::warn!("Deezer ISRC search failed: {}", e);
                            (0.0, "none", None)
                        }
                    }
                } else {
                    match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                            Ok(_) => (0.80, "metadata", Some(result.track_id)),
                            Err(_) => (0.80, "metadata", None),
                        },
                        _ => (0.0, "none", None),
                    }
                }
            } else if let Some(ref client) = soundcloud_client {
                // Real SoundCloud matching (no ISRC support)
                match client.match_by_metadata(title, artist).await {
                    Ok(Some(result)) => match client.add_to_favorites(&result.track_id).await {
                        Ok(_) => (0.75, "metadata", Some(result.track_id)),
                        Err(e) => {
                            tracing::warn!("SoundCloud add to favorites failed: {}", e);
                            (0.75, "metadata", None)
                        }
                    },
                    _ => (0.0, "none", None),
                }
            } else {
                // No destination client available, use simulated matching
                (0.85, "simulated", None)
            };

        // Determine status based on match
        let status = if match_confidence >= options.match_threshold && dest_track_id.is_some() {
            completed += 1;
            "transferred"
        } else if match_confidence >= options.match_threshold && dest_track_id.is_none() {
            // Match found but transfer failed
            failed += 1;
            "failed"
        } else if options.skip_unmatched {
            skipped += 1;
            "skipped"
        } else {
            failed += 1;
            "failed"
        };

        // Update item with match details
        sqlx::query(
            "UPDATE migration_items SET status = ?, match_confidence = ?, match_method = ?, dest_track_id = ?, processed_at = CURRENT_TIMESTAMP WHERE job_id = ? AND source_track_id = ?"
        )
        .bind(status)
        .bind(match_confidence)
        .bind(match_method)
        .bind(&dest_track_id)
        .bind(&job_id)
        .bind(ext_id)
        .execute(&state.db)
        .await
        .ok();

        // Emit progress every 5 items (more frequent for real API calls)
        if i % 5 == 0 {
            let _ = app.emit(
                "migration-progress",
                MigrationProgress {
                    job_id: job_id.clone(),
                    current_item: i as i64 + 1,
                    total_items,
                    current_track: format!("{} - {}", artist, title),
                    status: "running".to_string(),
                    completed_count: completed,
                    failed_count: failed,
                    skipped_count: skipped,
                },
            );
        }

        // Small delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Update job as completed
    sqlx::query(
        "UPDATE migration_jobs SET status = 'completed', completed_items = ?, failed_items = ?, skipped_items = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(completed)
    .bind(failed)
    .bind(skipped)
    .bind(&job_id)
    .execute(&state.db)
    .await
    .ok();

    // Emit final progress
    let _ = app.emit(
        "migration-progress",
        MigrationProgress {
            job_id: job_id.clone(),
            current_item: total_items,
            total_items,
            current_track: "Migration complete".to_string(),
            status: "completed".to_string(),
            completed_count: completed,
            failed_count: failed,
            skipped_count: skipped,
        },
    );

    Ok(job_id)
}

/// Cancel a running migration
#[tauri::command]
pub async fn cancel_migration(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    sqlx::query("UPDATE migration_jobs SET status = 'cancelled', completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'running'")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to cancel migration: {}", e))?;
    Ok("Migration cancelled".to_string())
}

/// Retry failed items in a migration
#[tauri::command]
pub async fn retry_failed_items(state: State<'_, AppState>, job_id: String) -> Result<i64, String> {
    let result = sqlx::query("UPDATE migration_items SET status = 'pending', error_message = NULL WHERE job_id = ? AND status = 'failed'")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to retry items: {}", e))?;

    // Reset job status to running
    sqlx::query("UPDATE migration_jobs SET status = 'running' WHERE id = ?")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .ok();

    Ok(result.rows_affected() as i64)
}

/// Delete a migration job
#[tauri::command]
pub async fn delete_migration(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    // Items are deleted via CASCADE
    sqlx::query("DELETE FROM migration_jobs WHERE id = ?")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to delete migration: {}", e))?;
    Ok("Migration deleted".to_string())
}

/// Get all migration templates
#[tauri::command]
pub async fn get_migration_templates(
    state: State<'_, AppState>,
) -> Result<Vec<MigrationTemplate>, String> {
    sqlx::query_as::<_, MigrationTemplate>("SELECT * FROM migration_templates ORDER BY name")
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("Failed to get templates: {}", e))
}

/// Save a migration template
#[tauri::command]
pub async fn save_migration_template(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    source_service: String,
    destination_service: String,
    options: MigrationOptions,
) -> Result<i64, String> {
    let options_json = serde_json::to_string(&options).map_err(|e| e.to_string())?;

    let result = sqlx::query(
        r#"INSERT INTO migration_templates (name, description, source_service, destination_service, options)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT(name) DO UPDATE SET
           description = excluded.description,
           source_service = excluded.source_service,
           destination_service = excluded.destination_service,
           options = excluded.options,
           updated_at = CURRENT_TIMESTAMP"#
    )
    .bind(&name)
    .bind(&description)
    .bind(&source_service)
    .bind(&destination_service)
    .bind(&options_json)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to save template: {}", e))?;

    Ok(result.last_insert_rowid())
}

/// Delete a migration template
#[tauri::command]
pub async fn delete_migration_template(
    state: State<'_, AppState>,
    template_id: i64,
) -> Result<String, String> {
    sqlx::query("DELETE FROM migration_templates WHERE id = ?")
        .bind(template_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to delete template: {}", e))?;
    Ok("Template deleted".to_string())
}

/// Use a migration template (returns template details)
#[tauri::command]
pub async fn use_migration_template(
    state: State<'_, AppState>,
    template_id: i64,
) -> Result<MigrationTemplate, String> {
    sqlx::query_as::<_, MigrationTemplate>("SELECT * FROM migration_templates WHERE id = ?")
        .bind(template_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| format!("Template {} not found", template_id))
}

/// Search for tracks in destination service for manual matching
#[tauri::command]
pub async fn search_destination_track(
    state: State<'_, AppState>,
    service: String,
    query: String,
) -> Result<Vec<DestinationTrackMatch>, String> {
    // If destination is Qobuz, try real API search first
    if service.to_lowercase() == "qobuz" {
        // Get Qobuz credentials from database
        let creds: Option<(String,)> = sqlx::query_as(
            "SELECT credentials FROM accounts WHERE service_name = 'qobuz' AND is_active = 1",
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((creds_json,)) = creds {
            if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                if let Some(token) = creds.get("user_auth_token").and_then(|v| v.as_str()) {
                    // Create authenticated Qobuz client
                    let app_id =
                        std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| "950096963".to_string());
                    let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_default();
                    let client = crate::services::QobuzClient::new_with_token(
                        app_id,
                        app_secret,
                        token.to_string(),
                    );

                    // Search real Qobuz API
                    match client.search_track(&query, 20).await {
                        Ok(results) => {
                            tracing::info!(
                                "Qobuz search for '{}' returned {} results",
                                query,
                                results.len()
                            );
                            return Ok(results
                                .into_iter()
                                .map(|r| {
                                    let quality = r
                                        .bit_depth
                                        .map(|d| format!("{}-bit", d))
                                        .or(r.sample_rate.map(|s| format!("{:.1}kHz", s / 1000.0)));
                                    DestinationTrackMatch {
                                        track_id: r.track_id,
                                        title: r.title,
                                        artist: r.artist,
                                        album: r.album,
                                        duration_ms: r.duration_ms,
                                        quality,
                                        confidence: if r.isrc.is_some() { 0.95 } else { 0.75 },
                                    }
                                })
                                .collect());
                        }
                        Err(e) => {
                            tracing::warn!("Qobuz search failed, falling back to local: {}", e);
                        }
                    }
                }
            }
        }
    }

    // Fallback: Search our local library for tracks from the destination service
    let results: Vec<(String, String, String, Option<String>, i64, Option<String>)> =
        sqlx::query_as(
            r#"SELECT external_id, title, artist, album, duration_ms, quality 
           FROM library_items 
           WHERE source_service = ? AND (title LIKE ? OR artist LIKE ?)
           ORDER BY title LIMIT 20"#,
        )
        .bind(&service)
        .bind(format!("%{}%", query))
        .bind(format!("%{}%", query))
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Ok(results
        .into_iter()
        .map(
            |(id, title, artist, album, duration, quality)| DestinationTrackMatch {
                track_id: id,
                title,
                artist,
                album,
                duration_ms: duration,
                quality,
                confidence: 0.80,
            },
        )
        .collect())
}

/// Manually match a migration item to a destination track
#[tauri::command]
pub async fn manual_match_item(
    state: State<'_, AppState>,
    item_id: i64,
    destination_track_id: String,
) -> Result<String, String> {
    sqlx::query(
        "UPDATE migration_items SET destination_track_id = ?, match_method = 'manual', match_confidence = 1.0, status = 'matched' WHERE id = ?"
    )
    .bind(&destination_track_id)
    .bind(item_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to match item: {}", e))?;
    Ok("Item matched".to_string())
}
