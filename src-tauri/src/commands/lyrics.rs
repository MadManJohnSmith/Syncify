// Lyrics Commands - included via include!() in mod.rs
//
// Full lyrics management: get, save, delete, stats, batch fetch, embed

// Note: Most imports come from mod.rs via include!()
// Only import what's specifically needed here
use sqlx::FromRow;

// ==============================================
// LYRICS PROGRESS EVENTS
// ==============================================

/// Emit lyrics fetch progress event
fn emit_lyrics_progress(
    window: &tauri::Window,
    status: &str,
    current: u64,
    total: u64,
    track_name: &str,
) {
    let _ = window.emit(
        "lyrics-fetch-progress",
        serde_json::json!({
            "status": status,
            "current": current,
            "total": total,
            "track": track_name,
            "message": format!("{}/{}: {}", current, total, track_name)
        }),
    );
}

// ==============================================
// LYRICS TYPES
// ==============================================

/// Lyrics record from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Lyrics {
    pub id: i64,
    pub track_id: i64,
    pub format: String,              // 'ttml', 'lrc', 'plain'
    pub sync_level: Option<String>,  // 'syllable', 'word', 'line', 'none'
    pub source: Option<String>,      // 'lrclib', 'genius', 'apple_ttml', etc.
    pub content: String,
    pub language: Option<String>,
    pub embedded_in_file: i64,       // 0 or 1
    pub created_at: Option<String>,
}

/// Lyrics statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsStats {
    pub total_tracks: i64,
    pub with_lyrics: i64,
    pub synced_lyrics: i64,
    pub embedded_lyrics: i64,
}

/// Parameters for saving lyrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveLyricsParams {
    pub track_id: i64,
    pub format: String,              // 'ttml', 'lrc', 'plain'
    pub content: String,
    pub sync_level: Option<String>,  // 'syllable', 'word', 'line', 'none'
    pub source: Option<String>,
    pub language: Option<String>,
}

/// Batch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchLyricsResult {
    pub fetched: i64,
    pub failed: i64,
    pub skipped: i64,
}

/// Search result from online sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsSearchResult {
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
    pub sync_type: String,           // 'LINE_SYNCED', 'WORD_SYNCED', 'NOT_SYNCED'
    pub instrumental: bool,
}

use syncify_lyrics_domain::{LyricsSyncType, ResolutionStatus};

/// Unified Domain Resolution Payload returned by Tauri command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsResolutionPayload {
    pub status: ResolutionStatus,
    pub provider: String,
    pub strategy: String,
    pub sync_type: LyricsSyncType,
    pub format: String,
    pub synced_content: Option<String>,
    pub plain_text: Option<String>,
    pub is_instrumental: bool,
    pub fallback_applied: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub provenance: String,
    pub embedded_in_file: bool,
}

/// Command: Resolve lyrics for a track using domain contract orchestrator.
/// If file_path is provided, verifies & embeds tags with mandatory re-read.
/// If track_id is provided and resolution is resolved (and file re-read verified),
/// updates SQLite lyrics record.
/// Command: Resolve lyrics for a track using domain contract orchestrator.
/// If file_path is provided, verifies & embeds tags with mandatory re-read.
/// If track_id is provided and resolution is resolved (and file re-read verified),
/// updates SQLite lyrics record.
#[tauri::command]
pub async fn resolve_track_lyrics(
    state: State<'_, AppState>,
    artist: String,
    title: String,
    album: Option<String>,
    duration_sec: Option<f64>,
    file_path: Option<String>,
    track_id: Option<i64>,
) -> Result<LyricsResolutionPayload, String> {
    let lyrics_client = crate::download::LyricsClient::new();
    resolve_track_lyrics_with_client(
        &lyrics_client,
        &state.db,
        artist,
        title,
        album,
        duration_sec,
        file_path,
        track_id,
    )
    .await
}

/// Inner execution for resolve_track_lyrics allowing dependency injection of client & db
pub async fn resolve_track_lyrics_with_client(
    lyrics_client: &crate::download::LyricsClient,
    db: &sqlx::SqlitePool,
    artist: String,
    title: String,
    album: Option<String>,
    duration_sec: Option<f64>,
    file_path: Option<String>,
    track_id: Option<i64>,
) -> Result<LyricsResolutionPayload, String> {
    tracing::info!(
        "resolve_track_lyrics: artist='{}', title='{}', duration={:?}, file_path={:?}, track_id={:?}",
        artist,
        title,
        duration_sec,
        file_path,
        track_id
    );

    let dur = duration_sec.unwrap_or(0.0);
    let (resolution, latency_ms) = lyrics_client
        .orchestrate_resolution(&artist, &title, album.as_deref(), dur)
        .await;

    let path_buf = file_path.as_deref().map(std::path::Path::new);
    process_and_persist_resolution(db, resolution, latency_ms, path_buf, track_id).await
}

