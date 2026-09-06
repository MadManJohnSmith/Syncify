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
            // Write sidecar .lrc if synced lyrics are available
            if let Some(lrc) = resolution.generate_sidecar_lrc() {
                let sidecar = path.with_extension("lrc");
                if let Err(e) = tokio::fs::write(&sidecar, &lrc).await {
                    tracing::warn!("Failed to write sidecar .lrc for {}: {}", path.display(), e);
                }
            }

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
                    INSERT INTO lyrics (track_id, format, sync_level, source, content, language, embedded_in_file)
                    VALUES (?, ?, ?, ?, ?, NULL, ?)
                    ON CONFLICT(track_id, format) DO UPDATE SET
                        content = excluded.content,
                        sync_level = excluded.sync_level,
                        source = excluded.source,
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
    upsert_lyrics(&state.db, &params).await
}

/// Shared INSERT..ON CONFLICT upsert used by `save_lyrics`, the S200 probe
/// and the harvest sweep. Returns the freshly-read row.
pub(crate) async fn upsert_lyrics(
    db: &crate::DbPool,
    params: &SaveLyricsParams,
) -> Result<Lyrics, String> {
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
    .execute(db)
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
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(lyrics)
}

/// Maximum allowed file size for imported lyrics files (1 MB).
pub const MAX_LYRICS_FILE_SIZE_BYTES: u64 = 1024 * 1024;

/// Permitted file extensions for manual lyrics file import.
pub const ALLOWED_LYRICS_EXTENSIONS: &[&str] = &["lrc", "txt"];

/// Returns the set of allowed base directories for reading lyrics files.
/// Strictly confined to the user's Music/Audio, Downloads, Documents, and app data directory.
pub fn get_allowed_lyrics_read_directories() -> Vec<std::path::PathBuf> {
    let mut bases = Vec::new();

    if let Some(audio) = dirs::audio_dir() {
        if let Ok(canon) = std::fs::canonicalize(&audio) {
            bases.push(canon);
        }
        bases.push(audio);
    }

    if let Some(home) = dirs::home_dir() {
        let music = home.join("Music");
        if let Ok(canon) = std::fs::canonicalize(&music) {
            bases.push(canon);
        }
        bases.push(music);
    }

    if let Some(download) = dirs::download_dir() {
        if let Ok(canon) = std::fs::canonicalize(&download) {
            bases.push(canon);
        }
        bases.push(download);
    }

    if let Some(doc) = dirs::document_dir() {
        if let Ok(canon) = std::fs::canonicalize(&doc) {
            bases.push(canon);
        }
        bases.push(doc);
    }

    if let Some(data_local) = dirs::data_local_dir() {
        let app_dir = data_local.join("com.syncify.app");
        if let Ok(canon) = std::fs::canonicalize(&app_dir) {
            bases.push(canon);
        }
        bases.push(app_dir);
    }

    if let Some(data) = dirs::data_dir() {
        let app_dir = data.join("com.syncify.app");
        if let Ok(canon) = std::fs::canonicalize(&app_dir) {
            bases.push(canon);
        }
        bases.push(app_dir);
    }

    bases.sort();
    bases.dedup();
    bases
}

/// Validates that a lyrics file path conforms to sandbox confinement, path traversal
/// restrictions, file extension whitelisting, existence, and size bounds (max 1 MB).
pub fn validate_safe_lyrics_read_path_with_bases(
    path: &std::path::Path,
    allowed_bases: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    // 1. Must be an absolute path
    if !path.is_absolute() {
        return Err("Acceso denegado: la ruta debe ser absoluta (sandbox violation)".to_string());
    }

    // 2. Reject path traversal sequences (.. or ParentDir)
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("Acceso denegado: secuencias de escape ('..') detectadas (sandbox violation)".to_string());
        }
    }

    // 3. Reject hidden files
    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| "Acceso denegado: nombre de archivo no válido (sandbox violation)".to_string())?;

    if file_name.starts_with('.') {
        return Err("Acceso denegado: no se permite leer archivos ocultos o de configuración (sandbox violation)".to_string());
    }

    // 4. Strict extension check: .lrc or .txt (case-insensitive)
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let ext_str = match &ext {
        Some(e) => e.as_str(),
        None => {
            return Err("Acceso denegado: el archivo debe tener extensión .lrc o .txt (sandbox violation)".to_string());
        }
    };

    if !ALLOWED_LYRICS_EXTENSIONS.contains(&ext_str) {
        return Err(format!(
            "Acceso denegado: extensión '.{}' no permitida para importación de letras. Solo se permite .lrc o .txt (sandbox violation)",
            ext_str
        ));
    }

    // 5. Check existence
    if !path.exists() {
        return Err(format!("El archivo de letras no existe: {}", path.display()));
    }

    // 6. Check size limit before full canonicalization/reading (1 MB limit)
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("Error al obtener metadatos del archivo {}: {}", path.display(), e))?;

    if meta.len() > MAX_LYRICS_FILE_SIZE_BYTES {
        return Err(format!(
            "Acceso denegado: el archivo de letras supera el tamaño máximo permitido de 1 MB (tamaño: {} bytes) (sandbox violation)",
            meta.len()
        ));
    }

    // 7. Canonicalize path
    let canonical_path = std::fs::canonicalize(path)
        .map_err(|e| format!("Error al canonicalizar archivo {}: {}", path.display(), e))?;

    // Recheck metadata on canonical target (in case it was a symlink)
    let target_meta = std::fs::metadata(&canonical_path)
        .map_err(|e| format!("Error al verificar metadatos de archivo canonicalizado: {}", e))?;

    if target_meta.len() > MAX_LYRICS_FILE_SIZE_BYTES {
        return Err(format!(
            "Acceso denegado: el archivo de letras supera el tamaño máximo permitido de 1 MB (tamaño: {} bytes) (sandbox violation)",
            target_meta.len()
        ));
    }

    // 8. Defense in depth: reject sensitive system directories
    let canonical_str = canonical_path.to_string_lossy();
    if canonical_str.starts_with("/etc")
        || canonical_str.starts_with("/proc")
        || canonical_str.starts_with("/sys")
        || canonical_str.starts_with("/dev")
        || canonical_str.starts_with("/var")
        || canonical_str.contains("/.ssh")
        || canonical_str.contains("/.gnupg")
        || canonical_str.contains("/.aws")
    {
        return Err("Acceso denegado: ruta en directorio protegido del sistema (sandbox violation)".to_string());
    }

    if allowed_bases.is_empty() {
        return Err("Acceso denegado: no se definieron directorios base permitidos (sandbox violation)".to_string());
    }

    let mut canonical_allowed_bases = Vec::new();
    for b in allowed_bases {
        if let Ok(c) = std::fs::canonicalize(b) {
            canonical_allowed_bases.push(c);
        }
        canonical_allowed_bases.push(b.clone());
    }

    if !canonical_allowed_bases.iter().any(|base| canonical_path.starts_with(base)) {
        return Err("Acceso denegado: la ruta del archivo de letras está fuera de los directorios permitidos (sandbox violation)".to_string());
    }

    Ok(canonical_path)
}

