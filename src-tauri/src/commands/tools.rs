// Tools Commands - included via include!() in mod.rs
// 
// Lyrics, metadata, fingerprint, conversion, scanner, organizer, progress, dependencies

// Handlers - remaining commands included via include!() in mod.rs
// 
// Lyrics, metadata, fingerprint, conversion, scanner, accounts, queue, migration commands


// ==============================================
// LYRICS COMMANDS
// ==============================================

/// Lyrics result from Python subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsResult {
    pub success: bool,
    pub data: Option<LyricsData>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsData {
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
    pub word_synced: bool,
    pub instrumental: Option<bool>,
    pub source: Option<String>,
}

/// Fetch lyrics for a track (Rust-native via LRCLIB)
#[tauri::command]
pub async fn fetch_lyrics(
    track: String,
    artist: String,
    _album: Option<String>,
) -> Result<LyricsResult, String> {
    tracing::info!("fetch_lyrics: {} - {}", artist, track);

    let lyrics_client = crate::download::LyricsClient::new();

    // Try fetching lyrics - use 0.0 duration to skip duration matching
    match lyrics_client.fetch_all_sources(&artist, &track, 0.0).await {
        Ok(lyrics) => {
            // Convert LyricsResponse to LyricsData format
            let synced_lyrics = if !lyrics.lines.is_empty() {
                Some(crate::download::LyricsClient::to_lrc_string(&lyrics))
            } else {
                None
            };

            Ok(LyricsResult {
                success: true,
                data: Some(LyricsData {
                    synced_lyrics,
                    plain_lyrics: lyrics.plain_lyrics,
                    word_synced: lyrics.sync_type == "WORD_SYNCED",
                    instrumental: Some(lyrics.instrumental),
                    source: Some(lyrics.source),
                }),
                error: None,
            })
        }
        Err(e) => Ok(LyricsResult {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

// ==============================================
// DOWNLOAD COMMANDS
// ==============================================

/// Download result from Python subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadBridgeResult {
    pub success: bool,
    pub data: Option<DownloadData>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadData {
    pub file_path: Option<String>,
    pub format: Option<String>,
    pub size_bytes: Option<i64>,
}

/// Download a track from a service
#[tauri::command]
pub async fn download_track(
    service: String,
    track_id: String,
    output_path: Option<String>,
    quality: Option<String>,
) -> Result<DownloadBridgeResult, String> {
    tracing::info!("download_track: {} track {}", service, track_id);

    let python = if cfg!(windows) {
        std::path::Path::new(".venv/Scripts/python.exe")
    } else {
        std::path::Path::new(".venv/bin/python")
    };

    let python_cmd = if python.exists() {
        python.to_string_lossy().to_string()
    } else {
        "python".to_string()
    };

    let mut cmd = tokio::process::Command::new(&python_cmd);
    cmd.arg("scripts/download_bridge.py")
        .arg(&service)
        .arg(&track_id);

    if let Some(ref path) = output_path {
        cmd.arg("--output").arg(path);
    }

    if let Some(ref q) = quality {
        cmd.arg("--quality").arg(q);
    }

    let output = cmd
        .current_dir(
            std::env::current_dir()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )
        .output()
        .await
        .map_err(|e| format!("Failed to run download: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(result) = serde_json::from_str::<DownloadBridgeResult>(&stdout) {
        Ok(result)
    } else {
        Err(format!("Failed to parse download result: {}", stdout))
    }
}

// ==============================================
// METADATA ENRICHMENT COMMANDS
// ==============================================

/// Metadata enrichment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Enrich track metadata using MusicBrainz/Last.fm
#[tauri::command]
pub async fn enrich_metadata(
    track: String,
    artist: String,
    isrc: Option<String>,
    album: Option<String>,
) -> Result<MetadataResult, String> {
    tracing::info!("enrich_metadata: {} - {}", artist, track);

    let python = if cfg!(windows) {
        std::path::Path::new(".venv/Scripts/python.exe")
    } else {
        std::path::Path::new(".venv/bin/python")
    };

    let python_cmd = if python.exists() {
        python.to_string_lossy().to_string()
    } else {
        "python".to_string()
    };

    let mut cmd = tokio::process::Command::new(&python_cmd);
    cmd.arg("scripts/metadata_bridge.py")
        .arg("enrich")
        .arg(&track)
        .arg(&artist);

    if let Some(ref i) = isrc {
        cmd.arg("--isrc").arg(i);
    }
    if let Some(ref a) = album {
        cmd.arg("--album").arg(a);
    }

    let output = cmd
        .current_dir(
            std::env::current_dir()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )
        .output()
        .await
        .map_err(|e| format!("Failed to run metadata enrichment: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(result) = serde_json::from_str::<MetadataResult>(&stdout) {
        Ok(result)
    } else {
        Err(format!("Failed to parse metadata result: {}", stdout))
    }
}



// ==============================================
// FINGERPRINT / ACOUSTID COMMANDS
// ==============================================

/// Fingerprint result from Python subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Check if fpcalc (Chromaprint) is available
#[tauri::command]
pub async fn check_fingerprint_available() -> Result<FingerprintResult, String> {
    tracing::info!("check_fingerprint_available");

    let python = if cfg!(windows) {
        std::path::Path::new(".venv/Scripts/python.exe")
    } else {
        std::path::Path::new(".venv/bin/python")
    };

    let python_cmd = if python.exists() {
        python.to_string_lossy().to_string()
    } else {
        "python".to_string()
    };

    let output = tokio::process::Command::new(&python_cmd)
        .arg("scripts/fingerprint_bridge.py")
        .arg("check")
        .current_dir(
            std::env::current_dir()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )
        .output()
        .await
        .map_err(|e| format!("Failed to check fingerprint: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(result) = serde_json::from_str::<FingerprintResult>(&stdout) {
        Ok(result)
    } else {
        Err(format!("Failed to parse result: {}", stdout))
    }
}

/// Identify a track using audio fingerprint
#[tauri::command]
pub async fn identify_audio(file_path: String) -> Result<FingerprintResult, String> {
    tracing::info!("identify_audio: {}", file_path);

    let python = if cfg!(windows) {
        std::path::Path::new(".venv/Scripts/python.exe")
    } else {
        std::path::Path::new(".venv/bin/python")
    };

    let python_cmd = if python.exists() {
        python.to_string_lossy().to_string()
    } else {
        "python".to_string()
    };

    let output = tokio::process::Command::new(&python_cmd)
        .arg("scripts/fingerprint_bridge.py")
        .arg("identify")
        .arg(&file_path)
        .current_dir(
            std::env::current_dir()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )
        .output()
        .await
        .map_err(|e| format!("Failed to identify audio: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(result) = serde_json::from_str::<FingerprintResult>(&stdout) {
        Ok(result)
    } else {
        Err(format!("Failed to parse result: {}", stdout))
    }
}

/// Find duplicate audio files in given paths
#[tauri::command]
pub async fn find_audio_duplicates(paths: Vec<String>) -> Result<FingerprintResult, String> {
    tracing::info!("find_audio_duplicates: {:?}", paths);

    let python = if cfg!(windows) {
        std::path::Path::new(".venv/Scripts/python.exe")
    } else {
        std::path::Path::new(".venv/bin/python")
    };

    let python_cmd = if python.exists() {
        python.to_string_lossy().to_string()
    } else {
        "python".to_string()
    };

    let mut cmd = tokio::process::Command::new(&python_cmd);
    cmd.arg("scripts/fingerprint_bridge.py").arg("duplicates");

    for path in &paths {
        cmd.arg(path);
    }

    let output = cmd
        .current_dir(
            std::env::current_dir()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )
        .output()
        .await
        .map_err(|e| format!("Failed to find duplicates: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(result) = serde_json::from_str::<FingerprintResult>(&stdout) {
        Ok(result)
    } else {
        Err(format!("Failed to parse result: {}", stdout))
    }
}

// ==============================================
// CONVERSION COMMANDS (FFmpeg)
// ==============================================

/// Generic bridge result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Check if FFmpeg is available
#[tauri::command]
pub async fn check_ffmpeg_available() -> Result<BridgeResult, String> {
    run_bridge_command("conversion_bridge.py", &["check"]).await
}

/// Get audio file info
#[tauri::command]
pub async fn get_audio_info(file_path: String) -> Result<BridgeResult, String> {
    run_bridge_command("conversion_bridge.py", &["info", &file_path]).await
}

/// Convert audio file format
#[tauri::command]
pub async fn convert_audio(
    input_path: String,
    output_path: String,
    format: String,
    quality: Option<String>,
) -> Result<BridgeResult, String> {
    let quality_arg = quality.as_deref().unwrap_or("high");
    run_bridge_command(
        "conversion_bridge.py",
        &[
            "convert",
            &input_path,
            &output_path,
            "--format",
            &format,
            "--quality",
            quality_arg,
        ],
    )
    .await
}

// ==============================================
// SCANNER COMMANDS (Local Library)
// ==============================================

/// Scan a directory for audio files
#[tauri::command]
pub async fn scan_local_library(
    directory: String,
    recursive: Option<bool>,
    limit: Option<i32>,
) -> Result<BridgeResult, String> {
    let mut args = vec!["scan", &directory];

    if recursive == Some(false) {
        args.push("--no-recursive");
    }

    let limit_str = limit.map(|l| l.to_string());
    if let Some(ref l) = limit_str {
        args.push("--limit");
        args.push(l);
    }

    run_bridge_command("scanner_bridge.py", &args).await
}

/// Get metadata for a single audio file
#[tauri::command]
pub async fn get_local_track_metadata(file_path: String) -> Result<BridgeResult, String> {
    run_bridge_command("scanner_bridge.py", &["metadata", &file_path]).await
}

// ==============================================
// HELPER FUNCTION
// ==============================================

/// Get the project root directory (Syncify repo root)
/// Tries multiple detection methods for reliability
pub fn get_project_root() -> std::path::PathBuf {
    // Method 1: Check for CARGO_MANIFEST_DIR (available during cargo run/build)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = std::path::Path::new(&manifest_dir);
        if manifest_path.join("scripts").exists() {
            return manifest_path.to_path_buf();
        }
        // src-tauri/Cargo.toml -> go up one level
        if let Some(parent) = manifest_path.parent() {
            if parent.join("scripts").exists() {
                return parent.to_path_buf();
            }
        }
    }

    // Method 2: Check current executable path
    if let Ok(exe_path) = std::env::current_exe() {
        let mut path = exe_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        // Walk up looking for scripts/ directory
        for _ in 0..5 {
            if path.join("scripts").exists() {
                return path;
            }
            if let Some(parent) = path.parent() {
                path = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    // Method 3: Check current directory
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("scripts").exists() {
            return cwd;
        }
        if let Some(parent) = cwd.parent() {
            if parent.join("scripts").exists() {
                return parent.to_path_buf();
            }
        }
    }

    // Fallback to current directory
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Get the Python executable path
pub fn get_python_executable() -> String {
    let project_root = get_project_root();

    // Method 1: Check for .venv in project
    let venv_python = if cfg!(windows) {
        project_root
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    } else {
        project_root.join(".venv").join("bin").join("python")
    };

    if venv_python.exists() {
        return venv_python.to_string_lossy().to_string();
    }

    // Method 2: Check common Windows Python paths
    #[cfg(windows)]
    {
        let common_paths = [
            r"C:\Users\madma\AppData\Local\Programs\Python\Python313\python.exe",
            r"C:\Users\madma\AppData\Local\Programs\Python\Python312\python.exe",
            r"C:\Users\madma\AppData\Local\Programs\Python\Python311\python.exe",
            r"C:\Python313\python.exe",
            r"C:\Python312\python.exe",
            r"C:\Python311\python.exe",
            r"C:\Python310\python.exe",
            r"C:\Python39\python.exe",
        ];

        for path in common_paths {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }
    }

    // Method 3: Try to find python via where command
    #[cfg(windows)]
    {
        if let Ok(output) = std::process::Command::new("where").arg("python").output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    if let Some(first_line) = path.lines().next() {
                        let trimmed = first_line.trim();
                        if !trimmed.is_empty() && std::path::Path::new(trimmed).exists() {
                            return trimmed.to_string();
                        }
                    }
                }
            }
        }
    }

    // Fallback to PATH
    "python".to_string()
}

/// Run a Python bridge command and return the result
async fn run_bridge_command(script: &str, args: &[&str]) -> Result<BridgeResult, String> {
    let project_root = get_project_root();
    let python_cmd = get_python_executable();

    let script_path = project_root.join("scripts").join(script);

    tracing::debug!(
        "Running bridge: {} {:?} (cwd: {:?})",
        script,
        args,
        project_root
    );

    let mut cmd = tokio::process::Command::new(&python_cmd);
    cmd.arg(&script_path);

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd
        .current_dir(&project_root)
        .output()
        .await
        .map_err(|e| format!("Failed to run {}: {}", script, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        tracing::warn!("Bridge {} stderr: {}", script, stderr);
    }

    if let Ok(result) = serde_json::from_str::<BridgeResult>(&stdout) {
        Ok(result)
    } else {
        Err(format!(
            "Failed to parse result from {}: {}",
            script, stdout
        ))
    }
}

// ==============================================
// ORGANIZER COMMANDS (File Organization)
// ==============================================

/// Preview how files would be organized
#[tauri::command]
pub async fn preview_organization(
    source_dir: String,
    pattern: Option<String>,
) -> Result<BridgeResult, String> {
    let pattern_arg = pattern
        .as_deref()
        .unwrap_or("{artist}/{album}/{track:02d} - {title}");
    run_bridge_command(
        "organizer_bridge.py",
        &["preview", &source_dir, "--pattern", pattern_arg],
    )
    .await
}

/// Organize audio files into folder structure
#[tauri::command]
pub async fn organize_files(
    source_dir: String,
    target_dir: String,
    pattern: Option<String>,
    copy: Option<bool>,
) -> Result<BridgeResult, String> {
    let pattern_arg = pattern
        .as_deref()
        .unwrap_or("{artist}/{album}/{track:02d} - {title}");
    let mut args = vec![
        "organize",
        &source_dir,
        &target_dir,
        "--pattern",
        pattern_arg,
    ];

    if copy == Some(true) {
        args.push("--copy");
    }

    run_bridge_command("organizer_bridge.py", &args).await
}

// ==============================================
// PROGRESS-ENABLED COMMANDS
// ==============================================

/// Emit a progress event to the frontend
fn emit_progress(app_handle: &tauri::AppHandle, event: ProgressEvent) {
    let _ = app_handle.emit("syncify:progress", event);
}

/// Scan local library with progress events
#[tauri::command]
pub async fn scan_local_library_with_progress(
    app_handle: tauri::AppHandle,
    directory: String,
    recursive: Option<bool>,
) -> Result<BridgeResult, String> {
    let scan_id = uuid::Uuid::new_v4().to_string();

    // Emit start event
    emit_progress(&app_handle, ProgressEvent::new("scan", &scan_id));

    // Run the actual scan
    let mut args = vec!["scan", &directory];
    if recursive == Some(false) {
        args.push("--no-recursive");
    }

    let result = run_bridge_command("scanner_bridge.py", &args).await;

    // Emit completion event
    match &result {
        Ok(r) if r.success => {
            let total = r
                .data
                .as_ref()
                .and_then(|d| d.get("total_files"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            emit_progress(
                &app_handle,
                ProgressEvent::new("scan", &scan_id).completed(&format!("Scanned {} files", total)),
            );
        }
        Ok(r) => {
            emit_progress(
                &app_handle,
                ProgressEvent::new("scan", &scan_id)
                    .failed(r.error.as_deref().unwrap_or("Unknown error")),
            );
        }
        Err(e) => {
            emit_progress(&app_handle, ProgressEvent::new("scan", &scan_id).failed(e));
        }
    }

    result
}

/// Batch download tracks with progress events
#[tauri::command]
pub async fn batch_download_tracks(
    app_handle: tauri::AppHandle,
    tracks: Vec<serde_json::Value>,
    service: String,
    quality: Option<String>,
) -> Result<BridgeResult, String> {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let total = tracks.len() as u64;

    // Emit start event
    emit_progress(&app_handle, ProgressEvent::new("download", &batch_id));

    let mut results = Vec::new();
    let mut failed = 0u64;

    for (i, track) in tracks.iter().enumerate() {
        let track_id = track
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let track_title = track
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Track");

        // Emit progress
        emit_progress(
            &app_handle,
            ProgressEvent::new("download", &batch_id).progress(
                i as u64,
                total,
                &format!("Downloading: {}", track_title),
            ),
        );

        // Download the track
        let download_result = run_bridge_command(
            "download_bridge.py",
            &[
                "download",
                &service,
                track_id,
                "--quality",
                quality.as_deref().unwrap_or("high"),
            ],
        )
        .await;

        match download_result {
            Ok(r) if r.success => {
                results.push(serde_json::json!({
                    "track_id": track_id,
                    "success": true
                }));
            }
            _ => {
                failed += 1;
                results.push(serde_json::json!({
                    "track_id": track_id,
                    "success": false
                }));
            }
        }
    }

    // Emit completion
    let message = if failed == 0 {
        format!("Downloaded {} tracks", total)
    } else {
        format!("Downloaded {} tracks, {} failed", total - failed, failed)
    };

    emit_progress(
        &app_handle,
        ProgressEvent::new("download", &batch_id).completed(&message),
    );

    Ok(BridgeResult {
        success: failed == 0,
        data: Some(serde_json::json!({
            "total": total,
            "successful": total - failed,
            "failed": failed,
            "results": results
        })),
        error: if failed > 0 {
            Some(format!("{} downloads failed", failed))
        } else {
            None
        },
    })
}

/// Batch enrich track metadata with progress
#[tauri::command]
pub async fn batch_enrich_metadata(
    app_handle: tauri::AppHandle,
    tracks: Vec<serde_json::Value>,
) -> Result<BridgeResult, String> {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let total = tracks.len() as u64;

    emit_progress(&app_handle, ProgressEvent::new("enrich", &batch_id));

    let mut results = Vec::new();
    let mut enriched = 0u64;

    for (i, track) in tracks.iter().enumerate() {
        let title = track.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let artist = track.get("artist").and_then(|v| v.as_str()).unwrap_or("");
        let isrc = track.get("isrc").and_then(|v| v.as_str());

        emit_progress(
            &app_handle,
            ProgressEvent::new("enrich", &batch_id).progress(
                i as u64,
                total,
                &format!("Enriching: {} - {}", artist, title),
            ),
        );

        let mut args = vec!["enrich", title, artist];
        if let Some(i) = isrc {
            args.push("--isrc");
            args.push(i);
        }

        let result = run_bridge_command("metadata_bridge.py", &args).await;

        if let Ok(r) = result {
            if r.success {
                enriched += 1;
                results.push(serde_json::json!({
                    "title": title,
                    "artist": artist,
                    "enriched": true,
                    "data": r.data
                }));
            } else {
                results.push(serde_json::json!({
                    "title": title,
                    "artist": artist,
                    "enriched": false
                }));
            }
        }
    }

    emit_progress(
        &app_handle,
        ProgressEvent::new("enrich", &batch_id)
            .completed(&format!("Enriched {} of {} tracks", enriched, total)),
    );

    Ok(BridgeResult {
        success: true,
        data: Some(serde_json::json!({
            "total": total,
            "enriched": enriched,
            "results": results
        })),
        error: None,
    })
}

// ==============================================
// PLAYLIST COMMANDS
// ==============================================

/// List playlists from a service
#[tauri::command]
pub async fn list_playlists(service: String) -> Result<BridgeResult, String> {
    run_bridge_command("playlist_bridge.py", &["list", &service]).await
}

/// Get tracks from a playlist
#[tauri::command]
pub async fn get_playlist_tracks(
    service: String,
    playlist_id: String,
) -> Result<BridgeResult, String> {
    run_bridge_command("playlist_bridge.py", &["get", &service, &playlist_id]).await
}

/// Export playlist to JSON or M3U format
#[tauri::command]
pub async fn export_playlist(
    service: String,
    playlist_id: String,
    format: Option<String>,
) -> Result<BridgeResult, String> {
    let format_arg = format.as_deref().unwrap_or("json");
    run_bridge_command(
        "playlist_bridge.py",
        &["export", &service, &playlist_id, "--format", format_arg],
    )
    .await
}

/// Match playlist tracks to another service using ISRC
#[tauri::command]
pub async fn match_playlist_to_service(
    playlist_file: String,
    target_service: String,
) -> Result<BridgeResult, String> {
    run_bridge_command(
        "playlist_bridge.py",
        &["match", &playlist_file, &target_service],
    )
    .await
}

// ==============================================
// DEPENDENCY MANAGEMENT COMMANDS
// ==============================================

/// Check status of all external dependencies (FFmpeg, fpcalc)
#[tauri::command]
pub async fn check_dependencies() -> Result<BridgeResult, String> {
    run_bridge_command("dependency_manager.py", &["check"]).await
}

/// Install a specific dependency (auto-download)
#[tauri::command]
pub async fn install_dependency(tool: String) -> Result<BridgeResult, String> {
    tracing::info!("Installing dependency: {}", tool);
    run_bridge_command("dependency_manager.py", &["install", &tool]).await
}

/// Install all missing dependencies
#[tauri::command]
pub async fn install_all_dependencies() -> Result<BridgeResult, String> {
    tracing::info!("Installing all missing dependencies");
    run_bridge_command("dependency_manager.py", &["install-all"]).await
}

/// Ensure a dependency is available, installing if needed
#[tauri::command]
pub async fn ensure_dependency(tool: String) -> Result<BridgeResult, String> {
    // First check
    let check_result = run_bridge_command("dependency_manager.py", &["check"]).await?;

    if let Some(data) = &check_result.data {
        if let Some(tools) = data.get("tools") {
            if let Some(tool_info) = tools.get(&tool) {
                if tool_info
                    .get("available")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    return Ok(BridgeResult {
                        success: true,
                        data: Some(serde_json::json!({
                            "status": "already_available",
                            "path": tool_info.get("path")
                        })),
                        error: None,
                    });
                }
            }
        }
    }

    // Not available, install it
    tracing::info!("Dependency {} not found, auto-installing...", tool);
    run_bridge_command("dependency_manager.py", &["install", &tool]).await
}
