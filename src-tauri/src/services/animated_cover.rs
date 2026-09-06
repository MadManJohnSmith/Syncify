//! Apple Music Motion / Animated Cover Art Downloader & Converter for `src-tauri`
//!
//! Pipeline:
//! 1. Extracts Apple Music developer token (JWT) from web player JS bundle.
//! 2. Searches iTunes API to resolve exact collectionId.
//! 3. Queries Apple Music Catalog API for `editorialVideo.motionDetailSquare.video` (HLS .m3u8).
//! 4. Converts HLS stream to animated WebP sidecars (`cover.webp`, `cover.animated.webp`) using ffmpeg.
//! 5. Preserves standard JPEG front cover in FLAC files, maintaining animated artwork purely as external sidecars.

use reqwest::Client;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use syncify_core_domain::byte_validators::ImageByteValidator;

/// Explicit Animated Cover Resolution Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedCoverStatus {
    Success(PathBuf),
    NotFound,
    SourceUnavailable(String),
    Failed(String),
}

/// Redact sensitive stream/HLS URLs by retaining only the scheme, host, high-level resource type,
/// and a non-reversible truncated SHA-256 hash, stripping all query parameters, tokens,
/// signatures, and cookies.
pub fn redact_stream_url(raw_url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(raw_url) {
        let host = parsed.host_str().unwrap_or("[unknown_host]");
        let path = parsed.path();
        let resource_type = if path.ends_with(".m3u8") {
            "HLS playlist (.m3u8)"
        } else if path.ends_with(".mpd") {
            "DASH manifest (.mpd)"
        } else if path.ends_with(".mp4") || path.ends_with(".m4v") {
            "Video stream (.mp4)"
        } else if path.ends_with(".webp") {
            "WebP image (.webp)"
        } else if path.ends_with(".js") {
            "JavaScript bundle (.js)"
        } else {
            "Media resource"
        };

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(raw_url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let short_hash = &hash[..8];

        format!("https://{}/.../{} [id_hash:{}]", host, resource_type, short_hash)
    } else {
        "[REDACTED_STREAM_URL]".to_string()
    }
}

use std::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Session-level Apple Music Developer Token cache with TTL (12 hours)
static CACHED_APPLE_MUSIC_TOKEN: RwLock<Option<(String, Instant)>> = RwLock::new(None);

/// Clear the Apple Music token cache (useful for testing or re-authentication)
#[allow(dead_code)]
pub fn clear_apple_music_token_cache() {
    if let Ok(mut guard) = CACHED_APPLE_MUSIC_TOKEN.write() {
        *guard = None;
    }
}

/// Set the Apple Music token in the cache directly
#[allow(dead_code)]
pub fn set_cached_apple_music_token(token: &str) {
    if let Ok(mut guard) = CACHED_APPLE_MUSIC_TOKEN.write() {
        *guard = Some((token.to_string(), Instant::now()));
    }
}

/// Get the currently cached Apple Music token if valid
pub fn get_cached_apple_music_token() -> Option<String> {
    if let Ok(guard) = CACHED_APPLE_MUSIC_TOKEN.read() {
        if let Some((ref token, ref instant)) = *guard {
            if instant.elapsed() < Duration::from_secs(12 * 3600) {
                return Some(token.clone());
            }
        }
    }
    None
}

/// Extract Apple Music developer token (JWT) from the web player JavaScript bundle with session-level caching.
pub async fn extract_apple_music_token(client: &Client) -> Option<String> {
    if let Some(cached) = get_cached_apple_music_token() {
        return Some(cached);
    }

    use regex::Regex;

    info!("[AnimatedCover] Extracting Apple Music web player token...");

    // Step 1: Fetch music.apple.com to find JS bundle URL
    let page = match client
        .get("https://music.apple.com/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            warn!("[AnimatedCover] Failed to fetch music.apple.com");
            return None;
        }
    };

    // Step 2: Find JS bundle path (index-legacy~*.js, index-*.js, web-player-*.js)
    let js_re = match Regex::new(r#"(/assets/(?:index|web-player|app)[^"'\s>]+\.js)"#)
        .or_else(|_| Regex::new(r#"(/assets/[^"'\s>]+\.js)"#)) {
        Ok(re) => re,
        Err(_) => return None,
    };
    let js_path = match js_re.captures(&page).and_then(|c| c.get(1)) {
        Some(m) => m.as_str(),
        None => return None,
    };

    let js_url = format!("https://music.apple.com{}", js_path);
    debug!("[AnimatedCover] Fetching JS bundle from: {}", redact_stream_url(&js_url));

    // Step 3: Download JS bundle and extract JWT token
    let js_content = match client
        .get(&js_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            warn!("[AnimatedCover] Failed to download JS bundle");
            return None;
        }
    };

    let token_re = match Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+") {
        Ok(re) => re,
        Err(_) => return None,
    };

    for cap in token_re.find_iter(&js_content) {
        let token = cap.as_str();
        if token.starts_with("eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6IldlYlBsYXlLaWQifQ") {
            info!("[AnimatedCover] Successfully extracted Apple Music WebPlayKid token");
            set_cached_apple_music_token(token);
            return Some(token.to_string());
        }
    }

    if let Some(cap) = token_re.find(&js_content) {
        let token = cap.as_str().to_string();
        info!("[AnimatedCover] Using fallback JWT token from Apple Music JS bundle");
        set_cached_apple_music_token(&token);
        return Some(token);
    }

    warn!("[AnimatedCover] No JWT token found in Apple Music JS bundle");
    None
}