/// Helper to validate a lyrics read path against default allowed directories.
pub fn validate_safe_lyrics_read_path(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let allowed_bases = get_allowed_lyrics_read_directories();
    validate_safe_lyrics_read_path_with_bases(path, &allowed_bases)
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
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err("Acceso denegado: la ruta no puede estar vacía (sandbox violation)".to_string());
    }
    tracing::info!("import_lyrics_file: track_id={} path={}", track_id, trimmed);

    let safe_path = validate_safe_lyrics_read_path(std::path::Path::new(trimmed))?;

    let content = std::fs::read_to_string(&safe_path)
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

// ==============================================
// S200 — LOCAL LYRICS HARVEST (embedded FLAC tags + sidecar files)
// ==============================================
//
// The owner reported lyrics that ARE in his library going undetected: either
// embedded as Vorbis comments inside the FLAC files (LYRICS / UNSYNCEDLYRICS /
// SYNCEDLYRICS) or sitting next to the audio as `.lrc`/`.txt` sidecars. Until
// now `get_lyrics` only read the DB, so neither source ever reached the UI.
//
// Two commands close the gap:
//   * `probe_track_lyrics(track_id)` — one-shot probe used when a track shows
//     no lyrics; persists what it finds so the next lookup is a DB hit.
//   * `harvest_missing_lyrics(limit)` — sweep over every downloaded track with
//     no lyrics rows (sidecar first, then embedded), returning honest counts.
//
// Capability boundary stays FLAC-only (same metaflac boundary as the tag
// writer); M4A ©lyr remains out of scope and is documented as such.

/// LRC detection shared by the import and harvest paths: any of the first 10
/// lines starts with a `[dd:dd…]`-shaped timestamp.
pub(crate) fn looks_like_lrc(content: &str) -> bool {
    content.lines().take(10).any(|line| {
        let l = line.trim_start();
        l.starts_with('[')
            && l.len() > 3
            && l.as_bytes()[1].is_ascii_digit()
            && l.contains(':')
            && l.contains(']')
    })
}

/// Candidate sidecar files for an audio path: same stem, `.lrc` then `.txt`,
/// plus lowercase-extension variants (some rippers write `SONG.LRC`).
fn sidecar_candidates(audio_path: &str) -> Vec<std::path::PathBuf> {
    let p = std::path::Path::new(audio_path);
    let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let dir = match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let mut out = Vec::new();
    for ext in ["lrc", "txt", "LRC", "TXT"] {
        let cand = dir.join(format!("{}.{}", stem, ext));
        if !out.contains(&cand) {
            out.push(cand);
        }
    }
    out
}