/// Process domain resolution, execute strict FLAC verification (if path provided),
/// and persist to SQLite ONLY if re-read verification succeeded.
pub async fn process_and_persist_resolution(
    db: &sqlx::SqlitePool,
    resolution: syncify_lyrics_domain::LyricsResolution,
    latency_ms: u64,
    file_path: Option<&std::path::Path>,
    track_id: Option<i64>,
) -> Result<LyricsResolutionPayload, String> {
    let mut embedded_in_file = false;

    // Phase 4: File validation and embedding
    if let Some(path) = file_path {
        if resolution.status == ResolutionStatus::Resolved {
            match crate::download::lyrics::validate_and_embed_flac_lyrics(path, &resolution) {
                Ok(true) => {
                    tracing::info!("Successfully embedded and verified lyrics in {}", path.display());
                    embedded_in_file = true;
                }
                Ok(false) => {
                    tracing::warn!("Lyrics embedding skipped for {}", path.display());
                }
                Err(e) => {
                    tracing::error!("Lyrics embedding/verification failed for {}: {}", path.display(), e);
                    return Err(format!("File verification failed: {}", e));
                }
            }
        } else {
            // When resolution is not resolved, cannot embed in file
            tracing::debug!("Resolution not resolved ({:?}), skipping file embed", resolution.status);
        }
    }

    // Persist to database ONLY if resolved and file verification (if requested) succeeded
    if let Some(tid) = track_id {
        if resolution.status == ResolutionStatus::Resolved {
            let format = match resolution.sync_type {
                LyricsSyncType::KaraokeWordSynced | LyricsSyncType::LineSynced => "lrc",
                LyricsSyncType::Plain => "plain",
                LyricsSyncType::Instrumental => "instrumental",
                LyricsSyncType::None => "none",
            };

            let sync_level = match resolution.sync_type {
                LyricsSyncType::KaraokeWordSynced => "word",
                LyricsSyncType::LineSynced => "line",
                LyricsSyncType::Plain | LyricsSyncType::Instrumental | LyricsSyncType::None => "none",
            };

            let content = resolution
                .synced_content
                .as_deref()
                .or(resolution.plain_text.as_deref())
                .unwrap_or("");

            if !content.is_empty() || resolution.is_instrumental {
                sqlx::query(
                    r#"
                    INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT(track_id, format) DO UPDATE SET
                        sync_level = excluded.sync_level,
                        source = excluded.source,
                        content = excluded.content,
                        embedded_in_file = excluded.embedded_in_file
                    "#,
                )
                .bind(tid)
                .bind(format)
                .bind(sync_level)
                .bind(&resolution.provider)
                .bind(content)
                .bind(if embedded_in_file { 1 } else { 0 })
                .execute(db)
                .await
                .map_err(|e| format!("Database error persisting lyrics: {}", e))?;
            }
        }
    }

    Ok(LyricsResolutionPayload {
        status: resolution.status,
        provider: resolution.provider,
        strategy: resolution.strategy,
        sync_type: resolution.sync_type,
        format: resolution.format,
        synced_content: resolution.synced_content,
        plain_text: resolution.plain_text,
        is_instrumental: resolution.is_instrumental,
        fallback_applied: resolution.fallback_applied,
        error: resolution.error,
        duration_ms: latency_ms,
        provenance: resolution.provenance,
        embedded_in_file,
    })
}

// ==============================================
// LYRICS QUERY COMMANDS
// ==============================================