/// Cached album animated cover entry
#[derive(Debug, Clone)]
enum CachedAlbumCover {
    Bytes(Vec<u8>),
    NotFound,
    SourceUnavailable(String),
}

/// Album-level cache for motion covers (prevents duplicate queries for multiple tracks in the same album)
static ANIMATED_COVER_ALBUM_CACHE: RwLock<Option<HashMap<String, CachedAlbumCover>>> = RwLock::new(None);

/// Clear the album-level animated cover cache (useful for testing)
#[allow(dead_code)]
pub fn clear_animated_cover_cache() {
    if let Ok(mut guard) = ANIMATED_COVER_ALBUM_CACHE.write() {
        *guard = Some(HashMap::new());
    }
}

/// Set an animated cover in the album-level cache directly (useful for testing)
#[allow(dead_code)]
pub fn set_cached_animated_cover_bytes(artist: &str, album: &str, bytes: Vec<u8>) {
    let cache_key = format!("{}:::{}", artist.to_lowercase().trim(), album.to_lowercase().trim());
    if let Ok(mut guard) = ANIMATED_COVER_ALBUM_CACHE.write() {
        let cache = guard.get_or_insert_with(HashMap::new);
        cache.insert(cache_key, CachedAlbumCover::Bytes(bytes));
    }
}

pub fn strip_album_edition_suffixes(title: &str) -> String {
    let mut cleaned = title.to_string();
    let suffixes = [
        "(Deluxe Edition)", "(Deluxe)", "(Extended Edition)", "(Extended)",
        "(The Complete Edition)", "(Complete Edition)", "(Special Edition)",
        "[Deluxe Edition]", "[Deluxe]", "[Extended Edition]", "[Extended]",
        "Deluxe Edition", "Extended Edition", "The Complete Edition",
    ];
    for suf in &suffixes {
        if let Some(pos) = cleaned.to_lowercase().find(&suf.to_lowercase()) {
            cleaned = cleaned[..pos].trim().to_string();
            break;
        }
    }
    cleaned
}

/// Strip leading and trailing punctuation/ellipses (e.g. "...Like Clockwork" -> "Like Clockwork")
pub fn strip_leading_punctuation(s: &str) -> String {
    s.trim_start_matches(|c: char| c == '.' || c == '…' || c == '!' || c == '?' || c == '-' || c == '_' || c == ':' || c.is_whitespace())
     .trim_end_matches(|c: char| c == '.' || c == '…' || c == '!' || c == '?' || c == '-' || c == '_' || c == ':' || c.is_whitespace())
     .to_string()
}

/// Normalize text for comparison by replacing unicode ellipsis with dots,
/// stripping edition suffixes, and keeping alphanumeric tokens.
pub fn normalize_for_comparison(s: &str) -> String {
    let un_ellipsed = s.replace('…', "...");
    let cleaned = strip_album_edition_suffixes(&un_ellipsed);
    let stripped = strip_leading_punctuation(&cleaned);
    stripped
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Check if iTunes / Apple Music artist and album match the requested search target
pub fn matches_artist_and_album(r_artist: &str, r_album: &str, target_artist: &str, target_album: &str) -> bool {
    let norm_r_art = normalize_for_comparison(r_artist);
    let norm_tgt_art = normalize_for_comparison(target_artist);
    let norm_r_alb = normalize_for_comparison(r_album);
    let norm_tgt_alb = normalize_for_comparison(target_album);

    let artist_match = norm_r_art.is_empty() || norm_tgt_art.is_empty()
        || norm_r_art.contains(&norm_tgt_art)
        || norm_tgt_art.contains(&norm_r_art);

    let album_match = norm_r_alb == norm_tgt_alb
        || norm_r_alb.contains(&norm_tgt_alb)
        || norm_tgt_alb.contains(&norm_r_alb);

    artist_match && album_match
}

/// Validate whether bytes contain a valid animated WebP image with VP8X, ANIM, and ANMF frames
pub fn validate_animated_webp_bytes(bytes: &[u8]) -> Result<usize, &'static str> {
    match syncify_core_domain::byte_validators::WebpByteValidator::validate_animated_webp(bytes) {
        Ok(info) => Ok(info.anmf_frame_count),
        Err(syncify_core_domain::byte_validators::WebpValidationError::TooSmall { .. }) => Err("WebP data too small (< 30 bytes)"),
        Err(syncify_core_domain::byte_validators::WebpValidationError::InvalidRiffHeader) |
        Err(syncify_core_domain::byte_validators::WebpValidationError::InvalidWebpHeader) => Err("Not a valid RIFF WEBP container"),
        Err(syncify_core_domain::byte_validators::WebpValidationError::MissingVp8xChunk) |
        Err(syncify_core_domain::byte_validators::WebpValidationError::AnimationBitNotSet) => Err("WebP does not declare VP8X animation flag"),
        Err(syncify_core_domain::byte_validators::WebpValidationError::NoAnmfFramesFound) => Err("WebP contains 0 ANMF animation frames"),
        Err(_) => Err("Invalid animated WebP"),
    }
}