/// Embedded-lyrics probe for one FLAC file (blocking IO — call from
/// `spawn_blocking`). Multi-line values are stored as ONE vorbis comment whose
/// value contains `\n`; metaflac surfaces them verbatim.
fn read_embedded_flac_lyrics(path: &std::path::Path) -> Option<(String, bool)> {
    let tag = metaflac::Tag::read_from_path(path).ok()?;
    let comments = tag.vorbis_comments()?;
    const KEYS: [&str; 3] = ["LYRICS", "UNSYNCEDLYRICS", "SYNCEDLYRICS"];
    for key in KEYS {
        if let Some(values) = comments.get(key) {
            let joined = values.join("\n");
            let trimmed = joined.trim().to_string();
            if !trimmed.is_empty() {
                return Some((joined, looks_like_lrc(&trimmed)));
            }
        }
    }
    None
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LyricsHarvestResult {
    pub scanned: i64,
    pub sidecar_found: i64,
    pub embedded_found: i64,
    pub failed: i64,
}

/// One-shot probe for a single track: embedded FLAC lyrics first, then
/// sidecars beside the audio file. Persists whatever it finds (source =
/// `embedded` / `sidecar`) through the standard upsert so subsequent reads are
/// plain DB hits. Returns `None` when nothing is found on disk.
#[tauri::command]
pub async fn probe_track_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<Option<Lyrics>, String> {
    tracing::info!("probe_track_lyrics: track_id={}", track_id);
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1",
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let Some((file_path,)) = row else {
        return Ok(None);
    };

    // Embedded probe (blocking metaflac read off the async runtime).
    let probe_path = std::path::PathBuf::from(&file_path);
    let embedded = tauri::async_runtime::spawn_blocking(move || {
        read_embedded_flac_lyrics(&probe_path)
    })
    .await
    .map_err(|e| format!("join error: {}", e))?;

    let found = if let Some((content, synced)) = embedded {
        Some(SaveLyricsParams {
            track_id,
            format: if synced { "lrc".into() } else { "plain".into() },
            content,
            sync_level: Some(if synced { "line".into() } else { "none".into() }),
            source: Some("embedded".into()),
            language: None,
        })
    } else {
        // Sidecar probe: first existing readable sibling wins.
        for cand in sidecar_candidates(&file_path) {
            if let Ok(content) = tokio::fs::read_to_string(&cand).await {
                if !content.trim().is_empty() {
                    let synced = looks_like_lrc(&content);
                    tracing::info!(
                        "[S200] sidecar lyrics found: {} ({})",
                        cand.display(),
                        if synced { "lrc" } else { "plain" }
                    );
                    return upsert_lyrics(
                        &state.db,
                        &SaveLyricsParams {
                            track_id,
                            format: if synced { "lrc".into() } else { "plain".into() },
                            content,
                            sync_level: Some(if synced { "line".into() } else { "none".into() }),
                            source: Some("sidecar".into()),
                            language: None,
                        },
                    )
                    .await
                    .map(Some);
                }
            }
        }
        None
    };

    match found {
        Some(params) => upsert_lyrics(&state.db, &params).await.map(Some),
        None => Ok(None),
    }
}

/// Sweep every downloaded track that has NO lyrics rows and try to fill it
/// from disk: sidecar files beside the audio first (cheap), then embedded
/// FLAC tags. Honest counts in/out; per-track failures never abort the sweep.
#[tauri::command]
pub async fn harvest_missing_lyrics(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<LyricsHarvestResult, String> {
    let limit = limit.unwrap_or(500).clamp(1, 5_000);
    tracing::info!("harvest_missing_lyrics: limit={}", limit);

    let rows: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT d.track_id, d.file_path
        FROM downloads d
        WHERE NOT EXISTS (SELECT 1 FROM lyrics l WHERE l.track_id = d.track_id)
        ORDER BY d.track_id
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let mut result = LyricsHarvestResult {
        scanned: rows.len() as i64,
        sidecar_found: 0,
        embedded_found: 0,
        failed: 0,
    };

    for (track_id, file_path) in rows {
        let probe_path = std::path::PathBuf::from(&file_path);
        let embedded = tauri::async_runtime::spawn_blocking(move || {
            read_embedded_flac_lyrics(&probe_path)
        })
        .await
        .unwrap_or(None);

        let params = if let Some((content, synced)) = embedded {
            result.embedded_found += 1;
            SaveLyricsParams {
                track_id,
                format: if synced { "lrc".into() } else { "plain".into() },
                content,
                sync_level: Some(if synced { "line".into() } else { "none".into() }),
                source: Some("embedded".into()),
                language: None,
            }
        } else {
            let mut sidecar = None;
            for cand in sidecar_candidates(&file_path) {
                if let Ok(content) = tokio::fs::read_to_string(&cand).await {
                    if !content.trim().is_empty() {
                        sidecar = Some((cand.to_string_lossy().to_string(), content));
                        break;
                    }
                }
            }
            match sidecar {
                Some((_, content)) => {
                    result.sidecar_found += 1;
                    let synced = looks_like_lrc(&content);
                    SaveLyricsParams {
                        track_id,
                        format: if synced { "lrc".into() } else { "plain".into() },
                        content,
                        sync_level: Some(if synced { "line".into() } else { "none".into() }),
                        source: Some("sidecar".into()),
                        language: None,
                    }
                }
                None => continue,
            }
        };

        if upsert_lyrics(&state.db, &params).await.is_err() {
            // De-count on persist failure so counts stay honest.
            if params.source.as_deref() == Some("embedded") {
                result.embedded_found -= 1;
            } else {
                result.sidecar_found -= 1;
            }
            result.failed += 1;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod s200_harvest_tests {
    use super::{looks_like_lrc, sidecar_candidates};

    #[test]
    fn s200_lrc_detection_positive_and_negative() {
        assert!(looks_like_lrc("[00:12.34]hello world\n[00:15.00]next"));
        assert!(looks_like_lrc("[01:02]plain minute-second"));
        assert!(!looks_like_lrc("Just some plain lyrics\nsecond line"));
        assert!(!looks_like_lrc(""));
        // Metadata-only LRC headers without timestamps do NOT count.
        assert!(!looks_like_lrc("[ti:Song]\n[ar:Artist]\nbody text"));
    }

    #[test]
    fn s200_sidecar_candidates_cover_both_extensions() {
        let cands = sidecar_candidates("/music/album/01 Song.flac");
        assert_eq!(cands.len(), 4);
        assert!(cands.contains(&std::path::PathBuf::from("/music/album/01 Song.lrc")));
        assert!(cands.contains(&std::path::PathBuf::from("/music/album/01 Song.TXT")));
        // Same directory as the audio file, same stem.
        assert!(cands
            .iter()
            .all(|p| p.parent() == Some(std::path::Path::new("/music/album"))));
    }
}

// ==============================================
// S202 — LIBRARY-WIDE KARAOKE REFETCH + ANIMATED COVER SWEEP
// ==============================================
//
// GUI parity for two CLI capabilities (legacy/syncify-cli syncify_cli.rs):
//   * re-fetch lyrics for tracks that ALREADY have lyrics, asking the
//     karaoke-first cascade again; a stored word-synced lyric is NEVER replaced
//     by line/plain (NO-DEGRADE rule — same rule as the CLI's
//     "Retained existing ... no karaoke upgrade found").
//   * sweep animated covers into existing album directories using exactly the
//     download-pipeline storage contract: sidecar files (cover.webp,
//     cover.animated.webp, folder.webp, animated.webp) written NEXT TO the
//     audio plus metaflac CoverFront re-tag of every FLAC in that directory
//     (services/animated_cover.rs::resolve_and_download_animated_cover).
//
// Runtime SQL only (S182 convention). Progress via window events. Cancellation
// mirrors commands/tempo.rs static AtomicBool pattern — fully-qualified paths
// here because this file is include!()'d into the shared `commands` module.

static S202_KARAOKE_REFETCH_CANCEL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static S202_KARAOKE_REFETCH_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static S202_COVER_SWEEP_CANCEL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static S202_COVER_SWEEP_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Honest counters for the karaoke refetch sweep.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KaraokeRefetchResult {
    pub checked: i64,
    pub upgraded_to_word: i64,
    pub upgraded_other: i64,
    pub filled_from_missing: i64,
    pub kept: i64,
    pub downgraded_rejected: i64,
    pub failed: i64,
    pub embed_skipped: i64,
    pub cancelled: bool,
}

/// Rank a stored `lyrics.sync_level` so upgrades never degrade:
/// syllable > word > line > none/plain/unknown.
fn s202_sync_level_rank(level: Option<&str>) -> u8 {
    match level.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "syllable" => 4,
        "word" => 3,
        "line" => 2,
        _ => 1,
    }
}

/// Rank a freshly-resolved sync type on the same scale.
fn s202_sync_type_rank(sync_type: &LyricsSyncType) -> u8 {
    match sync_type {
        LyricsSyncType::KaraokeWordSynced => 3,
        LyricsSyncType::LineSynced => 2,
        _ => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum S202UpgradeDecision {
    /// Track had NO lyrics rows at all — any resolved payload is a fill.
    FillMissing,
    /// Existing lyrics are strictly worse than what we just resolved.
    ApplyUpgrade,
    /// Equal level (e.g. word == word) — keep the existing row untouched.
    KeepExisting,
    /// New result is WORSE than what is stored — reject and keep existing.
    RejectDowngrade,
}

/// Pure NO-DEGRADE decision. `existing_level` is `None` when the track has no
/// lyrics row at all (row-with-null-level is `Some(None)`).
fn s202_decide_upgrade(
    existing_level: Option<Option<&str>>,
    new_sync_type: &LyricsSyncType,
) -> S202UpgradeDecision {
    let Some(level) = existing_level else {
        return S202UpgradeDecision::FillMissing;
    };
    let new_rank = s202_sync_type_rank(new_sync_type);
    let cur_rank = s202_sync_level_rank(level);
    if new_rank > cur_rank {
        S202UpgradeDecision::ApplyUpgrade
    } else if new_rank == cur_rank {
        S202UpgradeDecision::KeepExisting
    } else {
        S202UpgradeDecision::RejectDowngrade
    }
}

fn s202_emit_karaoke_progress(
    window: &tauri::Window,
    status: &str,
    current: u64,
    total: u64,
    track_name: &str,
    message: &str,
) {
    let _ = window.emit(
        "karaoke-refetch-progress",
        serde_json::json!({
            "status": status,
            "current": current,
            "total": total,
            "track": track_name,
            "message": message,
        }),
    );
}

struct S202TrackRef {
    track_id: i64,
    file_path: Option<String>,
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<i64>,
}

async fn s202_load_tracks(db: &sqlx::SqlitePool, scope: &str, explicit_ids: &[i64], limit: i64) -> Result<Vec<S202TrackRef>, String> {
    let mut refs = Vec::new();
    if !explicit_ids.is_empty() {
        for id in explicit_ids.iter().take(limit as usize) {
            let row: Option<(i64, Option<String>, String, Option<String>, Option<String>, Option<i64>)> = sqlx::query_as(
                r#"
                SELECT t.id,
                       (SELECT d.file_path FROM downloads d WHERE d.track_id = t.id ORDER BY d.id LIMIT 1),
                       t.title,
                       COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                                 WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1),
                                (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta
                                 JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id)),
                       al.title,
                       t.duration_ms
                FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
                WHERE t.id = ?
                "#,
            )
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(|e| e.to_string())?;
            if let Some((track_id, file_path, title, artist, album, duration_ms)) = row {
                refs.push(S202TrackRef {
                    track_id,
                    file_path,
                    title,
                    artist: artist.unwrap_or_default(),
                    album,
                    duration_ms,
                });
            }
        }
        return Ok(refs);
    }

    let scope_clause = match scope {
        "downloaded" => "WHERE EXISTS (SELECT 1 FROM downloads d WHERE d.track_id = t.id)",
        _ => "",
    };
    let sql = format!(
        r#"
        SELECT t.id,
               (SELECT d.file_path FROM downloads d WHERE d.track_id = t.id ORDER BY d.id LIMIT 1),
               t.title,
               COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                         WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1),
                        (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta
                         JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id)),
               al.title,
               t.duration_ms
        FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
        {scope_clause}
        ORDER BY t.id
        LIMIT ?
        "#
    );
    let rows: Vec<(i64, Option<String>, String, Option<String>, Option<String>, Option<i64>)> =
        sqlx::query_as(&sql)
            .bind(limit)
            .fetch_all(db)
            .await
            .map_err(|e| e.to_string())?;
    for (track_id, file_path, title, artist, album, duration_ms) in rows {
        refs.push(S202TrackRef {
            track_id,
            file_path,
            title,
            artist: artist.unwrap_or_default(),
            album,
            duration_ms,
        });
    }
    Ok(refs)
}

/// Re-fetch lyrics for library tracks INCLUDING those that already have them,
/// aiming at karaoke / word-synced level. Applies an upgrade only when the new
/// resolution ranks STRICTLY higher than what is stored; equal or worse results
/// keep the existing row (honest counters report every outcome).
#[tauri::command]
pub async fn refetch_karaoke_lyrics(
    window: tauri::Window,
    state: State<'_, AppState>,
    scope: Option<String>,
    track_ids: Option<Vec<i64>>,
    limit: Option<i64>,
) -> Result<KaraokeRefetchResult, String> {
    if S202_KARAOKE_REFETCH_RUNNING
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return Err("Ya hay un re-chequeo de letras en curso".to_string());
    }
    S202_KARAOKE_REFETCH_CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);

    let scope = scope.unwrap_or_else(|| "downloaded".to_string());
    let limit = limit.unwrap_or(2_000).clamp(1, 20_000);
    let ids = track_ids.unwrap_or_default();

    let run = async {
        let tracks = s202_load_tracks(&state.db, &scope, &ids, limit).await?;
        let total = tracks.len() as u64;
        tracing::info!("[S202] refetch_karaoke_lyrics: {} pistas (scope={})", total, scope);

        let mut result = KaraokeRefetchResult {
            checked: 0,
            upgraded_to_word: 0,
            upgraded_other: 0,
            filled_from_missing: 0,
            kept: 0,
            downgraded_rejected: 0,
            failed: 0,
            embed_skipped: 0,
            cancelled: false,
        };

        s202_emit_karaoke_progress(&window, "started", 0, total, "Iniciando re-chequeo karaoke...", "");

        let client = crate::download::LyricsClient::new();

        for (idx, tr) in tracks.into_iter().enumerate() {
            if S202_KARAOKE_REFETCH_CANCEL.load(std::sync::atomic::Ordering::SeqCst) {
                result.cancelled = true;
                break;
            }
            let current = (idx + 1) as u64;
            result.checked += 1;

            // Best stored sync_level for this track (NULL-safe).
            let existing: Option<(Option<String>,)> = sqlx::query_as(
                r#"
                SELECT sync_level FROM lyrics WHERE track_id = ?
                ORDER BY CASE WHEN sync_level = 'syllable' THEN 4
                              WHEN sync_level = 'word' THEN 3
                              WHEN sync_level = 'line' THEN 2
                              ELSE 1 END DESC, id ASC
                LIMIT 1
                "#,
            )
            .bind(tr.track_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| e.to_string())?;

            let label = format!("{} - {}", tr.artist, tr.title);
            s202_emit_karaoke_progress(&window, "checking", current, total, &label, "Consultando proveedores...");

            // Shared global rate limiter (services/rate_limiter.rs profiles).
            crate::services::rate_limiter::GLOBAL_RATE_LIMITER.acquire("lrclib").await;

            let duration_sec = tr.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);
            let (resolution, _latency) = client
                .orchestrate_resolution(&tr.artist, &tr.title, tr.album.as_deref(), duration_sec)
                .await;

            if resolution.status != ResolutionStatus::Resolved {
                result.failed += 1;
                s202_emit_karaoke_progress(&window, "not_found", current, total, &label, "Sin resultado utilizable");
                continue;
            }
            let content = resolution
                .synced_content
                .clone()
                .or(resolution.plain_text.clone())
                .unwrap_or_default();
            if content.trim().is_empty() && !resolution.is_instrumental {
                result.failed += 1;
                s202_emit_karaoke_progress(&window, "not_found", current, total, &label, "Contenido vacío");
                continue;
            }

            let decision = s202_decide_upgrade(
                existing.map(|(lvl,)| lvl).as_ref().map(|lvl| lvl.as_deref()),
                &resolution.sync_type,
            );

            match decision {
                S202UpgradeDecision::KeepExisting => {
                    result.kept += 1;
                    s202_emit_karaoke_progress(&window, "kept", current, total, &label, "Ya tiene el mejor nivel disponible");
                }
                S202UpgradeDecision::RejectDowngrade => {
                    result.downgraded_rejected += 1;
                    s202_emit_karaoke_progress(
                        &window,
                        "downgrade_rejected",
                        current,
                        total,
                        &label,
                        "Resultado peor que la letra almacenada: se conserva la actual",
                    );
                }
                _ => {
                    // Embed FIRST with strict FLAC verification (same contract as
                    // resolve_track_lyrics), then persist to SQLite, then sidecar.
                    let mut embedded_ok = false;
                    if let Some(fp) = tr.file_path.as_ref().filter(|p| !p.trim().is_empty()) {
                        let path = std::path::PathBuf::from(fp);
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        if ext == "flac" && path.is_file() {
                            let embed_path = path.clone();
                            let embed_res = resolution.clone();
                            let verified = tauri::async_runtime::spawn_blocking(move || {
                                crate::download::lyrics::validate_and_embed_flac_lyrics(&embed_path, &embed_res)
                            })
                            .await
                            .map_err(|e| format!("join error: {}", e))?;
                            match verified {
                                Ok(true) => embedded_ok = true,
                                Ok(false) => {
                                    result.embed_skipped += 1;
                                    tracing::warn!("[S202] Embed omitido para {} (sin letra embebible)", fp);
                                }
                                Err(e) => {
                                    // Do not persist unverified payloads — keep DB and file consistent.
                                    result.failed += 1;
                                    tracing::error!("[S202] Verificación de embed falló para {}: {}", fp, e);
                                    s202_emit_karaoke_progress(&window, "error", current, total, &label, &format!("Embed falló: {}", e));
                                    continue;
                                }
                            }
                        } else {
                            // Capability boundary stays FLAC-only (same as tag writer).
                            result.embed_skipped += 1;
                        }
                    }

                    // Sidecar `.lrc` ONLY for valid synced lyrics — exact pipeline §6b contract.
                    if let (Some(lrc), Some(fp)) = (resolution.generate_sidecar_lrc(), tr.file_path.as_deref()) {
                        let sidecar = std::path::Path::new(fp).with_extension("lrc");
                        if let Err(e) = tokio::fs::write(&sidecar, &lrc).await {
                            tracing::warn!("[S202] No se pudo escribir sidecar {}: {}", sidecar.display(), e);
                        }
                    }

                    let format = match resolution.sync_type {
                        LyricsSyncType::KaraokeWordSynced | LyricsSyncType::LineSynced => "lrc",
                        LyricsSyncType::Plain => "plain",
                        LyricsSyncType::Instrumental => "instrumental",
                        LyricsSyncType::None => "none",
                    };
                    let sync_level = match resolution.sync_type {
                        LyricsSyncType::KaraokeWordSynced => "word",
                        LyricsSyncType::LineSynced => "line",
                        _ => "none",
                    };
                    let params = SaveLyricsParams {
                        track_id: tr.track_id,
                        format: format.to_string(),
                        content: content.clone(),
                        sync_level: Some(sync_level.to_string()),
                        source: Some(resolution.provider.clone()),
                        language: None,
                    };
                    if upsert_lyrics(&state.db, &params).await.is_err() {
                        result.failed += 1;
                        s202_emit_karaoke_progress(&window, "error", current, total, &label, "No se pudo guardar la letra");
                        continue;
                    }

                    // F5.3: Ensure embedded_in_file is accurately persisted in lyrics table (mitiga A11)
                    let _ = sqlx::query(
                        r#"INSERT INTO lyrics (track_id, format, sync_level, source, content, language, embedded_in_file)
                           VALUES (?, ?, ?, ?, ?, NULL, ?)
                           ON CONFLICT(track_id, format) DO UPDATE SET
                               content = excluded.content,
                               sync_level = excluded.sync_level,
                               source = excluded.source,
                               embedded_in_file = excluded.embedded_in_file"#
                    )
                    .bind(tr.track_id)
                    .bind(format)
                    .bind(sync_level)
                    .bind(&resolution.provider)
                    .bind(&content)
                    .bind(if embedded_ok { 1i64 } else { 0i64 })
                    .execute(&state.db)
                    .await;

                    if matches!(decision, S202UpgradeDecision::ApplyUpgrade) {
                        if resolution.sync_type == LyricsSyncType::KaraokeWordSynced {
                            result.upgraded_to_word += 1;
                            let msg = if embedded_ok { "🚀 Mejorado a KARAOKE (palabra)" } else { "Mejorado a palabra (sin archivo)" };
                            s202_emit_karaoke_progress(&window, "upgraded_to_word", current, total, &label, msg);
                        } else {
                            result.upgraded_other += 1;
                            s202_emit_karaoke_progress(&window, "upgraded_other", current, total, &label, "Nivel de sincronía mejorado");
                        }
                    } else {
                        result.filled_from_missing += 1;
                        s202_emit_karaoke_progress(&window, "filled", current, total, &label, "Letra obtenida (no tenía)");
                    }
                }
            }
        }

        let final_status = if result.cancelled { "cancelled" } else { "completed" };
        s202_emit_karaoke_progress(
            &window,
            final_status,
            total,
            total,
            "Re-chequeo terminado",
            &format!(
                "Verificadas: {}, a palabra: {}, otras mejoras: {}, rellenadas: {}, intactas: {}, rechazadas por empeorar: {}, fallidas: {}",
                result.checked, result.upgraded_to_word, result.upgraded_other, result.filled_from_missing,
                result.kept, result.downgraded_rejected, result.failed
            ),
        );
        Ok(result)
    };

    let out = run.await;
    S202_KARAOKE_REFETCH_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    out
}