/// Get lyrics for a specific track
#[tauri::command]
pub async fn get_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<Option<Lyrics>, String> {
    tracing::info!("get_lyrics: track_id={}", track_id);
    
    // Get the best available lyrics (prefer synced over plain)
    let lyrics: Option<Lyrics> = sqlx::query_as(
        r#"
        SELECT id, track_id, format, sync_level, source, content, language, embedded_in_file, created_at
        FROM lyrics
        WHERE track_id = ?
        ORDER BY 
            CASE format 
                WHEN 'ttml' THEN 1 
                WHEN 'lrc' THEN 2 
                WHEN 'plain' THEN 3 
            END
        LIMIT 1
        "#
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(lyrics)
}

/// Get all lyrics with pagination
#[tauri::command]
pub async fn get_all_lyrics(
    state: State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    format: Option<String>,
) -> Result<Vec<Lyrics>, String> {
    tracing::info!("get_all_lyrics: limit={:?}, offset={:?}, format={:?}", limit, offset, format);
    
    let limit = limit.unwrap_or(100);
    let offset = offset.unwrap_or(0);
    
    let lyrics: Vec<Lyrics> = if let Some(fmt) = format {
        sqlx::query_as(
            r#"
            SELECT id, track_id, format, sync_level, source, content, language, embedded_in_file, created_at
            FROM lyrics
            WHERE format = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(fmt)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(
            r#"
            SELECT id, track_id, format, sync_level, source, content, language, embedded_in_file, created_at
            FROM lyrics
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?
    };
    
    Ok(lyrics)
}

/// Get lyrics coverage statistics
#[tauri::command]
pub async fn get_lyrics_stats(
    state: State<'_, AppState>,
) -> Result<LyricsStats, String> {
    tracing::info!("get_lyrics_stats");
    
    // Total tracks
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    
    // Tracks with any lyrics
    let with_lyrics: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT track_id) FROM lyrics"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    // Tracks with synced lyrics (lrc or ttml)
    let synced: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT track_id) FROM lyrics WHERE format IN ('lrc', 'ttml')"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    // Tracks with embedded lyrics
    let embedded: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT track_id) FROM lyrics WHERE embedded_in_file = 1"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(LyricsStats {
        total_tracks: total.0,
        with_lyrics: with_lyrics.0,
        synced_lyrics: synced.0,
        embedded_lyrics: embedded.0,
    })
}

// ==============================================
// LYRICS MANAGEMENT COMMANDS
// ==============================================

/// Save or update lyrics for a track
#[tauri::command]
pub async fn save_lyrics(
    state: State<'_, AppState>,
    params: SaveLyricsParams,
) -> Result<Lyrics, String> {
    tracing::info!("save_lyrics: track_id={}, format={}", params.track_id, params.format);
    
    // Upsert lyrics (INSERT OR REPLACE based on UNIQUE(track_id, format))
    sqlx::query(
        r#"
        INSERT INTO lyrics (track_id, format, sync_level, source, content, language, created_at)
        VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(track_id, format) DO UPDATE SET
            sync_level = excluded.sync_level,
            source = excluded.source,
            content = excluded.content,
            language = excluded.language,
            created_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(params.track_id)
    .bind(&params.format)
    .bind(&params.sync_level)
    .bind(&params.source)
    .bind(&params.content)
    .bind(&params.language)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    // Return the saved lyrics
    let lyrics: Lyrics = sqlx::query_as(
        r#"
        SELECT id, track_id, format, sync_level, source, content, language, embedded_in_file, created_at
        FROM lyrics
        WHERE track_id = ? AND format = ?
        "#
    )
    .bind(params.track_id)
    .bind(&params.format)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(lyrics)
}

/// S192: associate an external lyrics file (.lrc / .txt) with a track.
///
/// Reads the file from disk (the webview has no fs read scope by design),
/// detects the format from content (LRC line timestamps `[mm:ss.xx]` vs plain
/// text), and persists through the same upsert contract as `save_lyrics`
/// with `source = "manual_import"`.
#[tauri::command]
pub async fn import_lyrics_file(
    state: State<'_, AppState>,
    track_id: i64,
    file_path: String,
) -> Result<Lyrics, String> {
    tracing::info!("import_lyrics_file: track_id={} path={}", track_id, file_path);
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("No se pudo leer el archivo de letras: {}", e))?;
    if content.trim().is_empty() {
        return Err("El archivo de letras está vacío".to_string());
    }

    // LRC detection: any of the first lines starts with `[dd:dd.dd]`-shaped
    // timestamp. Plain text otherwise.
    let is_lrc = content.lines().take(10).any(|line| {
        let l = line.trim_start();
        l.starts_with('[')
            && l.len() > 3
            && l.as_bytes()[1].is_ascii_digit()
            && l.contains(':')
            && l.contains(']')
    });

    let params = SaveLyricsParams {
        track_id,
        format: if is_lrc { "lrc".to_string() } else { "plain".to_string() },
        content,
        sync_level: Some(if is_lrc { "line".to_string() } else { "none".to_string() }),
        source: Some("manual_import".to_string()),
        language: None,
    };
    save_lyrics(state, params).await
}