/// Authorized protocols for FFmpeg HLS stream ingestion [SEC-015].
pub const FFMPEG_HLS_PROTOCOL_WHITELIST: &str = "https,tls,tcp";

/// Validate an Apple Music animated artwork HLS stream URL [SEC-015 / TASK-99].
///
/// Ensures:
/// 1. The URL is syntactically valid and can be parsed by `reqwest::Url`.
/// 2. The URL scheme is strictly HTTPS (`url.scheme() == "https"`).
/// 3. The host is present and belongs to authorized Apple Music media domains:
///    `apple.com`, `*.apple.com`, `mzstatic.com`, or `*.mzstatic.com` (case-insensitive).
/// 4. Disallows dangerous protocols (`file://`, `http://`, `concat:`, `gopher://`, etc.)
///    preventing SSRF and local file inclusion when passed to FFmpeg.
pub fn validate_hls_stream_url(m3u8_url: &str) -> Result<reqwest::Url, String> {
    validate_hls_stream_url_opts(m3u8_url, false)
}

/// Helper specifically for testing or local development environments to validate
/// stream URLs allowing loopback/localhost.
#[allow(dead_code)]
pub fn validate_hls_stream_url_for_test(m3u8_url: &str) -> Result<reqwest::Url, String> {
    validate_hls_stream_url_opts(m3u8_url, true)
}

/// Validate an Apple Music animated artwork HLS stream URL with configurable loopback permission.
#[allow(dead_code)]
pub fn validate_hls_stream_url_opts(m3u8_url: &str, allow_loopback: bool) -> Result<reqwest::Url, String> {
    let trimmed = m3u8_url.trim();
    if trimmed.is_empty() {
        return Err("HLS stream URL cannot be empty".to_string());
    }

    let url = reqwest::Url::parse(trimmed)
        .map_err(|e| format!("Invalid stream URL format: {}", e))?;

    // Scheme validation: strictly https, unless loopback is explicitly allowed for testing
    let scheme = url.scheme();
    let is_https = scheme == "https";
    let is_test_http = allow_loopback && scheme == "http";

    if !is_https && !is_test_http {
        return Err(format!(
            "Insecure URL scheme '{}': only 'https' is permitted",
            scheme
        ));
    }

    // Userinfo check (credentials in URL like https://user:pass@host/ are disallowed)
    if !url.username().is_empty() || url.password().is_some() {
        return Err("HLS stream URL must not contain user credentials".to_string());
    }

    // Host validation
    let host = url.host_str().ok_or_else(|| "Stream URL does not contain a valid host".to_string())?;
    let host_lower = host.to_ascii_lowercase();
    let host_clean = host_lower.trim_end_matches('.');

    let is_authorized_domain = host_clean == "apple.com"
        || host_clean.ends_with(".apple.com")
        || host_clean == "mzstatic.com"
        || host_clean.ends_with(".mzstatic.com");

    if is_authorized_domain {
        return Ok(url);
    }

    if allow_loopback {
        let is_named_loopback = host_clean == "localhost"
            || host_clean == "127.0.0.1"
            || host_clean == "::1"
            || host_clean == "[::1]";

        if is_named_loopback {
            return Ok(url);
        }

        let ip_str = host_clean.trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
            if ip.is_loopback() {
                return Ok(url);
            }
        }
    }

    Err(format!(
        "Unauthorized stream host '{}': host must belong to .apple.com or .mzstatic.com",
        host
    ))
}