/// Ask the running karaoke refetch to stop after the current track.
#[tauri::command]
pub async fn cancel_karaoke_refetch() -> Result<bool, String> {
    S202_KARAOKE_REFETCH_CANCEL.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(true)
}

// ==============================================
// S202b — ANIMATED COVER SWEEP (post-hoc, pipeline contract)
// ==============================================

/// Honest counters for the animated-cover sweep.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnimatedCoverSweepResult {
    pub scanned_albums: i64,
    pub already_animated: i64,
    pub downloaded: i64,
    pub not_found: i64,
    pub source_unavailable: i64,
    pub failed: i64,
    pub cancelled: bool,
}

/// Idempotency guard: `cover.webp` beside the audio is the marker the pipeline
/// writes first on success; it counts as fresh ONLY when it still validates as
/// a real animated WebP (VP8X animation bit + ANMF frames).
pub(crate) fn s202_animated_cover_marker_fresh(dir: &std::path::Path) -> bool {
    let marker = dir.join("cover.webp");
    if !marker.is_file() {
        return false;
    }
    match std::fs::read(&marker) {
        Ok(bytes) => crate::services::animated_cover::validate_animated_webp_bytes(&bytes).is_ok(),
        Err(_) => false,
    }
}

/// Deduplicate (artist, album) pairs from downloaded tracks, keeping the first
/// valid parent directory per album. Pure helper — unit tested below.
fn s202_collect_album_dirs(rows: Vec<(String, String, String)>) -> Vec<(String, String, std::path::PathBuf)> {
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (artist, album, file_path) in rows {
        let key = (artist.trim().to_lowercase(), album.trim().to_lowercase());
        if artist.trim().is_empty() || album.trim().is_empty() || !seen.insert(key) {
            continue;
        }
        let Some(parent) = std::path::Path::new(&file_path).parent() else {
            continue;
        };
        if parent.as_os_str().is_empty() {
            continue;
        }
        out.push((artist, album, parent.to_path_buf()));
    }
    out
}

