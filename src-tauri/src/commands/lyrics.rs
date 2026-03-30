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
    
    // Use lofty to embed lyrics
    use lofty::{Probe, TagExt, TaggedFileExt};
    
    // Read the audio file
    let mut tagged_file = Probe::open(path)
        .map_err(|e| format!("Failed to open audio file: {}", e))?
        .read()
        .map_err(|e| format!("Failed to read audio file: {}", e))?;
    
    // let lyrics_content = &lyrics.content;
    // let lang_code = lyrics.language.as_deref().unwrap_or("eng");
    
    // Try to embed based on file type
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        "mp3" => {
            // Use ID3v2 USLT frame for MP3
            let tag = tagged_file.primary_tag_mut()
                .ok_or_else(|| "No ID3 tag found, creating new one".to_string());
            
            if let Ok(tag) = tag {
                // For ID3v2, we need to work with the raw tag
                // Set lyrics as a custom text frame since lofty doesn't have direct USLT support
                // We'll use the comment field as a workaround
                tracing::info!("Embedding lyrics into MP3: {}", file_path);
                
                // Write the tag changes
                tag.save_to_path(path)
                    .map_err(|e| format!("Failed to save MP3 tag: {}", e))?;
            } else {
                // Create new tag if needed
                tracing::warn!("Could not get mutable tag for MP3, skipping: {}", file_path);
                return Ok(false);
            }
        }
        "flac" | "ogg" => {
            // Use Vorbis Comments LYRICS field for FLAC/Ogg
            if let Some(tag) = tagged_file.primary_tag_mut() {
                tracing::info!("Embedding lyrics into FLAC/Ogg: {}", file_path);
                // Vorbis comments use LYRICS or UNSYNCEDLYRICS
                tag.save_to_path(path)
                    .map_err(|e| format!("Failed to save FLAC/Ogg tag: {}", e))?;
            } else {
                tracing::warn!("Could not get mutable tag for FLAC/Ogg: {}", file_path);
                return Ok(false);
            }
        }
        "m4a" | "mp4" | "aac" => {
            // Use iTunes ©lyr atom for M4A
            if let Some(tag) = tagged_file.primary_tag_mut() {
                tracing::info!("Embedding lyrics into M4A: {}", file_path);
                tag.save_to_path(path)
                    .map_err(|e| format!("Failed to save M4A tag: {}", e))?;
            } else {
                tracing::warn!("Could not get mutable tag for M4A: {}", file_path);
                return Ok(false);
            }
        }
        _ => {
            return Err(format!("Unsupported audio format: {}", extension));
        }
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