/// Construct secure FFmpeg command line arguments for HLS stream conversion to animated WebP.
/// Enforces protocol whitelist before the input argument to prevent SSRF and local file leaks.
pub fn build_ffmpeg_animated_cover_args<'a>(m3u8_url: &'a str, output_path: &'a str) -> Vec<&'a str> {
    vec![
        "-y",
        "-protocol_whitelist",
        FFMPEG_HLS_PROTOCOL_WHITELIST,
        "-i",
        m3u8_url,
        "-t",
        "8",
        "-vf",
        "fps=15,scale=500:500:flags=lanczos",
        "-vcodec",
        "libwebp",
        "-loop",
        "0",
        "-q:v",
        "75",
        "-an",
        output_path,
    ]
}

/// Construct FFmpeg arguments to transcode an animated WebP cover to an MP4 sidecar (`animated_cover.mp4`)
/// for Symfonium compatibility [TASK-77].
///
/// Parameters:
/// - `-y`: overwrite output file without prompting
/// - `-i`: input animated WebP path
/// - `-movflags +faststart`: enables progressive streaming playback (moves moov atom to beginning)
/// - `-pix_fmt yuv420p`: ensures baseline/main H.264 profile compatibility across all mobile decoders
/// - `-vf "scale='min(1000,iw)':-2,crop='trunc(iw/2)*2':'trunc(ih/2)*2',fps=30"`: scales to <=1000px, crops to even dimensions, enforces 30 fps
/// - `-c:v libx264`: standard H.264 video codec
/// - `-crf 23`: high visual fidelity with modest file size
/// - `-an`: disables audio track completely (cover video has no sound)
pub fn build_ffmpeg_webp_to_mp4_args<'a>(webp_path: &'a str, output_path: &'a str) -> Vec<&'a str> {
    vec![
        "-y",
        "-i",
        webp_path,
        "-movflags",
        "+faststart",
        "-pix_fmt",
        "yuv420p",
        "-vf",
        "scale='min(1000,iw)':-2,crop='trunc(iw/2)*2':'trunc(ih/2)*2',fps=30",
        "-c:v",
        "libx264",
        "-crf",
        "23",
        "-an",
        output_path,
    ]
}

/// Validate whether a specific static cover JPEG exists and meets the minimum 1000x1000
/// resolution standard required for Symfonium static fallback [TASK-77].
pub fn validate_static_cover_jpg(cover_path: &Path) -> Result<(u32, u32), String> {
    if !cover_path.exists() {
        return Err(format!("Static cover does not exist at {:?}", cover_path));
    }

    let bytes = std::fs::read(cover_path)
        .map_err(|e| format!("Failed to read static cover {:?}: {}", cover_path, e))?;

    if bytes.len() < 4 {
        return Err(format!("Static cover {:?} is too small (< 4 bytes)", cover_path));
    }

    let dims = ImageByteValidator::parse_dimensions(&bytes)
        .ok_or_else(|| format!("Failed to parse image dimensions from {:?}", cover_path))?;

    if dims.mime_type != "image/jpeg" {
        return Err(format!(
            "Static cover {:?} is not a JPEG (detected mime type '{}')",
            cover_path, dims.mime_type
        ));
    }

    if dims.width < 1000 || dims.height < 1000 {
        return Err(format!(
            "Static cover {:?} resolution {}x{} is below the minimum required 1000x1000",
            cover_path, dims.width, dims.height
        ));
    }

    Ok((dims.width, dims.height))
}

/// Validate high-resolution static cover.jpg in an album directory [TASK-77].
pub fn validate_high_res_static_cover(album_dir: &Path) -> Result<(u32, u32), String> {
    let cover_jpg = album_dir.join("cover.jpg");
    validate_static_cover_jpg(&cover_jpg)
}