fn s202_emit_sweep_progress(window: &tauri::Window, status: &str, current: u64, total: u64, album: &str, message: &str) {
    let _ = window.emit(
        "animated-cover-sweep-progress",
        serde_json::json!({
            "status": status,
            "current": current,
            "total": total,
            "album": album,
            "message": message,
        }),
    );
}

/// Sweep animated covers for every album that has downloaded tracks. Uses the
/// SAME storage contract as the download pipeline: the service writes its
/// sidecars next to the audio and re-tags FLACs in that directory. Idempotent:
/// albums whose `cover.webp` already validates are skipped unless `force`.
#[tauri::command]
pub async fn sweep_animated_covers(
    window: tauri::Window,
    state: State<'_, AppState>,
    force: Option<bool>,
    limit: Option<i64>,
) -> Result<AnimatedCoverSweepResult, String> {
    if S202_COVER_SWEEP_RUNNING
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return Err("Ya hay un barrido de portadas en curso".to_string());
    }
    S202_COVER_SWEEP_CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);

    let force = force.unwrap_or(false);
    let limit = limit.unwrap_or(2_000).clamp(1, 10_000);

    let run = async {
        let rows: Vec<(Option<String>, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT COALESCE((SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                             WHERE ta.track_id = t.id AND ta.role = 'primary' LIMIT 1),
                            (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta
                             JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id)),
                   al.title,
                   (SELECT d.file_path FROM downloads d WHERE d.track_id = t.id ORDER BY d.id LIMIT 1)
            FROM tracks t
            JOIN albums al ON al.id = t.album_id
            WHERE EXISTS (SELECT 1 FROM downloads dw WHERE dw.track_id = t.id)
            ORDER BY t.id
            "#,
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

        let triplets = rows
            .into_iter()
            .filter_map(|(artist, album, path)| Some((artist?, album?, path)))
            .collect::<Vec<_>>();
        let mut albums = s202_collect_album_dirs(triplets);
        albums.truncate(limit as usize);
        let total = albums.len() as u64;

        tracing::info!("[S202] sweep_animated_covers: {} álbumes (force={})", total, force);

        let mut result = AnimatedCoverSweepResult {
            scanned_albums: 0,
            already_animated: 0,
            downloaded: 0,
            not_found: 0,
            source_unavailable: 0,
            failed: 0,
            cancelled: false,
        };

        s202_emit_sweep_progress(&window, "started", 0, total, "", "Iniciando barrido de portadas animadas...");

        let client = crate::download::http_client::shared_http_client();

        for (idx, (artist, album, dir)) in albums.iter().enumerate() {
            if S202_COVER_SWEEP_CANCEL.load(std::sync::atomic::Ordering::SeqCst) {
                result.cancelled = true;
                break;
            }
            let current = (idx + 1) as u64;
            result.scanned_albums += 1;
            let label = format!("{} - {}", artist, album);

            // Blocking small-file IO off the runtime, mirroring probe_track_lyrics.
            let check_dir = dir.clone();
            let fresh = tauri::async_runtime::spawn_blocking(move || s202_animated_cover_marker_fresh(&check_dir))
                .await
                .unwrap_or(false);
            if !force && fresh {
                result.already_animated += 1;
                s202_emit_sweep_progress(&window, "skipped_already", current, total, &label, "Portada animada ya presente y válida");
                continue;
            }

            s202_emit_sweep_progress(&window, "resolving", current, total, &label, "Resolviendo en Apple Music...");

            crate::services::rate_limiter::GLOBAL_RATE_LIMITER.acquire("apple_music").await;

            match crate::services::animated_cover::resolve_and_download_animated_cover(client, artist, album, dir).await {
                crate::services::animated_cover::AnimatedCoverStatus::Success(_) => {
                    result.downloaded += 1;
                    s202_emit_sweep_progress(&window, "downloaded", current, total, &label, "Portada animada descargada");
                }
                crate::services::animated_cover::AnimatedCoverStatus::NotFound => {
                    result.not_found += 1;
                    s202_emit_sweep_progress(&window, "not_found", current, total, &label, "Apple Music no tiene portada animada");
                }
                crate::services::animated_cover::AnimatedCoverStatus::SourceUnavailable(reason) => {
                    result.source_unavailable += 1;
                    s202_emit_sweep_progress(&window, "source_unavailable", current, total, &label, &reason);
                }
                crate::services::animated_cover::AnimatedCoverStatus::Failed(e) => {
                    result.failed += 1;
                    s202_emit_sweep_progress(&window, "failed", current, total, &label, &e);
                }
            }
        }

        let final_status = if result.cancelled { "cancelled" } else { "completed" };
        s202_emit_sweep_progress(
            &window,
            final_status,
            total,
            total,
            "Barrido terminado",
            &format!(
                "Álbumes: {}, ya animadas: {}, descargadas: {}, sin animación: {}, fuente caída: {}, fallos: {}",
                result.scanned_albums, result.already_animated, result.downloaded,
                result.not_found, result.source_unavailable, result.failed
            ),
        );
        Ok(result)
    };

    let out = run.await;
    S202_COVER_SWEEP_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    out
}

