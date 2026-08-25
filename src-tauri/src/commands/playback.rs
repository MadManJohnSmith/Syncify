// S194 residual — local playback of downloaded tracks.
//
// The webview has no filesystem read scope by design, so audio playback of
// downloaded files goes through a purpose-built `syncify-media://` protocol
// registered in main.rs. Security model:
//   * A file becomes playable ONLY when `resolve_playback_source` verifies
//     it belongs to a row of the `downloads` table, exists on disk, and is
//     explicitly added to the in-memory grant set.
//   * The protocol handler serves byte ranges exclusively for granted paths
//     (canonicalized comparison); everything else gets 403/404.
//   * No directory grants, no static scope, no new dependencies.


use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use tauri::http::{Request, Response};

/// Cap on how many distinct files may stay granted during one app session.
const MAX_GRANTS: usize = 20_000;

fn grants() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// What the frontend needs to start playing one local file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaybackSource {
    pub track_id: i64,
    pub file_path: String,
    pub format: Option<String>,
}

#[tauri::command]
pub async fn resolve_playback_source(
    state: State<'_, crate::AppState>,
    track_id: i64,
) -> Result<PlaybackSource, String> {
    tracing::info!("resolve_playback_source: track_id={}", track_id);

    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT file_path, file_format FROM downloads WHERE track_id = ? LIMIT 1",
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let Some((file_path, format)) = row else {
        return Err(format!(
            "El track {} no tiene archivo local descargado; descárgalo primero",
            track_id
        ));
    };

    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(format!("El archivo ya no existe en disco: {}", file_path));
    }

    // Canonicalize once here; the handler compares canonicalized too.
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| format!("No se pudo resolver la ruta del archivo: {}", e))?;
    {
        let mut set = grants()
            .lock()
            .map_err(|_| "Estado interno de reproducción bloqueado".to_string())?;
        if set.len() >= MAX_GRANTS {
            set.clear(); // bounded memory: oldest grants are simply dropped
        }
        set.insert(canonical.to_string_lossy().to_string());
    }

    Ok(PlaybackSource {
        track_id,
        file_path,
        format,
    })
}

// ---------------------------------------------------------------------------
// Protocol handler (wired in main.rs as "syncify-media")
// ---------------------------------------------------------------------------

fn content_type_for(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".flac") {
        "audio/flac"
    } else if lower.ends_with(".mp3") {
        "audio/mpeg"
    } else if lower.ends_with(".m4a") || lower.ends_with(".mp4") {
        "audio/mp4"
    } else if lower.ends_with(".wav") {
        "audio/wav"
    } else if lower.ends_with(".ogg") || lower.ends_with(".opus") {
        "audio/ogg"
    } else {
        "application/octet-stream"
    }
}

fn simple_response(status: u16, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(message.as_bytes().to_vec())
        .unwrap_or_else(|_| Response::builder().status(500).body(Vec::new()).unwrap())
}

/// Decode the `syncify-media://localhost/<encoded-path>` URI into the raw
/// file path. convertFileSrc percent-encodes the whole absolute path, and
/// Windows additionally carries it behind a single `/` (drive-letter form).
fn extract_file_path(uri: &str) -> Option<String> {
    // "syncify-media://localhost/<encoded-abs-path>" or its Windows
    // "http://syncify-media.localhost/<encoded-abs-path>" shape.
    let after_scheme = uri.split("://").nth(1)?;
    let encoded = after_scheme.split_once('/')?.1;
    let decoded = urlencoding::decode(encoded).ok()?.to_string();
    let decoded = decoded.trim_start_matches('/').to_string();
    if decoded.is_empty() {
        None
    } else {
        Some(decoded)
    }
}

/// Serve a byte range of a granted audio file. Public for main.rs wiring.
pub fn handle_media_protocol_request(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(raw_path) = extract_file_path(&request.uri().to_string()) else {
        return simple_response(400, "syncify-media: ruta inválida");
    };

    let path = std::path::PathBuf::from(&raw_path);
    let Ok(canonical) = std::fs::canonicalize(&path) else {
        return simple_response(404, "Archivo no encontrado");
    };

    let granted = grants()
        .lock()
        .map(|set| set.contains(&canonical.to_string_lossy().to_string()))
        .unwrap_or(false);
    if !granted {
        tracing::warn!(
            "[syncify-media] denied (not granted): {}",
            canonical.display()
        );
        return simple_response(403, "Archivo no autorizado para reproducción");
    }

    let Ok(meta) = std::fs::metadata(&canonical) else {
        return simple_response(404, "Archivo no encontrado");
    };
    let total_len = meta.len();

    // Range handling for HTML5 audio seeking: "bytes=start[-end]".
    let range_header = request
        .headers()
        .get("range")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let (start, end) = match range_header.as_deref().and_then(parse_bytes_range) {
        Some((s, e)) => {
            if s >= total_len {
                return Response::builder()
                    .status(416)
                    .header("Content-Range", format!("bytes */{}", total_len))
                    .body(Vec::new())
                    .unwrap();
            }
            let end = e.unwrap_or(total_len - 1).min(total_len - 1);
            (s, end)
        }
        None => (0u64, total_len.saturating_sub(1)),
    };

    const MAX_CHUNK: u64 = 8 * 1024 * 1024; // bounds allocation per request
    let end = end.min(start.saturating_add(MAX_CHUNK - 1));

    use std::io::{Read, Seek, SeekFrom};
    let mut file = match std::fs::File::open(&canonical) {
        Ok(f) => f,
        Err(e) => return simple_response(500, &format!("No se pudo abrir el archivo: {}", e)),
    };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return simple_response(500, "Seek falló");
    }
    let chunk_len = (end - start + 1) as usize;
    let mut body = vec![0u8; chunk_len];
    if file.read_exact(&mut body).is_err() {
        return simple_response(500, "Lectura truncada");
    }

    let partial = !(start == 0 && end == total_len - 1);
    let mut builder = Response::builder()
        .status(if partial { 206 } else { 200 })
        .header("Content-Type", content_type_for(&raw_path))
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "no-store");
    if partial {
        builder = builder.header(
            "Content-Range",
            format!("bytes {}-{}/{}", start, end, total_len),
        );
    }
    builder
        .body(body)
        .unwrap_or_else(|_| simple_response(500, "Respuesta mal formada"))
}

/// Parse "bytes=start-end" / "bytes=start-" per RFC 7233 (single range).
fn parse_bytes_range(header: &str) -> Option<(u64, Option<u64>)> {
    let spec = header.strip_prefix("bytes=")?.trim();
    let (start_s, end_s) = spec.split_once('-')?;
    let start: u64 = start_s.trim().parse().ok()?;
    let end = end_s.trim().parse::<u64>().ok();
    Some((start, end))
}