/// Associate the `animated_cover.mp4` path to an album in SQLite if the `animated_cover_path`
/// column exists in the `albums` table [TASK-77].
/// Returns Ok(true) if the association was performed, Ok(false) if the column/ledger is not yet present.
pub async fn associate_animated_cover_in_db(
    pool: &sqlx::SqlitePool,
    album_id: i64,
    mp4_path: &Path,
) -> Result<bool, String> {
    let check_query = "SELECT COUNT(*) FROM pragma_table_info('albums') WHERE name = 'animated_cover_path'";
    let has_col: bool = sqlx::query_scalar(check_query)
        .fetch_one(pool)
        .await
        .map(|cnt: i32| cnt > 0)
        .unwrap_or(false);

    if has_col {
        let path_str = mp4_path.to_string_lossy().to_string();
        sqlx::query("UPDATE albums SET animated_cover_path = ? WHERE id = ?")
            .bind(&path_str)
            .bind(album_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update animated_cover_path in SQLite: {}", e))?;
        info!("[AnimatedCover] Associated animated_cover.mp4 to album id {}", album_id);
        Ok(true)
    } else {
        debug!("[AnimatedCover] Column 'animated_cover_path' not present in 'albums' table; skipping association");
        Ok(false)
    }
}

/// Associate the `animated_cover.mp4` path to an album by title in SQLite if the column exists [TASK-77].
#[allow(dead_code)]
pub async fn associate_animated_cover_by_title_in_db(
    pool: &sqlx::SqlitePool,
    album_title: &str,
    mp4_path: &Path,
) -> Result<bool, String> {
    let check_query = "SELECT COUNT(*) FROM pragma_table_info('albums') WHERE name = 'animated_cover_path'";
    let has_col: bool = sqlx::query_scalar(check_query)
        .fetch_one(pool)
        .await
        .map(|cnt: i32| cnt > 0)
        .unwrap_or(false);

    if has_col {
        let path_str = mp4_path.to_string_lossy().to_string();
        sqlx::query("UPDATE albums SET animated_cover_path = ? WHERE title = ? COLLATE NOCASE")
            .bind(&path_str)
            .bind(album_title)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update animated_cover_path in SQLite by title: {}", e))?;
        info!("[AnimatedCover] Associated animated_cover.mp4 to album '{}'", album_title);
        Ok(true)
    } else {
        debug!("[AnimatedCover] Column 'animated_cover_path' not present in 'albums' table; skipping association");
        Ok(false)
    }
}

/// Transcode an animated WebP file to `animated_cover.mp4` sidecar for Symfonium [TASK-77].
///
/// Invariant: Preserves the CoverFront (0x03) = image/webp animated invariant and existing WebP
/// sidecars (`cover.webp`, `cover.animated.webp`). The MP4 sidecar is complementary to ensure
/// fluid playback on external media players and Symfonium.
pub async fn transcode_webp_to_animated_mp4(
    webp_path: &Path,
    output_mp4_path: &Path,
    require_static_cover: bool,
    db_pool: Option<&sqlx::SqlitePool>,
    album_id: Option<i64>,
) -> Result<PathBuf, String> {
    if !webp_path.exists() {
        return Err(format!("Input WebP file does not exist: {:?}", webp_path));
    }

    let webp_bytes = std::fs::read(webp_path)
        .map_err(|e| format!("Failed to read input WebP {:?}: {}", webp_path, e))?;

    validate_animated_webp_bytes(&webp_bytes)
        .map_err(|e| format!("Input file is not a valid animated WebP: {}", e))?;

    if require_static_cover {
        if let Some(parent) = webp_path.parent() {
            validate_high_res_static_cover(parent)
                .map_err(|e| format!("Static cover validation failed: {}", e))?;
        }
    }

    if let Some(parent) = output_mp4_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let webp_str = webp_path.to_str().ok_or("Invalid UTF-8 in webp path")?;
    let output_str = output_mp4_path.to_str().ok_or("Invalid UTF-8 in output path")?;
    let args = build_ffmpeg_webp_to_mp4_args(webp_str, output_str);

    let ffmpeg_child = crate::cmd_utils::create_tokio_command("ffmpeg")
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    let result = match tokio::time::timeout(std::time::Duration::from_secs(30), ffmpeg_child).await {
        Ok(res) => res,
        Err(_) => {
            let _ = tokio::fs::remove_file(output_mp4_path).await;
            return Err("FFmpeg WebP to MP4 transcode timed out after 30s".to_string());
        }
    };

    match result {
        Ok(output) => {
            if output_mp4_path.exists() {
                let size = std::fs::metadata(output_mp4_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                if size >= 100 {
                    info!(
                        "[AnimatedCover] ✓ Successfully transcoded animated WebP to {:?} ({} bytes)",
                        output_mp4_path, size
                    );

                    if let (Some(pool), Some(aid)) = (db_pool, album_id) {
                        let _ = associate_animated_cover_in_db(pool, aid, output_mp4_path).await;
                    }

                    return Ok(output_mp4_path.to_path_buf());
                } else {
                    let _ = tokio::fs::remove_file(output_mp4_path).await;
                    return Err(format!("FFmpeg generated undersized MP4 ({} bytes)", size));
                }
            }

            let err_msg = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "FFmpeg WebP to MP4 transcode failed: {}",
                err_msg.lines().next().unwrap_or("unknown error")
            ))
        }
        Err(e) => Err(format!("Failed to spawn FFmpeg: {}", e)),
    }
}