/// Ask the running animated-cover sweep to stop after the current album.
#[tauri::command]
pub async fn cancel_animated_cover_sweep() -> Result<bool, String> {
    S202_COVER_SWEEP_CANCEL.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(true)
}

#[cfg(test)]
mod s202_tests {
    use super::*;

    #[test]
    fn s202_no_degrade_decision_matrix() {
        use LyricsSyncType::{KaraokeWordSynced, LineSynced, Plain};

        // NO-DEGRADE core: stored word-synced must never be replaced by worse levels.
        assert_eq!(s202_decide_upgrade(Some(Some("word")), &LineSynced), S202UpgradeDecision::RejectDowngrade);
        assert_eq!(s202_decide_upgrade(Some(Some("word")), &Plain), S202UpgradeDecision::RejectDowngrade);
        assert_eq!(s202_decide_upgrade(Some(Some("word")), &KaraokeWordSynced), S202UpgradeDecision::KeepExisting);

        // syllable outranks word (finer granularity must be protected too).
        assert_eq!(s202_decide_upgrade(Some(Some("syllable")), &KaraokeWordSynced), S202UpgradeDecision::RejectDowngrade);

        // Legitimate upgrades apply.
        assert_eq!(s202_decide_upgrade(Some(Some("line")), &KaraokeWordSynced), S202UpgradeDecision::ApplyUpgrade);
        assert_eq!(s202_decide_upgrade(Some(Some("none")), &KaraokeWordSynced), S202UpgradeDecision::ApplyUpgrade);
        assert_eq!(s202_decide_upgrade(Some(Some("plain")), &LineSynced), S202UpgradeDecision::ApplyUpgrade);

        // Row present but NULL level behaves like 'none'.
        assert_eq!(s202_decide_upgrade(Some(None), &LineSynced), S202UpgradeDecision::ApplyUpgrade);
        // No row at all → fill-missing regardless of the found level.
        assert_eq!(s202_decide_upgrade(None, &Plain), S202UpgradeDecision::FillMissing);

        // Rank ordering sanity.
        assert!(s202_sync_level_rank(Some("syllable")) > s202_sync_level_rank(Some("word")));
        assert!(s202_sync_level_rank(Some("word")) > s202_sync_level_rank(Some("line")));
        assert_eq!(s202_sync_level_rank(None), s202_sync_level_rank(Some("nonsense")));
    }

