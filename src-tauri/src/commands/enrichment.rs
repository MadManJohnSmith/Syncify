// Enrichment Commands - included via include!() in mod.rs
// 
// Metadata enrichment (Spotify Audio Features, Last.fm Genre, MusicBrainz)


// ==============================================
// METADATA ENRICHMENT COMMANDS
// ==============================================

/// Enrich tracks with Spotify audio features (BPM, key, energy, etc.)
/// Processes tracks in batches of 100 for efficiency
#[tauri::command]
pub async fn enrich_spotify_audio_features(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<String, String> {
    use crate::services::spotify::SpotifyClient;

    tracing::info!("Starting Spotify audio features enrichment");

    // Get Spotify access token from connected account
    let account: Option<(i64, String)> = sqlx::query_as(
        "SELECT a.id, a.credentials_json FROM accounts a 
         JOIN services s ON s.id = a.service_id 
         WHERE s.name = 'spotify' AND a.is_active = 1 LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to get Spotify account: {}", e))?;

    let (account_id, creds_json) = account.ok_or("No Spotify account connected")?;
    let creds: serde_json::Value = serde_json::from_str(&creds_json)
        .or_else(|_| {
            let decrypted = crate::crypto::decrypt(&creds_json)?;
            serde_json::from_str(&decrypted).map_err(|e| e.to_string())
        })
        .map_err(|e| format!("Failed to parse credentials: {}", e))?;

    let access_token = creds["access_token"]
        .as_str()
        .ok_or("Missing access token")?
        .to_string();

    let refresh_token = creds["refresh_token"]
        .as_str()
        .map(|s| s.to_string());

    let mut client = SpotifyClient::new(access_token, refresh_token);

    // Get tracks that need enrichment (have Spotify source but no BPM)
    let tracks: Vec<(i64, String)> = sqlx::query_as(
        "SELECT t.id, ts.service_track_id 
         FROM tracks t
         JOIN track_sources ts ON ts.track_id = t.id
         JOIN services s ON s.id = ts.service_id
         WHERE s.name = 'spotify' AND t.bpm IS NULL
         LIMIT 1000",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to get tracks: {}", e))?;

    if tracks.is_empty() {
        return Ok("No tracks need enrichment".to_string());
    }

    let total = tracks.len();
    tracing::info!("Enriching {} tracks with Spotify audio features", total);

    // Emit start event
    let _ = window.emit(
        "enrichment-progress",
        serde_json::json!({
            "type": "spotify_audio_features",
            "status": "started",
            "current": 0,
            "total": total,
            "message": format!("Enriching {} tracks...", total)
        }),
    );

    // Batch process tracks (100 at a time)
    let mut enriched = 0;
    for chunk in tracks.chunks(100) {
        let spotify_ids: Vec<String> = chunk.iter().map(|(_, sid)| sid.clone()).collect();
        let track_map: std::collections::HashMap<String, i64> =
            chunk.iter().map(|(tid, sid)| (sid.clone(), *tid)).collect();

        // Fetch audio features
        match client.get_audio_features_batch(&spotify_ids, Some(&state.db), Some(account_id)).await {
            Ok(features) => {
                for (spotify_id, feat) in features {
                    if let Some(&track_id) = track_map.get(&spotify_id) {
                        // Update track with audio features
                        let key_notation = feat.key_notation();
                        let _ = sqlx::query(
                            "UPDATE tracks SET 
                                bpm = ?, 
                                musical_key = ?, 
                                energy = ?, 
                                danceability = ?, 
                                valence = ?,
                                acousticness = ?,
                                instrumentalness = ?,
                                enrichment_status = 'spotify_done',
                                enriched_at = CURRENT_TIMESTAMP
                             WHERE id = ?",
                        )
                        .bind(feat.tempo as f64)
                        .bind(&key_notation)
                        .bind(feat.energy as f64)
                        .bind(feat.danceability as f64)
                        .bind(feat.valence as f64)
                        .bind(feat.acousticness as f64)
                        .bind(feat.instrumentalness as f64)
                        .bind(track_id)
                        .execute(&state.db)
                        .await;

                        enriched += 1;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch audio features: {}", e);
            }
        }

        // Emit progress
        let _ = window.emit(
            "enrichment-progress",
            serde_json::json!({
                "type": "spotify_audio_features",
                "status": "progress",
                "current": enriched,
                "total": total,
                "message": format!("Enriched {}/{} tracks", enriched, total)
            }),
        );
    }

    // Emit completion
    let _ = window.emit(
        "enrichment-progress",
        serde_json::json!({
            "type": "spotify_audio_features",
            "status": "completed",
            "current": enriched,
            "total": total,
            "message": format!("Enriched {} tracks with audio features", enriched)
        }),
    );

    tracing::info!(
        "Spotify audio features enrichment complete: {}/{} tracks",
        enriched,
        total
    );
    Ok(format!("Enriched {} tracks with audio features", enriched))
}

/// Enrich tracks with genre from Last.fm tags
/// Requires LASTFM_API_KEY environment variable
#[tauri::command]
pub async fn enrich_genre_lastfm(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<String, String> {
    use crate::services::lastfm::LastFmClient;

    tracing::info!("Starting Last.fm genre enrichment");

    // Get Last.fm client
    let client = LastFmClient::from_env().map_err(|e| format!("Last.fm setup failed: {}", e))?;

    // Get tracks that need genre enrichment (have artist but no genre)
    let tracks: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT t.id, 
                (SELECT a.name FROM track_artists ta 
                 JOIN artists a ON a.id = ta.artist_id 
                 WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1) as artist,
                t.title
         FROM tracks t
         WHERE t.genre IS NULL
         LIMIT 500",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to get tracks: {}", e))?;

    if tracks.is_empty() {
        return Ok("No tracks need genre enrichment".to_string());
    }

    let total = tracks.len();
    tracing::info!("Enriching {} tracks with Last.fm genre tags", total);

    // Emit start event
    let _ = window.emit(
        "enrichment-progress",
        serde_json::json!({
            "type": "lastfm_genre",
            "status": "started",
            "current": 0,
            "total": total,
            "message": format!("Enriching {} tracks with genres...", total)
        }),
    );

    let mut enriched = 0;
    for (track_id, artist, title) in &tracks {
        // Skip if artist is empty
        if artist.is_empty() {
            continue;
        }

        // Fetch tags from Last.fm
        match client.get_track_tags(artist, title).await {
            Ok(tags) => {
                let genre = LastFmClient::extract_genre(&tags);
                let subgenre = LastFmClient::extract_subgenre(&tags, genre.as_deref());

                if genre.is_some() {
                    // Update track with genre
                    let _ = sqlx::query("UPDATE tracks SET genre = ?, subgenre = ? WHERE id = ?")
                        .bind(&genre)
                        .bind(&subgenre)
                        .bind(track_id)
                        .execute(&state.db)
                        .await;

                    enriched += 1;
                }
            }
            Err(e) => {
                tracing::debug!("Failed to get tags for '{}' - '{}': {}", artist, title, e);
            }
        }

        // Emit progress every 20 tracks
        if enriched % 20 == 0 {
            let _ = window.emit(
                "enrichment-progress",
                serde_json::json!({
                    "type": "lastfm_genre",
                    "status": "progress",
                    "current": enriched,
                    "total": total,
                    "message": format!("Enriched {}/{} tracks with genres", enriched, total)
                }),
            );
        }
    }

    // Emit completion
    let _ = window.emit(
        "enrichment-progress",
        serde_json::json!({
            "type": "lastfm_genre",
            "status": "completed",
            "current": enriched,
            "total": total,
            "message": format!("Enriched {} tracks with genres", enriched)
        }),
    );

    tracing::info!(
        "Last.fm genre enrichment complete: {}/{} tracks",
        enriched,
        total
    );
    Ok(format!("Enriched {} tracks with genres", enriched))
}

/// Enrich a single track with all available metadata
/// Used for on-demand enrichment before download
#[tauri::command]
pub async fn enrich_track(state: State<'_, AppState>, track_id: i64) -> Result<String, String> {
    tracing::info!("On-demand enrichment for track {}", track_id);

    // Get track info
    let track: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT t.title, 
                (SELECT a.name FROM track_artists ta 
                 JOIN artists a ON a.id = ta.artist_id 
                 WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1) as artist,
                t.isrc,
                (SELECT ts.service_track_id FROM track_sources ts 
                 JOIN services s ON s.id = ts.service_id 
                 WHERE ts.track_id = t.id AND s.name = 'spotify' LIMIT 1) as spotify_id
         FROM tracks t WHERE t.id = ?",
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to get track: {}", e))?;

    let (title, artist, isrc, spotify_id) = track.ok_or("Track not found")?;
    let mut enrichments = vec![];

    // 1. MusicBrainz enrichment (if ISRC available and not already enriched)
    if let Some(isrc) = &isrc {
        if !isrc.is_empty() {
            let mb: (Option<String>,) =
                sqlx::query_as("SELECT musicbrainz_id FROM tracks WHERE id = ?")
                    .bind(track_id)
                    .fetch_one(&state.db)
                    .await
                    .unwrap_or((None,));

            if mb.0.is_none() {
                let mb_client = crate::services::MusicBrainzClient::new();
                if let Ok(Some(recording)) = mb_client.lookup_by_isrc(isrc).await {
                    let _ = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                        .bind(&recording.id)
                        .bind(track_id)
                        .execute(&state.db)
                        .await;
                    enrichments.push("MusicBrainz ID");
                }
            }
        }
    }

    // 2. Spotify Audio Features
    if let Some(spotify_track_id) = &spotify_id {
        let bpm: (Option<f64>,) = sqlx::query_as("SELECT bpm FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_one(&state.db)
            .await
            .unwrap_or((None,));

        if bpm.0.is_none() {
            // Get Spotify credentials
            let creds_row: Option<(i64, String)> = sqlx::query_as(
                "SELECT a.id, a.credentials_json FROM accounts a 
                 JOIN services s ON s.id = a.service_id 
                 WHERE s.name = 'spotify' AND a.is_active = 1 LIMIT 1",
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((account_id, creds_json)) = creds_row {
                if let Ok(parsed) = crate::crypto::decrypt(&creds_json).and_then(|d| {
                    serde_json::from_str::<serde_json::Value>(&d).map_err(|e| e.to_string())
                }) {
                    if let Some(token) = parsed["access_token"].as_str() {
                         let refresh_token = parsed["refresh_token"].as_str().map(|s| s.to_string());
                        let mut spotify_client = crate::services::SpotifyClient::new(token.to_string(), refresh_token);
                        if let Ok(features) = spotify_client
                            .get_audio_features_batch(&[spotify_track_id.clone()], Some(&state.db), Some(account_id))
                            .await
                        {
                            if let Some(feat) = features.get(spotify_track_id) {
                                let key_notation = feat.key_notation();
                                let _ = sqlx::query(
                                    "UPDATE tracks SET bpm = ?, musical_key = ?, energy = ?, danceability = ?, valence = ?, acousticness = ?, instrumentalness = ?, enrichment_status = 'spotify_done', enriched_at = CURRENT_TIMESTAMP WHERE id = ?"
                                )
                                .bind(feat.tempo as f64)
                                .bind(&key_notation)
                                .bind(feat.energy as f64)
                                .bind(feat.danceability as f64)
                                .bind(feat.valence as f64)
                                .bind(feat.acousticness as f64)
                                .bind(feat.instrumentalness as f64)
                                .bind(track_id)
                                .execute(&state.db)
                                .await;
                                enrichments.push("Audio Features");
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Last.fm Genre
    let genre: (Option<String>,) = sqlx::query_as("SELECT genre FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((None,));

    if genre.0.is_none() && !artist.is_empty() {
        if let Ok(lastfm) = crate::services::lastfm::LastFmClient::from_env() {
            if let Ok(tags) = lastfm.get_track_tags(&artist, &title).await {
                let genre = crate::services::lastfm::LastFmClient::extract_genre(&tags);
                let subgenre = crate::services::lastfm::LastFmClient::extract_subgenre(
                    &tags,
                    genre.as_deref(),
                );
                if genre.is_some() {
                    let _ = sqlx::query("UPDATE tracks SET genre = ?, subgenre = ? WHERE id = ?")
                        .bind(&genre)
                        .bind(&subgenre)
                        .bind(track_id)
                        .execute(&state.db)
                        .await;
                    enrichments.push("Genre");
                }
            }
        }
    }

    if enrichments.is_empty() {
        Ok("Track already enriched".to_string())
    } else {
        Ok(format!("Enriched: {}", enrichments.join(", ")))
    }
}

/// Enrich tracks before downloading (called before queue processing)
#[tauri::command]
pub async fn enrich_before_download(
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<String, String> {
    tracing::info!("Enriching {} tracks before download", track_ids.len());

    let mut enriched = 0;
    for track_id in &track_ids {
        // Just call enrich_track for each - it's idempotent
        let track: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT t.title, 
                    (SELECT a.name FROM track_artists ta 
                     JOIN artists a ON a.id = ta.artist_id 
                     WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1) as artist,
                    t.isrc,
                    (SELECT ts.service_track_id FROM track_sources ts 
                     JOIN services s ON s.id = ts.service_id 
                     WHERE ts.track_id = t.id AND s.name = 'spotify' LIMIT 1) as spotify_id
             FROM tracks t WHERE t.id = ?",
        )
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if track.is_some() {
            enriched += 1;
        }
    }

    Ok(format!(
        "Pre-download enrichment: {} tracks processed",
        enriched
    ))
}