/// Convenience function to transcode and generate `animated_cover.mp4` in the same directory as `cover.webp`.
#[allow(dead_code)]
pub async fn transcode_album_cover_to_sidecar_mp4(
    album_dir: &Path,
    require_static_cover: bool,
    db_pool: Option<&sqlx::SqlitePool>,
    album_id: Option<i64>,
) -> Result<PathBuf, String> {
    let webp_path = album_dir.join("cover.webp");
    let mp4_path = album_dir.join("animated_cover.mp4");
    transcode_webp_to_animated_mp4(&webp_path, &mp4_path, require_static_cover, db_pool, album_id).await
}

/// Download animated album cover art from Apple Music with explicit status and album-level caching.
pub async fn resolve_and_download_animated_cover(
    client: &Client,
    artist: &str,
    album: &str,
    target_dir: &Path,
) -> AnimatedCoverStatus {
    if artist.trim().is_empty() || album.trim().is_empty() {
        return AnimatedCoverStatus::NotFound;
    }

    let cache_key = format!("{}:::{}", artist.to_lowercase().trim(), album.to_lowercase().trim());

    // Check album-level cache (lock is dropped immediately)
    let cached_entry = if let Ok(guard) = ANIMATED_COVER_ALBUM_CACHE.read() {
        guard.as_ref().and_then(|c| c.get(&cache_key).cloned())
    } else {
        None
    };

    if let Some(cached) = cached_entry {
        match cached {
            CachedAlbumCover::NotFound => {
                debug!("[AnimatedCover] Reusing cached NotFound for '{} - {}'", artist, album);
                return AnimatedCoverStatus::NotFound;
            }
            CachedAlbumCover::SourceUnavailable(reason) => {
                return AnimatedCoverStatus::SourceUnavailable(reason);
            }
            CachedAlbumCover::Bytes(bytes) => {
                let target_path = target_dir.join("cover.webp");
                let anim_path = target_dir.join("cover.animated.webp");
                let _ = tokio::fs::create_dir_all(target_dir).await;
                let target_is_valid = target_path.exists() && target_path.metadata().map(|m| m.len() > 0).unwrap_or(false);
                if !target_is_valid {
                    let _ = tokio::fs::write(&target_path, &bytes).await;
                }
                let anim_is_valid = anim_path.exists() && anim_path.metadata().map(|m| m.len() > 0).unwrap_or(false);
                if !anim_is_valid {
                    let _ = tokio::fs::write(&anim_path, &bytes).await;
                }
                let mp4_path = target_dir.join("animated_cover.mp4");
                let mp4_is_valid = mp4_path.exists() && mp4_path.metadata().map(|m| m.len() > 0).unwrap_or(false);
                if !mp4_is_valid {
                    let _ = transcode_webp_to_animated_mp4(&target_path, &mp4_path, false, None, None).await;
                }
                debug!("[AnimatedCover] Reusing cached animated WebP for '{} - {}'", artist, album);
                return AnimatedCoverStatus::Success(target_path);
            }
        }
    }

    let status = resolve_and_download_animated_cover_uncached(client, artist, album, target_dir).await;

    // Cache the result for this album
    let cached_to_store = match &status {
        AnimatedCoverStatus::Success(path) => {
            if let Ok(bytes) = tokio::fs::read(path).await {
                Some(CachedAlbumCover::Bytes(bytes))
            } else {
                None
            }
        }
        AnimatedCoverStatus::NotFound => Some(CachedAlbumCover::NotFound),
        AnimatedCoverStatus::SourceUnavailable(reason) => Some(CachedAlbumCover::SourceUnavailable(reason.clone())),
        _ => None,
    };

    if let Some(entry) = cached_to_store {
        if let Ok(mut guard) = ANIMATED_COVER_ALBUM_CACHE.write() {
            let cache = guard.get_or_insert_with(HashMap::new);
            cache.insert(cache_key, entry);
        }
    }

    status
}