/// Delete lyrics for a track
#[tauri::command]
pub async fn delete_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
    format: Option<String>,
) -> Result<(), String> {
    tracing::info!("delete_lyrics: track_id={}, format={:?}", track_id, format);
    
    if let Some(fmt) = format {
        // Delete specific format
        sqlx::query("DELETE FROM lyrics WHERE track_id = ? AND format = ?")
            .bind(track_id)
            .bind(fmt)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // Delete all formats for this track
        sqlx::query("DELETE FROM lyrics WHERE track_id = ?")
            .bind(track_id)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

// ==============================================
// LYRICS FETCHING COMMANDS
// ==============================================

/// Search for lyrics online (uses existing fetch_lyrics from tools.rs internally)
#[tauri::command]
pub async fn search_lyrics(
    _state: State<'_, AppState>,
    title: String,
    artist: String,
    _album: Option<String>,
    _duration_ms: Option<i64>,
) -> Result<Vec<LyricsSearchResult>, String> {
    tracing::info!("search_lyrics: {} - {}", artist, title);
    
    // Use the LyricsClient to search
    let lyrics_client = crate::download::LyricsClient::new();
    
    match lyrics_client.fetch_all_sources(&artist, &title, 0.0).await {
        Ok(response) => {
            // Convert to search result format
            let synced_lyrics = if !response.lines.is_empty() {
                Some(crate::download::LyricsClient::to_lrc_string(&response))
            } else {
                None
            };
            
            Ok(vec![LyricsSearchResult {
                source: response.source.clone(),
                title: title.clone(),
                artist: artist.clone(),
                album: None,
                duration_ms: None,
                synced_lyrics,
                plain_lyrics: response.plain_lyrics,
                sync_type: response.sync_type.clone(),
                instrumental: response.instrumental,
            }])
        }
        Err(e) => {
            tracing::warn!("Lyrics search failed: {}", e);
            Ok(vec![]) // Return empty list on error
        }
    }
}

/// Fetch lyrics for a track and save to database
#[tauri::command]
pub async fn fetch_and_save_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<Option<Lyrics>, String> {
    tracing::info!("fetch_and_save_lyrics: track_id={}", track_id);
    
    // Get track info
    let track: Option<(String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT t.title, 
               (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta 
                JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id) as artist
        FROM tracks t
        WHERE t.id = ?
        "#
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    let (title, artist) = match track {
        Some((t, a)) => (t, a.unwrap_or_default()),
        None => return Err("Track not found".to_string()),
    };
    
    // Fetch lyrics from online sources
    let lyrics_client = crate::download::LyricsClient::new();
    
    match lyrics_client.fetch_all_sources(&artist, &title, 0.0).await {
        Ok(response) => {
            // Determine format and content
            let (format, content, sync_level) = if !response.lines.is_empty() {
                let lrc = crate::download::LyricsClient::to_lrc_string(&response);
                let sync = if response.sync_type == "WORD_SYNCED" {
                    "word"
                } else {
                    "line"
                };
                ("lrc".to_string(), lrc, Some(sync.to_string()))
            } else if let Some(plain) = &response.plain_lyrics {
                ("plain".to_string(), plain.clone(), Some("none".to_string()))
            } else {
                return Ok(None); // No lyrics found
            };
            
            // Save to database
            let params = SaveLyricsParams {
                track_id,
                format,
                content,
                sync_level,
                source: Some(response.source),
                language: None,
            };
            
            let saved = save_lyrics(state, params).await?;
            Ok(Some(saved))
        }
        Err(e) => {
            tracing::warn!("Failed to fetch lyrics for track {}: {}", track_id, e);
            Ok(None)
        }
    }
}

/// Batch fetch lyrics for multiple tracks
#[tauri::command]
pub async fn batch_fetch_lyrics(
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<BatchLyricsResult, String> {
    tracing::info!("batch_fetch_lyrics: {} tracks", track_ids.len());
    
    let mut fetched = 0i64;
    let mut failed = 0i64;
    let mut skipped = 0i64;
    
    for track_id in track_ids {
        // Check if lyrics already exist
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM lyrics WHERE track_id = ?"
        )
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        
        if existing.map(|c| c.0 > 0).unwrap_or(false) {
            skipped += 1;
            continue;
        }
        
        // Fetch and save
        match fetch_and_save_lyrics(state.clone(), track_id).await {
            Ok(Some(_)) => fetched += 1,
            Ok(None) => failed += 1,
            Err(_) => failed += 1,
        }
        
        // Rate limiting - be kind to upstream APIs
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    Ok(BatchLyricsResult {
        fetched,
        failed,
        skipped,
    })
}

/// Fetch all missing lyrics
#[tauri::command]
pub async fn fetch_missing_lyrics(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<BatchLyricsResult, String> {
    tracing::info!("fetch_missing_lyrics");
    
    let limit = limit.unwrap_or(100);
    
    // Get tracks without lyrics
    let tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT t.id
        FROM tracks t
        LEFT JOIN lyrics l ON l.track_id = t.id
        WHERE l.id IS NULL
        LIMIT ?
        "#
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    let track_ids: Vec<i64> = tracks.into_iter().map(|t| t.0).collect();
    let total = track_ids.len() as i64;
    
    tracing::info!("Found {} tracks missing lyrics", total);
    
    batch_fetch_lyrics(state, track_ids).await
}

/// Batch fetch lyrics with real-time progress events
#[tauri::command]
pub async fn batch_fetch_lyrics_with_progress(
    window: tauri::Window,
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<BatchLyricsResult, String> {
    let total = track_ids.len() as u64;
    tracing::info!("batch_fetch_lyrics_with_progress: {} tracks", total);
    
    emit_lyrics_progress(&window, "started", 0, total, "Starting...");
    
    let mut fetched = 0i64;
    let mut failed = 0i64;
    let mut skipped = 0i64;
    let mut current = 0u64;
    
    for track_id in track_ids {
        current += 1;
        
        // Get track name for progress display
        let track_name: Option<(String,)> = sqlx::query_as(
            "SELECT title FROM tracks WHERE id = ?"
        )
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        
        let track_name = track_name.map(|t| t.0).unwrap_or_else(|| format!("Track {}", track_id));
        
        // Check if lyrics already exist
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM lyrics WHERE track_id = ?"
        )
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        
        if existing.map(|c| c.0 > 0).unwrap_or(false) {
            emit_lyrics_progress(&window, "skipped", current, total, &format!("{} (already has lyrics)", track_name));
            skipped += 1;
            continue;
        }
        
        // Emit progress before fetching
        emit_lyrics_progress(&window, "fetching", current, total, &track_name);
        
        // Fetch and save
        match fetch_and_save_lyrics(state.clone(), track_id).await {
            Ok(Some(_)) => {
                emit_lyrics_progress(&window, "found", current, total, &track_name);
                fetched += 1;
            }
            Ok(None) => {
                emit_lyrics_progress(&window, "not_found", current, total, &track_name);
                failed += 1;
            }
            Err(e) => {
                emit_lyrics_progress(&window, "error", current, total, &format!("{}: {}", track_name, e));
                failed += 1;
            }
        }
        
        // Rate limiting - be kind to upstream APIs
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    emit_lyrics_progress(&window, "completed", total, total, &format!("Done: {} found, {} failed, {} skipped", fetched, failed, skipped));
    
    Ok(BatchLyricsResult {
        fetched,
        failed,
        skipped,
    })
}

// ==============================================
// LYRICS EMBEDDING COMMANDS
// ==============================================

/// Embed lyrics into audio file
#[tauri::command]
pub async fn embed_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<bool, String> {
    tracing::info!("embed_lyrics: track_id={}", track_id);
    
    // Get lyrics content
    let lyrics: Option<Lyrics> = sqlx::query_as(
        r#"
        SELECT id, track_id, format, sync_level, source, content, language, embedded_in_file, created_at
        FROM lyrics
        WHERE track_id = ?
        ORDER BY CASE format WHEN 'lrc' THEN 1 WHEN 'ttml' THEN 2 WHEN 'plain' THEN 3 END
        LIMIT 1
        "#
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    let lyrics = match lyrics {
        Some(l) => l,
        None => return Err("No lyrics found for track".to_string()),
    };
    
    // Get file path from downloads
    let file_path: Option<(String,)> = sqlx::query_as(
        "SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1"
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    
    let file_path = match file_path {
        Some((p,)) => p,
        None => return Err("No downloaded file found for track".to_string()),
    };
    
    // Check file exists
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    if extension == "flac" {
        let mut tag = metaflac::Tag::read_from_path(path)
            .map_err(|e| format!("Failed to parse FLAC audio file {}: {}", file_path, e))?;
        
        let comments = tag.vorbis_comments_mut();
        comments.remove("LYRICS");
        comments.remove("UNSYNCEDLYRICS");

        if lyrics.format == "lrc" || lyrics.content.contains('[') {
            comments.set("LYRICS", vec![lyrics.content.clone()]);
            let clean = crate::download::lyrics::strip_lrc_timestamps(&lyrics.content);
            if !clean.is_empty() {
                comments.set("UNSYNCEDLYRICS", vec![clean]);
            }
        } else {
            comments.set("UNSYNCEDLYRICS", vec![lyrics.content.clone()]);
        }

        tag.write_to_path(path)
            .map_err(|e| format!("Failed to save FLAC tags to {}: {}", file_path, e))?;
        
        // Re-read verification
        let verified = metaflac::Tag::read_from_path(path)
            .map_err(|e| format!("Verification failed for {}: {}", file_path, e))?;
        let v_comments = verified.vorbis_comments()
            .ok_or_else(|| format!("Verification failed: no VorbisComments found in {}", file_path))?;
        
        if lyrics.format == "lrc" || lyrics.content.contains('[') {
            let read_lrc = v_comments.get("LYRICS").and_then(|v| v.first()).map(|s| s.as_str());
            if read_lrc != Some(&lyrics.content) {
                return Err(format!("Verification failed: LYRICS mismatch in {}", file_path));
            }
        }
    } else {
        return Err(format!("Unsupported audio format for embedding: {}", extension));
    }
    
    // Update database to mark as embedded
    sqlx::query("UPDATE lyrics SET embedded_in_file = 1 WHERE id = ?")
        .bind(lyrics.id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    
    tracing::info!("Successfully embedded lyrics for track_id={}", track_id);
    Ok(true)
}

/// Batch embed lyrics into audio files
#[tauri::command]
pub async fn batch_embed_lyrics(
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<BatchLyricsResult, String> {
    tracing::info!("batch_embed_lyrics: {} tracks", track_ids.len());
    
    let mut embedded = 0i64;
    let mut failed = 0i64;
    let mut skipped = 0i64;
    
    for track_id in track_ids {
        match embed_lyrics(state.clone(), track_id).await {
            Ok(true) => embedded += 1,
            Ok(false) => skipped += 1,
            Err(_) => failed += 1,
        }
    }
    
    Ok(BatchLyricsResult {
        fetched: embedded, // Reusing field for "embedded" count
        failed,
        skipped,
    })
}

#[cfg(test)]
mod lyrics_commands_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use syncify_lyrics_domain::{LyricsLineDomain, LyricsResolution, LyricsSyncType, ResolutionStatus};

    async fn setup_test_lyrics_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        sqlx::query(
            r#"
            CREATE TABLE tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                isrc TEXT UNIQUE
            );
            CREATE TABLE lyrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
                format TEXT NOT NULL,
                sync_level TEXT,
                source TEXT,
                content TEXT NOT NULL,
                language TEXT,
                embedded_in_file INTEGER DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(track_id, format)
            );
            INSERT INTO tracks (id, title, isrc) VALUES (1, 'Test Track', 'USRC12345678');
            "#
        )
        .execute(&pool)
        .await
        .expect("Failed to initialize test schema");

        pool
    }

    struct TempFlacFile {
        path: std::path::PathBuf,
    }

    impl Drop for TempFlacFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn create_valid_test_flac() -> TempFlacFile {
        let path = std::env::temp_dir().join(format!(
            "syncify_cmd_test_{}.flac",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        // Block 0: STREAMINFO (not last, len 34)
        data.push(0x00);
        data.extend_from_slice(&[0x00, 0x00, 0x22]);
        let mut streaminfo = vec![0u8; 34];
        streaminfo[0] = 0x10; streaminfo[1] = 0x00; // min block 4096
        streaminfo[2] = 0x10; streaminfo[3] = 0x00; // max block 4096
        streaminfo[10] = 0x0A; streaminfo[11] = 0xC4; streaminfo[12] = 0x42; // 44100Hz, 2 channels, 16 bps
        streaminfo[13] = 0xF0;
        streaminfo[14] = 0x00; streaminfo[15] = 0x00; streaminfo[16] = 0xAC; streaminfo[17] = 0x44; // total samples
        data.extend_from_slice(&streaminfo);

        // Block 1: VORBIS_COMMENT (last, 0x84)
        let mut comment_data = Vec::new();
        comment_data.extend_from_slice(&4u32.to_le_bytes());
        comment_data.extend_from_slice(b"test");
        comment_data.extend_from_slice(&0u32.to_le_bytes());
        data.push(0x84);
        let comment_len = comment_data.len() as u32;
        data.push((comment_len >> 16) as u8);
        data.push((comment_len >> 8) as u8);
        data.push(comment_len as u8);
        data.extend_from_slice(&comment_data);
        data.extend_from_slice(&[0xFF, 0xF8, 0x00, 0x00]);
        std::fs::write(&path, data).expect("Failed to write dummy FLAC");
        TempFlacFile { path }
    }

    #[tokio::test]
    async fn test_persistence_success_valid_flac_and_reread() {
        let db = setup_test_lyrics_db().await;
        let flac = create_valid_test_flac();
        let elrc = "[00:10.00] <00:10.00>Heroes <00:11.00>forever";
        let plain = "Heroes forever";

        let resolution = LyricsResolution::new_resolved(
            "NetEase Cloud Music",
            "music.163.com",
            LyricsSyncType::KaraokeWordSynced,
            Some(elrc.to_string()),
            Some(plain.to_string()),
            vec![LyricsLineDomain {
                start_time_ms: 10000,
                words: "Heroes forever".to_string(),
                end_time_ms: Some(12000),
            }],
            false,
            "music.163.com",
        );

        let res = process_and_persist_resolution(
            &db,
            resolution,
            120,
            Some(&flac.path),
            Some(1),
        )
        .await;

        assert!(res.is_ok(), "Processing should succeed: {:?}", res.err());
        let payload = res.unwrap();
        assert_eq!(payload.status, ResolutionStatus::Resolved);
        assert_eq!(payload.provider, "NetEase Cloud Music");
        assert_eq!(payload.sync_type, LyricsSyncType::KaraokeWordSynced);
        assert_eq!(payload.format, "KaraokeWordSynced");
        assert!(payload.embedded_in_file);

        // Verify SQLite record
        let row: (i64, String, Option<String>, Option<String>, String, i64) = sqlx::query_as(
            "SELECT track_id, format, sync_level, source, content, embedded_in_file FROM lyrics WHERE track_id = 1",
        )
        .fetch_one(&db)
        .await
        .expect("Row must exist in SQLite");

        assert_eq!(row.0, 1);
        assert_eq!(row.1, "lrc");
        assert_eq!(row.2.as_deref(), Some("word"));
        assert_eq!(row.3.as_deref(), Some("NetEase Cloud Music"));
        assert_eq!(row.4, elrc);
        assert_eq!(row.5, 1); // embedded_in_file is 1
    }

    #[tokio::test]
    async fn test_persistence_rejected_on_nonexistent_file() {
        let db = setup_test_lyrics_db().await;
        let non_existent = std::env::temp_dir().join("nonexistent_test_track_8888.flac");

        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Line 1".to_string()),
            Some("Line 1".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let res = process_and_persist_resolution(
            &db,
            resolution,
            50,
            Some(&non_existent),
            Some(1),
        )
        .await;

        assert!(res.is_err(), "Must reject nonexistent file");
        let err = res.unwrap_err();
        assert!(err.contains("File verification failed") && err.contains("does not exist"));

        // Assert SQLite is clean (0 rows)
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM lyrics WHERE track_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count.0, 0, "No records must be persisted when file validation fails");
    }

    #[tokio::test]
    async fn test_persistence_rejected_on_empty_file() {
        let db = setup_test_lyrics_db().await;
        let empty_file = std::env::temp_dir().join("empty_test_file_8888.flac");
        std::fs::write(&empty_file, b"").unwrap();

        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Line 1".to_string()),
            Some("Line 1".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let res = process_and_persist_resolution(
            &db,
            resolution,
            50,
            Some(&empty_file),
            Some(1),
        )
        .await;
        let _ = std::fs::remove_file(&empty_file);

        assert!(res.is_err(), "Must reject empty file");
        let err = res.unwrap_err();
        assert!(err.contains("File verification failed") && err.contains("empty"));

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM lyrics WHERE track_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count.0, 0, "No records must be persisted on empty file error");
    }

    #[tokio::test]
    async fn test_persistence_rejected_on_non_flac_file() {
        let db = setup_test_lyrics_db().await;
        let bad_file = std::env::temp_dir().join("corrupt_not_flac_8888.flac");
        std::fs::write(&bad_file, b"NOT_FLAC_CORRUPT_HEADER_BYTES_12345").unwrap();

        let resolution = LyricsResolution::new_resolved(
            "LRCLIB",
            "lrclib.net",
            LyricsSyncType::LineSynced,
            Some("[00:10.00]Line 1".to_string()),
            Some("Line 1".to_string()),
            vec![],
            false,
            "lrclib.net",
        );

        let res = process_and_persist_resolution(
            &db,
            resolution,
            50,
            Some(&bad_file),
            Some(1),
        )
        .await;
        let _ = std::fs::remove_file(&bad_file);

        assert!(res.is_err(), "Must reject non-FLAC corrupt file");
        let err = res.unwrap_err();
        assert!(err.contains("File verification failed"));

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM lyrics WHERE track_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count.0, 0, "No records must be persisted on non-FLAC file");
    }

    #[tokio::test]
    async fn test_persistence_skipped_when_status_not_resolved() {
        let db = setup_test_lyrics_db().await;
        let flac = create_valid_test_flac();

        // 1. NotFound
        let not_found_res = LyricsResolution::new_not_found("NetEase", "netease_search");
        let res_nf = process_and_persist_resolution(
            &db,
            not_found_res,
            30,
            Some(&flac.path),
            Some(1),
        )
        .await;
        assert!(res_nf.is_ok());
        assert_eq!(res_nf.unwrap().status, ResolutionStatus::NotFound);

        // 2. SourceUnavailable
        let src_unavail_res = LyricsResolution::new_source_unavailable("LyricsPlus", "lyricsplus_search", "HTTP 404");
        let res_su = process_and_persist_resolution(
            &db,
            src_unavail_res,
            40,
            Some(&flac.path),
            Some(1),
        )
        .await;
        assert!(res_su.is_ok());
        assert_eq!(res_su.unwrap().status, ResolutionStatus::SourceUnavailable);

        // Verify SQLite remains empty
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM lyrics WHERE track_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count.0, 0, "Non-resolved results must NEVER be written to SQLite");
    }

    #[tokio::test]
    async fn test_persistence_without_file_path() {
        let db = setup_test_lyrics_db().await;
        let plain = "Simple plain text lyrics without audio file";

        let resolution = LyricsResolution {
            status: ResolutionStatus::Resolved,
            provider: "LRCLIB".to_string(),
            strategy: "plain_lookup".to_string(),
            format: "PLAIN".to_string(),
            sync_type: LyricsSyncType::Plain,
            provenance: "lrclib.net".to_string(),
            fallback_applied: false,
            error: None,
            synced_content: None,
            plain_text: Some(plain.to_string()),
            lines: vec![],
            is_instrumental: false,
        };

        let res = process_and_persist_resolution(
            &db,
            resolution,
            75,
            None,
            Some(1),
        )
        .await;

        assert!(res.is_ok());
        let payload = res.unwrap();
        assert_eq!(payload.status, ResolutionStatus::Resolved);
        assert!(!payload.embedded_in_file);

        let row: (i64, String, Option<String>, String, i64) = sqlx::query_as(
            "SELECT track_id, format, sync_level, content, embedded_in_file FROM lyrics WHERE track_id = 1",
        )
        .fetch_one(&db)
        .await
        .unwrap();

        assert_eq!(row.0, 1);
        assert_eq!(row.1, "plain");
        assert_eq!(row.2.as_deref(), Some("none"));
        assert_eq!(row.3, plain);
        assert_eq!(row.4, 0); // embedded_in_file is 0
    }

    #[tokio::test]
    async fn test_persistence_instrumental_flag() {
        let db = setup_test_lyrics_db().await;

        let resolution = LyricsResolution {
            status: ResolutionStatus::Resolved,
            provider: "LRCLIB".to_string(),
            strategy: "instrumental_flag".to_string(),
            format: "INSTRUMENTAL".to_string(),
            sync_type: LyricsSyncType::Instrumental,
            provenance: "lrclib.net".to_string(),
            fallback_applied: false,
            error: None,
            synced_content: None,
            plain_text: None,
            lines: vec![],
            is_instrumental: true,
        };

        let res = process_and_persist_resolution(
            &db,
            resolution,
            25,
            None,
            Some(1),
        )
        .await;

        assert!(res.is_ok());
        let payload = res.unwrap();
        assert!(payload.is_instrumental);
        assert_eq!(payload.format, "INSTRUMENTAL");

        let row: (i64, String, Option<String>) = sqlx::query_as(
            "SELECT track_id, format, sync_level FROM lyrics WHERE track_id = 1",
        )
        .fetch_one(&db)
        .await
        .unwrap();

        assert_eq!(row.0, 1);
        assert_eq!(row.1, "instrumental");
        assert_eq!(row.2.as_deref(), Some("none"));
    }
}