    #[test]
    fn s202_animated_marker_fresh_requires_real_animation() {
        let dir = tempfile::tempdir().expect("tempdir");

        // Missing marker → not fresh.
        assert!(!s202_animated_cover_marker_fresh(dir.path()));

        // Static WebP (VP8X without the animation bit) must NOT count as fresh.
        let mut static_webp = Vec::new();
        static_webp.extend_from_slice(b"RIFF");
        static_webp.extend_from_slice(&22u32.to_le_bytes());
        static_webp.extend_from_slice(b"WEBP");
        static_webp.extend_from_slice(b"VP8X");
        static_webp.extend_from_slice(&10u32.to_le_bytes());
        static_webp.extend_from_slice(&[0u8; 10]);
        std::fs::write(dir.path().join("cover.webp"), &static_webp).unwrap();
        assert!(!s202_animated_cover_marker_fresh(dir.path()));

        // Synthetic animated WebP (VP8X animation bit + ANMF chunk) IS fresh.
        let mut animated = Vec::new();
        animated.extend_from_slice(b"RIFF");
        animated.extend_from_slice(&(4 + 18 + 32u32).to_le_bytes());
        animated.extend_from_slice(b"WEBP");
        animated.extend_from_slice(b"VP8X");
        animated.extend_from_slice(&10u32.to_le_bytes());
        let mut vp8x = vec![0u8; 10];
        vp8x[0] = 0x02; // animation flag
        vp8x[4] = 199;
        vp8x[7] = 199;
        animated.extend_from_slice(&vp8x);
        animated.extend_from_slice(b"ANMF");
        animated.extend_from_slice(&24u32.to_le_bytes());
        animated.extend_from_slice(&[0u8; 24]);
        assert_eq!(
            crate::services::animated_cover::validate_animated_webp_bytes(&animated).unwrap(),
            1
        );
        std::fs::write(dir.path().join("cover.webp"), &animated).unwrap();
        assert!(s202_animated_cover_marker_fresh(dir.path()));
    }

    #[test]
    fn s202_album_dir_dedup_keeps_first_directory() {
        let rows = vec![
            ("Radiohead".into(), "OK Computer".into(), "/m/Radiohead/OK Computer/01.flac".into()),
            ("radiohead".into(), "ok computer".into(), "/m/Radiohead/OK Computer/02.flac".into()),
            ("".into(), "Sin Artista".into(), "/x/01.flac".into()),
            ("Otros".into(), "".into(), "/y/02.flac".into()),
        ];
        let out = s202_collect_album_dirs(rows);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "Radiohead");
        assert_eq!(out[0].2, std::path::PathBuf::from("/m/Radiohead/OK Computer"));
    }
}
