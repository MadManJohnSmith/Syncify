#[allow(unused_imports)]
use super::*;

// Tools Commands - submodule of crate::commands
//
// Lyrics, metadata, fingerprint, conversion, scanner, organizer, progress, dependencies

// Handlers - remaining commands submodule of crate::commands
//
// Lyrics, metadata, fingerprint, conversion, scanner, accounts, queue, migration commands

// ==============================================
// LYRICS COMMANDS
// ==============================================

// ==============================================
// DOWNLOAD COMMANDS
// ==============================================

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

#[tauri::command]
pub async fn enrich_metadata(
    track: String,
    artist: String,
    isrc: Option<String>,
    album: Option<String>,
) -> Result<MetadataResult, String> {
    tracing::info!("enrich_metadata: {} - {}", artist, track);

    let mut args = vec!["enrich", &track, &artist];

    if let Some(ref i) = isrc {
        args.push("--isrc");
        args.push(i);
    }
    if let Some(ref a) = album {
        args.push("--album");
        args.push(a);
    }

    run_bridge_command::<MetadataResult>("metadata_bridge.py", &args).await
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

#[tauri::command]
pub async fn check_fingerprint_available() -> Result<FingerprintResult, String> {
    tracing::info!("check_fingerprint_available");
    run_bridge_command::<FingerprintResult>("fingerprint_bridge.py", &["check"]).await
}

#[tauri::command]
pub async fn identify_audio(file_path: String) -> Result<FingerprintResult, String> {
    tracing::info!("identify_audio: {}", file_path);
    run_bridge_command::<FingerprintResult>("fingerprint_bridge.py", &["identify", &file_path])
        .await
}

#[tauri::command]
pub async fn find_audio_duplicates(paths: Vec<String>) -> Result<FingerprintResult, String> {
    tracing::info!("find_audio_duplicates: {:?}", paths);

    let mut args = vec!["duplicates"];
    for path in &paths {
        args.push(path);
    }

    run_bridge_command::<FingerprintResult>("fingerprint_bridge.py", &args).await
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

    run_bridge_command::<BridgeResult>("scanner_bridge.py", &args).await
}

/// Get metadata for a single audio file
#[tauri::command]
pub async fn get_local_track_metadata(file_path: String) -> Result<BridgeResult, String> {
    run_bridge_command::<BridgeResult>("scanner_bridge.py", &["metadata", &file_path]).await
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
            if path.join("resources").join("scripts").exists() {
                return path.join("resources");
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
        if cwd.join("resources").join("scripts").exists() {
            return cwd.join("resources");
        }
        if let Some(parent) = cwd.parent() {
            if parent.join("scripts").exists() {
                return parent.to_path_buf();
            }
            if parent.join("resources").join("scripts").exists() {
                return parent.join("resources");
            }
        }
    }

    // Fallback to current directory
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Get the Python executable path
pub fn get_python_executable() -> String {
    let project_root = get_project_root();

    // Method 0: Check for bundled/embedded Python
    let bundled_python = if cfg!(windows) {
        project_root.join("python").join("python.exe")
    } else {
        project_root.join("python").join("bin").join("python")
    };
    if bundled_python.exists() {
        return bundled_python.to_string_lossy().to_string();
    }
    let res_python = if cfg!(windows) {
        project_root
            .join("resources")
            .join("python")
            .join("python.exe")
    } else {
        project_root
            .join("resources")
            .join("python")
            .join("bin")
            .join("python")
    };
    if res_python.exists() {
        return res_python.to_string_lossy().to_string();
    }

    // Layouts empaquetados de Linux: un CPython autónomo con las
    // dependencias instaladas, junto al `scripts/` que find_scripts_dir
    // resuelve (`<_up_>/python` en AppImage/DEB, `<raíz>/python` en el
    // tarball crudo). Es el mirror del `python/python.exe` de Windows.
    for candidate in crate::cmd_utils::packaged_python_candidates(
        &crate::cmd_utils::find_scripts_dir(&project_root),
    ) {
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

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
    let res_venv = if cfg!(windows) {
        project_root
            .join("resources")
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    } else {
        project_root
            .join("resources")
            .join(".venv")
            .join("bin")
            .join("python")
    };
    if res_venv.exists() {
        return res_venv.to_string_lossy().to_string();
    }

    // Method 2: Check common Windows Python paths
    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let local_path = std::path::Path::new(&local_app_data)
                .join("Programs")
                .join("Python");
            for ver in &[
                "Python313",
                "Python312",
                "Python311",
                "Python310",
                "Python39",
            ] {
                let p = local_path.join(ver).join("python.exe");
                if p.exists() {
                    return p.to_string_lossy().to_string();
                }
            }
        }
        for path in &[
            r"C:\Python313\python.exe",
            r"C:\Python312\python.exe",
            r"C:\Python311\python.exe",
            r"C:\Python310\python.exe",
            r"C:\Python39\python.exe",
        ] {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }
    }

    // Method 3: Try to find python via where command (ignoring WindowsApps redirector)
    #[cfg(windows)]
    {
        if let Ok(output) = crate::cmd_utils::create_std_command("where")
            .arg("python")
            .output()
        {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    for line in path.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty()
                            && !trimmed.contains("WindowsApps")
                            && std::path::Path::new(trimmed).exists()
                        {
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
async fn run_bridge_command<T>(script: &str, args: &[&str]) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let project_root = get_project_root();
    let python_cmd = get_python_executable();

    // Los puentes viven en layouts empaquetados que get_project_root no ve
    // (Tauri coloca `../scripts` en `_up_/scripts`); resolutor central.
    let scripts_dir = crate::cmd_utils::find_scripts_dir(&project_root);
    let script_path = scripts_dir.join(script);

    tracing::debug!(
        "Running bridge: {} {:?} (cwd: {:?})",
        script,
        args,
        project_root
    );

    let mut cmd = crate::cmd_utils::create_tokio_command(&python_cmd);
    crate::cmd_utils::apply_bundled_tool_env(&mut cmd);
    cmd.arg(&script_path);

    for arg in args {
        cmd.arg(arg);
    }

    cmd.current_dir(&project_root);
    cmd.kill_on_drop(true);
    let output = tokio::time::timeout(crate::cmd_utils::DEFAULT_BRIDGE_TIMEOUT, cmd.output())
        .await
        .map_err(|_| {
            format!(
                "Bridge {} timed out after {} seconds",
                script,
                crate::cmd_utils::DEFAULT_BRIDGE_TIMEOUT.as_secs()
            )
        })?
        .map_err(|e| format!("Failed to run {}: {}", script, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        tracing::warn!("Bridge {} stderr: {}", script, stderr);
    }

    let trimmed_stdout = stdout.trim();
    let trimmed_stderr = stderr.trim();

    if !output.status.success() && trimmed_stdout.is_empty() {
        let err_detail = if !trimmed_stderr.is_empty() {
            trimmed_stderr.to_string()
        } else {
            format!("Process exited with status {}", output.status)
        };
        return Err(format!("Bridge {} failed: {}", script, err_detail));
    }

    if let Ok(result) = serde_json::from_str::<T>(trimmed_stdout) {
        Ok(result)
    } else if trimmed_stdout.is_empty() && !trimmed_stderr.is_empty() {
        Err(format!(
            "Bridge {} produced no output. Error: {}",
            script, trimmed_stderr
        ))
    } else if !trimmed_stderr.is_empty() {
        Err(format!(
            "Failed to parse result from {}: {} (stderr: {})",
            script, trimmed_stdout, trimmed_stderr
        ))
    } else {
        Err(format!(
            "Failed to parse result from {}: {}",
            script, trimmed_stdout
        ))
    }
}

// ==============================================
// PROGRESS-ENABLED COMMANDS
// ==============================================

/// Emit a progress event to the frontend
///
/// Single canonical progress channel (IN-4): consumers distinguish operations
/// via `event.operation` ("scan" | "organize") and `event.status`.
fn emit_progress(app_handle: &tauri::AppHandle, event: ProgressEvent) {
    let _ = app_handle.emit("syncify:progress", &event);
}

// ==============================================
// WATCH DE BIBLIOTECA LOCAL («Watch for changes (auto-rescan)»)
// ==============================================

/// Umbral del checkbox «Skip files under 1 MB» del diálogo de scan: 1 MB.
pub const SCAN_SKIP_SMALL_FILES_MIN_BYTES: i64 = 1_048_576;

/// Intervalo de sondeo del watcher de biblioteca.
const LIBRARY_WATCH_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

type LibraryWatcherMap = std::collections::HashMap<String, tokio::task::JoinHandle<()>>;

/// Registro global de watchers activos, por directorio canónico.
/// Un nuevo scan del mismo directorio con watch activo reemplaza al anterior.
static LIBRARY_WATCHERS: std::sync::OnceLock<std::sync::Mutex<LibraryWatcherMap>> =
    std::sync::OnceLock::new();

fn library_watchers() -> &'static std::sync::Mutex<LibraryWatcherMap> {
    LIBRARY_WATCHERS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Clave estable para el registro de watchers (ruta canónica; fallback a la dada).
fn watcher_key(directory: &str) -> String {
    std::fs::canonicalize(directory)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| directory.to_string())
}

/// Firma barata del árbol de audio de un directorio: FNV-1a de
/// (ruta, tamaño, mtime) de cada archivo de audio + conteo. Detecta
/// añadidos, borrados, renombres y modificaciones sin leer contenido.
fn audio_tree_signature(dir: &std::path::Path, recursive: bool) -> (u64, u64) {
    // Directorios que nunca forman parte de una biblioteca musical.
    const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target"];

    fn walk(dir: &std::path::Path, recursive: bool, state: &mut (u64, u64)) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str());
                if recursive && !name.is_some_and(|n| SKIP_DIRS.contains(&n)) {
                    walk(&path, recursive, state);
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let is_audio = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| AUDIO_FILE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false);
            if !is_audio {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let rel = path.to_string_lossy();
            let mut h = 0xcbf2_9ce4_8422_2325u64;
            for byte in rel.as_bytes() {
                h ^= *byte as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            h ^= meta.len();
            h = h.wrapping_mul(0x100_0000_01b3);
            h ^= mtime;
            h = h.wrapping_mul(0x100_0000_01b3);
            state.0 ^= h;
            state.1 += 1;
        }
    }

    let mut state = (0u64, 0u64);
    walk(dir, recursive, &mut state);
    state
}

/// Detiene (y devuelve true si existía) el watcher de auto-rescan de un directorio.
pub fn stop_library_watcher(directory: &str) -> bool {
    let key = watcher_key(directory);
    let prev = library_watchers()
        .lock()
        .expect("library watchers mutex poisoned")
        .remove(&key);
    let existed = prev.is_some();
    if let Some(handle) = prev {
        handle.abort();
        tracing::info!("library watcher stopped for {}", directory);
    }
    existed
}

/// Lanza el watcher de auto-rescan: cada LIBRARY_WATCH_POLL_INTERVAL compara la
/// firma del árbol de audio; si cambió desde el último scan, re-ejecuta el scan
/// real (mismos filtros) y emite el progreso por el canal estándar. Un watcher
/// previo del mismo directorio se cancela.
fn spawn_library_watcher(
    app_handle: tauri::AppHandle,
    directory: String,
    recursive: bool,
    min_size_bytes: Option<i64>,
) {
    let key = watcher_key(&directory);

    {
        let mut watchers = library_watchers()
            .lock()
            .expect("library watchers mutex poisoned");
        if let Some(prev) = watchers.remove(&key) {
            prev.abort();
        }
    }

    let handle = tokio::spawn(async move {
        let mut last_sig = tokio::task::spawn_blocking({
            let dir = directory.clone();
            move || audio_tree_signature(std::path::Path::new(&dir), recursive)
        })
        .await
        .unwrap_or((0, 0));

        loop {
            tokio::time::sleep(LIBRARY_WATCH_POLL_INTERVAL).await;

            let sig = tokio::task::spawn_blocking({
                let dir = directory.clone();
                move || audio_tree_signature(std::path::Path::new(&dir), recursive)
            })
            .await;
            let Ok(sig) = sig else {
                break; // watcher cancelado o runtime en apagado
            };
            if sig == last_sig {
                continue;
            }
            last_sig = sig;

            // El árbol cambió: re-scan real con los mismos filtros del scan original.
            let min_size_str = min_size_bytes.map(|v| v.to_string());
            let mut args = vec!["scan", directory.as_str()];
            if !recursive {
                args.push("--no-recursive");
            }
            if let Some(ref m) = min_size_str {
                args.push("--min-size-bytes");
                args.push(m);
            }

            let scan_id = uuid::Uuid::new_v4().to_string();
            emit_progress(&app_handle, ProgressEvent::new("scan", &scan_id));
            let result = run_bridge_command::<BridgeResult>("scanner_bridge.py", &args).await;
            match &result {
                Ok(r) if r.success => {
                    let total = r
                        .data
                        .as_ref()
                        .and_then(|d| d.get("total_files"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    tracing::info!(
                        "library watcher: auto-rescan of {} found {} files",
                        directory,
                        total
                    );
                    emit_progress(
                        &app_handle,
                        ProgressEvent::new("scan", &scan_id)
                            .completed(&format!("Auto-rescan: {} files", total)),
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
        }
    });

    library_watchers()
        .lock()
        .expect("library watchers mutex poisoned")
        .insert(key, handle);
}

/// FE-6: detiene el watcher de auto-rescan de una ruta de biblioteca
/// (se invoca al quitar la ruta desde la UI).
#[tauri::command]
pub fn stop_library_watcher_command(directory: String) -> Result<bool, String> {
    Ok(stop_library_watcher(&directory))
}

/// Scan local library with progress events
///
/// Opciones del diálogo «Add Local Library Path» (AccountsView):
/// - `watch_for_changes`: tras un scan exitoso lanza un watcher de sondeo que
///   re-escanea el directorio cuando cambia el árbol de audio (auto-rescan).
///   Un scan posterior con watch desactivado detiene el watcher.
/// - `skip_small_files`: pasa `--min-size-bytes` al scanner bridge para que
///   ignore archivos por debajo de 1 MB (SCAN_SKIP_SMALL_FILES_MIN_BYTES).
#[tauri::command]
pub async fn scan_local_library_with_progress(
    app_handle: tauri::AppHandle,
    directory: String,
    recursive: Option<bool>,
    watch_for_changes: Option<bool>,
    skip_small_files: Option<bool>,
) -> Result<BridgeResult, String> {
    let scan_id = uuid::Uuid::new_v4().to_string();

    // Emit start event
    emit_progress(&app_handle, ProgressEvent::new("scan", &scan_id));

    // Run the actual scan
    let mut args = vec!["scan", &directory];
    if recursive == Some(false) {
        args.push("--no-recursive");
    }

    // FE-6: «Skip files under 1 MB» — el filtro real vive en scanner_bridge.py.
    let min_bytes = if skip_small_files == Some(true) {
        Some(SCAN_SKIP_SMALL_FILES_MIN_BYTES.to_string())
    } else {
        None
    };
    if let Some(ref mb) = min_bytes {
        args.push("--min-size-bytes");
        args.push(mb.as_str());
    }

    let result = run_bridge_command::<BridgeResult>("scanner_bridge.py", &args).await;

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

    // Watcher de auto-rescan (solo tras un scan exitoso).
    if watch_for_changes == Some(true) {
        if let Ok(r) = &result {
            if r.success {
                spawn_library_watcher(
                    app_handle,
                    directory,
                    recursive != Some(false),
                    if skip_small_files == Some(true) {
                        Some(SCAN_SKIP_SMALL_FILES_MIN_BYTES)
                    } else {
                        None
                    },
                );
            }
        }
    } else {
        // Scan con watch desactivado: no dejar watchers huérfanos.
        stop_library_watcher(&directory);
    }

    result
}

/// Batch enrich track metadata with progress
#[tauri::command]
pub async fn batch_enrich_metadata(
    app_handle: tauri::AppHandle,
    tracks: Vec<serde_json::Value>,
) -> Result<BridgeResult, String> {
    use tauri::Manager;
    let state = app_handle.try_state::<crate::AppState>();
    let batch_id = uuid::Uuid::new_v4().to_string();
    let total = tracks.len() as u64;

    emit_progress(&app_handle, ProgressEvent::new("enrich", &batch_id));

    let mut results = Vec::new();
    let mut enriched = 0u64;

    for (i, track) in tracks.iter().enumerate() {
        let (title, artist, isrc): (String, String, Option<String>) = if let Some(id) =
            track.as_i64().or_else(|| track.as_u64().map(|v| v as i64))
        {
            if let Some(ref state) = state {
                match sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
                        r#"SELECT t.title, t.isrc,
                                  COALESCE((
                                      SELECT a.name
                                      FROM track_artists ta
                                      JOIN artists a ON ta.artist_id = a.id
                                      WHERE ta.track_id = t.id
                                      ORDER BY CASE WHEN ta.role = 'primary' THEN 0 ELSE 1 END, ta.artist_id
                                      LIMIT 1
                                  ), '') as artist_name
                           FROM tracks t
                           WHERE t.id = $1"#,
                    )
                    .bind(id)
                    .fetch_optional(&state.db)
                    .await
                    {
                        Ok(Some((t, i, a))) => (t.unwrap_or_default(), a.unwrap_or_default(), i),
                        Ok(None) => (String::new(), String::new(), None),
                        Err(e) => {
                            tracing::error!("Failed to fetch track {} for metadata enrichment: {}", id, e);
                            (String::new(), String::new(), None)
                        }
                    }
            } else {
                (String::new(), String::new(), None)
            }
        } else {
            let title = track
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let artist = track
                .get("artist")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let isrc = track
                .get("isrc")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (title, artist, isrc)
        };

        if title.trim().is_empty() && artist.trim().is_empty() {
            results.push(serde_json::json!({
                "title": title,
                "artist": artist,
                "enriched": false
            }));
            continue;
        }

        emit_progress(
            &app_handle,
            ProgressEvent::new("enrich", &batch_id).progress(
                i as u64,
                total,
                &format!("Enriching: {} - {}", artist, title),
            ),
        );

        let mut args: Vec<&str> = vec!["enrich", &title, &artist];
        if let Some(ref i) = isrc {
            if !i.trim().is_empty() {
                args.push("--isrc");
                args.push(i.as_str());
            }
        }

        let result = run_bridge_command::<BridgeResult>("metadata_bridge.py", &args).await;

        match result {
            Ok(r) if r.success => {
                enriched += 1;
                results.push(serde_json::json!({
                    "title": title,
                    "artist": artist,
                    "enriched": true,
                    "data": r.data
                }));
            }
            _ => {
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

    let failed = total.saturating_sub(enriched);

    Ok(BridgeResult {
        success: true,
        data: Some(serde_json::json!({
            "batch_id": batch_id,
            "total": total,
            "enriched": enriched,
            "failed": failed,
            "skipped": 0,
            "results": results
        })),
        error: None,
    })
}

// ==============================================
// PLAYLIST COMMANDS
// ==============================================

/// Get tracks from a remote playlist via bridge
#[tauri::command]
pub async fn fetch_remote_playlist_tracks(
    service: String,
    playlist_id: String,
) -> Result<BridgeResult, String> {
    run_bridge_command::<BridgeResult>("playlist_bridge.py", &["get", &service, &playlist_id]).await
}

/// Export playlist to JSON or M3U format
#[tauri::command]
pub async fn export_playlist(
    service: String,
    playlist_id: String,
    format: Option<String>,
) -> Result<BridgeResult, String> {
    let format_arg = format.as_deref().unwrap_or("json");
    run_bridge_command::<BridgeResult>(
        "playlist_bridge.py",
        &["export", &service, &playlist_id, "--format", format_arg],
    )
    .await
}

/// Classify the entries of a playlist file by ISRC availability for a transfer to
/// `target_service` (`playlist_bridge.py match`).
/// The target service catalog is NOT queried: the bridge only reports which entries
/// carry an ISRC. Real ISRC matching against the local library lives in
/// `commands::playlists::match_entry_to_track`.
#[tauri::command]
pub async fn match_playlist_to_service(
    playlist_file: String,
    target_service: String,
) -> Result<BridgeResult, String> {
    run_bridge_command::<BridgeResult>(
        "playlist_bridge.py",
        &["match", &playlist_file, &target_service],
    )
    .await
}

// ==============================================
// DEPENDENCY MANAGEMENT COMMANDS
// ==============================================

pub const ALLOWED_TOOLS: &[&str] = &["ffmpeg", "fpcalc"];

/// Validates that a tool name is in the allowed whitelist and returns the normalized name.
pub fn validate_tool(tool: &str) -> Result<String, String> {
    let normalized_tool = tool.trim().to_lowercase();
    if !ALLOWED_TOOLS.contains(&normalized_tool.as_str()) {
        return Err(format!(
            "Herramienta no autorizada: '{}'. Herramientas permitidas: {:?}",
            tool, ALLOWED_TOOLS
        ));
    }
    Ok(normalized_tool)
}

/// Check status of all external dependencies (FFmpeg, fpcalc)
#[tauri::command]
pub async fn check_dependencies() -> Result<BridgeResult, String> {
    run_bridge_command::<BridgeResult>("dependency_manager.py", &["check"]).await
}

/// Install a specific dependency (auto-download)
#[tauri::command]
pub async fn install_dependency(tool: String) -> Result<BridgeResult, String> {
    let normalized_tool = validate_tool(&tool)?;
    tracing::info!("Installing dependency: {}", normalized_tool);
    run_bridge_command::<BridgeResult>("dependency_manager.py", &["install", &normalized_tool])
        .await
}

/// Install all missing dependencies
#[tauri::command]
pub async fn install_all_dependencies() -> Result<BridgeResult, String> {
    tracing::info!("Installing all missing dependencies");
    run_bridge_command::<BridgeResult>("dependency_manager.py", &["install-all"]).await
}

/// Ensure a dependency is available, installing if needed
#[tauri::command]
pub async fn ensure_dependency(tool: String) -> Result<BridgeResult, String> {
    let normalized_tool = validate_tool(&tool)?;

    // First check
    let check_result =
        run_bridge_command::<BridgeResult>("dependency_manager.py", &["check"]).await?;

    if let Some(data) = &check_result.data {
        if let Some(tools) = data.get("tools") {
            if let Some(tool_info) = tools.get(&normalized_tool) {
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
    tracing::info!(
        "Dependency {} not found, auto-installing...",
        normalized_tool
    );
    run_bridge_command::<BridgeResult>("dependency_manager.py", &["install", &normalized_tool])
        .await
}

// ==============================================
// FILE EXPORT COMMAND
// ==============================================

/// Write UTF-8 text content to an arbitrary path chosen by the user.
///
/// The webview has no filesystem scope by design, so frontend export flows
/// (lyrics `.lrc`/`.txt`/`.ttml`, metadata JSON) resolve the destination
/// through the dialog plugin and persist the payload through this command.
///
/// Allowed file extensions for export operations (safe text/metadata formats)
pub const ALLOWED_WRITE_EXTENSIONS: &[&str] =
    &["txt", "json", "csv", "m3u", "m3u8", "log", "lrc", "ttml"];

/// Returns the set of allowed base directories for export persistence.
/// Strictly confined to the user's Downloads, Documents, and app data directory.
pub fn get_allowed_write_directories() -> Vec<std::path::PathBuf> {
    let mut bases = Vec::new();

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

/// Validates that a target path conforms to sandbox confinement, path traversal
/// restrictions, and file extension whitelisting.
pub fn validate_safe_write_path_with_bases(
    target_path: &std::path::Path,
    allowed_bases: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    // 1. Must be an absolute path
    if !target_path.is_absolute() {
        return Err("Acceso denegado: la ruta debe ser absoluta (sandbox violation)".to_string());
    }

    // 2. Reject path traversal sequences (.. or ParentDir)
    for component in target_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(
                "Acceso denegado: secuencias de escape ('..') detectadas (sandbox violation)"
                    .to_string(),
            );
        }
    }

    // 3. Filename & extension validation
    let file_name = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| {
            "Acceso denegado: nombre de archivo no válido (sandbox violation)".to_string()
        })?;

    if file_name.starts_with('.') {
        return Err("Acceso denegado: no se permite escribir archivos ocultos o de configuración (sandbox violation)".to_string());
    }

    let ext = target_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let ext_str = match &ext {
        Some(e) => e.as_str(),
        None => {
            return Err(
                "Acceso denegado: el archivo debe tener una extensión válida permitida (sandbox violation)".to_string(),
            )
        }
    };

    if !ALLOWED_WRITE_EXTENSIONS.contains(&ext_str) {
        return Err(format!(
            "Acceso denegado: extensión '.{}' no permitida. Extensiones permitidas: {} (sandbox violation)",
            ext_str,
            ALLOWED_WRITE_EXTENSIONS.join(", ")
        ));
    }

    if allowed_bases.is_empty() {
        return Err(
            "Acceso denegado: no se definieron directorios base permitidos (sandbox violation)"
                .to_string(),
        );
    }

    // 4. Lexical containment check against allowed bases
    let matches_lexical = allowed_bases
        .iter()
        .any(|base| target_path.starts_with(base));
    if !matches_lexical {
        return Err(
            "Acceso denegado: la ruta está fuera de los directorios permitidos (sandbox violation)"
                .to_string(),
        );
    }

    // 5. Parent directory resolution and creation
    let parent = target_path.parent().ok_or_else(|| {
        "Acceso denegado: ruta sin directorio padre válido (sandbox violation)".to_string()
    })?;

    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear el directorio {}: {}", parent.display(), e))?;
    }

    // 6. Canonicalize parent directory and verify containment
    let canonical_parent = std::fs::canonicalize(parent).map_err(|e| {
        format!(
            "Error al canonicalizar directorio {}: {}",
            parent.display(),
            e
        )
    })?;

    let mut canonical_allowed_bases = Vec::new();
    for b in allowed_bases {
        if let Ok(c) = std::fs::canonicalize(b) {
            canonical_allowed_bases.push(c);
        }
        canonical_allowed_bases.push(b.clone());
    }

    if !canonical_allowed_bases
        .iter()
        .any(|base| canonical_parent.starts_with(base))
    {
        return Err("Acceso denegado: el directorio destino canonicalizado está fuera del sandbox permitido (sandbox violation)".to_string());
    }

    let safe_target = canonical_parent.join(file_name);

    // 7. Prevent symlink overwriting or escaping via existing symlinks
    if safe_target.is_symlink()
        || std::fs::symlink_metadata(&safe_target)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(
            "Acceso denegado: no se permite sobreescribir enlaces simbólicos (sandbox violation)"
                .to_string(),
        );
    }

    if safe_target.exists() {
        let canonical_target = std::fs::canonicalize(&safe_target)
            .map_err(|e| format!("Error al canonicalizar archivo existente: {}", e))?;
        if !canonical_allowed_bases
            .iter()
            .any(|base| canonical_target.starts_with(base))
        {
            return Err("Acceso denegado: el archivo destino existente resuelve fuera del sandbox permitido (sandbox violation)".to_string());
        }
    }

    Ok(safe_target)
}

/// Helper to validate a target path against the system's allowed base directories.
pub fn validate_safe_write_path(
    target_path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let allowed_bases = get_allowed_write_directories();
    validate_safe_write_path_with_bases(target_path, &allowed_bases)
}

/// Write UTF-8 text content to a destination path chosen by the user.
///
/// The webview has no filesystem scope by design, so frontend export flows
/// (lyrics `.lrc`/`.txt`/`.ttml`, metadata JSON) resolve the destination
/// through the dialog plugin and persist the payload through this command.
///
/// Security mitigations (TASK-87 / SEC-003):
/// - Confinement: Only allowed user directories (Downloads, Documents, app data).
/// - Path traversal: Rejects relative paths and any '..' components.
/// - Whitelisting: Only permitted text/metadata extensions (.txt, .json, .csv, .m3u, .m3u8, .log, .lrc, .ttml).
/// - Symlinks: Overwriting symlinks or symlink directory traversal is blocked.
///
/// Returns the byte count written on success.
#[tauri::command]
pub async fn write_text_file(path: String, contents: String) -> Result<u64, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("La ruta de destino está vacía".to_string());
    }
    if contents.is_empty() {
        return Err("Contenido vacío: nada que exportar".to_string());
    }

    let target_path = std::path::Path::new(trimmed);
    let safe_target = validate_safe_write_path(target_path)?;

    let bytes = contents.len() as u64;
    tokio::fs::write(&safe_target, contents)
        .await
        .map_err(|e| format!("No se pudo escribir {}: {}", safe_target.display(), e))?;

    tracing::info!(
        "write_text_file: {} bytes written to {}",
        bytes,
        safe_target.display()
    );
    Ok(bytes)
}

#[cfg(test)]
mod library_watch_tests {
    use super::*;

    #[test]
    fn test_audio_tree_signature_detects_changes() {
        let dir = std::env::temp_dir().join(format!(
            "syncify_watch_sig_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Directorio vacío.
        let empty = audio_tree_signature(&dir, true);
        assert_eq!(empty, (0, 0));

        // Añadir un archivo de audio cambia la firma y el conteo.
        std::fs::write(dir.join("song.mp3"), b"data").unwrap();
        let with_one = audio_tree_signature(&dir, true);
        assert_ne!(with_one, empty);
        assert_eq!(with_one.1, 1);

        // Los archivos que no son de audio no cuentan.
        std::fs::write(dir.join("notes.txt"), b"data").unwrap();
        assert_eq!(audio_tree_signature(&dir, true), with_one);

        // Renombrar un archivo de audio cambia la firma (mismo conteo).
        std::fs::rename(dir.join("song.mp3"), dir.join("renamed.flac")).unwrap();
        let renamed = audio_tree_signature(&dir, true);
        assert_ne!(renamed, with_one);
        assert_eq!(renamed.1, 1);

        // En modo no recursivo, un subdirectorio no afecta la firma.
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("deep.ogg"), b"data").unwrap();
        assert_eq!(audio_tree_signature(&dir, false), renamed);
        assert_ne!(audio_tree_signature(&dir, true), renamed);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_stop_library_watcher_reports_presence() {
        // Sin watcher lanzado, stop devuelve false y no hace nada.
        assert!(!stop_library_watcher("/definitely/not/watched/path"));
    }
}
