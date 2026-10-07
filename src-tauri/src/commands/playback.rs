#[allow(unused_imports)]
use super::*;

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
// Provider streaming stays out of scope for this sprint (S194).

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

    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT file_path, file_format FROM downloads WHERE track_id = ? LIMIT 1")
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
// Fase 1 del player: estadísticas de reproducción local (play_count).
// ---------------------------------------------------------------------------

/// Resultado de una invocación de `record_local_play`.
///
/// `counted` es `true` SOLO cuando esta invocación insertó la sesión de
/// escucha y por tanto incrementó `play_count`; un reintento con el mismo
/// `listen_session_id` (respuesta IPC perdida) devuelve `counted: false` con
/// las estadísticas intactas — nunca un segundo incremento.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayStats {
    pub track_id: i64,
    pub play_count: i64,
    pub last_played: Option<String>,
    pub counted: bool,
}

/// Normaliza el identificador de sesión de escucha a su forma canónica
/// (minúsculas con guiones) ANTES de tocar la PK de `playback_listens`:
/// `"ABC…"` y `"abc…"` son el mismo token y no deben poder contar dos veces.
/// Acepta las formas que parsea `uuid` (hifenada, simple, braced, urn).
fn normalize_listen_session_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("listen_session_id vacío: se esperaba un UUID".to_string());
    }
    uuid::Uuid::parse_str(trimmed)
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| format!("listen_session_id inválido (se esperaba UUID): {raw:?}"))
}