async fn resolve_and_download_animated_cover_uncached(
    client: &Client,
    artist: &str,
    album: &str,
    target_dir: &Path,
) -> AnimatedCoverStatus {
    if let Err(e) = tokio::fs::create_dir_all(target_dir).await {
        return AnimatedCoverStatus::Failed(format!("Failed to create target directory: {}", e));
    }

    // Step 1: Extract Apple Music developer token
    let am_token = match extract_apple_music_token(client).await {
        Some(token) => token,
        None => {
            return AnimatedCoverStatus::SourceUnavailable(
                "Could not extract Apple Music developer token from web player".to_string(),
            );
        }
    };

    // Step 2A: Query iTunes Search API to resolve exact Apple Music collectionIds
    let clean_album = strip_album_edition_suffixes(album);
    let stripped_album = strip_leading_punctuation(&clean_album);
    let un_ellipsed_album = album.replace('…', "...");

    let mut search_terms = vec![
        format!("{} {}", artist, stripped_album),
        format!("{} {}", artist, clean_album),
        format!("{} {}", artist, album),
        stripped_album.clone(),
        clean_album.clone(),
    ];

    if album.contains('&') {
        let and_variant = album.replace('&', "and");
        search_terms.push(format!("{} {}", artist, strip_leading_punctuation(&and_variant)));
    }
    if un_ellipsed_album != album {
        search_terms.push(format!("{} {}", artist, strip_leading_punctuation(&un_ellipsed_album)));
    }

    let mut collection_ids = Vec::new();
    let storefronts = vec!["us", "gb", "es", "de", "fr", "mx", "it", "ca", "au", "jp", "nl", "br"];

    for term in &search_terms {
        let itunes_url = format!(
            "https://itunes.apple.com/search?term={}&entity=album&limit=15",
            urlencoding::encode(term)
        );
        if let Ok(res) = client.get(&itunes_url).send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(results) = json["results"].as_array() {
                        for item in results {
                            let r_artist = item["artistName"].as_str().unwrap_or("");
                            let r_album = item["collectionName"].as_str().unwrap_or("");

                            if matches_artist_and_album(r_artist, r_album, artist, album) {
                                if let Some(cid) = item["collectionId"].as_u64() {
                                    if !collection_ids.contains(&cid) {
                                        collection_ids.push(cid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut m3u8_url: Option<String> = None;

    // Step 2B: Direct lookup by collectionId on Apple Music catalog API
    'id_lookup: for cid in &collection_ids {
        for sf in &storefronts {
            let album_url = format!(
                "https://amp-api.music.apple.com/v1/catalog/{}/albums/{}?extend=editorialVideo",
                sf, cid
            );
            let req = client
                .get(&album_url)
                .header("Authorization", format!("Bearer {}", am_token))
                .header("Origin", "https://music.apple.com")
                .header("Referer", "https://music.apple.com/")
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

            if let Ok(res) = req.send().await {
                if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                    continue;
                }
                if res.status().is_success() {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(albums_arr) = json["data"].as_array() {
                            if let Some(item) = albums_arr.first() {
                                let attrs = &item["attributes"];
                                let video = attrs["editorialVideo"]["motionDetailSquare"]["video"].as_str()
                                    .or_else(|| attrs["editorialVideo"]["motionSquareVideo1x1"]["video"].as_str())
                                    .or_else(|| attrs["editorialVideo"]["motionDetailTall"]["video"].as_str())
                                    .or_else(|| attrs["editorialArtwork"]["motionDetailSquare"]["video"].as_str());

                                if let Some(vid_url) = video {
                                    info!("[AnimatedCover] ✓ Found animated cover HLS stream via ID {} on '{}' for '{} - {}'", cid, sf, artist, album);
                                    m3u8_url = Some(vid_url.to_string());
                                    break 'id_lookup;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 2C: Fallback to Catalog Search across storefronts
    if m3u8_url.is_none() {
        let catalog_query_terms = vec![
            format!("{} {}", artist, stripped_album),
            format!("{} {}", artist, clean_album),
        ];

        'sf_search: for sf in &storefronts {
            for term in &catalog_query_terms {
                let catalog_search_url = format!(
                    "https://amp-api.music.apple.com/v1/catalog/{}/search?term={}&types=albums&extend=editorialVideo&limit=10",
                    sf,
                    urlencoding::encode(term)
                );

                let req = client
                    .get(&catalog_search_url)
                    .header("Authorization", format!("Bearer {}", am_token))
                    .header("Origin", "https://music.apple.com")
                    .header("Referer", "https://music.apple.com/")
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

                if let Ok(res) = req.send().await {
                    if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                        continue;
                    }
                    if res.status().is_success() {
                        if let Ok(json) = res.json::<serde_json::Value>().await {
                            if let Some(albums_arr) = json["results"]["albums"]["data"].as_array() {
                                for item in albums_arr {
                                    let attrs = &item["attributes"];
                                    let r_artist = attrs["artistName"].as_str().unwrap_or("");
                                    let r_album = attrs["name"].as_str().unwrap_or("");

                                    if matches_artist_and_album(r_artist, r_album, artist, album) {
                                        let video = attrs["editorialVideo"]["motionDetailSquare"]["video"].as_str()
                                            .or_else(|| attrs["editorialVideo"]["motionSquareVideo1x1"]["video"].as_str())
                                            .or_else(|| attrs["editorialVideo"]["motionDetailTall"]["video"].as_str())
                                            .or_else(|| attrs["editorialArtwork"]["motionDetailSquare"]["video"].as_str());

                                        if let Some(vid_url) = video {
                                            info!("[AnimatedCover] ✓ Found animated cover HLS stream on '{}' for '{} - {}'", sf, artist, album);
                                            m3u8_url = Some(vid_url.to_string());
                                            break 'sf_search;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let m3u8_url = match m3u8_url {
        Some(url) => url,
        None => {
            info!("[AnimatedCover] No animated artwork available for '{}' - '{}' across storefronts", artist, album);
            return AnimatedCoverStatus::NotFound;
        }
    };

    info!("[AnimatedCover] Found animated artwork HLS stream: {}", redact_stream_url(&m3u8_url));

    // Security Gate [SEC-015 / TASK-99]: Validate URL scheme and host whitelist before invoking FFmpeg
    let validated_url = match validate_hls_stream_url(&m3u8_url) {
        Ok(u) => u,
        Err(err) => {
            warn!("[AnimatedCover] Refusing to invoke ffmpeg: rejected untrusted or invalid stream URL '{}': {}", redact_stream_url(&m3u8_url), err);
            return AnimatedCoverStatus::Failed(format!("Untrusted or invalid stream URL: {}", err));
        }
    };

    // Step 3: Convert HLS stream to animated WebP using ffmpeg with 30s timeout
    let webp_path = target_dir.join("cover.webp");
    let output_str = webp_path.to_str().unwrap_or("cover.webp");
    let ffmpeg_args = build_ffmpeg_animated_cover_args(validated_url.as_str(), output_str);

    let ffmpeg_child = crate::cmd_utils::create_tokio_command("ffmpeg")
        .args(&ffmpeg_args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    let webp_result = match tokio::time::timeout(std::time::Duration::from_secs(30), ffmpeg_child).await {
        Ok(res) => res,
        Err(_) => {
            warn!("[AnimatedCover] ffmpeg conversion timed out after 30s for '{} - {}'", artist, album);
            let _ = tokio::fs::remove_file(&webp_path).await;
            return AnimatedCoverStatus::Failed("ffmpeg conversion timed out after 30s".to_string());
        }
    };

    match webp_result {
        Ok(r) => {
            if webp_path.exists() {
                let size = std::fs::metadata(&webp_path).map(|m| m.len()).unwrap_or(0);
                if size >= 30 {
                    if let Ok(bytes) = std::fs::read(&webp_path) {
                        match validate_animated_webp_bytes(&bytes) {
                            Ok(frames) => {
                                let cover_animated_webp = target_dir.join("cover.animated.webp");
                                let _ = std::fs::copy(&webp_path, &cover_animated_webp);

                                // Generate Symfonium animated_cover.mp4 complementary sidecar [TASK-77]
                                let mp4_path = target_dir.join("animated_cover.mp4");
                                if let Err(e) = transcode_webp_to_animated_mp4(&webp_path, &mp4_path, false, None, None).await {
                                    warn!("[AnimatedCover] Non-fatal: failed to generate animated_cover.mp4 sidecar: {}", e);
                                }

                                info!("[AnimatedCover] ✓ High-quality animated cover.webp sidecar saved ({} KB, {} frames): {:?}", size / 1024, frames, webp_path);
                                return AnimatedCoverStatus::Success(webp_path);
                            }
                            Err(e) => {
                                warn!("[AnimatedCover] Generated WebP failed animation validation: {}", e);
                                let _ = std::fs::remove_file(&webp_path);
                                return AnimatedCoverStatus::Failed(format!("Invalid animated WebP: {}", e));
                            }
                        }
                    }
                } else {
                    warn!("[AnimatedCover] ffmpeg generated undersized file ({} bytes, < 30 bytes minimum)", size);
                    let _ = std::fs::remove_file(&webp_path);
                    return AnimatedCoverStatus::Failed(format!("ffmpeg generated undersized cover.webp ({} bytes)", size));
                }
            }

            if r.status.success() {
                warn!("[AnimatedCover] ffmpeg completed successfully but cover.webp not found at {:?}", webp_path);
                AnimatedCoverStatus::Failed("ffmpeg completed successfully but cover.webp not found on disk".to_string())
            } else {
                let err_msg = String::from_utf8_lossy(&r.stderr);
                warn!("[AnimatedCover] ffmpeg animated WebP conversion failed: {}", err_msg);
                AnimatedCoverStatus::Failed(format!("ffmpeg exit error: {}", err_msg.lines().next().unwrap_or("unknown error")))
            }
        }
        Err(e) => {
            warn!("[AnimatedCover] Failed to launch ffmpeg: {}", e);
            AnimatedCoverStatus::Failed(format!("Failed to spawn ffmpeg: {}", e))
        }
    }
}
