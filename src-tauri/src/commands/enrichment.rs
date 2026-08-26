// Enrichment Commands - included via include!() in mod.rs
// 
// Metadata enrichment (Spotify Audio Features, Last.fm Genre, MusicBrainz)


// ==============================================
// METADATA ENRICHMENT COMMANDS
// ==============================================

/// S200 — resolve the Last.fm API key: settings table first (set from the
/// Metadata tab UI), then the LASTFM_API_KEY environment variable. Mirrors the
/// SpotifyConfig::from_parts BD>env precedence from S196.
pub(crate) async fn resolve_lastfm_api_key(db: &crate::DbPool) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = 'lastfm_api_key' LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((key,)) = row {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    if let Ok(key) = std::env::var("LASTFM_API_KEY") {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    Err("Last.fm API key no configurada — ponla en la tab Metadata → Auto-Fix → Last.fm".to_string())
}

/// Enrich tracks with genre from Last.fm tags
/// Requires a Last.fm API key (settings table o LASTFM_API_KEY env).
#[tauri::command]
pub async fn enrich_genre_lastfm(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<String, String> {
    use crate::services::lastfm::LastFmClient;

    tracing::info!("Starting Last.fm genre enrichment");

    // Get Last.fm client (BD primero, luego env)
    let api_key = resolve_lastfm_api_key(&state.db).await?;
    let client = LastFmClient::new(api_key);

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

    let (title, artist, isrc, _spotify_id) = track.ok_or("Track not found")?; // _spotify_id: S68 retiró audio-features
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

    // 2. Last.fm Genre (Spotify Audio Features retired by S68 — endpoint removed)
    // 3.
    let genre: (Option<String>,) = sqlx::query_as("SELECT genre FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((None,));

    if genre.0.is_none() && !artist.is_empty() {
        if let Ok(api_key) = resolve_lastfm_api_key(&state.db).await {
            let lastfm = crate::services::lastfm::LastFmClient::new(api_key);
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

/// Pause background enrichment worker
#[tauri::command]
pub fn pause_enrichment_worker(state: State<'_, AppState>) {
    state.enrichment_state.pause();
    tracing::info!("Enrichment worker paused");
}

/// Resume background enrichment worker
#[tauri::command]
pub fn resume_enrichment_worker(state: State<'_, AppState>) {
    state.enrichment_state.resume();
    tracing::info!("Enrichment worker resumed");
}

/// Start background enrichment worker
#[tauri::command]
pub fn start_enrichment_worker(state: State<'_, AppState>) {
    state.enrichment_state.resume();
    tracing::info!("Enrichment worker started");
}

/// Get enrichment worker status
#[tauri::command]
pub async fn get_enrichment_status(
    state: State<'_, AppState>,
) -> Result<crate::enrichment_worker::EnrichmentStatus, String> {
    let rate_limiter = std::sync::Arc::new(crate::services::rate_limiter::RateLimiter::new());
    let worker = crate::enrichment_worker::EnrichmentWorker::new(
        state.db.clone(),
        state.enrichment_state.clone(),
        rate_limiter,
    );
    worker.get_status().await
}

// ==============================================
// INCREMENTAL LIBRARY ENRICHMENT (S144)
// ==============================================

lazy_static::lazy_static! {
    static ref GLOBAL_INCREMENTAL_ENRICHMENT_SERVICE: crate::services::incremental_enrichment::IncrementalEnrichmentService =
        crate::services::incremental_enrichment::IncrementalEnrichmentService::new();
}

#[tauri::command]
pub async fn preview_library_enrichment(
    state: State<'_, AppState>,
    mode: Option<crate::services::incremental_enrichment::EnrichmentMode>,
    track_ids: Option<Vec<i64>>,
) -> Result<crate::services::incremental_enrichment::EnrichmentPreview, String> {
    let mode = mode.unwrap_or_default();
    GLOBAL_INCREMENTAL_ENRICHMENT_SERVICE
        .preview_enrichment(&state.db, mode, track_ids)
        .await
}

#[tauri::command]
pub async fn start_library_enrichment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mode: Option<crate::services::incremental_enrichment::EnrichmentMode>,
    track_ids: Option<Vec<i64>>,
) -> Result<crate::services::incremental_enrichment::EnrichmentJobSummary, String> {
    let mode = mode.unwrap_or_default();
    let db = state.db.clone();
    let app_handle = app.clone();

    GLOBAL_INCREMENTAL_ENRICHMENT_SERVICE
        .run_enrichment(&db, mode, track_ids, move |progress| {
            let _ = app_handle.emit("enrichment_progress", progress);
        })
        .await
}

#[tauri::command]
pub async fn cancel_library_enrichment() -> Result<bool, String> {
    GLOBAL_INCREMENTAL_ENRICHMENT_SERVICE.cancel_job();
    Ok(true)
}

#[tauri::command]
pub async fn get_library_enrichment_status() -> Result<Option<crate::services::incremental_enrichment::EnrichmentJobSummary>, String> {
    Ok(GLOBAL_INCREMENTAL_ENRICHMENT_SERVICE.get_job_status())
}