/// Núcleo testeable de `record_local_play` (patrón `perform_*` del resto de
/// comandos): recibe el pool directamente y no `State`.
///
/// Semántica transaccional — INSERT-primero, nunca SELECT-then-INSERT:
///   1. `BEGIN`.
///   2. `INSERT … ON CONFLICT(listen_session_id) DO NOTHING` — la PK decide
///      el doble conteo en una sola sentencia atómica.
///   3. `rows_affected == 1` → `UPDATE tracks …`; exigir `rows_affected == 1`
///      o ROLLBACK + Err (cinturón además de la FK, por si una regresión
///      futura desactiva `PRAGMA foreign_keys`).
///   4. `rows_affected == 0` → misma pista: devolver estadísticas con
///      `counted: false` SIN UPDATE ni cambio de `last_played`; pista
///      distinta → ROLLBACK + Err (el token pertenece a otra escucha).
///
/// Límite de confianza documentado (doc fase 1 / MPRIS): el backend garantiza
/// atomicidad e idempotencia, no puede certificar por sí solo los 30 s
/// audibles de la fase 1 — eso lo acredita la UI antes de invocar.
pub async fn perform_record_local_play(
    db: &DbPool,
    track_id: i64,
    listen_session_id: &str,
) -> Result<PlayStats, String> {
    if track_id <= 0 {
        return Err(format!("track_id inválido: {track_id}"));
    }
    let session_id = normalize_listen_session_id(listen_session_id)?;

    // (1) BEGIN. Primera sentencia es el INSERT (escritura): una transacción
    // DEFERRED no sufre SQLITE_BUSY_SNAPSHOT aquí — el lock de escritura se
    // toma en el INSERT y busy_timeout (30 s, db.rs) serializa escritores.
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    // (2) INSERT-primero: el conflicto de PK decide sin SELECT previo, también
    // bajo llamadas concurrentes con el mismo token. Con FK activa (db.rs
    // activa `PRAGMA foreign_keys = ON` en CADA conexión del pool), una pista
    // inexistente/borrada muere aquí: error definitivo, no transitorio.
    let inserted = sqlx::query(
        "INSERT INTO playback_listens (listen_session_id, track_id) VALUES (?, ?) \
         ON CONFLICT(listen_session_id) DO NOTHING",
    )
    .bind(&session_id)
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();

    if inserted == 1 {
        // (3) Primera vez que esta sesión cuenta.
        let updated = sqlx::query(
            "UPDATE tracks \
             SET play_count = play_count + 1, last_played = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(track_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if updated != 1 {
            // Cinturón además de la FK: si una regresión futura desactiva
            // FK, la sesión NUNCA se persiste sin su incremento asociado.
            tx.rollback()
                .await
                .map_err(|e| format!("rollback tras UPDATE sin fila: {e}"))?;
            return Err(format!(
                "El track {track_id} no existe; no se cuenta la escucha"
            ));
        }
        let (play_count, last_played): (i64, Option<String>) =
            sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
                .bind(track_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(
            "record_local_play: track_id={} contado (play_count={play_count})",
            track_id
        );
        return Ok(PlayStats {
            track_id,
            play_count,
            last_played,
            counted: true,
        });
    }

    // (4) Conflicto de PK: la sesión ya se registró antes.
    let existing_track_id: Option<i64> =
        sqlx::query_scalar("SELECT track_id FROM playback_listens WHERE listen_session_id = ?")
            .bind(&session_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    match existing_track_id {
        Some(existing) if existing == track_id => {
            // Reintento idempotente: SIN UPDATE, last_played intacto.
            let (play_count, last_played): (i64, Option<String>) =
                sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
                    .bind(track_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            tx.commit().await.map_err(|e| e.to_string())?;
            tracing::debug!(
                "record_local_play: track_id={} ya contado con esta sesión (dedupe)",
                track_id
            );
            Ok(PlayStats {
                track_id,
                play_count,
                last_played,
                counted: false,
            })
        }
        Some(existing) => {
            tx.rollback()
                .await
                .map_err(|e| format!("rollback tras conflicto con otra pista: {e}"))?;
            Err(format!(
                "El token de escucha ya pertenece al track {existing}, no al {track_id}"
            ))
        }
        None => {
            // Inalcanzable salvo carrera exótica (ON CONFLICT sin fila
            // visible): fallar de forma explícita sin persistir nada.
            tx.rollback()
                .await
                .map_err(|e| format!("rollback tras conflicto sin fila: {e}"))?;
            Err(format!(
                "Conflicto de sesión de escucha {session_id} sin fila registrada"
            ))
        }
    }
}

/// Cuenta una escucha de una pista local. La UI genera un UUID v4 canónico
/// por reproducción lógica y lo invoca UNA sola vez por sesión al cruzar el
/// umbral de 30 s de reproducción real acreditada; el backend deduplica por
/// PK ante reintentos (respuesta IPC perdida) y rechaza el token reutilizado
/// con otra pista.
#[tauri::command]
pub async fn record_local_play(
    state: State<'_, crate::AppState>,
    track_id: i64,
    listen_session_id: String,
) -> Result<PlayStats, String> {
    tracing::info!("record_local_play: track_id={track_id}");
    perform_record_local_play(&state.db, track_id, &listen_session_id).await
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
/// file path. `convertFileSrc` percent-encodes the whole absolute path:
///   * Linux/macOS: `syncify-media://localhost/%2Fhome%2Falan%2F…` — after the
///     authority split and decode this is ALREADY an absolute Unix path, so the
///     leading `/` must be KEPT (S200: the old blanket-trim broke every Unix
///     path → 404 en cada request de audio).
///   * Windows: `http://syncify-media.localhost/%2FC%3A%5CUsers%2F…` — decode
///     yields `/C:\Users\…`, exactly ONE leading slash to strip for the drive.
fn extract_file_path(uri: &str) -> Option<String> {
    // "syncify-media://localhost/<encoded-abs-path>" or its Windows
    // "http://syncify-media.localhost/<encoded-abs-path>" shape.
    let after_scheme = uri.split("://").nth(1)?;
    let encoded = after_scheme.split_once('/')?.1;
    let decoded = urlencoding::decode(encoded).ok()?.to_string();
    if decoded.is_empty() {
        return None;
    }
    if let Some(rest) = decoded.strip_prefix('/') {
        // Drive-letter form "/C:\…" or "/C:/…" → strip exactly that one slash.
        let b = rest.as_bytes();
        let looks_like_drive = rest.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
        if looks_like_drive {
            return Some(rest.to_string());
        }
        // Absolute Unix path: KEEP the leading `/`. Collapse any accidental
        // extra slashes left by URL normalization down to a single root.
        let mut path = decoded;
        while path.starts_with("//") {
            path.remove(0);
        }
        return Some(path);
    }
    Some(decoded)
}

/// S200 — asynchronous wrapper for main.rs registration.
///
/// `register_uri_scheme_protocol` handlers run on the MAIN thread in webkit
/// (Linux) and WebView2 (Windows): the previous synchronous implementation did
/// canonicalize + open + up-to-8MB reads per request there, freezing the whole
/// UI while audio streamed. The async variant hands the heavy work to a plain
/// OS thread and answers via `UriSchemeResponder`, keeping the main thread free.
pub fn handle_media_protocol_request_async(
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    std::thread::spawn(move || {
        let response = handle_media_protocol_request(request);
        responder.respond(response);
    });
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
    // parse_bytes_range garantiza end >= start; si una regresión futura lo
    // rompiera, responder 416 en vez de desbordar (asignación gigante/abort).
    let Some(chunk_len) = chunk_len_for(start, end) else {
        return simple_response(416, "syncify-media: rango de bytes inválido");
    };
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
///
/// RFC 7233 §2.1: a byte-range-spec whose last-byte-pos is less than its
/// first-byte-pos is INVALID and must be ignored — so is the whole header
/// here (`None` → the handler serves 200 full-body). Rejecting it at parse
/// time also keeps `end >= start` invariant downstream: "bytes=100-50" used
/// to reach the handler and wrap `end - start + 1` to ~1.8e19, making
/// `vec![0u8; chunk_len]` attempt an ~18 EiB allocation (abort in release).
fn parse_bytes_range(header: &str) -> Option<(u64, Option<u64>)> {
    let spec = header.strip_prefix("bytes=")?.trim();
    let (start_s, end_s) = spec.split_once('-')?;
    let start: u64 = start_s.trim().parse().ok()?;
    let end = end_s.trim().parse::<u64>().ok();
    if let Some(e) = end {
        if e < start {
            return None;
        }
    }
    Some((start, end))
}

/// Chunk length with checked arithmetic: any future regression that lets
/// `end < start` (or a length beyond usize) yields `None` instead of a u64
/// wrap-around feeding a huge allocation. The handler answers 416 on `None`.
fn chunk_len_for(start: u64, end: u64) -> Option<usize> {
    end.checked_sub(start)
        .and_then(|len| len.checked_add(1))
        .and_then(|len| usize::try_from(len).ok())
}

#[cfg(test)]
mod tests {
    use super::{chunk_len_for, extract_file_path, parse_bytes_range};

    // S200: convertFileSrc on Linux/macOS keeps the WHOLE absolute path
    // percent-encoded after `localhost/`. The old blanket-trim destroyed the
    // leading `/`, so every request 404'd and audio never started.
    #[test]
    fn s200_unix_absolute_path_is_preserved() {
        let uri = "syncify-media://localhost/%2Fhome%2Fuser%2FM%C3%BAsica%2Fsong.flac";
        assert_eq!(
            extract_file_path(uri).unwrap(),
            "/home/user/Música/song.flac"
        );
    }

    #[test]
    fn s200_literal_slash_form_still_resolves() {
        // Some webkit versions normalize %2F to literal slashes before we see it.
        let uri = "syncify-media://localhost//srv/music/a.flac";
        assert_eq!(extract_file_path(uri).unwrap(), "/srv/music/a.flac");
    }

    #[test]
    fn s200_windows_drive_form_strips_single_leading_slash() {
        let uri = "http://syncify-media.localhost/%2FC%3A%5CUsers%5Ctardis%5CMusic%5Cx.mp3";
        assert_eq!(
            extract_file_path(uri).unwrap(),
            "C:\\Users\\tardis\\Music\\x.mp3"
        );
        // Forward slashes are accepted by Windows file APIs too.
        let posix_drive = "http://syncify-media.localhost/%2FD%3A/music/y.flac";
        assert_eq!(extract_file_path(posix_drive).unwrap(), "D:/music/y.flac");
    }

    #[test]
    fn s200_garbage_uris_are_rejected() {
        assert!(extract_file_path("syncify-media://localhost/").is_none());
        assert!(extract_file_path("not-a-uri").is_none());
    }

    // Regresión: "bytes=100-50" pasaba el parseo con end < start y el handler
    // calculaba (end - start + 1) con wrap a ~1.8e19 → vec![0u8; chunk_len]
    // intentaba una asignación de ~18 EiB (abort del proceso en release).
    // RFC 7233 §2.1: el spec inválido se ignora (header → None → 200 full).
    #[test]
    fn range_with_end_before_start_is_ignored() {
        assert_eq!(parse_bytes_range("bytes=100-50"), None);
        assert_eq!(parse_bytes_range("bytes=1000-0"), None);
        // Con el fix, el header inválido cae en la rama "sin Range" del
        // handler: full-body 200, nunca aritmética con wrap.
        assert_eq!(chunk_len_for(100, 50), None);
    }

    #[test]
    fn valid_ranges_still_parse() {
        assert_eq!(parse_bytes_range("bytes=0-499"), Some((0, Some(499))));
        assert_eq!(parse_bytes_range("bytes=500-"), Some((500, None)));
        assert_eq!(parse_bytes_range("bytes=0-0"), Some((0, Some(0))));
        assert_eq!(
            parse_bytes_range("bytes= 100 - 200 "),
            Some((100, Some(200)))
        );
    }

    #[test]
    fn chunk_len_is_checked_and_bounded() {
        assert_eq!(chunk_len_for(0, 7), Some(8));
        // Rango completo de un solo byte.
        assert_eq!(chunk_len_for(42, 42), Some(1));
        // Longitud máxima de chunk del handler (8 MiB) cabe en usize.
        assert_eq!(chunk_len_for(0, 8 * 1024 * 1024 - 1), Some(8 * 1024 * 1024));
    }
}
