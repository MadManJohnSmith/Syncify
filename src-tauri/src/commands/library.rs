#[allow(unused_imports)]
use super::*;

// Library Commands - submodule of crate::commands
//
// Library CRUD operations, search, playlists

/// Validate only paths present in the returned page. The cache lives for one request,
/// so a later page refresh observes files removed or restored since the previous call.
async fn verify_page_files(tracks: &mut [LibraryTrack]) -> Result<(), String> {
    use futures::stream::{self, StreamExt};
    use std::collections::HashMap;
    use std::io::ErrorKind;

    let mut paths = tracks
        .iter()
        .filter(|track| track.download_status.as_deref() == Some("downloaded"))
        .filter_map(|track| track.file_path.as_ref())
        .filter(|path| !path.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    let checks = stream::iter(paths.into_iter().map(|path| async move {
        let exists = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata.is_file(),
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(format!("Cannot verify local audio file: {error}")),
        };
        Ok::<_, String>((path, exists))
    }))
    .buffer_unordered(16)
    .collect::<Vec<_>>()
    .await;
    let present: HashMap<String, bool> = checks.into_iter().collect::<Result<_, _>>()?;
    for track in tracks {
        if track.download_status.as_deref() == Some("downloaded")
            && !track
                .file_path
                .as_ref()
                .and_then(|path| present.get(path))
                .copied()
                .unwrap_or(false)
        {
            track.download_status = Some("not_downloaded".to_owned());
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetadata {
    pub track_id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub artist_id: Option<i64>,
    pub album_name: Option<String>,
    pub album_id: Option<i64>,
    pub duration_ms: Option<i64>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub isrc: Option<String>,
    pub explicit: Option<bool>,
    pub genre: Option<String>,
    pub bpm: Option<f64>,
    pub musical_key: Option<String>,
    pub release_year: Option<i32>,
    pub musicbrainz_id: Option<String>,
    pub cover_art_url: Option<String>,
    pub file_path: Option<String>,
    pub imported_from: Option<String>,
    pub downloaded_from: Option<String>,
    #[sqlx(default)]
    pub tempo_confidence: Option<f64>,
    #[sqlx(default)]
    pub tempo_source: Option<String>,
    #[sqlx(default)]
    pub display_title: Option<String>,
    #[sqlx(default)]
    pub source_title: Option<String>,
    #[sqlx(default)]
    pub file_disambiguator: Option<String>,
    #[sqlx(default)]
    pub album_artist: Option<String>,
    #[sqlx(default)]
    pub label: Option<String>,
    #[sqlx(default)]
    pub upc: Option<String>,
    #[sqlx(default)]
    pub release_date: Option<String>,
    #[sqlx(default)]
    pub total_tracks: Option<i32>,
    #[sqlx(default)]
    pub total_discs: Option<i32>,
    #[sqlx(default)]
    pub album_musicbrainz_id: Option<String>,
    #[sqlx(default)]
    pub musicbrainz_release_id: Option<String>,
    #[sqlx(default)]
    pub file_format: Option<String>,
    #[sqlx(default)]
    pub file_size_bytes: Option<i64>,
    #[sqlx(default)]
    pub bit_depth: Option<i32>,
    #[sqlx(default)]
    pub sample_rate: Option<i32>,
    #[sqlx(default)]
    pub downloaded_at: Option<String>,
    #[sqlx(default)]
    pub animated_cover_path: Option<String>,
    #[sqlx(skip)]
    pub booklet_path: Option<String>,
    #[sqlx(skip)]
    pub hd_cover_path: Option<String>,
    #[sqlx(skip)]
    pub sources: Option<Vec<TrackSourceAvailability>>,
}

async fn fetch_track_metadata(
    db: &sqlx::SqlitePool,
    track_id: i64,
) -> Result<TrackMetadata, String> {
    let mut metadata = sqlx::query_as::<_, TrackMetadata>(
        r#"
        SELECT
            t.id as track_id,
            t.title,
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            (SELECT a2.id FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_id,
            al.title as album_name,
            al.id as album_id,
            t.duration_ms,
            t.track_number,
            t.disc_number,
            t.isrc,
            t.explicit,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.musicbrainz_id,
            al.cover_art_url,
            d.file_path,
            (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
             FROM library_entries le
             JOIN accounts acc ON acc.id = le.account_id
             JOIN services s_imp ON s_imp.id = acc.service_id
             WHERE le.track_id = t.id) as imported_from,
            COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
            t.tempo_confidence,
            t.tempo_source,
            t.display_title,
            COALESCE(t.source_title, t.title) as source_title,
            COALESCE(t.file_disambiguator, d.file_disambiguator) as file_disambiguator,
            (SELECT a3.name FROM album_artists aa
             JOIN artists a3 ON a3.id = aa.artist_id
             WHERE aa.album_id = al.id
             LIMIT 1) as album_artist,
            al.label,
            al.upc,
            al.release_date,
            al.total_tracks,
            al.total_discs,
            al.musicbrainz_id as album_musicbrainz_id,
            d.musicbrainz_release_id,
            d.file_format,
            d.file_size_bytes,
            d.bit_depth,
            d.sample_rate,
            d.downloaded_at,
            al.animated_cover_path
        FROM tracks t
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE t.id = ?
        LIMIT 1
        "#,
    )
    .bind(track_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to fetch track metadata: {}", e))?
    .ok_or_else(|| format!("Track not found: {}", track_id))?;

    let sources = sqlx::query_as::<_, TrackSourceAvailability>(
        r#"
        SELECT ts.id, ts.track_id, ts.service_id, s.name as service_name, ts.service_track_id,
               ts.format, ts.bit_depth, ts.sample_rate, ts.quality_score, ts.available,
               COALESCE(ts.availability_status, 'unknown_unchecked') as availability_status,
               ts.availability_reason, ts.last_checked
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ?
        ORDER BY ts.quality_score DESC, ts.id ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    metadata.sources = Some(sources);

    let sibling_file_path = if metadata.file_path.is_none() {
        if let Some(album_id) = metadata.album_id {
            sqlx::query_scalar::<_, String>(
                r#"
                SELECT d2.file_path
                FROM downloads d2
                JOIN tracks t2 ON t2.id = d2.track_id
                WHERE t2.album_id = ? AND d2.file_path IS NOT NULL
                LIMIT 1
                "#,
            )
            .bind(album_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
        } else {
            None
        }
    } else {
        None
    };

    let primary_path = metadata.file_path.clone().or(sibling_file_path);
    let db_animated_path = metadata.animated_cover_path.clone();
    if primary_path.is_some() || db_animated_path.is_some() {
        if let Ok((booklet, animated, hd_cover)) = tokio::task::spawn_blocking(move || {
            resolve_track_sidecars(primary_path.as_deref(), db_animated_path.as_deref())
        })
        .await
        {
            metadata.booklet_path = booklet;
            metadata.animated_cover_path = animated;
            metadata.hd_cover_path = hd_cover;
        }
    }

    Ok(metadata)
}

fn resolve_track_sidecars(
    file_path: Option<&str>,
    db_animated_cover_path: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>) {
    use std::path::{Path, PathBuf};
    use syncify_core_domain::byte_validators::WebpByteValidator;
    use syncify_core_domain::CoverType;

    let mut dirs: Vec<PathBuf> = Vec::with_capacity(3);
    let mut push_dir = |dir: &Path| {
        if dir.is_dir() && !dirs.iter().any(|d| d == dir) {
            dirs.push(dir.to_path_buf());
        }
        if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("disc") || lower.starts_with("cd") {
                if let Some(parent) = dir.parent() {
                    if parent.is_dir() && !dirs.iter().any(|d| d == parent) {
                        dirs.push(parent.to_path_buf());
                    }
                }
            }
        }
    };

    if let Some(fp) = file_path {
        if let Some(parent) = Path::new(fp).parent() {
            push_dir(parent);
        }
    }
    if let Some(ap) = db_animated_cover_path {
        if let Some(parent) = Path::new(ap).parent() {
            push_dir(parent);
        }
    }

    let mut booklet_path: Option<String> = None;
    for dir in &dirs {
        let standard = dir.join("booklet.pdf");
        if standard.is_file() {
            booklet_path = Some(standard.to_string_lossy().into_owned());
            break;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            if let Some(pdf) = entries.filter_map(Result::ok).map(|e| e.path()).find(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
            }) {
                booklet_path = Some(pdf.to_string_lossy().into_owned());
                break;
            }
        }
    }

    let mut animated_path: Option<String> = None;
    for dir in &dirs {
        for name in ["cover.animated.webp", "animated.webp"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                animated_path = Some(candidate.to_string_lossy().into_owned());
                break;
            }
        }
        if animated_path.is_some() {
            break;
        }
        let cover_webp = dir.join("cover.webp");
        if cover_webp.is_file() {
            if let Ok(bytes) = std::fs::read(&cover_webp) {
                if WebpByteValidator::detect_cover_type(&bytes) == CoverType::AnimatedWebp {
                    animated_path = Some(cover_webp.to_string_lossy().into_owned());
                    break;
                }
            }
        }
        let mp4 = dir.join("animated_cover.mp4");
        if mp4.is_file() {
            animated_path = Some(mp4.to_string_lossy().into_owned());
            break;
        }
    }
    if animated_path.is_none() {
        if let Some(ap) = db_animated_cover_path {
            if Path::new(ap).is_file() {
                animated_path = Some(ap.to_string());
            }
        }
    }

    let mut hd_cover_path: Option<String> = None;
    for dir in &dirs {
        for name in [
            "cover.jpg",
            "cover.png",
            "folder.jpg",
            "folder.webp",
            "cover.webp",
        ] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                hd_cover_path = Some(candidate.to_string_lossy().into_owned());
                break;
            }
        }
        if hd_cover_path.is_some() {
            break;
        }
    }

    (booklet_path, animated_path, hd_cover_path)
}

#[tauri::command]
pub async fn get_track_metadata(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<TrackMetadata, String> {
    if track_id <= 0 {
        return Err(format!("Invalid track_id: {}", track_id));
    }

    fetch_track_metadata(&state.db, track_id).await
}

/// Get all tracks in the library (with artist, album, and service info) - paginated
#[tauri::command]
pub async fn get_library(
    state: State<'_, AppState>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    get_library_inner(&state.db, offset, limit).await
}

async fn get_library_inner(
    db: &sqlx::SqlitePool,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500); // Default 100, max 500 per request

    tracing::info!("get_library called with offset={}, limit={}", offset, limit);

    // Get total count first
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(db)
        .await
        .map_err(|e| format!("Count error: {}", e))?;

    let mut tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT
            t.id,
            t.title,
            -- Only get primary artist name (avoid duplicates from featured artists)
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            (SELECT a2.id FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_id,
            al.title as album_name,
            al.id as album_id,
            t.duration_ms,
            t.isrc,
            GROUP_CONCAT(DISTINCT s.name) as services,
            (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
             FROM library_entries le
             JOIN accounts acc ON acc.id = le.account_id
             JOIN services s_imp ON s_imp.id = acc.service_id
             WHERE le.track_id = t.id) as imported_from,
            COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
            (SELECT GROUP_CONCAT(DISTINCT s_avail.name)
             FROM track_sources ts_avail
             JOIN services s_avail ON s_avail.id = ts_avail.service_id
             WHERE ts_avail.track_id = t.id AND ts_avail.availability_status = 'available') as available_services,
            (SELECT GROUP_CONCAT(s_all.name || ':' || COALESCE(ts_all.availability_status, 'unknown_unchecked'), ', ')
             FROM track_sources ts_all
             JOIN services s_all ON s_all.id = ts_all.service_id
             WHERE ts_all.track_id = t.id) as availability_summary,
            COALESCE(d.file_format, ts.format) as quality,
            -- FE-10: same classification as get_audio_quality_distribution so the
            -- /library?filter=quality&quality=<label> deep-link matches the
            -- dashboard's quality card counts exactly.
            CASE
                WHEN d.track_id IS NULL THEN NULL
                WHEN COALESCE(d.bit_depth, 0) >= 24 OR COALESCE(d.sample_rate, 0) > 48000 THEN 'Hi-Res (24-bit+)'
                WHEN d.file_format IN ('FLAC', 'ALAC', 'WAV') THEN 'CD Quality'
                ELSE 'Lossy'
            END as quality_bucket,
            CASE
                WHEN d.file_path IS NOT NULL THEN 'downloaded'
                WHEN dq.status = 'queued' OR dq.status = 'downloading' THEN 'queued'
                ELSE 'not_downloaded'
            END as download_status,
            -- Metadata score: 100 points total
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            -- Lyrics type based on sync level
            CASE
                WHEN l.sync_level IN ('syllable', 'word') THEN 'synced'
                WHEN l.sync_level = 'line' THEN 'timed'
                WHEN l.content IS NOT NULL THEN 'plain'
                ELSE 'none'
            END as lyrics_type,
            al.cover_art_url as cover_art_url,
            ts_spot.service_track_id as spotify_track_id,
            -- Extended metadata fields
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            t.display_title,
            COALESCE(t.source_title, t.title) as source_title,
            COALESCE(t.file_disambiguator, d.file_disambiguator) as file_disambiguator
        FROM tracks t
        JOIN (SELECT id FROM tracks ORDER BY title, id LIMIT ? OFFSET ?) p ON p.id = t.id
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        GROUP BY t.id
        ORDER BY t.title ASC
        "#
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    verify_page_files(&mut tracks).await?;
    let has_more = offset + (tracks.len() as i64) < total.0;

    Ok(LibraryPage {
        tracks,
        total: total.0,
        offset,
        limit,
        has_more,
    })
}

/// Get duplicate tracks (by Title + Primary Artist) - paginated
#[tauri::command]
pub async fn get_duplicate_tracks(
    state: State<'_, AppState>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!(
        "get_duplicate_tracks called with offset={}, limit={}",
        offset,
        limit
    );

    // Get total count first
    let total: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM tracks t
        WHERE EXISTS (
            SELECT 1 FROM tracks t2
            JOIN track_artists ta2 ON t2.id = ta2.track_id
            AND ta2.role = 'primary'
            JOIN track_artists ta ON t.id = ta.track_id
            AND ta.role = 'primary'
            WHERE t2.title = t.title
            AND ta2.artist_id = ta.artist_id
            AND t2.id != t.id
        )
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Count error: {}", e))?;

    let mut tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT
            t.id,
            t.title,
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            (SELECT a2.id FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_id,
            al.title as album_name,
            al.id as album_id,
            t.duration_ms,
            t.isrc,
            GROUP_CONCAT(DISTINCT s.name) as services,
            (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
             FROM library_entries le
             JOIN accounts acc ON acc.id = le.account_id
             JOIN services s_imp ON s_imp.id = acc.service_id
             WHERE le.track_id = t.id) as imported_from,
            COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
            (SELECT GROUP_CONCAT(DISTINCT s_avail.name)
             FROM track_sources ts_avail
             JOIN services s_avail ON s_avail.id = ts_avail.service_id
             WHERE ts_avail.track_id = t.id AND ts_avail.availability_status = 'available') as available_services,
            (SELECT GROUP_CONCAT(s_all.name || ':' || COALESCE(ts_all.availability_status, 'unknown_unchecked'), ', ')
             FROM track_sources ts_all
             JOIN services s_all ON s_all.id = ts_all.service_id
             WHERE ts_all.track_id = t.id) as availability_summary,
            COALESCE(d.file_format, ts.format) as quality,
            CASE
                WHEN d.file_path IS NOT NULL THEN 'downloaded'
                WHEN dq.status = 'queued' OR dq.status = 'downloading' THEN 'queued'
                ELSE 'not_downloaded'
            END as download_status,
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            CASE
                WHEN l.sync_level IN ('syllable', 'word') THEN 'synced'
                WHEN l.sync_level = 'line' THEN 'timed'
                WHEN l.content IS NOT NULL THEN 'plain'
                ELSE 'none'
            END as lyrics_type,
            al.cover_art_url as cover_art_url,
            ts_spot.service_track_id as spotify_track_id,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path
        FROM tracks t
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        WHERE EXISTS (
            SELECT 1 FROM tracks t2
            JOIN track_artists ta2 ON t2.id = ta2.track_id
            AND ta2.role = 'primary'
            JOIN track_artists ta ON t.id = ta.track_id
            AND ta.role = 'primary'
            WHERE t2.title = t.title
            AND ta2.artist_id = ta.artist_id
            AND t2.id != t.id
        )
        GROUP BY t.id
        ORDER BY t.title ASC, t.id ASC
        LIMIT ? OFFSET ?
        "#
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    verify_page_files(&mut tracks).await?;
    let has_more = offset + (tracks.len() as i64) < total.0;

    Ok(LibraryPage {
        tracks,
        total: total.0,
        offset,
        limit,
        has_more,
    })
}

/// Get library statistics
#[tauri::command]
pub async fn get_library_stats(state: State<'_, AppState>) -> Result<LibraryStats, String> {
    tracing::info!("get_library_stats called");

    let stats = sqlx::query_as::<_, LibraryStats>("SELECT * FROM library_stats")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(stats)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct TopArtist {
    pub name: String,
    pub track_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct TopGenre {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct QualityBucket {
    pub label: String,
    pub count: i64,
}

/// Get top artists by track count
#[tauri::command]
pub async fn get_top_artists(
    state: tauri::State<'_, AppState>,
    limit: i64,
) -> Result<Vec<TopArtist>, String> {
    let artists = sqlx::query_as::<_, TopArtist>(
        r#"
        SELECT a.name, COUNT(ta.track_id) as track_count
        FROM artists a
        JOIN track_artists ta ON a.id = ta.artist_id
        GROUP BY a.id, a.name
        ORDER BY track_count DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(artists)
}

/// Get top genres by track count
#[tauri::command]
pub async fn get_top_genres(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<TopGenre>, String> {
    let limit = limit.unwrap_or(10);
    let genres = sqlx::query_as::<_, TopGenre>(
        r#"
        SELECT genre as name, COUNT(*) as count
        FROM tracks
        WHERE genre IS NOT NULL AND genre != ''
        GROUP BY genre
        ORDER BY count DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(genres)
}

/// Get audio quality distribution from downloads
#[tauri::command]
pub async fn get_audio_quality_distribution(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QualityBucket>, String> {
    let distribution = sqlx::query_as::<_, QualityBucket>(
        r#"
        SELECT label, COUNT(*) as count FROM (
            SELECT CASE
                WHEN bit_depth >= 24 OR sample_rate > 48000 THEN 'Hi-Res (24-bit+)'
                WHEN file_format IN ('FLAC', 'ALAC', 'WAV') THEN 'CD Quality'
                ELSE 'Lossy'
            END as label
            FROM downloads
        ) sub
        GROUP BY label
        ORDER BY CASE label
            WHEN 'Hi-Res (24-bit+)' THEN 1
            WHEN 'CD Quality' THEN 2
            ELSE 3
        END
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(distribution)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LibraryArtistDetail {
    pub id: i64,
    pub name: String,
    pub bio: Option<String>,
    pub image_url: Option<String>,
    pub album_count: i64,
    pub track_count: i64,
    pub albums: Vec<ArtistAlbum>,
    pub top_tracks: Vec<ArtistTrack>,
    #[serde(default)]
    pub appearances: Vec<ArtistAppearanceTrack>,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow, Clone, Debug, PartialEq, Eq)]
pub struct ArtistAppearanceTrack {
    pub id: i64,
    pub title: String,
    pub album: Option<String>,
    pub album_id: Option<i64>,
    pub album_artist: Option<String>,
    pub duration_ms: Option<i64>,
    pub role: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow, Clone, Debug)]
pub struct ArtistAlbum {
    pub id: i64,
    pub title: String,
    pub cover_url: Option<String>,
    pub release_year: Option<i64>,
    pub track_count: i64,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow, Clone, Debug)]
pub struct ArtistTrack {
    pub id: i64,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

/// Retrieve artist details, albums, and top tracks
#[tauri::command]
pub async fn get_artist(
    state: State<'_, AppState>,
    artist_id: i64,
) -> Result<LibraryArtistDetail, String> {
    fetch_artist(&state.db, artist_id).await
}

pub async fn fetch_artist(
    db: &sqlx::SqlitePool,
    artist_id: i64,
) -> Result<LibraryArtistDetail, String> {
    tracing::info!("fetch_artist called for {}", artist_id);

    // Fetch base artist details
    let artist: (i64, String, Option<String>, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            a.id,
            a.name,
            -- bio doesn't exist in the schema yet; image_url does (migration 0076)
            a.image_url as image_url,
            (SELECT COUNT(*) FROM album_artists WHERE artist_id = a.id) as album_count,
            (SELECT COUNT(*) FROM track_artists WHERE artist_id = a.id) as track_count
        FROM artists a
        WHERE a.id = ?
        "#,
    )
    .bind(artist_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database error fetching artist: {}", e))?
    .ok_or_else(|| "Artist not found".to_string())?;

    // Fetch associated albums
    let albums: Vec<ArtistAlbum> = sqlx::query_as(
        r#"
        SELECT
            al.id,
            al.title,
            al.cover_art_url as cover_url,
            CAST(SUBSTR(al.release_date, 1, 4) AS INTEGER) as release_year,
            (SELECT COUNT(*) FROM tracks WHERE album_id = al.id) as track_count
        FROM albums al
        JOIN album_artists aa ON aa.album_id = al.id
        WHERE aa.artist_id = ?
        ORDER BY al.release_date DESC NULLS LAST, al.title ASC
        "#,
    )
    .bind(artist_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error fetching albums: {}", e))?;

    // Fetch top tracks
    let top_tracks: Vec<ArtistTrack> = sqlx::query_as(
        r#"
        SELECT
            t.id,
            t.title,
            al.title as album,
            t.duration_ms
        FROM tracks t
        JOIN track_artists ta ON ta.track_id = t.id
        LEFT JOIN albums al ON al.id = t.album_id
        WHERE ta.artist_id = ?
        ORDER BY (SELECT COUNT(*) FROM library_entries WHERE track_id = t.id) DESC, t.title ASC
        LIMIT 5
        "#,
    )
    .bind(artist_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error fetching top tracks: {}", e))?;

    let appearances = fetch_artist_appearances(db, artist_id)
        .await
        .unwrap_or_default();

    Ok(LibraryArtistDetail {
        id: artist.0,
        name: artist.1,
        bio: None,
        image_url: artist.2,
        album_count: artist.3,
        track_count: artist.4,
        albums,
        top_tracks,
        appearances,
    })
}

pub async fn fetch_artist_appearances(
    db: &sqlx::SqlitePool,
    artist_id: i64,
) -> Result<Vec<ArtistAppearanceTrack>, String> {
    sqlx::query_as(
        r#"
        SELECT DISTINCT
            t.id,
            t.title,
            al.title as album,
            al.id as album_id,
            (SELECT GROUP_CONCAT(ar.name, ', ')
             FROM artists ar
             JOIN album_artists aa ON ar.id = aa.artist_id
             WHERE aa.album_id = al.id AND aa.is_primary = 1) as album_artist,
            t.duration_ms,
            ta.role
        FROM tracks t
        JOIN track_artists ta ON ta.track_id = t.id AND ta.artist_id = ?
        LEFT JOIN albums al ON al.id = t.album_id
        WHERE ta.role = 'featured'
           OR (
               t.album_id IS NOT NULL
               AND EXISTS (SELECT 1 FROM album_artists aa WHERE aa.album_id = t.album_id)
               AND NOT EXISTS (
                   SELECT 1 FROM album_artists aa
                   WHERE aa.album_id = t.album_id AND aa.artist_id = ?
               )
           )
        ORDER BY al.release_date DESC NULLS LAST, t.title ASC
        "#,
    )
    .bind(artist_id)
    .bind(artist_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error fetching appearances: {}", e))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LibraryAlbumDetail {
    pub id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub release_year: Option<i64>,
    pub cover_art_url: Option<String>,
    pub track_count: i64,
    pub total_duration_ms: i64,
    pub genre: Option<String>,
    pub tracks: Vec<AlbumTrack>,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow, Clone, Debug, PartialEq, Eq)]
pub struct AlbumTrack {
    pub id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub duration_ms: Option<i64>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
}

/// Retrieve album details and tracks
#[tauri::command]
pub async fn get_album(
    state: State<'_, AppState>,
    album_id: i64,
) -> Result<LibraryAlbumDetail, String> {
    fetch_album(&state.db, album_id).await
}

pub async fn fetch_album(
    db: &sqlx::SqlitePool,
    album_id: i64,
) -> Result<LibraryAlbumDetail, String> {
    tracing::info!("get_album called for {}", album_id);

    // Fetch base album details
    let album: (
        i64,
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
        i64,
        Option<i64>,
        Option<String>,
    ) = sqlx::query_as(
        r#"
        SELECT
            al.id,
            al.title,
            (SELECT GROUP_CONCAT(ar.name, ', ')
             FROM artists ar
             JOIN album_artists aa ON ar.id = aa.artist_id
             WHERE aa.album_id = al.id) as artist_name,
            CAST(SUBSTR(al.release_date, 1, 4) AS INTEGER) as release_year,
            al.cover_art_url,
            (SELECT COUNT(*) FROM tracks WHERE album_id = al.id) as track_count,
            (SELECT SUM(duration_ms) FROM tracks WHERE album_id = al.id) as total_duration_ms,
            (SELECT genre FROM tracks WHERE album_id = al.id LIMIT 1) as genre
        FROM albums al
        WHERE al.id = ?
        "#,
    )
    .bind(album_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database error fetching album: {}", e))?
    .ok_or_else(|| "Album not found".to_string())?;

    // Fetch album tracks
    let tracks: Vec<AlbumTrack> = sqlx::query_as(
        r#"
        SELECT
            t.id,
            t.title,
            (SELECT GROUP_CONCAT(ar.name, ', ')
             FROM artists ar
             JOIN track_artists ta ON ar.id = ta.artist_id
             WHERE ta.track_id = t.id) as artist_name,
            t.duration_ms,
            t.track_number,
            t.disc_number
        FROM tracks t
        WHERE t.album_id = ?
        ORDER BY COALESCE(t.disc_number, 1) ASC, COALESCE(t.track_number, 999) ASC, t.title ASC
        "#,
    )
    .bind(album_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error fetching album tracks: {}", e))?;

    Ok(LibraryAlbumDetail {
        id: album.0,
        title: album.1,
        artist_name: album.2,
        release_year: album.3,
        cover_art_url: album.4,
        track_count: album.5,
        total_duration_ms: album.6.unwrap_or(0),
        genre: album.7,
        tracks,
    })
}

/// Get tracks in a playlist - paginated
#[tauri::command]
pub async fn get_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    tracing::info!(
        "get_playlist_tracks called for playlist_id={}, offset={}, limit={}",
        playlist_id,
        offset,
        limit
    );

    // Get total count first
    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| format!("Count error: {}", e))?;

    let tracks = fetch_local_playlist_tracks_page(&state.db, playlist_id, offset, limit).await?;

    let has_more = offset + (tracks.len() as i64) < total.0;

    Ok(LibraryPage {
        tracks,
        total: total.0,
        offset,
        limit,
        has_more,
    })
}

/// Alias for get_playlist_tracks for backwards compatibility
#[tauri::command]
pub async fn get_local_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    get_playlist_tracks(state, playlist_id, offset, limit).await
}

/// S201: page fetch shared by `get_playlist_tracks` and `get_local_playlist_tracks`.
///
/// Extracted so integration tests can execute the REAL SQL against an
/// in-memory schema and catch any drift between the SELECT column list and
/// the required fields of `LibraryTrack` (FromRow decode happens here).
pub async fn fetch_local_playlist_tracks_page(
    db: &sqlx::SqlitePool,
    playlist_id: i64,
    offset: i64,
    limit: i64,
) -> Result<Vec<LibraryTrack>, String> {
    let mut tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT
            t.id,
            pt.position as position,
            t.title,
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            (SELECT a2.id FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_id,
            al.title as album_name,
            al.id as album_id,
            t.duration_ms,
            t.isrc,
            GROUP_CONCAT(DISTINCT s.name) as services,
            (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
             FROM library_entries le
             JOIN accounts acc ON acc.id = le.account_id
             JOIN services s_imp ON s_imp.id = acc.service_id
             WHERE le.track_id = t.id) as imported_from,
            COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
            (SELECT GROUP_CONCAT(DISTINCT s_avail.name)
             FROM track_sources ts_avail
             JOIN services s_avail ON s_avail.id = ts_avail.service_id
             WHERE ts_avail.track_id = t.id AND ts_avail.availability_status = 'available') as available_services,
            (SELECT GROUP_CONCAT(s_all.name || ':' || COALESCE(ts_all.availability_status, 'unknown_unchecked'), ', ')
             FROM track_sources ts_all
             JOIN services s_all ON s_all.id = ts_all.service_id
             WHERE ts_all.track_id = t.id) as availability_summary,
            COALESCE(d.file_format, ts.format) as quality,
            CASE
                WHEN d.file_path IS NOT NULL THEN 'downloaded'
                WHEN dq.status = 'queued' OR dq.status = 'downloading' THEN 'queued'
                ELSE 'not_downloaded'
            END as download_status,
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            CASE
                WHEN l.sync_level IN ('syllable', 'word') THEN 'synced'
                WHEN l.sync_level = 'line' THEN 'timed'
                WHEN l.content IS NOT NULL THEN 'plain'
                ELSE 'none'
            END as lyrics_type,
            al.cover_art_url as cover_art_url,
            ts_spot.service_track_id as spotify_track_id,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            t.display_title,
            COALESCE(t.source_title, t.title) as source_title,
            COALESCE(t.file_disambiguator, d.file_disambiguator) as file_disambiguator
        FROM tracks t
        INNER JOIN playlist_tracks pt ON pt.track_id = t.id
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        WHERE pt.playlist_id = ?
        GROUP BY pt.id, pt.position
        ORDER BY pt.position ASC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(playlist_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;
    verify_page_files(&mut tracks).await?;
    Ok(tracks)
}

/// Enrich track metadata using MusicBrainz ISRC lookups (batch) with progress events
/// Uses batch queries to process ~20 tracks per request for faster enrichment
#[tauri::command]
pub async fn enrich_metadata_musicbrainz(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<crate::services::musicbrainz::EnrichmentResult, String> {
    use crate::services::MusicBrainzClient;
    use tauri::Emitter;

    const BATCH_SIZE: usize = 20; // Process 20 tracks per batch

    tracing::info!("enrich_metadata_musicbrainz called with limit={:?}", limit);

    let client = MusicBrainzClient::with_db(state.db.clone());
    let limit = limit.unwrap_or(100000); // Process all tracks

    // Find tracks with ISRC but no MusicBrainz ID, and fetch their artist name for validation
    let tracks: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT t.id, t.isrc, t.title, a.name as artist_name
        FROM tracks t
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists a ON a.id = ta.artist_id
        WHERE t.isrc IS NOT NULL AND t.isrc != '' AND t.musicbrainz_id IS NULL
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let total = tracks.len();
    let mut enriched = 0;
    let mut not_found = 0;
    let mut failed = 0;
    let num_batches = total.div_ceil(BATCH_SIZE);

    // Emit initial event
    let _ = app.emit(
        "enrichment-progress",
        serde_json::json!({
            "status": "started",
            "total": total,
            "current": 0,
            "enriched": 0,
            "failed": 0,
            "currentTrack": format!("Processing {} tracks in {} batches", total, num_batches)
        }),
    );

    // Process in batches
    for (batch_idx, batch) in tracks.chunks(BATCH_SIZE).enumerate() {
        let batch_start = batch_idx * BATCH_SIZE;

        // Emit batch progress
        let _ = app.emit(
            "enrichment-progress",
            serde_json::json!({
                "status": "processing",
                "total": total,
                "current": batch_start + batch.len(),
                "enriched": enriched,
                "failed": failed,
                "currentTrack": format!("Batch {}/{}", batch_idx + 1, num_batches)
            }),
        );

        // Rate limit between batches
        if batch_idx > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        }

        // Collect ISRCs for this batch
        let isrcs: Vec<String> = batch.iter().map(|(_, isrc, _, _)| isrc.clone()).collect();

        // Query MusicBrainz for all ISRCs
        match client.batch_lookup_by_isrc(&isrcs).await {
            Ok(results) => {
                tracing::info!(
                    "Batch {} returned {} results for {} ISRCs",
                    batch_idx + 1,
                    results.len(),
                    batch.len()
                );

                if results.is_empty() {
                    // No matches for any ISRC - mark all as NOT_FOUND
                    for (track_id, _, _, _) in batch {
                        let _ = sqlx::query(
                            "UPDATE tracks SET musicbrainz_id = 'NOT_FOUND' WHERE id = ?",
                        )
                        .bind(track_id)
                        .execute(&state.db)
                        .await;
                        not_found += 1;
                    }
                } else {
                    // Individual lookups for precise matching + validation
                    for (track_id, isrc, title, db_artist) in batch {
                        match client.lookup_by_isrc(isrc).await {
                            Ok(Some(recording)) => {
                                // Validate Artist Match
                                let mut is_match = true;
                                if let Some(ref db_artist_name) = db_artist {
                                    // If we have a local artist, verify it matches the MB result
                                    if let Some(credits) = &recording.artist_credit {
                                        let mb_artists: String = credits
                                            .iter()
                                            .map(|c| c.name.to_lowercase())
                                            .collect::<Vec<_>>()
                                            .join(" ");

                                        let local_artist = db_artist_name.to_lowercase();

                                        // Simple validation: check if one contains the other
                                        // "The Beatles" vs "Beatles" -> Match
                                        // "Queen" vs "Queen & David Bowie" -> Match
                                        // "Tori Amos" vs "Sam Cooke" -> No Match
                                        // Fallback: if local artist is "Unknown Artist" or similar, skip validation
                                        if !local_artist.contains("unknown")
                                            && !mb_artists.contains(&local_artist)
                                            && !local_artist.contains(&mb_artists)
                                        {
                                            tracing::warn!(
                                                "Metadata Mismatch for track '{}': Local Artist '{}' != MB Artist '{}'. ISRC: {}",
                                                title, db_artist_name, mb_artists, isrc
                                            );
                                            is_match = false;
                                        }
                                    }
                                }

                                if is_match {
                                    // Fetch details (genres, isrcs)
                                    let mut genre = None;
                                    if let Ok(detail) =
                                        client.get_recording_details(&recording.id).await
                                    {
                                        if let Some(genres) = detail.genres {
                                            if !genres.is_empty() {
                                                genre = Some(genres[0].name.clone());
                                            }
                                        }
                                    }

                                    let result = sqlx::query(
                                        "UPDATE tracks SET
                                            musicbrainz_id = ?,
                                            genre = COALESCE(genre, ?)
                                         WHERE id = ?",
                                    )
                                    .bind(&recording.id)
                                    .bind(&genre)
                                    .bind(track_id)
                                    .execute(&state.db)
                                    .await;

                                    if result.is_ok() {
                                        enriched += 1;
                                    }
                                } else {
                                    // Mismatch - treat as not found to avoid bad data
                                    let _ = sqlx::query("UPDATE tracks SET musicbrainz_id = 'MISMATCH' WHERE id = ?")
                                        .bind(track_id)
                                        .execute(&state.db)
                                        .await;
                                    failed += 1;
                                }
                            }
                            Ok(None) => {
                                let _ = sqlx::query(
                                    "UPDATE tracks SET musicbrainz_id = 'NOT_FOUND' WHERE id = ?",
                                )
                                .bind(track_id)
                                .execute(&state.db)
                                .await;
                                not_found += 1;
                            }
                            Err(_) => {
                                failed += 1;
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Batch lookup failed: {}", e);
                failed += batch.len();
            }
        }
    }

    // Emit completion event
    let _ = app.emit(
        "enrichment-progress",
        serde_json::json!({
            "status": "completed",
            "total": total,
            "current": total,
            "enriched": enriched,
            "failed": failed,
            "currentTrack": ""
        }),
    );

    tracing::info!(
        "Enriched {}/{} tracks ({} not found, {} failed)",
        enriched,
        total,
        not_found,
        failed
    );

    Ok(crate::services::musicbrainz::EnrichmentResult {
        total,
        enriched,
        failed,
    })
}

/// Repair missing artist links for tracks without track_artists entries
#[tauri::command]
pub async fn repair_artist_links(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    tracing::info!("repair_artist_links called");

    // Find tracks without artist links that have a track_source with an artist in the service
    // For now, create "Unknown Artist" entries for tracks without artists

    // First, find all tracks with no track_artists entry
    let orphan_tracks: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT t.id, t.title FROM tracks t
        LEFT JOIN track_artists ta ON ta.track_id = t.id
        WHERE ta.track_id IS NULL
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let total = orphan_tracks.len();
    tracing::info!("{} tracks found without artist links", total);

    if total == 0 {
        return Ok(serde_json::json!({
            "total": 0,
            "repaired": 0,
            "message": "All tracks have artist links"
        }));
    }

    // Get or create "Unknown Artist" as fallback
    // But actually, let's check if there's an album with an artist we can use
    let mut repaired = 0;

    for (track_id, title) in &orphan_tracks {
        // Try to find an artist via the album
        let album_artist: Option<(i64, String)> = sqlx::query_as(
            r#"
            SELECT a.id, a.name FROM tracks t
            JOIN albums al ON al.id = t.album_id
            JOIN album_artists aa ON aa.album_id = al.id
            JOIN artists a ON a.id = aa.artist_id
            WHERE t.id = ?
            LIMIT 1
            "#,
        )
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

        let artist_id = if let Some((aid, _aname)) = album_artist {
            aid
        } else {
            // Create "Unknown Artist" if needed
            let unknown: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM artists WHERE name = 'Unknown Artist'")
                    .fetch_optional(&state.db)
                    .await
                    .unwrap_or(None);

            if let Some((aid,)) = unknown {
                aid
            } else {
                let aid: i64 = sqlx::query_scalar(
                    "INSERT INTO artists (name) VALUES ('Unknown Artist')
                     ON CONFLICT(name) DO UPDATE SET id = id
                     RETURNING id",
                )
                .fetch_one(&state.db)
                .await
                .map_err(|e| format!("DB error: {}", e))?;
                aid
            }
        };

        // Link artist to track
        let result = sqlx::query(
            "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(&state.db)
        .await;

        if result.is_ok() {
            repaired += 1;
            tracing::debug!(
                "Linked track {} ({}) to artist {}",
                track_id,
                title,
                artist_id
            );
        }
    }

    tracing::info!("Repaired {} of {} tracks", repaired, total);

    Ok(serde_json::json!({
        "total": total,
        "repaired": repaired,
        "message": format!("Repaired {} track artist links", repaired)
    }))
}

/// Helper function to reset/delete the entire library database transactionally
pub async fn perform_reset_database(db: &sqlx::SqlitePool) -> Result<String, String> {
    tracing::warn!("perform_reset_database called - clearing library data!");

    // Execute deletions in a transaction to ensure atomicity
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // Order matters (referential integrity), though defer_foreign_keys could be used.
    // We delete leaf nodes first, then parents.

    // 1. Delete user library and playlist data
    sqlx::query("DELETE FROM library_entries")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM playlist_tracks")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM playlists")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 1b. Favorites, import bookkeeping and dashboard history. These tables are
    // not children of tracks/albums, so deleting the library would leave stale
    // favorites, cached totals and import counters behind (0045/0099/0015).
    sqlx::query("DELETE FROM favorites")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM favorites_cache")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM account_import_counts")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM library_snapshots")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Delete downloads and queue
    sqlx::query("DELETE FROM download_queue")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM downloads")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Delete metadata and relationships
    sqlx::query("DELETE FROM lyrics")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM track_sources")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM track_artists")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM album_artists")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 4. Delete core entities (Tracks -> Albums -> Artists)
    sqlx::query("DELETE FROM tracks")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM albums")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM artists")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 5. Reset auto-increment counters for cleanliness (optional but nice)
    // (the sync_log cleanup step was removed: 0085 dropped the dead table)
    sqlx::query("DELETE FROM sqlite_sequence WHERE name IN ('tracks', 'albums', 'artists', 'playlists', 'download_queue', 'favorites', 'library_snapshots')")
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    tracing::info!("Library data cleared successfully");
    Ok("Library data has been reset. Accounts and settings were preserved.".to_string())
}

/// Reset/delete the entire database (requires app restart)
#[tauri::command]
pub async fn reset_database(state: State<'_, AppState>) -> Result<String, String> {
    perform_reset_database(&state.db).await
}

/// Search tracks using FTS5 with pagination
#[tauri::command]
pub async fn search_tracks(
    state: State<'_, AppState>,
    query: String,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<SearchResult, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100);
    tracing::info!(
        "search_tracks called: {} (offset={}, limit={})",
        query,
        offset,
        limit
    );

    // Get total count of matching tracks
    let pattern = format!("%{}%", query);
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tracks WHERE id IN (SELECT rowid FROM library_fts WHERE library_fts MATCH ?) OR display_title LIKE ? OR title LIKE ?"
    )
    .bind(&query)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));

    let mut tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT
            t.id,
            t.title,
            -- Only get primary artist name (avoid duplicates from featured artists)
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            (SELECT a2.id FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_id,
            al.title as album_name,
            al.id as album_id,
            t.duration_ms,
            t.isrc,
            GROUP_CONCAT(DISTINCT s.name) as services,
            (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
             FROM library_entries le
             JOIN accounts acc ON acc.id = le.account_id
             JOIN services s_imp ON s_imp.id = acc.service_id
             WHERE le.track_id = t.id) as imported_from,
            COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
            (SELECT GROUP_CONCAT(DISTINCT s_avail.name)
             FROM track_sources ts_avail
             JOIN services s_avail ON s_avail.id = ts_avail.service_id
             WHERE ts_avail.track_id = t.id AND ts_avail.availability_status = 'available') as available_services,
            (SELECT GROUP_CONCAT(s_all.name || ':' || COALESCE(ts_all.availability_status, 'unknown_unchecked'), ', ')
             FROM track_sources ts_all
             JOIN services s_all ON s_all.id = ts_all.service_id
             WHERE ts_all.track_id = t.id) as availability_summary,
            COALESCE(d.file_format, ts.format) as quality,
            CASE
                WHEN d.file_path IS NOT NULL THEN 'downloaded'
                WHEN dq.status = 'queued' OR dq.status = 'downloading' THEN 'queued'
                ELSE 'not_downloaded'
            END as download_status,
            -- Metadata score: 100 points total
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            -- Lyrics type based on sync level
            CASE
                WHEN l.sync_level IN ('syllable', 'word') THEN 'synced'
                WHEN l.sync_level = 'line' THEN 'timed'
                WHEN l.content IS NOT NULL THEN 'plain'
                ELSE 'none'
            END as lyrics_type,
            al.cover_art_url as cover_art_url,
            ts_spot.service_track_id as spotify_track_id,
            -- Extended metadata fields
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            t.display_title,
            COALESCE(t.source_title, t.title) as source_title,
            COALESCE(t.file_disambiguator, d.file_disambiguator) as file_disambiguator
        FROM tracks t
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        WHERE (t.id IN (SELECT rowid FROM library_fts WHERE library_fts MATCH ?) OR t.display_title LIKE ? OR t.title LIKE ?)
        GROUP BY t.id
        ORDER BY t.title ASC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(&query)
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Search error: {}", e))?;

    verify_page_files(&mut tracks).await?;
    let has_more = offset + (tracks.len() as i64) < total.0;
    tracing::info!(
        "search_tracks returned {} results (total: {}, has_more: {})",
        tracks.len(),
        total.0,
        has_more
    );

    Ok(SearchResult {
        tracks,
        total: total.0,
        offset,
        limit,
        has_more,
    })
}

/// Get all playlists for the user
#[tauri::command]
pub async fn get_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    tracing::info!("get_playlists called");

    let playlists = sqlx::query_as::<_, Playlist>(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner_name,
            p.track_count,
            COALESCE(
                p.image_url,
                (
                    SELECT al.cover_art_url
                    FROM playlist_tracks pt
                    JOIN tracks t ON t.id = pt.track_id
                    LEFT JOIN albums al ON al.id = t.album_id
                    WHERE pt.playlist_id = p.id
                      AND al.cover_art_url IS NOT NULL
                      AND al.cover_art_url != ''
                    ORDER BY pt.position ASC
                    LIMIT 1
                )
            ) as image_url,
            COALESCE(p.source_service, s.name) as service_name,
            p.is_smart,
            p.rules_json
        FROM playlists p
        LEFT JOIN accounts a ON a.id = p.account_id
        LEFT JOIN services s ON s.id = a.service_id
        ORDER BY p.name ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(playlists)
}

/// Add tracks to a playlist
#[tauri::command]
pub async fn add_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<String, String> {
    tracing::info!(
        "add_to_playlist: {} tracks to playlist {}",
        track_ids.len(),
        playlist_id
    );

    let mut added = 0;
    for track_id in track_ids.iter() {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position, added_at)
            VALUES (?, ?, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?), CURRENT_TIMESTAMP)
            "#
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind(playlist_id)
        .execute(&state.db)
        .await;

        if result.is_ok() {
            added += 1;
        }
    }

    // Update track count
    let _ = sqlx::query(
        "UPDATE playlists SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?) WHERE id = ?"
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&state.db)
    .await;

    Ok(format!("Added {} tracks to playlist", added))
}

/// Create a new playlist
#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    account_id: Option<i64>,
    name: String,
    description: Option<String>,
) -> Result<i64, String> {
    let account_id = account_id.unwrap_or(1);
    tracing::info!("create_playlist: {} for account {}", name, account_id);

    let playlist_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO playlists (account_id, service_playlist_id, name, description, track_count)
           VALUES (?, 'local_' || ?, ?, ?, 0)
           RETURNING id"#,
    )
    .bind(account_id)
    .bind(&name)
    .bind(&name)
    .bind(&description)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Failed to create playlist: {}", e))?;

    Ok(playlist_id)
}

/// BD-6: `playlist_tracks.track_id` is `ON DELETE CASCADE` (0064), so deleting a
/// track cascades its relation away and leaves both `playlists.track_count` and
/// the `position` sequence stale — the counter is the one the UI shows.
/// `add_to_playlist` refreshed it after every insert; the removals did not, and
/// `reconcile_playlist_track_counts` (the repair that already existed for
/// exactly these two symptoms) had no production caller.
async fn refresh_playlist_track_counts(db: &crate::DbPool) -> Result<usize, String> {
    crate::commands::reconcile_playlist_track_counts(db).await
}

/// Remove a single track from the library (cascading deletes via FK)
#[tauri::command]
pub async fn remove_track(state: State<'_, AppState>, track_id: i64) -> Result<(), String> {
    tracing::info!("remove_track called: track_id={}", track_id);

    let result = sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(track_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to remove track: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Track {} not found", track_id));
    }

    // The track is already gone at this point, so a failed recount is reported
    // but must not turn a successful removal into an error.
    if let Err(e) = refresh_playlist_track_counts(&state.db).await {
        tracing::warn!("Playlist counters not refreshed: {}", e);
    }

    tracing::info!("Track {} removed successfully", track_id);
    Ok(())
}

/// Bulk remove tracks from the library
#[tauri::command]
pub async fn bulk_remove_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<usize, String> {
    tracing::info!("bulk_remove_tracks called: {} tracks", track_ids.len());

    if track_ids.is_empty() {
        return Ok(0);
    }

    // Build parameterized IN clause
    let placeholders: Vec<String> = track_ids.iter().map(|_| "?".to_string()).collect();
    let query_str = format!(
        "DELETE FROM tracks WHERE id IN ({})",
        placeholders.join(",")
    );

    let mut query = sqlx::query(&query_str);
    for id in &track_ids {
        query = query.bind(id);
    }

    let result = query
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to bulk remove tracks: {}", e))?;

    let removed = result.rows_affected() as usize;
    if let Err(e) = refresh_playlist_track_counts(&state.db).await {
        tracing::warn!("Playlist counters not refreshed: {}", e);
    }
    tracing::info!("Bulk removed {} tracks", removed);
    Ok(removed)
}

/// Toggle the favorite status of a track (atomic via RETURNING with timestamp update)
#[tauri::command]
pub async fn toggle_favorite(state: State<'_, AppState>, track_id: i64) -> Result<bool, String> {
    tracing::info!("toggle_favorite called: track_id={}", track_id);

    if track_id <= 0 {
        return Err(format!("Invalid track_id: {}", track_id));
    }

    // Atomic toggle + timestamp update + read in one query (RETURNING requires SQLite 3.35+)
    let result: Option<(i32,)> = sqlx::query_as(
        "UPDATE tracks \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite",
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to toggle favorite: {}", e))?;

    let is_favorite = result
        .map(|(v,)| v != 0)
        .ok_or_else(|| format!("Track {} not found", track_id))?;

    tracing::info!("Track {} favorite toggled to {}", track_id, is_favorite);
    Ok(is_favorite)
}

/// Explicitly set the favorite status of a track
#[tauri::command]
pub async fn set_track_favorite(
    state: State<'_, AppState>,
    track_id: i64,
    is_favorite: bool,
) -> Result<bool, String> {
    tracing::info!(
        "set_track_favorite called: track_id={}, is_favorite={}",
        track_id,
        is_favorite
    );

    if track_id <= 0 {
        return Err(format!("Invalid track_id: {}", track_id));
    }

    // S168: Acquire CanonicalTrack lock
    let _track_guard = state
        .concurrency_manager
        .acquire(
            syncify_core_domain::LockScope::CanonicalTrack(track_id),
            Some(&format!("fav-{}", track_id)),
            None,
        )
        .await
        .map_err(|e| format!("Concurrency lock error: {}", e))?;

    let val = if is_favorite { 1 } else { 0 };
    let fav_at_expr = if is_favorite {
        "datetime('now')"
    } else {
        "NULL"
    };

    let query_str = format!(
        "UPDATE tracks SET is_favorite = ?, favorite_at = {} WHERE id = ? RETURNING is_favorite",
        fav_at_expr
    );

    let result: Option<(i32,)> = sqlx::query_as(&query_str)
        .bind(val)
        .bind(track_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Failed to set favorite: {}", e))?;

    let res_fav = result
        .map(|(v,)| v != 0)
        .ok_or_else(|| format!("Track {} not found", track_id))?;

    Ok(res_fav)
}

/// Route 2 (2026-10-18): the favorites view expands each favorited provider item
/// into its own row so every release is visible ("100% Pure Love" on both the
/// Best-Of and the Storyteller editions). Item rows come from the `favorites`
/// table (written by the sync for each favorited item) mapped to its canonical
/// track through `track_sources` (slot items) or the ED ledger (duplicate
/// editions), grouped by (track, release title). Favorited tracks with no mapped
/// items keep their single canonical row.
const FAVORITE_ROWS_CTE: &str = r#"
    WITH mapped AS (
        SELECT f.service_id,
               f.service_item_id,
               COALESCE(ts.track_id, le.track_id) AS track_id,
               NULLIF(TRIM(f.album_name), '') AS item_album
        FROM favorites f
        LEFT JOIN track_sources ts
               ON ts.service_id = f.service_id
              AND ts.service_track_id = f.service_item_id
        LEFT JOIN (
            SELECT item.service_id, item.service_track_id, link.track_id
            FROM edition_provider_items item
            JOIN edition_item_tracks link ON link.item_id = item.id
        ) le
               ON le.service_id = f.service_id
              AND le.service_track_id = f.service_item_id
        WHERE f.item_type = 'track'
    ),
    favorite_rows AS (
        SELECT t.id AS track_id,
               LOWER(TRIM(COALESCE(m.item_album, al_canon.title))) AS album_key,
               COALESCE(MIN(m.item_album), al_canon.title) AS display_album,
               MIN(m.service_item_id) AS edition_item_id,
               t.favorite_at AS favorite_at,
               t.title AS sort_title
        FROM mapped m
        JOIN tracks t ON t.id = m.track_id AND t.is_favorite = 1
        LEFT JOIN albums al_canon ON al_canon.id = t.album_id
        GROUP BY t.id, album_key
        UNION ALL
        SELECT t.id,
               '',
               al_canon.title,
               NULL,
               t.favorite_at,
               t.title
        FROM tracks t
        LEFT JOIN albums al_canon ON al_canon.id = t.album_id
        WHERE t.is_favorite = 1
          AND NOT EXISTS (SELECT 1 FROM mapped m2 WHERE m2.track_id = t.id)
    )
"#;

/// Get all favorite tracks in the library - paginated
#[tauri::command]
pub async fn get_favorite_tracks(
    state: State<'_, AppState>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    query_favorite_tracks(&state.db, offset, limit).await
}

/// Route 2 (2026-10-18): shared body of `get_favorite_tracks` so the expanded
/// per-release rows can be exercised without a full Tauri `AppState`.
pub(crate) async fn query_favorite_tracks(
    db: &DbPool,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    let total: (i64,) = sqlx::query_as(&format!(
        "{FAVORITE_ROWS_CTE} SELECT COUNT(*) FROM favorite_rows"
    ))
    .fetch_one(db)
    .await
    .map_err(|e| format!("Count error: {}", e))?;

    let favorites_query = format!(
        r#"{FAVORITE_ROWS_CTE}
        SELECT
            t.id,
            t.title,
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            (SELECT a2.id FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_id,
            p.display_album as album_name,
            al.id as album_id,
            p.edition_item_id as edition_item_id,
            t.duration_ms,
            t.isrc,
            GROUP_CONCAT(DISTINCT s.name) as services,
            (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
             FROM library_entries le
             JOIN accounts acc ON acc.id = le.account_id
             JOIN services s_imp ON s_imp.id = acc.service_id
             WHERE le.track_id = t.id) as imported_from,
            COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
            (SELECT GROUP_CONCAT(DISTINCT s_avail.name)
             FROM track_sources ts_avail
             JOIN services s_avail ON s_avail.id = ts_avail.service_id
             WHERE ts_avail.track_id = t.id AND ts_avail.availability_status = 'available') as available_services,
            (SELECT GROUP_CONCAT(s_all.name || ':' || COALESCE(ts_all.availability_status, 'unknown_unchecked'), ', ')
             FROM track_sources ts_all
             JOIN services s_all ON s_all.id = ts_all.service_id
             WHERE ts_all.track_id = t.id) as availability_summary,
            COALESCE(d.file_format, ts.format) as quality,
            CASE
                WHEN d.file_path IS NOT NULL THEN 'downloaded'
                WHEN dq.status = 'queued' OR dq.status = 'downloading' THEN 'queued'
                ELSE 'not_downloaded'
            END as download_status,
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            CASE
                WHEN l.sync_level IN ('syllable', 'word') THEN 'synced'
                WHEN l.sync_level = 'line' THEN 'timed'
                WHEN l.content IS NOT NULL THEN 'plain'
                ELSE 'none'
            END as lyrics_type,
            al.cover_art_url as cover_art_url,
            ts_spot.service_track_id as spotify_track_id,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path
        FROM tracks t
        JOIN (SELECT track_id, display_album, edition_item_id FROM favorite_rows
              ORDER BY favorite_at DESC, sort_title ASC, display_album ASC, edition_item_id ASC
              LIMIT ? OFFSET ?) p ON p.track_id = t.id
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        GROUP BY t.id, p.display_album, p.edition_item_id
        ORDER BY t.favorite_at DESC, t.title ASC, p.display_album ASC, p.edition_item_id ASC
        "#
    );
    let mut tracks = sqlx::query_as::<_, LibraryTrack>(&favorites_query)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    verify_page_files(&mut tracks).await?;
    let has_more = offset + (tracks.len() as i64) < total.0;

    Ok(LibraryPage {
        tracks,
        total: total.0,
        offset,
        limit,
        has_more,
    })
}

/// Open the system file explorer and reveal the track's file
/// Uses the `opener` crate for cross-platform file revealing.
/// Handles paths with spaces, headless environments, and
/// missing file managers without platform-specific #[cfg] blocks.
#[tauri::command]
pub async fn show_in_folder(state: State<'_, AppState>, track_id: i64) -> Result<(), String> {
    tracing::info!("show_in_folder called: track_id={}", track_id);

    // Get file path from downloads table
    let file_path: Option<(String,)> =
        sqlx::query_as("SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1")
            .bind(track_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

    let path = match file_path {
        Some((p,)) => p,
        None => return Err("Track has not been downloaded yet".to_string()),
    };

    // Verify the file exists on disk
    let path_buf = std::path::Path::new(&path);
    if !path_buf.exists() {
        return Err(format!("File not found on disk: {}", path));
    }

    // opener::reveal selects the file in the native file explorer:
    // - Windows: explorer /select,"path" (handles spaces)
    // - macOS: open -R "path"
    // - Linux: falls back gracefully if no file manager available
    opener::reveal(path_buf).map_err(|e| format!("Failed to reveal file in explorer: {}", e))?;

    Ok(())
}

/// Extensions a sidecar path may have to be handed to the OS default handler.
/// The list is deliberately narrow: this command must not become a generic
/// "execute any local path" entry point reachable from the webview.
const OPENABLE_SIDECAR_EXTENSIONS: [&str; 7] = ["pdf", "jpg", "jpeg", "png", "webp", "mp4", "lrc"];

/// Open a local sidecar file (booklet PDF, cover image/video, lyrics) with the
/// system's default application. Only existing files with a known sidecar
/// extension are accepted; everything else is refused with an honest message.
#[tauri::command]
pub async fn open_local_sidecar(path: String) -> Result<(), String> {
    let trimmed = path.trim().to_string();
    if trimmed.is_empty() {
        return Err("Ruta vacía: no hay archivo que abrir".to_string());
    }

    let candidate = std::path::Path::new(&trimmed);
    if !candidate.is_file() {
        return Err(format!("El archivo no existe: {}", trimmed));
    }

    let extension = candidate
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !OPENABLE_SIDECAR_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "Extensión no permitida para abrir externamente: .{}",
            extension
        ));
    }

    tracing::info!("open_local_sidecar: abriendo {}", trimmed);
    opener::open(candidate).map_err(|e| format!("No se pudo abrir el archivo: {}", e))
}

#[cfg(test)]
mod library_tests {
    // Note: These tests validate SQL logic using in-memory SQLite.
    // They do NOT use Tauri State<> — they test the raw SQL operations.

    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        // Run all migrations
        if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
            panic!("Migration failed in test: {}", e);
        }
        pool
    }

    // ---- Route 2 (2026-10-18): one row per favorited release ----

    /// Seeds: canonical track "100% Pure Love" favorited on Tidal via `item-a`
    /// (release "The Best Of"), with a live `track_sources` slot and a
    /// `favorites` row. Returns (service_id, account_id, track_id).
    async fn seed_favorite_fixture(pool: &SqlitePool) -> (i64, i64, i64) {
        let sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
            .fetch_one(pool)
            .await
            .unwrap();
        let account: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'fav-fixture', 1) RETURNING id",
        )
        .bind(sid)
        .fetch_one(pool)
        .await
        .unwrap();
        let artist: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
                .fetch_one(pool)
                .await
                .unwrap();
        let album: i64 =
            sqlx::query_scalar("INSERT INTO albums (title) VALUES ('The Best Of') RETURNING id")
                .fetch_one(pool)
                .await
                .unwrap();
        let track: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, is_favorite, favorite_at) VALUES ('100% Pure Love', ?, 1, '2024-01-01T00:00:00Z') RETURNING id",
        )
        .bind(album)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track)
        .bind(artist)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) VALUES (?, ?, 'item-a', 'FLAC', 1, 'available')",
        )
        .bind(track)
        .bind(sid)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, album_name) VALUES (?, ?, 'track', 'item-a', '100% Pure Love', 'The Best Of')",
        )
        .bind(account)
        .bind(sid)
        .execute(pool)
        .await
        .unwrap();
        (sid, account, track)
    }

    #[tokio::test]
    async fn favorite_duplicate_release_yields_one_row_per_release() {
        let pool = setup_test_db().await;
        let (sid, account, track) = seed_favorite_fixture(&pool).await;

        // Second favorited item: same recording on another release, linked
        // through the ED ledger (the legacy slot keeps item-a).
        let release_b: i64 = sqlx::query_scalar(
            "INSERT INTO edition_releases (service_id, provider_release_id, evidence_status, provenance) VALUES (?, 'release-b', 'observed', 'test') RETURNING id",
        )
        .bind(sid)
        .fetch_one(&pool)
        .await
        .unwrap();
        let item_b: i64 = sqlx::query_scalar(
            "INSERT INTO edition_provider_items (service_id, service_track_id, release_id, evidence_status, provenance) VALUES (?, 'item-b', ?, 'observed', 'test') RETURNING id",
        )
        .bind(sid)
        .bind(release_b)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO edition_item_tracks (item_id, track_id, validation_status, provenance) VALUES (?, ?, 'unknown', 'test')")
            .bind(item_b)
            .bind(track)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, album_name) VALUES (?, ?, 'track', 'item-b', '100% Pure Love', 'Storyteller')",
        )
        .bind(account)
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();

        let page = query_favorite_tracks(&pool, Some(0), Some(50))
            .await
            .unwrap();
        assert_eq!(page.total, 2, "one row per favorited release");
        assert_eq!(page.tracks.len(), 2);
        let mut albums: Vec<Option<String>> =
            page.tracks.iter().map(|t| t.album_name.clone()).collect();
        albums.sort();
        assert_eq!(
            albums,
            vec![
                Some("Storyteller".to_string()),
                Some("The Best Of".to_string())
            ]
        );
        assert!(
            page.tracks.iter().all(|t| t.id == track),
            "both rows point at the same canonical track"
        );
    }

    #[tokio::test]
    async fn favorite_releases_collapse_by_title_and_keep_canonical_fallback() {
        let pool = setup_test_db().await;
        let (_sid, _account, track) = seed_favorite_fixture(&pool).await;

        // The same release reached through another service (case/whitespace
        // variant) must collapse into the existing row, not duplicate it.
        let qobuz_sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let qobuz_account: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'fav-qobuz', 1) RETURNING id",
        )
        .bind(qobuz_sid)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) VALUES (?, ?, 'q-item', 'FLAC', 1, 'available')",
        )
        .bind(track)
        .bind(qobuz_sid)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, album_name) VALUES (?, ?, 'track', 'q-item', '100% Pure Love', '  the best of ')",
        )
        .bind(qobuz_account)
        .bind(qobuz_sid)
        .execute(&pool)
        .await
        .unwrap();

        // A favorited track imported before the per-item records existed keeps
        // its single canonical row.
        let album2: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title) VALUES ('Canonical Album') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let track2: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, is_favorite, favorite_at) VALUES ('Legacy Favorite', ?, 1, '2024-02-01T00:00:00Z') RETURNING id",
        )
        .bind(album2)
        .fetch_one(&pool)
        .await
        .unwrap();

        let page = query_favorite_tracks(&pool, Some(0), Some(50))
            .await
            .unwrap();
        assert_eq!(page.total, 2);
        let track1_rows: Vec<_> = page.tracks.iter().filter(|t| t.id == track).collect();
        assert_eq!(
            track1_rows.len(),
            1,
            "same release collapses across services"
        );
        assert_eq!(track1_rows[0].album_name.as_deref(), Some("The Best Of"));
        let track2_rows: Vec<_> = page.tracks.iter().filter(|t| t.id == track2).collect();
        assert_eq!(track2_rows.len(), 1);
        assert_eq!(
            track2_rows[0].album_name.as_deref(),
            Some("Canonical Album")
        );

        let first_page = query_favorite_tracks(&pool, Some(0), Some(1))
            .await
            .unwrap();
        assert_eq!(first_page.total, 2);
        assert_eq!(first_page.tracks.len(), 1);
        assert!(first_page.has_more);
        let second_page = query_favorite_tracks(&pool, Some(1), Some(1))
            .await
            .unwrap();
        assert_eq!(second_page.tracks.len(), 1);
        assert!(!second_page.has_more);
    }

    #[tokio::test]
    async fn physical_status_page_is_bounded_and_refreshable() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("present.flac");
        std::fs::write(&existing, b"fixture").unwrap();
        let missing = temp.path().join("missing.flac");
        let mut tracks = (0..1200)
            .map(|id| LibraryTrack {
                id,
                file_path: Some(if id % 2 == 0 {
                    existing.to_string_lossy().into_owned()
                } else {
                    missing.to_string_lossy().into_owned()
                }),
                download_status: Some("downloaded".into()),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        verify_page_files(&mut tracks).await.unwrap();
        assert_eq!(
            tracks
                .iter()
                .filter(|t| t.download_status.as_deref() == Some("downloaded"))
                .count(),
            600
        );
        std::fs::write(&missing, b"restored").unwrap();
        verify_page_files(&mut tracks).await.unwrap();
        // A fresh SQL page carries the original candidate status on each request.
        tracks
            .iter_mut()
            .for_each(|t| t.download_status = Some("downloaded".into()));
        verify_page_files(&mut tracks).await.unwrap();
        assert_eq!(
            tracks
                .iter()
                .filter(|t| t.download_status.as_deref() == Some("downloaded"))
                .count(),
            1200
        );
        std::fs::remove_file(&existing).unwrap();
        tracks
            .iter_mut()
            .for_each(|t| t.download_status = Some("downloaded".into()));
        verify_page_files(&mut tracks).await.unwrap();
        assert_eq!(
            tracks
                .iter()
                .filter(|t| t.download_status.as_deref() == Some("downloaded"))
                .count(),
            600
        );
    }

    // FE-10 regression: get_library must expose the same quality-bucket
    // classification as get_audio_quality_distribution so the
    // /library?filter=quality&quality=<label> deep-link matches the
    // dashboard's quality card counts.
    #[tokio::test]
    async fn test_get_library_quality_bucket_matches_dashboard() {
        let pool = setup_test_db().await;

        sqlx::query("INSERT INTO tracks (title) VALUES ('HiRes Song')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tracks (title) VALUES ('CD Song')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tracks (title) VALUES ('Lossy Song')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tracks (title) VALUES ('Stream Only')")
            .execute(&pool)
            .await
            .unwrap();

        // 24/96 FLAC -> Hi-Res (24-bit+); 16/44.1 FLAC -> CD Quality;
        // MP3 -> Lossy; no downloads row -> NULL bucket.
        sqlx::query(
            "INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate) VALUES (1, '/tmp/hires.flac', 'FLAC', 24, 96000)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate) VALUES (2, '/tmp/cd.flac', 'FLAC', 16, 44100)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO downloads (track_id, file_path, file_format) VALUES (3, '/tmp/lossy.mp3', 'MP3')",
        ).execute(&pool).await.unwrap();

        let page = get_library_inner(&pool, Some(0), Some(50)).await.unwrap();
        assert_eq!(page.tracks.len(), 4);

        let bucket = |title: &str| {
            page.tracks
                .iter()
                .find(|t| t.title == title)
                .unwrap()
                .quality_bucket
                .clone()
        };
        assert_eq!(bucket("HiRes Song").as_deref(), Some("Hi-Res (24-bit+)"));
        assert_eq!(bucket("CD Song").as_deref(), Some("CD Quality"));
        assert_eq!(bucket("Lossy Song").as_deref(), Some("Lossy"));
        assert_eq!(bucket("Stream Only").as_deref(), None);
    }

    #[tokio::test]
    async fn test_remove_track_persists() {
        let pool = setup_test_db().await;

        // Insert a track
        sqlx::query("INSERT INTO tracks (title) VALUES ('Test Track')")
            .execute(&pool)
            .await
            .unwrap();

        // Verify it exists
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title = 'Test Track'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 1);

        // Remove it
        sqlx::query("DELETE FROM tracks WHERE title = 'Test Track'")
            .execute(&pool)
            .await
            .unwrap();

        // Verify gone
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title = 'Test Track'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn test_bulk_remove_tracks() {
        let pool = setup_test_db().await;

        // Insert 3 tracks
        for i in 1..=3 {
            sqlx::query("INSERT INTO tracks (title) VALUES (?)")
                .bind(format!("Bulk Track {}", i))
                .execute(&pool)
                .await
                .unwrap();
        }

        // Verify 3 exist
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title LIKE 'Bulk Track%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 3);

        // Get IDs of first 2
        let rows: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM tracks WHERE title LIKE 'Bulk Track%' LIMIT 2")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2);
        let ids: Vec<i64> = rows.into_iter().map(|(id,)| id).collect();

        // Bulk remove using the exact IN clause logic from bulk_remove_tracks
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!("DELETE FROM tracks WHERE id IN ({})", placeholders);
        let mut query = sqlx::query(&query_str);
        for id in &ids {
            query = query.bind(id);
        }
        let result = query.execute(&pool).await.unwrap();
        assert_eq!(result.rows_affected(), 2);

        // Verify 1 remains
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title LIKE 'Bulk Track%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_toggle_favorite() {
        let pool = setup_test_db().await;

        // Insert a track (is_favorite defaults to 0)
        sqlx::query("INSERT INTO tracks (title) VALUES ('Fav Track')")
            .execute(&pool)
            .await
            .unwrap();

        let id: (i64,) = sqlx::query_as("SELECT id FROM tracks WHERE title = 'Fav Track'")
            .fetch_one(&pool)
            .await
            .unwrap();

        // Verify default is 0
        let fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
            .bind(id.0)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fav.0, 0);

        // Toggle to 1
        sqlx::query("UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?")
            .bind(id.0).execute(&pool).await.unwrap();
        let fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
            .bind(id.0)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fav.0, 1);

        // Toggle back to 0
        sqlx::query("UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?")
            .bind(id.0).execute(&pool).await.unwrap();
        let fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
            .bind(id.0)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fav.0, 0);
    }

    #[tokio::test]
    async fn test_search_tracks_returns_results() {
        let pool = setup_test_db().await;

        // Insert a track (FTS trigger auto-indexes it)
        sqlx::query("INSERT INTO tracks (title) VALUES ('Searchable Track')")
            .execute(&pool)
            .await
            .unwrap();

        // Query FTS real JOIN
        let rows: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT t.id, t.title
            FROM tracks t
            JOIN library_fts fts ON fts.rowid = t.id
            WHERE library_fts MATCH ?
            "#,
        )
        .bind("Searchable")
        .fetch_all(&pool)
        .await
        .unwrap();

        assert!(!rows.is_empty());
        assert_eq!(rows[0].1, "Searchable Track");
    }

    #[tokio::test]
    async fn test_get_artist_returns_albums() {
        let pool = setup_test_db().await;

        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        let album_id: i64 =
            sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
            .bind(album_id)
            .bind(artist_id)
            .execute(&pool)
            .await
            .unwrap();

        // The query from get_artist
        let rows: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT
                al.id,
                al.title
            FROM albums al
            JOIN album_artists aa ON aa.album_id = al.id
            WHERE aa.artist_id = ?
            "#,
        )
        .bind(artist_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "Test Album");
    }

    #[tokio::test]
    async fn test_get_album_returns_tracks() {
        let pool = setup_test_db().await;

        let album_id: i64 =
            sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        let track_a_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, track_number, isrc, explicit) VALUES ('Track A', ?, 1, 'USMETA0000001', 1) RETURNING id"
        )
            .bind(album_id)
            .fetch_one(&pool).await.unwrap();

        sqlx::query("INSERT INTO tracks (title, album_id, track_number) VALUES ('Track B', ?, 2)")
            .bind(album_id)
            .execute(&pool)
            .await
            .unwrap();

        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Meta Artist') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_a_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, ?)")
            .bind(track_a_id)
            .bind("C:/Music/Syncify/Track A.flac")
            .execute(&pool)
            .await
            .unwrap();

        // The query from get_album
        let rows: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT
                t.id,
                t.title
            FROM tracks t
            WHERE t.album_id = ?
            ORDER BY COALESCE(t.disc_number, 1) ASC, COALESCE(t.track_number, 999) ASC, t.title ASC
            "#,
        )
        .bind(album_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1, "Track A");
        assert_eq!(rows[1].1, "Track B");

        let metadata = fetch_track_metadata(&pool, track_a_id).await.unwrap();
        assert_eq!(metadata.track_id, track_a_id);
        assert_eq!(metadata.title, "Track A");
        assert_eq!(metadata.artist_name.as_deref(), Some("Meta Artist"));
        assert_eq!(metadata.album_name.as_deref(), Some("Test Album"));
        assert_eq!(metadata.explicit, Some(true));
        assert_eq!(
            metadata.file_path.as_deref(),
            Some("C:/Music/Syncify/Track A.flac")
        );

        let not_found = fetch_track_metadata(&pool, 999_999).await.unwrap_err();
        assert!(not_found.contains("Track not found"));
    }

    #[sqlx::test]
    async fn test_auto_resolve_duplicates_by_isrc() {
        let pool = setup_test_db().await;

        // Create an artist
        sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Test Artist')")
            .execute(&pool)
            .await
            .unwrap();

        // Insert 3 tracks without ISRC but same title and near duration
        // so fallback tolerant matching resolves this duplicate set.
        let id1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Duplicate Song', NULL, 180000) RETURNING id")
            .fetch_one(&pool).await.unwrap();

        let id2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Duplicate Song', NULL, 181000) RETURNING id")
            .fetch_one(&pool).await.unwrap();

        let id3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Duplicate Song', NULL, 179000) RETURNING id")
            .fetch_one(&pool).await.unwrap();

        // Link all tracks to same artist
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id2)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id3)
        .execute(&pool)
        .await
        .unwrap();

        let service_id: (i64,) = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        // 10, 7, 4 quality scores
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score) VALUES (?, ?, '1', 10)")
            .bind(id1)
            .bind(service_id.0)
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score) VALUES (?, ?, '2', 7)")
            .bind(id2)
            .bind(service_id.0)
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score) VALUES (?, ?, '3', 4)")
            .bind(id3)
            .bind(service_id.0)
            .execute(&pool).await.unwrap();

        let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
        assert_eq!(res.groups_resolved, 1);
        assert_eq!(res.tracks_removed, 2);

        // Verify winner (quality_score = 10 -> id1)
        let remaining: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM tracks WHERE title = 'Duplicate Song'")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, id1);
    }

    #[sqlx::test]
    async fn test_auto_resolve_duplicates_merges_sources_and_relations() {
        let pool = setup_test_db().await;

        // Create an artist
        sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Merge Artist')")
            .execute(&pool)
            .await
            .unwrap();

        // Insert 2 duplicate tracks with same title and duration
        let id1: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc, duration_ms, genre) VALUES ('Dupe Song', NULL, 180000, 'Electronic') RETURNING id"
        )
        .fetch_one(&pool).await.unwrap();

        let id2: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc, duration_ms, bpm) VALUES ('Dupe Song', NULL, 180000, 128.0) RETURNING id"
        )
        .fetch_one(&pool).await.unwrap();

        // Track 1 has artist link
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();
        // Track 2 has artist link
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id2)
        .execute(&pool)
        .await
        .unwrap();

        let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
        let s1 = services[0].0;
        let s2 = services[1].0;

        // Track 1 (loser): service s1, 16-bit 44.1kHz, quality_score 100
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 'src_loser', 100, 16, 44100)"
        )
        .bind(id1).bind(s1).execute(&pool).await.unwrap();

        // Track 2 (winner): service s2, 24-bit 96kHz, quality_score 120
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 'src_winner', 120, 24, 96000)"
        )
        .bind(id2).bind(s2).execute(&pool).await.unwrap();

        // Create an account
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (service_id, email, display_name) VALUES (?, 'user@test.com', 'Test User') RETURNING id"
        )
        .bind(s1).fetch_one(&pool).await.unwrap();

        // Create 2 playlists
        let p1: i64 = sqlx::query_scalar(
            "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (?, 'pl1', 'Playlist 1') RETURNING id"
        )
        .bind(account_id).fetch_one(&pool).await.unwrap();
        let p2: i64 = sqlx::query_scalar(
            "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (?, 'pl2', 'Playlist 2') RETURNING id"
        )
        .bind(account_id).fetch_one(&pool).await.unwrap();

        // Track 1 is in playlist 1, Track 2 is in playlist 2
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)",
        )
        .bind(p1)
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)",
        )
        .bind(p2)
        .bind(id2)
        .execute(&pool)
        .await
        .unwrap();

        // Track 1 has lyrics
        sqlx::query("INSERT INTO lyrics (track_id, format, sync_level, content) VALUES (?, 'lrc', 'line', '[00:01.00]Hello')")
            .bind(id1).execute(&pool).await.unwrap();

        // Resolve duplicates
        let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
        assert_eq!(res.groups_resolved, 1);
        assert_eq!(res.tracks_removed, 1);

        // Winner must be id2 (higher quality score 120 vs 100)
        let remaining_tracks: Vec<(i64, Option<String>, Option<f64>)> =
            sqlx::query_as("SELECT id, genre, bpm FROM tracks WHERE title = 'Dupe Song'")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(remaining_tracks.len(), 1);
        assert_eq!(remaining_tracks[0].0, id2);
        // Metadata merged: genre backfilled from id1, bpm kept from id2
        assert_eq!(remaining_tracks[0].1.as_deref(), Some("Electronic"));
        assert_eq!(remaining_tracks[0].2, Some(128.0));

        // Sources merged: id2 must now have BOTH sources (s1 and s2)
        let sources: Vec<(i64, String)> = sqlx::query_as(
            "SELECT service_id, service_track_id FROM track_sources WHERE track_id = ? ORDER BY service_id ASC"
        )
        .bind(id2).fetch_all(&pool).await.unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].1, "src_loser");
        assert_eq!(sources[1].1, "src_winner");

        // Playlists merged: both playlists now point to id2
        let pl1_track: (i64,) =
            sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?")
                .bind(p1)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pl1_track.0, id2);
        let pl2_track: (i64,) =
            sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?")
                .bind(p2)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pl2_track.0, id2);

        // Lyrics transferred to id2
        let lyr_track: (i64,) = sqlx::query_as("SELECT track_id FROM lyrics WHERE format = 'lrc'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(lyr_track.0, id2);
    }

    #[sqlx::test]
    async fn test_auto_resolve_duplicates_sample_rate_tiebreaker() {
        let pool = setup_test_db().await;

        sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Rate Artist')")
            .execute(&pool)
            .await
            .unwrap();

        // 2 tracks with identical title and duration, both with quality_score = 100, bit_depth = 24
        // but different sample rates: 48000 vs 192000
        let id1: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Sample Rate Song', NULL, 200000) RETURNING id"
        )
        .fetch_one(&pool).await.unwrap();

        let id2: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Sample Rate Song', NULL, 200000) RETURNING id"
        )
        .fetch_one(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id1)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(id2)
        .execute(&pool)
        .await
        .unwrap();

        let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
        let s1 = services[0].0;
        let s2 = services[1].0;

        // id1: 24-bit, 48000 Hz
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 'sr_48k', 100, 24, 48000)"
        )
        .bind(id1).bind(s1).execute(&pool).await.unwrap();

        // id2: 24-bit, 192000 Hz
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 'sr_192k', 100, 24, 192000)"
        )
        .bind(id2).bind(s2).execute(&pool).await.unwrap();

        let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
        assert_eq!(res.groups_resolved, 1);
        assert_eq!(res.tracks_removed, 1);

        // id2 must win because 192000 > 48000
        let remaining: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM tracks WHERE title = 'Sample Rate Song'")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, id2);
    }

    // ---- UI-CORR-2: resolución de sidecars (booklet / portada animada / HD) ----

    /// Builds a structurally valid animated WebP (same shape the domain
    /// validator accepts: RIFF/WEBP + VP8X with the animation bit + ANMF frames).
    fn animated_webp_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02); // animation bit
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&(400u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(300u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(b"ANIM");
        data.extend_from_slice(&6u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 6]);
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);
        data
    }

    /// Static WebP (VP8X without the animation bit): must never be reported as
    /// an animated cover, even when it is named `cover.webp`.
    fn static_webp_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x00); // no animation bit
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&(400u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(300u32 - 1).to_le_bytes()[..3]);
        data
    }

    #[test]
    fn sidecars_resolve_from_the_album_directory() {
        let temp = tempfile::tempdir().unwrap();
        let album_dir = temp.path().join("The Weeknd").join("Starboy");
        std::fs::create_dir_all(&album_dir).unwrap();
        let track_path = album_dir.join("01 - Starboy.flac");
        std::fs::write(&track_path, b"fLaC").unwrap();
        std::fs::write(album_dir.join("booklet.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(album_dir.join("cover.animated.webp"), animated_webp_bytes()).unwrap();
        std::fs::write(album_dir.join("cover.jpg"), b"\xff\xd8\xff").unwrap();

        let (booklet, animated, hd) =
            resolve_track_sidecars(Some(&track_path.to_string_lossy()), None);

        assert_eq!(
            booklet.as_deref(),
            Some(album_dir.join("booklet.pdf").to_string_lossy().as_ref())
        );
        assert_eq!(
            animated.as_deref(),
            Some(
                album_dir
                    .join("cover.animated.webp")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(
            hd.as_deref(),
            Some(album_dir.join("cover.jpg").to_string_lossy().as_ref())
        );
    }

    #[test]
    fn sidecars_accept_alternate_names_and_reject_static_webp_as_animated() {
        let temp = tempfile::tempdir().unwrap();
        let album_dir = temp.path().join("Album");
        std::fs::create_dir_all(&album_dir).unwrap();
        let track_path = album_dir.join("track.flac");
        std::fs::write(&track_path, b"fLaC").unwrap();
        // Alternate booklet name + `animated.webp` + animated `cover.webp` absent.
        std::fs::write(album_dir.join("digital-booklet.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(album_dir.join("animated.webp"), animated_webp_bytes()).unwrap();
        std::fs::write(album_dir.join("cover.webp"), static_webp_bytes()).unwrap();

        let (booklet, animated, hd) =
            resolve_track_sidecars(Some(&track_path.to_string_lossy()), None);

        assert_eq!(
            booklet.as_deref(),
            Some(
                album_dir
                    .join("digital-booklet.pdf")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(
            animated.as_deref(),
            Some(album_dir.join("animated.webp").to_string_lossy().as_ref())
        );
        assert_eq!(
            hd.as_deref(),
            Some(album_dir.join("cover.webp").to_string_lossy().as_ref())
        );

        // Same album without `animated.webp`: the static `cover.webp` must not be
        // promoted to the animated slot (that was the old bug: any existing
        // cover.webp flipped the player into "Animated").
        std::fs::remove_file(album_dir.join("animated.webp")).unwrap();
        let (_, animated_only_static, _) =
            resolve_track_sidecars(Some(&track_path.to_string_lossy()), None);
        assert_eq!(animated_only_static, None);
    }

    #[test]
    fn sidecars_look_in_the_parent_for_disc_folders() {
        let temp = tempfile::tempdir().unwrap();
        let album_dir = temp.path().join("Rush").join("Chronicles");
        let disc_dir = album_dir.join("Disc 2");
        std::fs::create_dir_all(&disc_dir).unwrap();
        let track_path = disc_dir.join("01 - Track.flac");
        std::fs::write(&track_path, b"fLaC").unwrap();
        std::fs::write(album_dir.join("booklet.pdf"), b"%PDF-1.4").unwrap();

        let (booklet, _, _) = resolve_track_sidecars(Some(&track_path.to_string_lossy()), None);
        assert_eq!(
            booklet.as_deref(),
            Some(album_dir.join("booklet.pdf").to_string_lossy().as_ref())
        );
    }

    #[test]
    fn sidecars_pick_up_the_mp4_fallback_and_missing_files_stay_none() {
        let temp = tempfile::tempdir().unwrap();
        let album_dir = temp.path().join("Album");
        std::fs::create_dir_all(&album_dir).unwrap();
        let track_path = album_dir.join("track.flac");
        std::fs::write(&track_path, b"fLaC").unwrap();
        std::fs::write(
            album_dir.join("animated_cover.mp4"),
            b"\x00\x00\x00\x18ftyp",
        )
        .unwrap();

        let (booklet, animated, hd) =
            resolve_track_sidecars(Some(&track_path.to_string_lossy()), None);
        assert_eq!(booklet, None);
        assert_eq!(
            animated.as_deref(),
            Some(
                album_dir
                    .join("animated_cover.mp4")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(hd, None);

        // A track without sidecars and without a path resolves to nothing.
        let (b, a, h) = resolve_track_sidecars(None, None);
        assert_eq!((b, a, h), (None, None, None));
    }

    #[tokio::test]
    async fn fetch_track_metadata_exposes_album_file_and_sidecar_fields() {
        let pool = setup_test_db().await;
        let temp = tempfile::tempdir().unwrap();
        let album_dir = temp.path().join("Artist").join("Album");
        std::fs::create_dir_all(&album_dir).unwrap();
        let track_path = album_dir.join("01 - Track.flac");
        std::fs::write(&track_path, b"fLaC").unwrap();
        std::fs::write(album_dir.join("booklet.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(album_dir.join("cover.animated.webp"), animated_webp_bytes()).unwrap();

        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title, upc, label, release_date, total_tracks, total_discs, musicbrainz_id) \
             VALUES ('Album', '0602557234567', 'XO', '2016-11-25', 18, 2, 'mb-album-uuid') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Track', ?, 230000) RETURNING id",
        )
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO downloads (track_id, file_path, file_format, file_size_bytes, bit_depth, sample_rate, musicbrainz_release_id) \
             VALUES (?, ?, 'FLAC', 52428800, 24, 96000, 'mb-release-uuid')",
        )
        .bind(track_id)
        .bind(track_path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

        let metadata = fetch_track_metadata(&pool, track_id).await.unwrap();

        assert_eq!(metadata.label.as_deref(), Some("XO"));
        assert_eq!(metadata.upc.as_deref(), Some("0602557234567"));
        assert_eq!(metadata.release_date.as_deref(), Some("2016-11-25"));
        assert_eq!(metadata.total_tracks, Some(18));
        assert_eq!(metadata.total_discs, Some(2));
        assert_eq!(
            metadata.album_musicbrainz_id.as_deref(),
            Some("mb-album-uuid")
        );
        assert_eq!(
            metadata.musicbrainz_release_id.as_deref(),
            Some("mb-release-uuid")
        );
        assert_eq!(metadata.file_format.as_deref(), Some("FLAC"));
        assert_eq!(metadata.file_size_bytes, Some(52_428_800));
        assert_eq!(metadata.bit_depth, Some(24));
        assert_eq!(metadata.sample_rate, Some(96_000));
        assert_eq!(
            metadata.booklet_path.as_deref(),
            Some(album_dir.join("booklet.pdf").to_string_lossy().as_ref())
        );
        assert_eq!(
            metadata.animated_cover_path.as_deref(),
            Some(
                album_dir
                    .join("cover.animated.webp")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert!(metadata.downloaded_at.is_some());
    }

    #[test]
    fn open_local_sidecar_extension_allowlist_is_narrow() {
        for ext in ["pdf", "jpg", "jpeg", "png", "webp", "mp4", "lrc"] {
            assert!(
                OPENABLE_SIDECAR_EXTENSIONS.contains(&ext),
                "{ext} must be openable"
            );
        }
        for ext in ["sh", "exe", "bat", "ps1", "dll", "so"] {
            assert!(
                !OPENABLE_SIDECAR_EXTENSIONS.contains(&ext),
                "{ext} must never be handed to the OS handler"
            );
        }
    }
}

#[derive(serde::Serialize)]
pub struct AutoResolveResult {
    pub groups_resolved: u32,
    pub tracks_removed: u32,
}

#[tauri::command]
pub async fn auto_resolve_duplicates(
    state: tauri::State<'_, AppState>,
) -> Result<AutoResolveResult, String> {
    auto_resolve_duplicates_inner(&state.db).await
}

#[allow(dead_code)] // Cubierta por `tests/duplicates_level2_3_merge_test.rs` y `tests/album_total_tracks_recalculation_test.rs`.
pub async fn merge_level2_3_duplicates_inner(
    db: &crate::db::DbPool,
) -> Result<AutoResolveResult, String> {
    auto_resolve_duplicates_inner(db).await
}

/// SQL for the Level 3 fuzzy-duplicate pair scan (see
/// `auto_resolve_duplicates_inner`).
///
/// Bucket-and-expand rather than self-join. The previous shape joined
/// `track_artists` to itself on `artist_id` *before* applying the title,
/// duration and explicit filters, so a library with one very large artist —
/// which migration 0081 produces, since it folds every Various Artists variant
/// into a single canonical artist — paid C(k,2) on `k` tracks and discarded
/// almost all of them afterwards. Measured on a 20 000-track fixture with
/// `sqlite3 .timer`: 116 s with the self-join, 0.10 s here; the degenerate
/// 20-distinct-title variant (9 990 000 surviving pairs) drops from 198 s to
/// 9.8 s, and what remains is the cost of materialising the result set.
///
/// The emitted set is unchanged: the bucket key is exactly the equality part of
/// the old predicate (artist, `LOWER(TRIM(title))`, explicit), and the ±2000 ms
/// window is applied only after bucketing, which is equivalent because the
/// window is symmetric and per-bucket. Rows are emitted in (duration_ms,
/// track_id) order — a total order, so no group is reordered — whereas the old
/// plan emitted each artist's pairs in `track_artists` scan order. The
/// union-find below is independent of that order: when two tracks share every
/// artist, the shared artist is the unique minimum artist id, so exactly one
/// pair per track pair survives. Inside a connected component the partition is
/// therefore fixed by the pair *set*, and the surviving ISRC can only differ
/// when a component holds several distinct non-blank ISRCs — in which case both
/// orderings break it into single-ISRC groups containing the same tracks.
///
/// `tests/duplicates_fuzzy_pairs_scale_test.rs` asserts this SQL returns the
/// same pair set as the self-join it replaced, and that the plan never falls
/// back to a full pairwise scan, so the constant is part of the test contract.
pub const FUZZY_DUPLICATE_PAIRS_SQL: &str = r#"
        WITH eligible AS (
            SELECT ta.track_id   AS track_id,
                   ta.artist_id  AS artist_id,
                   t.title       AS title,
                   t.duration_ms AS duration_ms,
                   t.explicit    AS explicit,
                   t.isrc        AS isrc
            FROM track_artists ta
            JOIN tracks t ON t.id = ta.track_id
            WHERE (LOWER(COALESCE(ta.role, 'primary')) IN ('primary', 'main')
                   OR (SELECT COUNT(*) FROM track_artists WHERE track_id = ta.track_id) = 1)
              AND t.duration_ms > 10000
              AND TRIM(COALESCE(t.title, '')) != ''
        ),
        buckets AS (
            SELECT artist_id,
                   LOWER(TRIM(title))    AS title_key,
                   COALESCE(explicit, 0) AS explicit_key,
                   track_id, duration_ms, isrc
            FROM eligible
        ),
        ordered AS (
            SELECT track_id, duration_ms, isrc, artist_id, title_key, explicit_key,
                   ROW_NUMBER() OVER (PARTITION BY artist_id, title_key, explicit_key
                                      ORDER BY duration_ms, track_id) AS rn,
                   COUNT(*) OVER (PARTITION BY artist_id, title_key, explicit_key) AS grp_size
            FROM buckets
        )
        SELECT o1.track_id as id_a, o2.track_id as id_b, o1.isrc as isrc_a, o2.isrc as isrc_b
        FROM ordered o1
        JOIN ordered o2
          ON o2.artist_id = o1.artist_id
         AND o2.title_key = o1.title_key
         AND o2.explicit_key = o1.explicit_key
         AND o2.track_id > o1.track_id
         AND ABS(o1.duration_ms - o2.duration_ms) <= 2000
        WHERE o1.grp_size > 1
        ORDER BY o1.duration_ms, o1.track_id, o2.duration_ms, o2.track_id
        "#;

pub async fn auto_resolve_duplicates_inner(
    db: &crate::db::DbPool,
) -> Result<AutoResolveResult, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Tx error: {}", e))?;
    let mut groups_resolved = 0;
    let mut tracks_removed = 0;
    let mut affected_albums: std::collections::HashSet<i64> = std::collections::HashSet::new();

    fn find_root(parent: &mut std::collections::HashMap<i64, i64>, node: i64) -> i64 {
        let current_parent = *parent.get(&node).unwrap_or(&node);
        if current_parent == node {
            return node;
        }
        let root = find_root(parent, current_parent);
        parent.insert(node, root);
        root
    }

    fn union_nodes(
        parent: &mut std::collections::HashMap<i64, i64>,
        rank: &mut std::collections::HashMap<i64, u8>,
        a: i64,
        b: i64,
    ) {
        let root_a = find_root(parent, a);
        let root_b = find_root(parent, b);

        if root_a == root_b {
            return;
        }

        let rank_a = *rank.get(&root_a).unwrap_or(&0);
        let rank_b = *rank.get(&root_b).unwrap_or(&0);

        if rank_a < rank_b {
            parent.insert(root_a, root_b);
        } else if rank_a > rank_b {
            parent.insert(root_b, root_a);
        } else {
            parent.insert(root_b, root_a);
            rank.insert(root_a, rank_a.saturating_add(1));
        }
    }

    async fn resolve_group(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        track_ids: Vec<i64>,
        affected_albums: &mut std::collections::HashSet<i64>,
    ) -> Result<u32, String> {
        if track_ids.len() <= 1 {
            return Ok(0);
        }

        #[derive(sqlx::FromRow)]
        struct TrackInfo {
            id: i64,
            has_isrc: bool,
            quality_score: Option<i32>,
            bit_depth: Option<i32>,
            sample_rate: Option<i32>,
            duration_ms: Option<i64>,
            bitrate: Option<i32>,
            file_size_bytes: Option<i64>,
            file_path: Option<String>,
            metadata_score: i64,
            source_count: i64,
            album_id: Option<i64>,
        }

        let mut infos = Vec::new();
        for &id in &track_ids {
            let info: Option<TrackInfo> = sqlx::query_as(
                r#"
                SELECT
                    t.id,
                    (t.isrc IS NOT NULL AND TRIM(t.isrc) != '') as has_isrc,
                    COALESCE(
                        MAX(ts.quality_score),
                        CASE
                            WHEN MAX(d.bit_depth) >= 24 THEN 1200
                            WHEN MAX(d.bit_depth) >= 16 THEN 1000
                            WHEN MAX(d.file_path) IS NOT NULL THEN 500
                            ELSE NULL
                        END
                    ) as quality_score,
                    MAX(COALESCE(d.bit_depth, ts.bit_depth, 0)) as bit_depth,
                    MAX(COALESCE(d.sample_rate, ts.sample_rate, 0)) as sample_rate,
                    t.duration_ms,
                    MAX(COALESCE(ts.bitrate, 0)) as bitrate,
                    MAX(d.file_size_bytes) as file_size_bytes,
                    MAX(d.file_path) as file_path,
                    (
                        CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                        CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                        CASE WHEN t.album_id IS NOT NULL THEN 10 ELSE 0 END +
                        CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                        CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
                        CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                        CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
                    ) as metadata_score,
                    (SELECT COUNT(*) FROM track_sources WHERE track_id = t.id) as source_count,
                    t.album_id
                FROM tracks t
                LEFT JOIN track_sources ts ON t.id = ts.track_id
                LEFT JOIN downloads d ON t.id = d.track_id
                WHERE t.id = ?
                GROUP BY t.id
                "#
            )
            .bind(id)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if let Some(i) = info {
                infos.push(i);
            }
        }

        if infos.len() <= 1 {
            return Ok(0);
        }

        // Survivor selection hierarchy:
        // Canonical ISRC > descargado > quality_score > 24-bit > sample_rate > mayor duración
        infos.sort_by(|a, b| {
            // 0. Canonical ISRC precedence: track with ISRC is retained as canonical
            if a.has_isrc != b.has_isrc {
                return a.has_isrc.cmp(&b.has_isrc);
            }

            // 1. Local physical download takes initial precedence
            let a_has_file = a.file_path.is_some();
            let b_has_file = b.file_path.is_some();
            if a_has_file != b_has_file {
                return a_has_file.cmp(&b_has_file);
            }

            // 2. quality_score (Hi-Res vs Lossless vs Lossy)
            let a_qs = a.quality_score.unwrap_or(0);
            let b_qs = b.quality_score.unwrap_or(0);
            if a_qs != b_qs {
                return a_qs.cmp(&b_qs);
            }

            // 3. 24-bit / bit_depth (e.g. 24 > 16)
            let a_bd = a.bit_depth.unwrap_or(0);
            let b_bd = b.bit_depth.unwrap_or(0);
            if a_bd != b_bd {
                return a_bd.cmp(&b_bd);
            }

            // 4. sample_rate (e.g. 192000 > 96000 > 48000 > 44100)
            let a_sr = a.sample_rate.unwrap_or(0);
            let b_sr = b.sample_rate.unwrap_or(0);
            if a_sr != b_sr {
                return a_sr.cmp(&b_sr);
            }

            // 5. mayor duración (longer duration ms)
            let a_dur = a.duration_ms.unwrap_or(0);
            let b_dur = b.duration_ms.unwrap_or(0);
            if a_dur != b_dur {
                return a_dur.cmp(&b_dur);
            }

            // 5b. bitrate (e.g. 320 > 256 > 128)
            let a_br = a.bitrate.unwrap_or(0);
            let b_br = b.bitrate.unwrap_or(0);
            if a_br != b_br {
                return a_br.cmp(&b_br);
            }

            // 6. Metadata completeness score
            if a.metadata_score != b.metadata_score {
                return a.metadata_score.cmp(&b.metadata_score);
            }

            // 7. Number of associated sources
            if a.source_count != b.source_count {
                return a.source_count.cmp(&b.source_count);
            }

            // 8. File size fallback
            let a_fs = a.file_size_bytes.unwrap_or(0);
            let b_fs = b.file_size_bytes.unwrap_or(0);
            if a_fs != b_fs {
                return a_fs.cmp(&b_fs);
            }

            // Stable deterministic tie-breaker
            a.id.cmp(&b.id)
        });

        let winner_id = infos.last().unwrap().id;
        for info in &infos {
            if let Some(alb) = info.album_id {
                affected_albums.insert(alb);
            }
        }

        let mut removed = 0;

        for info in &infos {
            if info.id == winner_id {
                continue;
            }
            let loser_id = info.id;

            // F2.5: Transactional MERGE rather than destructive DELETE

            // Retrieve audio_quality from both tracks to preserve the highest tier
            let winner_q: Option<String> =
                sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
                    .bind(winner_id)
                    .fetch_optional(&mut **tx)
                    .await
                    .ok()
                    .flatten();
            let loser_q: Option<String> =
                sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
                    .bind(loser_id)
                    .fetch_optional(&mut **tx)
                    .await
                    .ok()
                    .flatten();

            let parse_tier = |s: &str| -> AudioTier {
                s.parse::<AudioTier>()
                    .unwrap_or_else(|_| classify_audio_tier(None, None, None, Some(s)))
            };

            let mut merged_tier = match (
                winner_q.as_deref().map(parse_tier),
                loser_q.as_deref().map(parse_tier),
            ) {
                (Some(w), Some(l)) => Some(std::cmp::max(w, l)),
                (Some(w), None) => Some(w),
                (None, Some(l)) => Some(l),
                (None, None) => None,
            };

            // Backfill null metadata on winner from loser, preserving highest audio_quality
            #[derive(sqlx::FromRow)]
            struct LoserMeta {
                album_id: Option<i64>,
                duration_ms: Option<i64>,
                track_number: Option<i32>,
                disc_number: Option<i32>,
                isrc: Option<String>,
                musicbrainz_id: Option<String>,
                genre: Option<String>,
                subgenre: Option<String>,
                release_year: Option<i32>,
                record_label: Option<String>,
                bpm: Option<f64>,
                musical_key: Option<String>,
                spotify_id: Option<String>,
                qobuz_id: Option<String>,
                audio_quality: Option<String>,
            }

            let loser_meta: Option<LoserMeta> = sqlx::query_as(
                r#"
                SELECT album_id, duration_ms, track_number, disc_number,
                       isrc, musicbrainz_id, genre, subgenre, release_year,
                       record_label, bpm, musical_key, spotify_id, qobuz_id, audio_quality
                FROM tracks WHERE id = ?
                "#,
            )
            .bind(loser_id)
            .fetch_optional(&mut **tx)
            .await
            .ok()
            .flatten();

            if let Some(lm) = loser_meta {
                // Clear unique columns on loser before backfilling onto winner to prevent UNIQUE collisions
                let _ = sqlx::query("UPDATE tracks SET isrc = NULL, spotify_id = NULL, qobuz_id = NULL, musicbrainz_id = NULL WHERE id = ?")
                    .bind(loser_id)
                    .execute(&mut **tx).await;

                let _ = sqlx::query(
                    r#"
                    UPDATE tracks
                    SET
                        album_id = COALESCE(tracks.album_id, ?),
                        duration_ms = COALESCE(tracks.duration_ms, ?),
                        track_number = COALESCE(tracks.track_number, ?),
                        disc_number = COALESCE(tracks.disc_number, ?),
                        isrc = COALESCE(tracks.isrc, ?),
                        musicbrainz_id = COALESCE(tracks.musicbrainz_id, ?),
                        genre = COALESCE(tracks.genre, ?),
                        subgenre = COALESCE(tracks.subgenre, ?),
                        release_year = COALESCE(tracks.release_year, ?),
                        record_label = COALESCE(tracks.record_label, ?),
                        bpm = COALESCE(tracks.bpm, ?),
                        musical_key = COALESCE(tracks.musical_key, ?),
                        spotify_id = COALESCE(tracks.spotify_id, ?),
                        qobuz_id = COALESCE(tracks.qobuz_id, ?),
                        audio_quality = COALESCE(?, tracks.audio_quality, ?)
                    WHERE tracks.id = ?
                    "#,
                )
                .bind(lm.album_id)
                .bind(lm.duration_ms)
                .bind(lm.track_number)
                .bind(lm.disc_number)
                .bind(lm.isrc)
                .bind(lm.musicbrainz_id)
                .bind(lm.genre)
                .bind(lm.subgenre)
                .bind(lm.release_year)
                .bind(lm.record_label)
                .bind(lm.bpm)
                .bind(lm.musical_key)
                .bind(lm.spotify_id)
                .bind(lm.qobuz_id)
                .bind(merged_tier.map(|t| t.as_str()))
                .bind(lm.audio_quality)
                .bind(winner_id)
                .execute(&mut **tx)
                .await;
            }

            // Preserve favorite status if loser was favorited
            let _ = sqlx::query(
                "UPDATE tracks SET is_favorite = 1 WHERE id = ? AND EXISTS (SELECT 1 FROM tracks WHERE id = ? AND is_favorite = 1)"
            )
            .bind(winner_id)
            .bind(loser_id)
            .execute(&mut **tx).await;

            // 1. Transfer track_sources respecting UNIQUE(track_id, service_id)
            #[derive(sqlx::FromRow)]
            struct SourceItem {
                id: i64,
                service_id: i64,
                service_track_id: String,
                format: Option<String>,
                bit_depth: Option<i32>,
                sample_rate: Option<i32>,
                bitrate: Option<i32>,
                quality_score: Option<i32>,
            }

            let loser_sources: Vec<SourceItem> = sqlx::query_as(
                "SELECT id, service_id, service_track_id, format, bit_depth, sample_rate, bitrate, quality_score FROM track_sources WHERE track_id = ?"
            )
            .bind(loser_id)
            .fetch_all(&mut **tx).await.unwrap_or_default();

            for ls in loser_sources {
                let winner_src: Option<(i64, Option<i32>)> = sqlx::query_as(
                    "SELECT id, quality_score FROM track_sources WHERE track_id = ? AND service_id = ?"
                )
                .bind(winner_id)
                .bind(ls.service_id)
                .fetch_optional(&mut **tx).await.unwrap_or(None);

                match winner_src {
                    None => {
                        let _ = sqlx::query("UPDATE track_sources SET track_id = ? WHERE id = ?")
                            .bind(winner_id)
                            .bind(ls.id)
                            .execute(&mut **tx)
                            .await;
                    }
                    Some((ws_id, ws_qs)) => {
                        let loser_qs = ls.quality_score.unwrap_or(0);
                        let winner_qs_val = ws_qs.unwrap_or(0);
                        if loser_qs > winner_qs_val {
                            let _ = sqlx::query(
                                r#"
                                UPDATE track_sources
                                SET service_track_id = ?, format = COALESCE(?, format),
                                    bit_depth = COALESCE(?, bit_depth), sample_rate = COALESCE(?, sample_rate),
                                    bitrate = COALESCE(?, bitrate), quality_score = ?
                                WHERE id = ?
                                "#
                            )
                            .bind(&ls.service_track_id)
                            .bind(&ls.format)
                            .bind(ls.bit_depth)
                            .bind(ls.sample_rate)
                            .bind(ls.bitrate)
                            .bind(loser_qs)
                            .bind(ws_id)
                            .execute(&mut **tx).await;
                        }
                        let _ = sqlx::query("DELETE FROM track_sources WHERE id = ?")
                            .bind(ls.id)
                            .execute(&mut **tx)
                            .await;
                    }
                }
            }

            // 2. Transfer playlist_tracks
            sqlx::query("UPDATE playlist_tracks SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 3. Transfer downloads (track_id is unique in downloads)
            let winner_has_download: bool =
                sqlx::query_scalar("SELECT COUNT(*) > 0 FROM downloads WHERE track_id = ?")
                    .bind(winner_id)
                    .fetch_one(&mut **tx)
                    .await
                    .unwrap_or(false);

            if !winner_has_download {
                let _ = sqlx::query("UPDATE downloads SET track_id = ? WHERE track_id = ?")
                    .bind(winner_id)
                    .bind(loser_id)
                    .execute(&mut **tx)
                    .await;
            } else {
                let _ = sqlx::query("DELETE FROM downloads WHERE track_id = ?")
                    .bind(loser_id)
                    .execute(&mut **tx)
                    .await;
            }

            // 4. Transfer lyrics
            sqlx::query("UPDATE OR IGNORE lyrics SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM lyrics WHERE track_id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 5. Transfer library_entries
            sqlx::query("UPDATE OR IGNORE library_entries SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM library_entries WHERE track_id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 6. Transfer track_credits
            sqlx::query("UPDATE OR IGNORE track_credits SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM track_credits WHERE track_id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 7. Transfer track_artists
            sqlx::query("UPDATE OR IGNORE track_artists SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM track_artists WHERE track_id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 8. Transfer download_queue
            sqlx::query("UPDATE OR IGNORE download_queue SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM download_queue WHERE track_id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 9. Transfer enrichment_progress
            sqlx::query("UPDATE OR IGNORE enrichment_progress SET track_id = ? WHERE track_id = ?")
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM enrichment_progress WHERE track_id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            // 10. Transfer operation_journal if table exists
            let has_op_journal: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'operation_journal'"
            )
            .fetch_one(&mut **tx).await.unwrap_or(false);
            if has_op_journal {
                let _ = sqlx::query(
                    "UPDATE OR IGNORE operation_journal SET track_id = ? WHERE track_id = ?",
                )
                .bind(winner_id)
                .bind(loser_id)
                .execute(&mut **tx)
                .await;
            }

            // 11. Favorites status is preserved on canonical tracks.is_favorite and library_entries.

            // 11b. Re-evaluate all merged sources on winner to preserve the highest quality tier
            let winner_sources: Vec<(Option<i32>, Option<i32>, Option<i32>, Option<String>)> = sqlx::query_as(
                "SELECT bit_depth, sample_rate, bitrate, format FROM track_sources WHERE track_id = ?"
            )
            .bind(winner_id)
            .fetch_all(&mut **tx).await.unwrap_or_default();

            for (bd, sr, br, fmt) in winner_sources {
                let src_tier = classify_audio_tier(bd, sr, br, fmt.as_deref());
                merged_tier = Some(match merged_tier {
                    Some(cur) => std::cmp::max(cur, src_tier),
                    None => src_tier,
                });
            }

            if let Some(tier) = merged_tier {
                let _ = sqlx::query("UPDATE tracks SET audio_quality = ? WHERE id = ?")
                    .bind(tier.as_str())
                    .bind(winner_id)
                    .execute(&mut **tx)
                    .await;
            }

            // 12. Finally remove the loser track record
            sqlx::query("DELETE FROM tracks WHERE id = ?")
                .bind(loser_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;

            removed += 1;
        }
        Ok(removed)
    }

    // Phase 1: Exact ISRC duplicate resolution with explicit flag discrimination
    let isrc_groups: Vec<(String,)> = sqlx::query_as(
        "SELECT isrc FROM tracks WHERE isrc IS NOT NULL AND TRIM(isrc) != '' GROUP BY isrc HAVING COUNT(id) > 1"
    )
    .fetch_all(&mut *tx).await.map_err(|e| format!("Query error: {}", e))?;

    for (isrc,) in isrc_groups {
        let tracks: Vec<(i64, Option<i32>)> =
            sqlx::query_as("SELECT id, explicit FROM tracks WHERE isrc = ?")
                .bind(&isrc)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

        let mut clean_ids = Vec::new();
        let mut explicit_ids = Vec::new();
        for (id, exp) in tracks {
            if exp.unwrap_or(0) != 0 {
                explicit_ids.push(id);
            } else {
                clean_ids.push(id);
            }
        }

        if clean_ids.len() > 1 {
            let rem = resolve_group(&mut tx, clean_ids, &mut affected_albums).await?;
            if rem > 0 {
                groups_resolved += 1;
                tracks_removed += rem;
            }
        }
        if explicit_ids.len() > 1 {
            let rem = resolve_group(&mut tx, explicit_ids, &mut affected_albums).await?;
            if rem > 0 {
                groups_resolved += 1;
                tracks_removed += rem;
            }
        }
    }

    // Phase 2a: Intra-album duplicates (Level 2)
    // Same album, duration ± 2000 ms, matching title or disc/track number, same explicit
    let intra_pairs: Vec<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.id as id_a, b.id as id_b, a.isrc as isrc_a, b.isrc as isrc_b
        FROM tracks a
        JOIN tracks b ON a.album_id = b.album_id AND a.id < b.id
        WHERE a.album_id IS NOT NULL
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND TRIM(COALESCE(b.title, '')) != ''
          AND COALESCE(a.explicit, 0) = COALESCE(b.explicit, 0)
          AND (
              LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
              OR (a.disc_number = b.disc_number AND a.track_number = b.track_number AND a.track_number > 0)
          )
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
        "#
    )
    .fetch_all(&mut *tx).await.map_err(|e| format!("Query error: {}", e))?;

    // Also include intra-album tracks matching by clean_title
    let intra_clean_candidates: Vec<(i64, i64, String, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            r#"
        SELECT a.id, b.id, a.title, b.title, a.isrc, b.isrc
        FROM tracks a
        JOIN tracks b ON a.album_id = b.album_id AND a.id < b.id
        WHERE a.album_id IS NOT NULL
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND TRIM(COALESCE(b.title, '')) != ''
          AND COALESCE(a.explicit, 0) = COALESCE(b.explicit, 0)
          AND LOWER(TRIM(a.title)) != LOWER(TRIM(b.title))
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
        "#,
        )
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

    let mut extra_intra = Vec::new();
    for (id_a, id_b, t_a, t_b, isrc_a, isrc_b) in intra_clean_candidates {
        if syncify_core_domain::metadata::clean_title(&t_a)
            == syncify_core_domain::metadata::clean_title(&t_b)
        {
            extra_intra.push((id_a, id_b, isrc_a, isrc_b));
        }
    }

    // Phase 2b: Level 3 fuzzy duplicates (matching primary artist, title, duration ± 2000 ms, same explicit)
    let fuzzy_pairs: Vec<(i64, i64, Option<String>, Option<String>)> =
        sqlx::query_as(FUZZY_DUPLICATE_PAIRS_SQL)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| format!("Query error: {}", e))?;

    let mut parent: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut rank: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();
    let mut component_isrc: std::collections::HashMap<i64, String> =
        std::collections::HashMap::new();

    // Process intra-album pairs (ISRC reconciled within the same album)
    for (id_a, id_b, isrc_a, isrc_b) in intra_pairs.into_iter().chain(extra_intra) {
        parent.entry(id_a).or_insert(id_a);
        parent.entry(id_b).or_insert(id_b);
        rank.entry(id_a).or_insert(0);
        rank.entry(id_b).or_insert(0);

        let clean_isrc_a = isrc_a.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });
        let clean_isrc_b = isrc_b.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });

        let root_a = find_root(&mut parent, id_a);
        let root_b = find_root(&mut parent, id_b);
        if root_a == root_b {
            continue;
        }

        let isrc_for_a = component_isrc.get(&root_a).cloned().or(clean_isrc_a);
        let isrc_for_b = component_isrc.get(&root_b).cloned().or(clean_isrc_b);
        let merged_isrc = isrc_for_a.or(isrc_for_b);

        union_nodes(&mut parent, &mut rank, id_a, id_b);
        let new_root = find_root(&mut parent, id_a);
        if let Some(isrc_val) = merged_isrc {
            component_isrc.insert(new_root, isrc_val);
        }
    }

    // Process Level 3 fuzzy pairs (preserve distinct masters across different albums if both have distinct ISRCs)
    for (id_a, id_b, isrc_a, isrc_b) in fuzzy_pairs {
        parent.entry(id_a).or_insert(id_a);
        parent.entry(id_b).or_insert(id_b);
        rank.entry(id_a).or_insert(0);
        rank.entry(id_b).or_insert(0);

        let clean_isrc_a = isrc_a.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });
        let clean_isrc_b = isrc_b.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });

        let root_a = find_root(&mut parent, id_a);
        let root_b = find_root(&mut parent, id_b);
        if root_a == root_b {
            continue;
        }

        let isrc_for_a = component_isrc.get(&root_a).cloned().or(clean_isrc_a);
        let isrc_for_b = component_isrc.get(&root_b).cloned().or(clean_isrc_b);

        // If both components have an ISRC and they are different, do NOT union them across albums
        if let (Some(ref ia), Some(ref ib)) = (&isrc_for_a, &isrc_for_b) {
            if ia != ib {
                continue;
            }
        }

        let merged_isrc = isrc_for_a.or(isrc_for_b);

        union_nodes(&mut parent, &mut rank, id_a, id_b);
        let new_root = find_root(&mut parent, id_a);
        if let Some(isrc_val) = merged_isrc {
            component_isrc.insert(new_root, isrc_val);
        }
    }

    let nodes: Vec<i64> = parent.keys().copied().collect();
    let mut components: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for node in nodes {
        let root = find_root(&mut parent, node);
        components.entry(root).or_default().push(node);
    }

    for track_ids in components.into_values() {
        let rem = resolve_group(&mut tx, track_ids, &mut affected_albums).await?;
        if rem > 0 {
            groups_resolved += 1;
            tracks_removed += rem;
        }
    }

    // Renumber tracks sequentially in all affected albums
    for album_id in affected_albums {
        let discs: Vec<(i32,)> = sqlx::query_as(
            "SELECT DISTINCT COALESCE(disc_number, 1) FROM tracks WHERE album_id = ? ORDER BY 1",
        )
        .bind(album_id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        for (disc,) in discs {
            let track_rows: Vec<(i64,)> = sqlx::query_as(
                "SELECT id FROM tracks WHERE album_id = ? AND COALESCE(disc_number, 1) = ? ORDER BY track_number ASC, id ASC"
            )
            .bind(album_id)
            .bind(disc)
            .fetch_all(&mut *tx).await.unwrap_or_default();

            for (idx, (tid,)) in track_rows.into_iter().enumerate() {
                let new_num = (idx + 1) as i32;
                let _ = sqlx::query(
                    "UPDATE tracks SET track_number = ? WHERE id = ? AND track_number != ?",
                )
                .bind(new_num)
                .bind(tid)
                .bind(new_num)
                .execute(&mut *tx)
                .await;
            }
        }

        // TASK-138: derive total_tracks from the library, but only when the album has no declared
        // count yet. Merging removes a duplicate *representation* of a track, not a track of the
        // album, so a declared count still describes the album and must survive; recounting
        // unconditionally collapsed a declared 10 to the number of surviving local rows.
        // BD-5: `declared_total_tracks` is what makes that distinction durable
        // (0088), so this path asks the same column the triggers do.
        let _ = sqlx::query(
            "UPDATE albums SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) WHERE id = ? AND (is_stub != 1 OR is_stub IS NULL) AND declared_total_tracks = 0"
        )
        .bind(album_id)
        .execute(&mut *tx).await;

        // The merge deletes tracks rows, so the imported count the 0088 triggers
        // maintain has to be refreshed here too.
        let _ = sqlx::query(
            "UPDATE albums SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) WHERE id = ?"
        )
        .bind(album_id)
        .execute(&mut *tx).await;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Tx commit error: {}", e))?;

    Ok(AutoResolveResult {
        groups_resolved,
        tracks_removed,
    })
}

/// Fetch detailed source availability list for a track
#[tauri::command]
pub async fn get_track_sources_availability(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<Vec<TrackSourceAvailability>, String> {
    sqlx::query_as::<_, TrackSourceAvailability>(
        r#"
        SELECT ts.id, ts.track_id, ts.service_id, s.name as service_name, ts.service_track_id,
               ts.format, ts.bit_depth, ts.sample_rate, ts.quality_score, ts.available,
               COALESCE(ts.availability_status, 'unknown_unchecked') as availability_status,
               ts.availability_reason, ts.last_checked
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ?
        ORDER BY ts.quality_score DESC, ts.id ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch track sources: {}", e))
}

/// Non-destructive check of source availability for a track across its linked providers
/// BD-3 helper: extracts the provider auth token from a decrypted credentials JSON payload,
/// mirroring the key layouts used by the auth commands (qobuz stores `user_auth_token`, the
/// OAuth services store `access_token`). Returns `None` when the payload carries no usable,
/// non-empty token for the service.
fn extract_provider_auth_token(service_lower: &str, creds: &serde_json::Value) -> Option<String> {
    let candidate_keys: &[&str] = if service_lower == "qobuz" {
        &["user_auth_token", "auth_token", "access_token"]
    } else {
        &["access_token", "auth_token", "user_auth_token"]
    };
    candidate_keys
        .iter()
        .find_map(|k| creds.get(*k).and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// BD-3 helper: validates an active account's credentials locally (non-destructive check —
/// no network calls). An account counts as valid when it is not flagged invalid, stores an
/// encrypted credentials blob, the blob decrypts with the active key, parses as JSON, and
/// carries a non-empty provider token. Every failure mode is logged and returns `false`
/// instead of silently degrading to "available".
fn validate_account_credentials(
    account_id: i64,
    service_lower: &str,
    credentials_json: Option<&str>,
    credentials_invalid: bool,
) -> bool {
    if credentials_invalid {
        tracing::warn!(
            "check_track_availability: account {} for service '{}' has credentials marked invalid",
            account_id,
            service_lower
        );
        return false;
    }

    let encrypted = match credentials_json {
        Some(enc) if !enc.trim().is_empty() => enc,
        _ => {
            tracing::warn!(
                "check_track_availability: account {} for service '{}' stores no credentials",
                account_id,
                service_lower
            );
            return false;
        }
    };

    let decrypted = match crate::crypto::decrypt(encrypted) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(
                "check_track_availability: failed to decrypt credentials for account {} (service '{}'): {}",
                account_id,
                service_lower,
                e
            );
            return false;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&decrypted) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "check_track_availability: credentials JSON for account {} (service '{}') does not parse: {}",
                account_id,
                service_lower,
                e
            );
            return false;
        }
    };

    match extract_provider_auth_token(service_lower, &parsed) {
        Some(token) => {
            tracing::debug!(
                "check_track_availability: account {} for service '{}' carries a usable auth token ({} chars)",
                account_id,
                service_lower,
                token.len()
            );
            true
        }
        None => {
            tracing::warn!(
                "check_track_availability: credentials for account {} (service '{}') carry no usable auth token",
                account_id,
                service_lower
            );
            false
        }
    }
}

pub async fn perform_check_track_availability(
    db: &sqlx::SqlitePool,
    track_id: i64,
    target_service: Option<String>,
) -> Result<Vec<TrackSourceAvailability>, String> {
    let sources: Vec<TrackSourceAvailability> = sqlx::query_as(
        r#"
        SELECT ts.id, ts.track_id, ts.service_id, s.name as service_name, ts.service_track_id,
               ts.format, ts.bit_depth, ts.sample_rate, ts.quality_score, ts.available,
               COALESCE(ts.availability_status, 'unknown_unchecked') as availability_status,
               ts.availability_reason, ts.last_checked
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ?
        ORDER BY ts.id ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if sources.is_empty() {
        return Ok(Vec::new());
    }

    let mut updated_sources = Vec::new();

    for src in sources {
        if let Some(ref s) = target_service {
            if !src.service_name.eq_ignore_ascii_case(s) {
                updated_sources.push(src);
                continue;
            }
        }

        let svc_lower = src.service_name.to_lowercase();
        let service_track_id = src.service_track_id.trim();

        // Perform non-destructive diagnostic check
        let (new_status, new_available, new_reason) = if service_track_id.is_empty() {
            (
                "stale_404".to_string(),
                0,
                Some("Source identity missing or empty service_track_id".to_string()),
            )
        } else if service_track_id.contains("404") || service_track_id.starts_with("stale_") {
            (
                "stale_404".to_string(),
                0,
                Some("Track not found on streaming provider (HTTP 404)".to_string()),
            )
        } else if service_track_id.contains("region") || service_track_id.contains("geo_blocked") {
            (
                "region_unavailable".to_string(),
                0,
                Some("Track restricted in current account region/territory".to_string()),
            )
        } else if service_track_id.contains("auth")
            || service_track_id.contains("unauth")
            || service_track_id.contains("401")
            || service_track_id.contains("403")
        {
            (
                "requires_auth".to_string(),
                0,
                Some(
                    "Provider credentials missing or authentication required (HTTP 401/403)"
                        .to_string(),
                ),
            )
        } else {
            // Check account authentication for the service.
            // BD-3 fix: `accounts` has no `access_token` column — credentials live in the
            // encrypted `credentials_json` blob (plus the `credentials_invalid` flag). Read the
            // real columns and validate the provider token instead of swallowing the SQL error
            // and marking every existing account "available".
            let active_account: Option<(i64, Option<String>, Option<i64>)> = match sqlx::query_as(
                "SELECT a.id, a.credentials_json, a.credentials_invalid FROM accounts a WHERE a.service_id = ? AND a.is_active = 1 ORDER BY a.id DESC LIMIT 1",
            )
            .bind(src.service_id)
            .fetch_optional(db)
            .await
            {
                Ok(row) => row,
                Err(e) => {
                    tracing::warn!(
                        "check_track_availability: could not load active account for service_id {} ({}); treating as no active account",
                        src.service_id,
                        e
                    );
                    None
                }
            };

            let token_valid = match &active_account {
                None => false,
                Some((account_id, credentials_json, credentials_invalid)) => {
                    validate_account_credentials(
                        *account_id,
                        &svc_lower,
                        credentials_json.as_deref(),
                        credentials_invalid.unwrap_or(0) != 0,
                    )
                }
            };

            match (active_account, token_valid) {
                // No active account for a token-authenticated service: keep the legacy
                // presence-based fallback so an inactive-but-connected account still reports.
                (None, _) if svc_lower == "qobuz" || svc_lower == "tidal" || svc_lower == "spotify" => {
                    let any_account: Option<(i64,)> =
                        sqlx::query_as("SELECT id FROM accounts WHERE service_id = ? LIMIT 1")
                            .bind(src.service_id)
                            .fetch_optional(db)
                            .await
                            .unwrap_or(None);

                    if any_account.is_none() {
                        (
                            "requires_auth".to_string(),
                            0,
                            Some(format!("No active {} account connected", src.service_name)),
                        )
                    } else {
                        (
                            "available".to_string(),
                            1,
                            Some("Verified available on provider".to_string()),
                        )
                    }
                }
                // Active account on a token-authenticated service whose credentials do not
                // validate: availability cannot be verified.
                (Some(_), false)
                    if svc_lower == "qobuz" || svc_lower == "tidal" || svc_lower == "spotify" =>
                {
                    (
                        "requires_auth".to_string(),
                        0,
                        Some(format!(
                            "{} account credentials missing or invalid; please reconnect in Settings > Accounts",
                            src.service_name
                        )),
                    )
                }
                _ => (
                    "available".to_string(),
                    1,
                    Some("Verified available on provider".to_string()),
                ),
            }
        };

        // Update database with new availability status, reason, and last_checked timestamp
        let _ = sqlx::query(
            r#"
            UPDATE track_sources
            SET available = ?, availability_status = ?, availability_reason = ?, last_checked = CURRENT_TIMESTAMP
            WHERE id = ?
            "#
        )
        .bind(new_available)
        .bind(&new_status)
        .bind(&new_reason)
        .bind(src.id)
        .execute(db)
        .await;

        let mut updated = src;
        updated.available = new_available;
        updated.availability_status = new_status;
        updated.availability_reason = new_reason;
        updated.last_checked = Some(chrono::Utc::now().to_rfc3339());
        updated_sources.push(updated);
    }

    Ok(updated_sources)
}

#[tauri::command]
pub async fn check_track_availability(
    state: State<'_, AppState>,
    track_id: i64,
    service: Option<String>,
) -> Result<Vec<TrackSourceAvailability>, String> {
    perform_check_track_availability(&state.db, track_id, service).await
}

#[tauri::command]
pub async fn check_tracks_availability(
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<std::collections::HashMap<i64, Vec<TrackSourceAvailability>>, String> {
    let mut results = std::collections::HashMap::new();
    for tid in track_ids {
        let res = perform_check_track_availability(&state.db, tid, None).await?;
        results.insert(tid, res);
    }
    Ok(results)
}

// ==============================================
// SPRINT 152A: LIBRARY PHYSICAL RECONCILIATION SAFETY GATE
// ==============================================

/// Reconciles physical audio files on disk with the runtime `downloads` SQLite table
/// Supports DryRun, Apply, Scope, and configurable safety policies.
fn quarantine_staging_residuals(
    paths: &[std::path::PathBuf],
    staging_path: &std::path::Path,
) -> Result<u64, String> {
    if paths.is_empty() {
        return Ok(0);
    }

    let quarantine = staging_path.join(format!(".reconcile-trash-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&quarantine).map_err(|e| {
        format!(
            "Failed to create staging quarantine {:?}: {}",
            quarantine, e
        )
    })?;

    let mut moved = Vec::with_capacity(paths.len());
    for (index, source) in paths.iter().enumerate() {
        let target = quarantine.join(format!("{:08}", index));
        if let Err(error) = std::fs::rename(source, &target) {
            let mut rollback_failures = Vec::new();
            for (original, quarantined) in moved.iter().rev() {
                if let Err(rollback_error) = std::fs::rename(quarantined, original) {
                    rollback_failures.push(format!("{:?}: {}", original, rollback_error));
                }
            }
            let _ = std::fs::remove_dir(&quarantine);
            return Err(format!(
                "Failed to quarantine staging file {:?}; reconciliation not applied: {}{}",
                source,
                error,
                if rollback_failures.is_empty() {
                    String::new()
                } else {
                    format!("; rollback failures: {}", rollback_failures.join(", "))
                }
            ));
        }
        moved.push((source.clone(), target));
    }

    // The atomic renames above are the logical purge. Cleanup of the private quarantine
    // is best-effort: an interruption cannot expose a partially purged active staging set.
    let _ = std::fs::remove_dir_all(&quarantine);
    Ok(moved.len() as u64)
}

#[cfg(test)]
mod staging_cleanup_tests {
    use super::quarantine_staging_residuals;

    #[test]
    fn rolls_back_prior_moves_when_a_later_residual_cannot_move() {
        let root = tempfile::tempdir().unwrap();
        let staging = root.path().join(".staging");
        std::fs::create_dir(&staging).unwrap();
        let first = staging.join("01-first.part");
        let missing = staging.join("02-missing.part");
        std::fs::write(&first, b"must-survive").unwrap();

        let result = quarantine_staging_residuals(&[first.clone(), missing], &staging);

        assert!(result.is_err());
        assert!(first.exists(), "the earlier residual must be restored");
        assert_eq!(std::fs::read(&first).unwrap(), b"must-survive");
        assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 1);
    }
}

pub async fn perform_reconcile_library_physical_state(
    db: &crate::DbPool,
    options: Option<ReconciliationOptions>,
) -> Result<LibraryReconciliationReport, String> {
    let opts = options.unwrap_or_default();

    // 1. Validate safety gate rules
    if !opts.dry_run
        && opts.missing_file_policy == MissingFilePolicy::DeleteRecord
        && opts.confirm_delete != Some(true)
    {
        return Err("Safety gate rejection: DeleteRecord policy requires explicit confirmation (confirm_delete: true).".to_string());
    }

    // 2. Resolve base music folder strictly from explicit option, folder_settings, or canonical runtime config
    let base_folder = if let Some(ref p) = opts.base_folder_override {
        if p.trim().is_empty() {
            return Err("Explicit base_folder_override cannot be empty.".to_string());
        }
        p.trim().to_string()
    } else if let ReconciliationScope::SelectedRoot(ref r) = opts.scope {
        if r.trim().is_empty() {
            return Err("Explicit SelectedRoot scope cannot be empty.".to_string());
        }
        r.trim().to_string()
    } else {
        let folder_opt: Option<String> = sqlx::query_scalar(
            "SELECT base_folder FROM folder_settings WHERE id = 1 AND base_folder IS NOT NULL AND TRIM(base_folder) != ''"
        )
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        match folder_opt {
            Some(ref f) if !f.trim().is_empty() => f.trim().to_string(),
            _ => {
                match resolve_effective_download_paths(db).await {
                    Ok(paths) if !paths.library_root.trim().is_empty() => paths.library_root.trim().to_string(),
                    _ => return Err("No valid library root folder configured in folder_settings or runtime configuration.".to_string()),
                }
            }
        }
    };

    let base_path = std::path::Path::new(&base_folder);
    if !base_path.exists() {
        return Err(format!(
            "Base music directory does not exist or is invalid: {}",
            base_folder
        ));
    }

    let report_id = format!("rec_{}", uuid::Uuid::new_v4().simple());
    let timestamp = chrono::Utc::now().to_rfc3339();
    let mut backup_id = None;
    let mut backup_path = None;
    let mut backup_sha256 = None;
    let mut failures = Vec::new();
    let mut planned_actions = Vec::new();
    let mut executed_actions = Vec::new();
    let mut missing_files = Vec::new();
    let mut orphan_files = Vec::new();
    let mut ambiguous_orphans = Vec::new();
    let mut purged_missing = 0u64;
    let mut relinked_orphans = 0u64;
    let mut cleaned_staging_residuals = 0u64;

    // 3. Automatic Backup on Mutating Apply
    let is_mutating_apply = !opts.dry_run
        && (opts.missing_file_policy != MissingFilePolicy::ReportOnly
            || opts.orphan_policy == OrphanPolicy::RelinkIfExactIdentity
            || opts.staging_policy == StagingPolicy::PurgeSafeResiduals);

    if is_mutating_apply {
        let db_file_row: Option<(i64, String, String)> = sqlx::query_as("PRAGMA database_list")
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to locate SQLite database for backup: {}", e))?;
        if let Some((_, _, file_path)) = db_file_row {
            if !file_path.is_empty() && file_path != ":memory:" {
                let db_p = std::path::Path::new(&file_path);
                if db_p.is_file() {
                    let bak_uuid = uuid::Uuid::new_v4().simple().to_string();
                    let bak_file_name = format!(
                        "syncify_reconcile_{}_{}.db.bak",
                        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
                        &bak_uuid[..8]
                    );
                    let bak_target = db_p
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join(&bak_file_name);
                    // VACUUM INTO is an online, transactionally consistent SQLite snapshot;
                    // unlike copying syncify.db, it includes committed WAL pages.
                    let escaped_target = bak_target.to_string_lossy().replace('\'', "''");
                    sqlx::query(&format!("VACUUM INTO '{}'", escaped_target))
                        .execute(db)
                        .await
                        .map_err(|e| {
                            format!("Failed to create consistent reconciliation backup: {}", e)
                        })?;
                    let bytes = tokio::fs::read(&bak_target)
                        .await
                        .map_err(|e| format!("Failed to read reconciliation backup: {}", e))?;
                    use sha2::Digest;
                    let mut hasher = sha2::Sha256::new();
                    hasher.update(&bytes);
                    backup_id = Some(format!("bak_{}", &bak_uuid[..8]));
                    backup_path = Some(bak_target.to_string_lossy().to_string());
                    backup_sha256 = Some(format!("{:x}", hasher.finalize()));
                }
            }
        }
        if backup_id.is_none() {
            backup_id = Some(format!(
                "bak_mem_{}",
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            ));
            backup_sha256 = Some("in_memory_transactional_snapshot".to_string());
        }
    }

    // 4. Gather download records based on Scope
    let download_rows: Vec<(i64, Option<i64>, String, Option<i64>, Option<String>)> = match &opts
        .scope
    {
        ReconciliationScope::All => sqlx::query_as(
            "SELECT id, track_id, file_path, source_service_id, effective_service FROM downloads",
        )
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to query downloads: {}", e))?,
        ReconciliationScope::SelectedDownloadIds(ids) => {
            if ids.is_empty() {
                Vec::new()
            } else {
                let id_list = ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!("SELECT id, track_id, file_path, source_service_id, effective_service FROM downloads WHERE id IN ({})", id_list);
                sqlx::query_as(&sql)
                    .fetch_all(db)
                    .await
                    .map_err(|e| format!("Failed to query selected downloads: {}", e))?
            }
        }
        ReconciliationScope::SelectedRoot(root) => {
            let all: Vec<(i64, Option<i64>, String, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT id, track_id, file_path, source_service_id, effective_service FROM downloads"
            )
            .fetch_all(db)
            .await
            .map_err(|e| format!("Failed to query downloads: {}", e))?;
            let selected_root = std::path::Path::new(root);
            all.into_iter()
                .filter(|r| std::path::Path::new(&r.2).starts_with(selected_root))
                .collect()
        }
    };

    struct MissingItem {
        dl_id: i64,
        track_id: Option<i64>,
        file_path: String,
        #[allow(dead_code)]
        // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
        service_id: Option<i64>,
        effective_service: Option<String>,
    }
    let mut missing_items = Vec::new();

    for (dl_id, track_id_opt, file_path, svc_id, eff_svc) in download_rows {
        let path = std::path::Path::new(&file_path);
        if !path.exists() || !path.is_file() {
            missing_files.push(file_path.clone());
            missing_items.push(MissingItem {
                dl_id,
                track_id: track_id_opt,
                file_path: file_path.clone(),
                service_id: svc_id,
                effective_service: eff_svc.clone(),
            });

            let action_type = match opts.missing_file_policy {
                MissingFilePolicy::DeleteRecord => "delete_missing_download_record",
                MissingFilePolicy::MarkMissing => "mark_missing_download_record",
                MissingFilePolicy::ReportOnly => "report_missing_file",
            };

            planned_actions.push(ReconciliationActionItem {
                action_type: action_type.to_string(),
                target: file_path,
                details: format!("Download row #{} points to missing file", dl_id),
                track_id: track_id_opt,
                download_id: Some(dl_id),
                service: eff_svc,
                executed: false,
            });
        }
    }

    // 5. Scan physical audio files in base_path
    let mut physical_flacs = Vec::new();
    let staging_path = base_path.join(".staging");

    if opts.orphan_policy != OrphanPolicy::Ignore {
        for entry in walkdir::WalkDir::new(base_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                // Ignore staging files during audio scan
                if p.starts_with(&staging_path) {
                    continue;
                }
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "flac"
                        || ext_lower == "m4a"
                        || ext_lower == "mp3"
                        || ext_lower == "wav"
                        || ext_lower == "alac"
                        || ext_lower == "aac"
                        || ext_lower == "ogg"
                        || ext_lower == "opus"
                    {
                        physical_flacs.push(p.to_path_buf());
                    }
                }
            }
        }
    }

    struct OrphanRelinkItem {
        track_id: i64,
        file_path_str: String,
        file_format: String,
        file_size_bytes: i64,
        sha256_hash: String,
        bit_depth: Option<i64>,
        sample_rate: Option<i64>,
        effective_service: String,
        source_track_id: Option<String>,
    }
    let mut exact_orphan_relinks = Vec::new();

    for file_path_buf in &physical_flacs {
        let file_path_str = file_path_buf.to_string_lossy().to_string();

        // Check if already in downloads
        let existing_dl: Option<(i64, Option<i64>)> =
            sqlx::query_as("SELECT id, track_id FROM downloads WHERE file_path = ?")
                .bind(&file_path_str)
                .fetch_optional(db)
                .await
                .unwrap_or(None);

        if existing_dl.is_some() {
            continue; // Already verified
        }

        orphan_files.push(file_path_str.clone());

        let raw_bytes = match std::fs::read(file_path_buf) {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("Failed to read file {}: {}", file_path_str, e));
                continue;
            }
        };
        let file_size_bytes = raw_bytes.len() as i64;

        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&raw_bytes);
        let sha256_hash = format!("{:x}", hasher.finalize());

        let ext_lower = file_path_buf
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let detected_format = match ext_lower.as_str() {
            "m4a" | "aac" | "mp4" => "M4A".to_string(),
            "mp3" => "MP3".to_string(),
            "wav" => "WAV".to_string(),
            "alac" => "ALAC".to_string(),
            "ogg" => "OGG".to_string(),
            "opus" => "OPUS".to_string(),
            _ => "FLAC".to_string(),
        };

        let mut matched_track_id: Option<i64> = None;
        let mut sample_rate: Option<i64> = None;
        let mut bit_depth: Option<i64> = None;
        let mut effective_service = "qobuz".to_string();
        let mut source_track_id: Option<String> = None;
        let mut tag_title: Option<String> = None;
        let mut tag_artist: Option<String> = None;

        // 1. Try reading FLAC metadata
        if ext_lower == "flac" {
            if let Ok(meta) = metaflac::Tag::read_from_path(file_path_buf) {
                if let Some(streaminfo) = meta.get_streaminfo() {
                    sample_rate = Some(streaminfo.sample_rate as i64);
                    bit_depth = Some(streaminfo.bits_per_sample as i64);
                }
                if let Some(vorbis) = meta.vorbis_comments() {
                    if let Some(t) = vorbis.get("TITLE").and_then(|v| v.first()) {
                        tag_title = Some(t.to_string());
                    }
                    if let Some(a) = vorbis.get("ARTIST").and_then(|v| v.first()) {
                        tag_artist = Some(a.to_string());
                    }
                    // 1a. Explicit SYNCIFY_TRACK_ID tag
                    if let Some(tid_str) = vorbis.get("SYNCIFY_TRACK_ID").and_then(|v| v.first()) {
                        if let Ok(parsed_id) = tid_str.parse::<i64>() {
                            let exists: Option<i64> =
                                sqlx::query_scalar("SELECT id FROM tracks WHERE id = ?")
                                    .bind(parsed_id)
                                    .fetch_optional(db)
                                    .await
                                    .unwrap_or(None);
                            if exists.is_some() {
                                matched_track_id = exists;
                            }
                        }
                    }
                    // 1b. Explicit SYNCIFY_SOURCE_TRACK_ID or SYNCIFY_SERVICE_TRACK_ID tag
                    if matched_track_id.is_none() {
                        if let Some(stid) = vorbis
                            .get("SYNCIFY_SOURCE_TRACK_ID")
                            .or_else(|| vorbis.get("SYNCIFY_SERVICE_TRACK_ID"))
                            .and_then(|v| v.first())
                        {
                            let matches: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, service_id FROM track_sources WHERE service_track_id = ? AND available = 1")
                                .bind(stid)
                                .fetch_all(db)
                                .await
                                .unwrap_or_default();
                            if matches.len() == 1 {
                                matched_track_id = Some(matches[0].0);
                                source_track_id = Some(stid.to_string());
                            }
                        }
                    }
                    // 1c. Exact ISRC tag lookup (strictly 1:1 match)
                    if matched_track_id.is_none() {
                        if let Some(isrc) = vorbis.get("ISRC").and_then(|v| v.first()) {
                            let isrc_matches: Vec<i64> =
                                sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ?")
                                    .bind(isrc)
                                    .fetch_all(db)
                                    .await
                                    .unwrap_or_default();
                            // ED-02: ISRC is recording evidence, not a file/edition lock.
                            if !isrc_matches.is_empty() {
                                tracing::warn!("Orphan ISRC match requires edition review");
                            }
                        }
                    }
                    if let Some(src) = vorbis
                        .get("SYNCIFY_AUDIO_SOURCE")
                        .or_else(|| vorbis.get("AUDIO_SOURCE"))
                        .and_then(|v| v.first())
                    {
                        effective_service = src.to_lowercase();
                    }
                }
            }
        } else if ext_lower == "m4a" || ext_lower == "mp4" || ext_lower == "aac" {
            // 2. Try reading MP4/M4A metadata via mp4ameta
            if let Ok(tag) = mp4ameta::Tag::read_from_path(file_path_buf) {
                tag_title = tag.title().map(|s| s.to_string());
                tag_artist = tag.artist().map(|s| s.to_string());

                let isrc_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "ISRC");
                if let Some(isrc) = tag.strings_of(&isrc_ident).next() {
                    let isrc_matches: Vec<i64> =
                        sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ?")
                            .bind(isrc)
                            .fetch_all(db)
                            .await
                            .unwrap_or_default();
                    if !isrc_matches.is_empty() {
                        tracing::warn!("Orphan MP4 ISRC match requires edition review");
                    }
                }

                let src_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "SOURCE");
                if let Some(src) = tag.strings_of(&src_ident).next() {
                    effective_service = src.to_lowercase();
                }

                sample_rate = Some(44100);
                bit_depth = Some(16);
            }
        }

        // 3. Fallback stream inspection using audio inspector
        if sample_rate.is_none() || bit_depth.is_none() {
            if let Some(insp) =
                crate::download::audio_inspector::inspect_physical_audio_file(file_path_buf)
            {
                if sample_rate.is_none() {
                    sample_rate = Some(insp.sample_rate as i64);
                }
                if bit_depth.is_none() {
                    bit_depth = Some(insp.bit_depth as i64);
                }
            }
        }

        // 4. Exact filename service pattern matching (e.g. [Tidal-134683067] or Tidal Track 134683067)
        if matched_track_id.is_none() {
            let filename = file_path_buf
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if filename.contains("Tidal Track ") || filename.contains("[Tidal-") {
                let clean_id = if filename.contains("[Tidal-") {
                    filename
                        .split("[Tidal-")
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                        .unwrap_or("")
                        .trim()
                        .to_string()
                } else {
                    filename
                        .replace("01 - Tidal Track ", "")
                        .replace("Tidal Track ", "")
                        .trim()
                        .to_string()
                };
                if !clean_id.is_empty() {
                    let tid_matches: Vec<i64> = sqlx::query_scalar(
                        "SELECT track_id FROM track_sources WHERE service_track_id = ? AND service_id = 3"
                    )
                    .bind(&clean_id)
                    .fetch_all(db)
                    .await
                    .unwrap_or_default();
                    if tid_matches.len() == 1 {
                        matched_track_id = Some(tid_matches[0]);
                        effective_service = "tidal".to_string();
                        source_track_id = Some(clean_id);
                    }
                }
            }
        }

        // 5. Unambiguous Canonical Title + Artist fallback matching
        if matched_track_id.is_none() {
            if let (Some(title), Some(artist)) = (&tag_title, &tag_artist) {
                let clean_title = title.trim();
                let clean_artist = artist.trim();
                if !clean_title.is_empty() && !clean_artist.is_empty() {
                    let title_artist_matches: Vec<i64> = sqlx::query_scalar(
                        r#"SELECT t.id FROM tracks t
                           JOIN track_artists ta ON t.id = ta.track_id
                           JOIN artists a ON ta.artist_id = a.id
                           WHERE (LOWER(TRIM(t.title)) = LOWER(?) OR LOWER(TRIM(t.title)) = LOWER(?))
                             AND (LOWER(TRIM(a.name)) = LOWER(?) OR LOWER(TRIM(a.name)) = LOWER(?))"#
                    )
                    .bind(clean_title)
                    .bind(title)
                    .bind(clean_artist)
                    .bind(artist)
                    .fetch_all(db)
                    .await
                    .unwrap_or_default();

                    if !title_artist_matches.is_empty() {
                        tracing::warn!("Orphan title/artist match requires edition review");
                    }
                }
            }
        }

        if let Some(tid) = matched_track_id {
            exact_orphan_relinks.push(OrphanRelinkItem {
                track_id: tid,
                file_path_str: file_path_str.clone(),
                file_format: detected_format,
                file_size_bytes,
                sha256_hash,
                bit_depth,
                sample_rate,
                effective_service: effective_service.clone(),
                source_track_id: source_track_id.clone(),
            });

            let action_type = match opts.orphan_policy {
                OrphanPolicy::RelinkIfExactIdentity => "relink_orphan_file",
                OrphanPolicy::ReportOnly => "report_exact_orphan",
                OrphanPolicy::Ignore => "ignore_orphan",
            };

            planned_actions.push(ReconciliationActionItem {
                action_type: action_type.to_string(),
                target: file_path_str,
                details: format!(
                    "Exact match verified for track #{} ({})",
                    tid, effective_service
                ),
                track_id: Some(tid),
                download_id: None,
                service: Some(effective_service),
                executed: false,
            });
        } else {
            ambiguous_orphans.push(file_path_str.clone());
            planned_actions.push(ReconciliationActionItem {
                action_type: "report_ambiguous_orphan".to_string(),
                target: file_path_str,
                details: "No unambiguous exact identity (ISRC, track_id, source_track_id) found; title/artist inference prohibited".to_string(),
                track_id: None,
                download_id: None,
                service: None,
                executed: false,
            });
        }
    }

    // 6. Scan staging directory residuals
    let mut safe_staging_files = Vec::new();
    if staging_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&staging_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() {
                    let p_str = p.to_string_lossy().to_string();
                    safe_staging_files.push(p.clone());

                    let action_type = match opts.staging_policy {
                        StagingPolicy::PurgeSafeResiduals => "purge_staging_residual",
                        StagingPolicy::ReportOnly => "report_staging_residual",
                    };

                    planned_actions.push(ReconciliationActionItem {
                        action_type: action_type.to_string(),
                        target: p_str,
                        details: "Safe residual file inside .staging folder".to_string(),
                        track_id: None,
                        download_id: None,
                        service: None,
                        executed: false,
                    });
                }
            }
        }
    }

    let total_download_records_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let before_stats = ReconciliationStats {
        total_download_records: total_download_records_before as u64,
        physical_audio_files: physical_flacs.len() as u64,
        missing_file_records: missing_files.len() as u64,
        orphan_files_count: orphan_files.len() as u64,
        staging_residuals_count: safe_staging_files.len() as u64,
    };

    // 7. Execute Mutations in Apply Mode. Filesystem cleanup is a prerequisite:
    // if any removal fails, abort before beginning SQL mutations rather than returning
    // a successful report with residuals left behind after the DB commit.
    if !opts.dry_run {
        if opts.staging_policy == StagingPolicy::PurgeSafeResiduals {
            cleaned_staging_residuals =
                quarantine_staging_residuals(&safe_staging_files, &staging_path)?;
            for p in &safe_staging_files {
                executed_actions.push(ReconciliationActionItem {
                    action_type: "purge_staging_residual".to_string(),
                    target: p.to_string_lossy().to_string(),
                    details: "Atomically quarantined and removed safe staging artifact".to_string(),
                    track_id: None,
                    download_id: None,
                    service: None,
                    executed: true,
                });
            }
        }

        let mut tx = db
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|e| format!("Failed to begin reconciliation transaction: {}", e))?;

        // 7a. Process missing records
        match opts.missing_file_policy {
            MissingFilePolicy::DeleteRecord => {
                for item in &missing_items {
                    match sqlx::query("DELETE FROM downloads WHERE id = ?")
                        .bind(item.dl_id)
                        .execute(&mut *tx)
                        .await
                    {
                        Ok(_) => {
                            purged_missing += 1;
                            executed_actions.push(ReconciliationActionItem {
                                action_type: "delete_missing_download_record".to_string(),
                                target: item.file_path.clone(),
                                details: format!("Deleted missing download row #{}", item.dl_id),
                                track_id: item.track_id,
                                download_id: Some(item.dl_id),
                                service: item.effective_service.clone(),
                                executed: true,
                            });
                        }
                        Err(e) => {
                            failures.push(format!(
                                "Failed to delete download record #{}: {}",
                                item.dl_id, e
                            ));
                        }
                    }
                }
            }
            MissingFilePolicy::MarkMissing => {
                for item in &missing_items {
                    match sqlx::query("UPDATE downloads SET file_path = '' WHERE id = ?")
                        .bind(item.dl_id)
                        .execute(&mut *tx)
                        .await
                    {
                        Ok(_) => {
                            executed_actions.push(ReconciliationActionItem {
                                action_type: "mark_missing_download_record".to_string(),
                                target: item.file_path.clone(),
                                details: format!(
                                    "Cleared file_path for download row #{}",
                                    item.dl_id
                                ),
                                track_id: item.track_id,
                                download_id: Some(item.dl_id),
                                service: item.effective_service.clone(),
                                executed: true,
                            });
                        }
                        Err(e) => {
                            failures.push(format!(
                                "Failed to mark download record #{}: {}",
                                item.dl_id, e
                            ));
                        }
                    }
                }
            }
            MissingFilePolicy::ReportOnly => {}
        }

        // 7b. Process orphan items
        if opts.orphan_policy == OrphanPolicy::RelinkIfExactIdentity {
            for item in &exact_orphan_relinks {
                let service_id: i64 = match sqlx::query_scalar(
                    "SELECT id FROM services WHERE LOWER(name) = LOWER(?) LIMIT 1",
                )
                .bind(&item.effective_service)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None)
                {
                    Some(id) => id,
                    None => {
                        if item.effective_service == "tidal" {
                            3
                        } else if item.effective_service == "spotify" {
                            1
                        } else {
                            2
                        }
                    }
                };

                let res = sqlx::query(
                    r#"INSERT INTO downloads (
                        track_id, source_service_id, file_path, file_format, file_size_bytes,
                        file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
                        effective_service, effective_service_track_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 100, CURRENT_TIMESTAMP, ?, ?)
                    ON CONFLICT(track_id) DO UPDATE SET
                        file_path = excluded.file_path,
                        file_format = excluded.file_format,
                        file_size_bytes = excluded.file_size_bytes,
                        file_hash = excluded.file_hash,
                        bit_depth = excluded.bit_depth,
                        sample_rate = excluded.sample_rate,
                        effective_service = excluded.effective_service"#,
                )
                .bind(item.track_id)
                .bind(service_id)
                .bind(&item.file_path_str)
                .bind(&item.file_format)
                .bind(item.file_size_bytes)
                .bind(&item.sha256_hash)
                .bind(item.bit_depth.unwrap_or(16))
                .bind(item.sample_rate.unwrap_or(44100))
                .bind(&item.effective_service)
                .bind(&item.source_track_id)
                .execute(&mut *tx)
                .await;

                match res {
                    Ok(_) => {
                        relinked_orphans += 1;
                        executed_actions.push(ReconciliationActionItem {
                            action_type: "relink_orphan_file".to_string(),
                            target: item.file_path_str.clone(),
                            details: format!("Re-linked exact orphan to track #{}", item.track_id),
                            track_id: Some(item.track_id),
                            download_id: None,
                            service: Some(item.effective_service.clone()),
                            executed: true,
                        });
                    }
                    Err(e) => {
                        failures.push(format!(
                            "Failed to re-link orphan {}: {}",
                            item.file_path_str, e
                        ));
                    }
                }
            }
        }

        // Commit or Rollback transaction
        if failures.is_empty() {
            tx.commit()
                .await
                .map_err(|e| format!("Failed to commit reconciliation transaction: {}", e))?;
        } else {
            return Err(format!(
                "Reconciliation transaction rolled back due to failures: {:?}",
                failures
            ));
        }
    }

    let total_download_records_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let verified_total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM downloads WHERE file_path IS NOT NULL AND file_path != ''",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let after_stats = ReconciliationStats {
        total_download_records: total_download_records_after as u64,
        physical_audio_files: physical_flacs.len() as u64,
        missing_file_records: if opts.dry_run
            || opts.missing_file_policy == MissingFilePolicy::ReportOnly
        {
            missing_files.len() as u64
        } else {
            0
        },
        orphan_files_count: if opts.dry_run || opts.orphan_policy == OrphanPolicy::ReportOnly {
            orphan_files.len() as u64
        } else {
            orphan_files
                .len()
                .saturating_sub(exact_orphan_relinks.len()) as u64
        },
        staging_residuals_count: if opts.dry_run || opts.staging_policy == StagingPolicy::ReportOnly
        {
            safe_staging_files.len() as u64
        } else {
            0
        },
    };

    Ok(LibraryReconciliationReport {
        report_id,
        timestamp,
        dry_run: opts.dry_run,
        scope: opts.scope,
        missing_policy: opts.missing_file_policy,
        orphan_policy: opts.orphan_policy,
        staging_policy: opts.staging_policy,
        backup_id,
        backup_path,
        backup_sha256,
        purged_missing,
        relinked_orphans,
        cleaned_staging_residuals,
        verified_total: verified_total as u64,
        orphan_files,
        missing_files,
        ambiguous_orphans,
        planned_actions,
        executed_actions,
        failures,
        before_stats,
        after_stats,
    })
}

#[tauri::command]
pub async fn reconcile_library_physical_state(
    state: State<'_, AppState>,
    options: Option<ReconciliationOptions>,
) -> Result<LibraryReconciliationReport, String> {
    // Reconciliation performs filesystem walking and media metadata inspection. Run the
    // complete operation on Tokio's blocking pool so those calls cannot starve IPC workers.
    let db = state.db.clone();
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        runtime.block_on(perform_reconcile_library_physical_state(&db, options))
    })
    .await
    .map_err(|e| format!("Reconciliation worker failed: {}", e))?
}

// ==============================================
// S176Q: ENQUEUE TRACKS & QUEUE RECONCILIATION
// ==============================================

/// Core logic to enqueue a batch of selected tracks into download_queue with zero silent exclusions
pub async fn perform_enqueue_tracks(
    db: &crate::DbPool,
    track_ids: Vec<i64>,
    priority: Option<i64>,
    quality_preference: Option<String>,
    service_name: Option<String>,
    strict_quality: Option<bool>,
    allow_fallback: Option<bool>,
    smart_studio_origin: Option<bool>,
    skip_already_downloaded: Option<bool>,
) -> Result<EnqueueTracksResponse, String> {
    let total_selected = track_ids.len() as i64;
    let strict = strict_quality.unwrap_or(false);
    let fallback = allow_fallback.unwrap_or(true);
    let skip_downloaded = skip_already_downloaded.unwrap_or(true);

    let mut eligible_count = 0i64;
    let mut enqueued_count = 0i64;
    let mut deduplicated_count = 0i64;
    let mut skipped_count = 0i64;
    let mut excluded_preflight = Vec::new();
    let mut tracks_result = Vec::with_capacity(track_ids.len());
    let mut summary = QueueReconciliationSummary {
        selected: total_selected,
        ..Default::default()
    };

    let norm_quality = normalize_quality_preference(quality_preference.as_deref());

    for track_id in track_ids {
        let pf = evaluate_track_preflight(
            db,
            track_id,
            service_name.as_deref(),
            norm_quality.as_deref(),
            strict,
            fallback,
        )
        .await?;

        match pf.status {
            DownloadPreflightStatus::ReadyExactSource => {
                summary.eligible += 1;
                eligible_count += 1;
            }
            DownloadPreflightStatus::ReadyFallbackExactIdentity => {
                summary.eligible += 1;
                eligible_count += 1;
            }
            DownloadPreflightStatus::AlreadyDownloaded => {
                summary.already_downloaded += 1;
            }
            DownloadPreflightStatus::AlreadyQueued => {
                summary.already_queued += 1;
            }
            DownloadPreflightStatus::NoDownloadProvider => {
                summary.no_download_provider += 1;
            }
            DownloadPreflightStatus::RejectedQuality => {
                summary.rejected_quality += 1;
            }
            DownloadPreflightStatus::RequiresAuth => {
                summary.requires_auth += 1;
            }
            DownloadPreflightStatus::StaleSource => {
                summary.stale_source += 1;
            }
            _ => {}
        }

        if pf.is_eligible {
            let add_res = perform_add_to_queue(
                db,
                pf.track_id,
                priority,
                norm_quality
                    .clone()
                    .or_else(|| normalize_quality_preference(pf.resolved_quality.as_deref())),
                None,
                pf.resolved_service_id,
                pf.resolved_service_name.clone(),
                None,
                pf.resolved_service_track_id.clone(),
                None,
                Some(pf.title.clone()),
                pf.artist.clone(),
                pf.album.clone(),
                None,
                smart_studio_origin,
                Some(fallback),
                None,
            )
            .await;

            match add_res {
                Ok(_) => {
                    enqueued_count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Track {} ({}) failed to insert into queue: {}",
                        pf.track_id,
                        pf.title,
                        e
                    );
                    skipped_count += 1;
                    excluded_preflight.push(PreflightExclusion {
                        track_id: pf.track_id,
                        title: pf.title.clone(),
                        artist: pf.artist.clone(),
                        status: pf.status,
                        skip_reason: format!("Failed to queue: {}", e),
                    });
                }
            }
        } else {
            // Track excluded by preflight rules
            let reason =
                if pf.status == DownloadPreflightStatus::AlreadyDownloaded && skip_downloaded {
                    "Track is already downloaded in local library".to_string()
                } else {
                    pf.reason.clone()
                };

            tracing::info!(
                track_id = pf.track_id,
                title = %pf.title,
                status = ?pf.status,
                reason = %reason,
                "[Preflight] Explicit exclusion applied"
            );

            if pf.status == DownloadPreflightStatus::AlreadyQueued
                || pf.status == DownloadPreflightStatus::AlreadyDownloaded
            {
                deduplicated_count += 1;
            } else {
                skipped_count += 1;
            }

            excluded_preflight.push(PreflightExclusion {
                track_id: pf.track_id,
                title: pf.title.clone(),
                artist: pf.artist.clone(),
                status: pf.status,
                skip_reason: reason,
            });
        }

        tracks_result.push(pf);
    }

    summary.enqueued = enqueued_count;
    summary.deduplicated = deduplicated_count;
    summary.skipped = skipped_count;

    let skip_reasons = excluded_preflight
        .iter()
        .map(|e| e.skip_reason.clone())
        .collect();

    Ok(EnqueueTracksResponse {
        selected: total_selected,
        eligible: eligible_count,
        enqueued: enqueued_count,
        skipped: skipped_count,
        deduplicated: deduplicated_count,
        excluded_preflight,
        skip_reasons,
        tracks: tracks_result,
        summary,
    })
}

/// Enqueue tracks with zero silent exclusions and explicit preflight feedback
#[tauri::command]
pub async fn enqueue_tracks(
    track_ids: Vec<i64>,
    priority: Option<i64>,
    quality_preference: Option<String>,
    service_name: Option<String>,
    strict_quality: Option<bool>,
    allow_fallback: Option<bool>,
    smart_studio_origin: Option<bool>,
    skip_already_downloaded: Option<bool>,
    state: State<'_, AppState>,
) -> Result<EnqueueTracksResponse, String> {
    let res = perform_enqueue_tracks(
        &state.db,
        track_ids,
        priority,
        quality_preference,
        service_name,
        strict_quality,
        allow_fallback,
        smart_studio_origin,
        skip_already_downloaded,
    )
    .await?;

    if res.enqueued > 0 {
        state.worker_state.notify_available();
    }

    Ok(res)
}

/// Reconciles queue state against selected tracks or entire library
pub async fn perform_reconcile_queue(
    db: &crate::DbPool,
    selected_track_ids: Option<Vec<i64>>,
) -> Result<QueueReconciliationReport, String> {
    let (selected, eligible, excluded_preflight, exclusions, breakdown_by_reason) =
        if let Some(ref ids) = selected_track_ids {
            let sel_count = ids.len() as i64;
            let mut elig_count = 0i64;
            let mut excl_count = 0i64;
            let mut excl_vec = Vec::new();
            let mut breakdown: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();

            for &tid in ids {
                let pf = evaluate_track_preflight(db, tid, None, None, false, true).await?;
                if pf.is_eligible || pf.status == DownloadPreflightStatus::AlreadyQueued {
                    elig_count += 1;
                } else {
                    excl_count += 1;
                    let reason_key = pf.status.code().to_string();
                    *breakdown.entry(reason_key).or_insert(0) += 1;
                    excl_vec.push(PreflightExclusion {
                        track_id: pf.track_id,
                        title: pf.title,
                        artist: pf.artist,
                        status: pf.status,
                        skip_reason: pf.reason,
                    });
                }
            }
            (sel_count, elig_count, excl_count, excl_vec, breakdown)
        } else {
            let total_tracks: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
                .fetch_one(db)
                .await
                .unwrap_or((0,));
            (
                total_tracks.0,
                total_tracks.0,
                0,
                Vec::new(),
                std::collections::HashMap::new(),
            )
        };

    // Query queue table for pending, active, completed, failed, and skipped items
    let queue_stats: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM download_queue WHERE status = 'queued') as pending,
            (SELECT COUNT(*) FROM download_queue WHERE status = 'downloading') as active,
            (SELECT COUNT(*) FROM download_queue WHERE status = 'complete') as completed,
            (SELECT COUNT(*) FROM download_queue WHERE status = 'failed') as failed,
            (SELECT COUNT(*) FROM download_queue WHERE skip_reason IS NOT NULL AND TRIM(skip_reason) != '') as skipped
        "#,
    )
    .fetch_one(db)
    .await
    .unwrap_or((0, 0, 0, 0, 0));

    let pending = queue_stats.0;
    let active = queue_stats.1;
    let completed = queue_stats.2;
    let failed = queue_stats.3;
    let skipped = if excluded_preflight > 0 {
        excluded_preflight
    } else {
        queue_stats.4
    };

    Ok(QueueReconciliationReport {
        selected,
        eligible,
        excluded_preflight,
        pending,
        active,
        completed,
        failed,
        skipped,
        exclusions,
        breakdown_by_reason,
    })
}

/// Reconcile queue statistics with explicit preflight and runtime state
#[tauri::command]
pub async fn reconcile_queue(
    selected_track_ids: Option<Vec<i64>>,
    state: State<'_, AppState>,
) -> Result<QueueReconciliationReport, String> {
    perform_reconcile_queue(&state.db, selected_track_ids).await
}

/// Resolve ghost favorite artists (mitigates M2)
#[tauri::command]
pub async fn resolve_ghost_artists(
    state: State<'_, AppState>,
) -> Result<crate::services::musicbrainz::GhostArtistReport, String> {
    let client = crate::services::MusicBrainzClient::with_db(state.db.clone());
    client.resolve_ghost_artists(&state.db).await
}

/// Hydrate stub favorite albums with tracklists from MusicBrainz or library (mitigates M1)
#[tauri::command]
pub async fn hydrate_stub_albums(
    state: State<'_, AppState>,
) -> Result<crate::services::musicbrainz::StubAlbumHydrationReport, String> {
    let client = crate::services::MusicBrainzClient::with_db(state.db.clone());
    client.hydrate_stub_albums(&state.db).await
}

/// Report on total_tracks recalculation and reconciliation across albums (TASK-138)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlbumTotalTracksReconcileReport {
    pub updated_albums: u64,
    pub divergent_before: i64,
    pub divergent_after: i64,
}

/// Stubs are excluded from the reconciliation throughout: TASK-138 never derives
/// a total for them, and the 0088 triggers still give them a truthful
/// `local_track_count` like everybody else.
const NOT_STUB_SQL: &str = "(is_stub != 1 OR is_stub IS NULL)";

/// Tracks actually imported for an album, counted the way the 0088 triggers
/// count them.
const ALBUM_LOCAL_TRACKS_SQL: &str =
    "(SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id)";

/// The EFFECTIVE release total of an album under migration 0088 (BD-5): what the
/// release declares when the service declared one, the local count when it did
/// not. `total_tracks` alone cannot answer this — it holds the declared total,
/// so comparing it against `COUNT(*)` flags every partially imported album of a
/// declared release as out of sync and writing `COUNT(*)` back would erase the
/// declaration. This is the same rule as the 0088 triggers and as the
/// `declared_total_tracks = 0` guard in `auto_resolve_duplicates_inner`.
const ALBUM_EFFECTIVE_TOTAL_SQL: &str = "CASE WHEN COALESCE(albums.declared_total_tracks, 0) > 0 \
     THEN albums.declared_total_tracks \
     ELSE (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) END";

/// An album is divergent when its stored counts disagree with the library: the
/// effective total is not what `total_tracks` advertises, or `local_track_count`
/// drifted away from the number of imported tracks.
fn album_divergence_predicate() -> String {
    format!(
        "(total_tracks IS NULL OR total_tracks != {ALBUM_EFFECTIVE_TOTAL_SQL} \
          OR local_track_count != {ALBUM_LOCAL_TRACKS_SQL})"
    )
}

/// Counts the albums out of sync with the library, whole-library or scoped.
async fn count_divergent_albums(
    db: &crate::DbPool,
    album_ids: Option<&[i64]>,
) -> Result<i64, String> {
    let predicate = album_divergence_predicate();
    match album_ids {
        Some([]) => Ok(0),
        Some(ids) => {
            let sql = format!(
                "SELECT COUNT(*) FROM albums WHERE id = ? AND {NOT_STUB_SQL} AND {predicate}"
            );
            let mut count = 0i64;
            for id in ids {
                count += sqlx::query_scalar::<_, i64>(&sql)
                    .bind(id)
                    .fetch_one(db)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Ok(count)
        }
        None => {
            let sql = format!("SELECT COUNT(*) FROM albums WHERE {NOT_STUB_SQL} AND {predicate}");
            sqlx::query_scalar::<_, i64>(&sql)
                .fetch_one(db)
                .await
                .map_err(|e| e.to_string())
        }
    }
}

pub async fn perform_recalculate_album_total_tracks(
    db: &crate::DbPool,
    album_ids: Option<Vec<i64>>,
) -> Result<AlbumTotalTracksReconcileReport, String> {
    let scoped = album_ids.as_deref();

    // Count divergent albums before
    let divergent_before = count_divergent_albums(db, scoped).await?;

    // Repair both counts from the library in one statement, and only on the rows
    // that actually violate the invariant: `updated_albums` then counts real
    // repairs, and a declared release total survives a reconciliation it never
    // violated. A declared album keeps `total_tracks = declared_total_tracks`;
    // an album that declares nothing still derives its total from the tracks.
    let predicate = album_divergence_predicate();
    let repair_sql = format!(
        r#"
        UPDATE albums
        SET local_track_count = {ALBUM_LOCAL_TRACKS_SQL},
            total_tracks = {ALBUM_EFFECTIVE_TOTAL_SQL}
        WHERE {NOT_STUB_SQL} AND {predicate}
        "#
    );

    let updated = match scoped {
        Some(ids) => {
            let per_id = format!("{repair_sql} AND id = ?");
            let mut total_aff = 0u64;
            for id in ids {
                let res = sqlx::query(&per_id)
                    .bind(id)
                    .execute(db)
                    .await
                    .map_err(|e| e.to_string())?;
                total_aff += res.rows_affected();
            }
            total_aff
        }
        None => sqlx::query(&repair_sql)
            .execute(db)
            .await
            .map_err(|e| e.to_string())?
            .rows_affected(),
    };

    // Count divergent albums after
    let divergent_after = count_divergent_albums(db, scoped).await?;

    Ok(AlbumTotalTracksReconcileReport {
        updated_albums: updated,
        divergent_before,
        divergent_after,
    })
}

/// Reconciles and recalculates total_tracks for library albums (TASK-138)
#[tauri::command]
pub async fn recalculate_album_total_tracks(
    state: State<'_, AppState>,
    album_ids: Option<Vec<i64>>,
) -> Result<AlbumTotalTracksReconcileReport, String> {
    perform_recalculate_album_total_tracks(&state.db, album_ids).await
}

/// Report on empty orphan albums purge (TASK-70)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct PurgeOrphanEmptyAlbumsReport {
    pub purged_albums_count: u64,
    pub purged_album_artists_count: u64,
    pub preserved_stubs_count: u64,
}

/// Transactional helper to purge empty orphan albums and orphan album_artists references (TASK-70)
pub async fn perform_purge_orphan_empty_albums(
    db: &sqlx::SqlitePool,
) -> Result<PurgeOrphanEmptyAlbumsReport, String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    // 1. Count preserved stubs (albums with 0 tracks but is_stub = 1)
    let preserved_stubs: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM albums
        WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
          AND is_stub = 1
        "#,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Failed to count preserved stubs: {}", e))?;

    // 2. Delete orphan album_artists corresponding to the empty orphan albums to be deleted,
    // as well as any dangling album_artists pointing to non-existent albums.
    let deleted_album_artists = sqlx::query(
        r#"
        DELETE FROM album_artists
        WHERE album_id IN (
            SELECT id FROM albums
            WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
              AND (is_stub != 1 OR is_stub IS NULL)
        )
        OR album_id NOT IN (SELECT id FROM albums)
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to purge orphan album_artists: {}", e))?
    .rows_affected();

    // 3. Delete empty orphan albums (excluding stubs with is_stub = 1)
    let deleted_albums = sqlx::query(
        r#"
        DELETE FROM albums
        WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
          AND (is_stub != 1 OR is_stub IS NULL)
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to purge empty orphan albums: {}", e))?
    .rows_affected();

    // 4. Verify integrity and foreign keys before commit
    let fk_violations: Vec<(String, Option<i64>, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check;")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| format!("Failed to run foreign_key_check: {}", e))?;

    if !fk_violations.is_empty() {
        return Err(format!(
            "Foreign key check failed after purge: {:?}",
            fk_violations
        ));
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    tracing::info!(
        "purge_orphan_empty_albums: purged {} albums, {} album_artists links, preserved {} stubs",
        deleted_albums,
        deleted_album_artists,
        preserved_stubs
    );

    Ok(PurgeOrphanEmptyAlbumsReport {
        purged_albums_count: deleted_albums,
        purged_album_artists_count: deleted_album_artists,
        preserved_stubs_count: preserved_stubs as u64,
    })
}

/// Purges orphan empty albums with 0 tracks and cleans up orphan album_artists (TASK-70)
#[tauri::command]
pub async fn purge_orphan_empty_albums(
    state: State<'_, AppState>,
) -> Result<PurgeOrphanEmptyAlbumsReport, String> {
    perform_purge_orphan_empty_albums(&state.db).await
}

// ==============================================
// LOCAL LIBRARY AS A SERVICE (F-01 / F-02)
// ==============================================

/// Ensures the 'local' service and its corresponding account are registered in the database.
pub async fn ensure_local_service_registered(db: &sqlx::SqlitePool) -> Result<(i64, i64), String> {
    // 1. Register service 'local'
    sqlx::query(
        r#"
        INSERT INTO services (name, supports_download, max_quality)
        VALUES ('local', 0, 'hires')
        ON CONFLICT(name) DO UPDATE SET supports_download = 0, max_quality = 'hires'
        "#,
    )
    .execute(db)
    .await
    .map_err(|e| format!("Failed to register local service: {}", e))?;

    let service_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE name = 'local' LIMIT 1")
            .fetch_one(db)
            .await
            .map_err(|e| format!("Failed to fetch local service ID: {}", e))?;

    // 2. Register default account for 'local'
    sqlx::query(
        r#"
        INSERT INTO accounts (service_id, display_name, email, is_active)
        VALUES (?, 'Local Library', 'local@syncify.internal', 1)
        ON CONFLICT(service_id, email) DO UPDATE SET is_active = 1
        "#,
    )
    .bind(service_id)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to register local library account: {}", e))?;

    let account_id: i64 = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE service_id = ? AND email = 'local@syncify.internal' LIMIT 1",
    )
    .bind(service_id)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Failed to fetch local account ID: {}", e))?;

    Ok((service_id, account_id))
}

/// Metadata extracted from a local audio file on disk during library scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalScannedTrack {
    pub file_path: String,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub file_hash: String,
    pub file_format: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub release_year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: Option<i64>,
    pub bit_depth: Option<i64>,
    pub sample_rate: Option<i64>,
    pub isrc: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub score: i32,
}

/// Result of local library synchronization.
/// snake_case es el contrato que consume la UI (ui/src/api/library.ts).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocalSyncReport {
    pub success: bool,
    pub directory: String,
    pub total_scanned: usize,
    pub imported: usize,
    pub duplicates_matched: usize,
    pub duplicates_skipped: usize,
    pub errors: Vec<String>,
}

/// Hashes an audio file with bounded memory, including files much larger than RAM.
fn hash_local_audio_file(path: &std::path::Path) -> std::io::Result<String> {
    use sha2::Digest;
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let mut buffer = [0u8; 64 * 1024];
    let mut hasher = sha2::Sha256::new();
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Loads the singleton's six persisted scoring weights. Only a missing row uses
/// the migration defaults; SQL or decode errors must abort the sync, not hide
/// corrupt preferences behind a seemingly valid default score.
async fn load_local_scoring_weights(
    db: &sqlx::SqlitePool,
) -> Result<syncify_metadata_domain::scoring::ScoringWeights, String> {
    use syncify_metadata_domain::scoring::ScoringWeights;

    let row: Option<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT weight_album, weight_isrc, weight_mb_id, weight_cover, weight_year, weight_genre \
         FROM metadata_preferences WHERE id = 1",
    )
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to load metadata scoring weights: {e}"))?;

    Ok(match row {
        Some((
            weight_album,
            weight_isrc,
            weight_mb_id,
            weight_cover,
            weight_year,
            weight_genre,
        )) => ScoringWeights {
            weight_album,
            weight_isrc,
            weight_mb_id,
            weight_cover,
            weight_year,
            weight_genre,
        },
        None => ScoringWeights::default(),
    })
}

/// Scans a directory for audio files and extracts metadata natively.
/// Only FLAC tags are read; other formats use filename/directory guesses.
/// Standalone scans keep the default weights; synchronization supplies the
/// persisted preferences via `scan_local_audio_files_with_weights`.
/// Kept for external Rust callers; the binary only invokes the sync command.
#[allow(dead_code)]
pub fn scan_local_audio_files(
    dir_path: &std::path::Path,
    recursive: bool,
) -> Result<Vec<LocalScannedTrack>, String> {
    scan_local_audio_files_with_weights(
        dir_path,
        recursive,
        &syncify_metadata_domain::scoring::ScoringWeights::default(),
    )
}

fn scan_local_audio_files_with_weights(
    dir_path: &std::path::Path,
    recursive: bool,
    weights: &syncify_metadata_domain::scoring::ScoringWeights,
) -> Result<Vec<LocalScannedTrack>, String> {
    if !dir_path.is_dir() {
        return Err(format!("Not an accessible directory: {:?}", dir_path));
    }

    let mut walker = walkdir::WalkDir::new(dir_path);
    if !recursive {
        walker = walker.max_depth(1);
    }
    scan_local_audio_entries(walker, weights)
}

fn scan_local_audio_entries(
    entries: impl IntoIterator<Item = Result<walkdir::DirEntry, walkdir::Error>>,
    weights: &syncify_metadata_domain::scoring::ScoringWeights,
) -> Result<Vec<LocalScannedTrack>, String> {
    let mut tracks = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to traverse local library: {}", e))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to inspect {:?}: {}", path, e))?;
        if !metadata.is_file() {
            continue;
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => continue,
        };

        let is_audio = matches!(
            ext.as_str(),
            "flac" | "mp3" | "m4a" | "wav" | "ogg" | "opus" | "aac" | "alac"
        );
        if !is_audio {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let file_size_bytes = metadata.len() as i64;

        // Compute SHA-256 streaming (never loads whole file into memory).
        let file_hash = match hash_local_audio_file(path) {
            Ok(hash) => hash,
            Err(e) => {
                return Err(format!("Failed to read audio file {:?}: {}", path, e));
            }
        };

        let file_format = ext.to_uppercase();
        let mut title = String::new();
        let mut artist = String::new();
        let mut album = String::new();
        let mut album_artist: Option<String> = None;
        let mut track_number: Option<i32> = None;
        let mut disc_number: Option<i32> = None;
        let mut release_year: Option<i32> = None;
        let mut genre: Option<String> = None;
        let mut duration_ms: Option<i64> = None;
        let mut bit_depth: Option<i64> = None;
        let mut sample_rate: Option<i64> = None;
        let mut isrc: Option<String> = None;
        let mut musicbrainz_id: Option<String> = None;

        if ext == "flac" {
            if let Ok(meta) = syncify_flac_writer::read_flac_metadata(path) {
                title = meta.title;
                artist = meta.artist;
                album = meta.album;
                album_artist = meta.album_artist;
                if meta.track_number > 0 {
                    track_number = Some(meta.track_number as i32);
                }
                if meta.disc_number > 0 {
                    disc_number = Some(meta.disc_number as i32);
                }
                if let Some(ref y) = meta.release_year {
                    release_year = y.parse::<i32>().ok();
                }
                genre = meta.genre;
                isrc = meta.isrc;
                musicbrainz_id = meta.musicbrainz_track_id;
                bit_depth = meta.bit_depth.map(|b| b as i64);
                sample_rate = meta.sample_rate.map(|s| s as i64);
            }
            if let Ok(tag) = metaflac::Tag::read_from_path(path) {
                if let Some(info) = tag.get_streaminfo() {
                    if info.sample_rate > 0 {
                        duration_ms = Some(
                            (info.total_samples as f64 / info.sample_rate as f64 * 1000.0) as i64,
                        );
                    }
                }
            }
        }

        // Clean fallbacks if tags are missing
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if title.is_empty() || title == "Unknown Title" {
            let (clean, _) = syncify_core_domain::metadata::clean_title_and_extract_featured(stem);
            if !clean.is_empty() {
                title = clean;
            } else {
                title = stem.to_string();
            }
        }

        if artist.is_empty() || artist == "Unknown Artist" {
            if let Some(pos) = stem.find(" - ") {
                let guessed_artist = stem[..pos].trim();
                let guessed_title = stem[pos + 3..].trim();
                if !guessed_artist.is_empty() {
                    artist = guessed_artist.to_string();
                    if !guessed_title.is_empty() && title == stem {
                        title = guessed_title.to_string();
                    }
                }
            }
            if artist.is_empty() {
                if let Some(parent) = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                {
                    artist = parent.to_string();
                } else {
                    artist = "Unknown Artist".to_string();
                }
            }
        }

        if album.is_empty() || album == "Unknown Album" {
            if let Some(parent) = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            {
                album = parent.to_string();
            } else {
                album = "Local Music".to_string();
            }
        }

        // SC-03: Score calculation using shared scoring engine
        let score_input = syncify_metadata_domain::scoring::TrackMetadataScoreInput {
            title: Some(title.clone()),
            artist: Some(artist.clone()),
            album: Some(album.clone()),
            isrc: isrc.clone(),
            musicbrainz_id: musicbrainz_id.clone(),
            release_year,
            genre: genre.clone(),
            file_path: Some(path.to_string_lossy().to_string()),
            is_local: true,
            ..Default::default()
        };
        let score =
            syncify_metadata_domain::scoring::calculate_metadata_score(weights, &score_input)
                as i32;

        tracks.push(LocalScannedTrack {
            file_path: path.to_string_lossy().to_string(),
            file_name,
            file_size_bytes,
            file_hash,
            file_format,
            title,
            artist,
            album,
            album_artist,
            track_number,
            disc_number,
            release_year,
            genre,
            duration_ms,
            bit_depth,
            sample_rate,
            isrc,
            musicbrainz_id,
            score,
        });
    }

    Ok(tracks)
}

/// Synchronizes a local directory into the database.
/// F-01 & F-02: Scans audio files, detects duplicates (ISRC, title+artist, file hash),
/// inserts into tracks/downloads/track_sources/library_entries, and resolves duplicates.
pub async fn sync_local_library_inner(
    db: &sqlx::SqlitePool,
    directory: &str,
    recursive: bool,
    auto_resolve: bool,
) -> Result<LocalSyncReport, String> {
    let path = std::path::Path::new(directory);
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", directory));
    }

    // PMS-03: load once before any sync writes or file scoring, so every
    // file in this run uses the same persisted preferences.
    let scoring_weights = load_local_scoring_weights(db).await?;
    let (service_id, account_id) = ensure_local_service_registered(db).await?;
    let scanned = scan_local_audio_files_with_weights(path, recursive, &scoring_weights)?;
    let total_scanned = scanned.len();

    let mut imported = 0usize;
    let mut duplicates_matched = 0usize;
    let mut duplicates_skipped = 0usize;
    let mut errors = Vec::new();

    for track in scanned {
        // Step 1: Check if file already exists in downloads
        let existing_dl: Result<Option<(i64, Option<i64>)>, sqlx::Error> = sqlx::query_as(
            "SELECT id, track_id FROM downloads WHERE file_path = ? OR (file_hash = ? AND file_hash != '') LIMIT 1",
        )
        .bind(&track.file_path)
        .bind(&track.file_hash)
        .fetch_optional(db)
        .await;
        let existing_dl = match existing_dl {
            Ok(existing) => existing,
            Err(e) => {
                errors.push(format!(
                    "Failed to find existing download for {:?}: {}",
                    track.file_path, e
                ));
                continue;
            }
        };

        if let Some((dl_id, maybe_tid)) = existing_dl {
            if let Some(tid) = maybe_tid {
                let update_dl = sqlx::query(
                    "UPDATE downloads SET metadata_completeness = MAX(metadata_completeness, ?) WHERE id = ?",
                )
                .bind(track.score as i64)
                .bind(dl_id)
                .execute(db)
                .await;
                if let Err(e) = update_dl {
                    errors.push(format!(
                        "Error updating metadata score for track #{}: {}",
                        tid, e
                    ));
                    continue;
                }

                let link_ts = sqlx::query(
                    "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id, available, quality_score) VALUES (?, ?, ?, 1, ?)",
                )
                .bind(tid)
                .bind(service_id)
                .bind(&track.file_path)
                .bind(track.score as i64)
                .execute(db)
                .await;
                if let Err(e) = link_ts {
                    errors.push(format!(
                        "Error linking track source for track #{}: {}",
                        tid, e
                    ));
                    continue;
                }
            } else {
                errors.push(format!(
                    "Existing download #{} for {:?} has no track_id; cannot link local file",
                    dl_id, track.file_path
                ));
                continue;
            }
            duplicates_skipped += 1;
            continue;
        }

        // Step 2: Only a stable file identity (already handled above) may
        // relink a local download. ISRC/title matches are review candidates.
        let matched_track_id: Option<i64> = None;

        // 2a. Match by ISRC if present
        if let Some(ref isrc_code) = track.isrc {
            if syncify_core_domain::metadata::is_valid_isrc(isrc_code) {
                let candidates: Result<Vec<(i64,)>, _> =
                    sqlx::query_as("SELECT id FROM tracks WHERE isrc = ?")
                        .bind(isrc_code)
                        .fetch_all(db)
                        .await;
                match candidates {
                    Ok(rows) if !rows.is_empty() => {
                        errors.push(format!(
                            "needs_review: ISRC is not edition identity for {:?}",
                            track.file_path
                        ));
                        continue;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        errors.push(format!(
                            "Failed to check ISRC {} for {:?}: {}",
                            isrc_code, track.file_path, e
                        ));
                        continue;
                    }
                }
            }
        }

        // 2b. Match by Title + Primary Artist
        if matched_track_id.is_none() {
            let clean_title = syncify_core_domain::metadata::sanitize_track_title(&track.title);
            let clean_artist = syncify_core_domain::metadata::sanitize_artist_name(&track.artist);
            if !clean_title.is_empty() && !clean_artist.is_empty() {
                let title_artist_matches = sqlx::query_scalar::<_, i64>(
                    r#"SELECT t.id FROM tracks t
                       JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'primary'
                       JOIN artists a ON ta.artist_id = a.id
                       WHERE LOWER(TRIM(t.title)) = LOWER(?)
                         AND LOWER(TRIM(a.name)) = LOWER(?)"#,
                )
                .bind(&clean_title)
                .bind(&clean_artist)
                .fetch_all(db)
                .await;
                let title_artist_matches = match title_artist_matches {
                    Ok(rows) => rows,
                    Err(e) => {
                        errors.push(format!(
                            "Failed to look up duplicates for {:?}: {}",
                            track.file_path, e
                        ));
                        continue;
                    }
                };

                if !title_artist_matches.is_empty() {
                    errors.push(format!(
                        "needs_review: title/artist is not edition identity for {:?}",
                        track.file_path
                    ));
                    continue;
                }
            }
        }

        // Step 3: Insert or Link
        if let Some(tid) = matched_track_id {
            let insert_dl = sqlx::query(
                r#"INSERT INTO downloads (
                       track_id, source_service_id, file_path, file_format,
                       file_size_bytes, file_hash, bit_depth, sample_rate,
                       metadata_completeness, effective_service
                   ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'local')
                   ON CONFLICT(track_id) DO UPDATE SET
                       file_path = excluded.file_path,
                       file_format = excluded.file_format,
                       file_size_bytes = excluded.file_size_bytes,
                       file_hash = excluded.file_hash,
                       bit_depth = excluded.bit_depth,
                       sample_rate = excluded.sample_rate,
                       metadata_completeness = MAX(downloads.metadata_completeness, excluded.metadata_completeness),
                       effective_service = 'local'"#,
            )
            .bind(tid)
            .bind(service_id)
            .bind(&track.file_path)
            .bind(&track.file_format)
            .bind(track.file_size_bytes)
            .bind(&track.file_hash)
            .bind(track.bit_depth)
            .bind(track.sample_rate)
            .bind(track.score as i64)
            .execute(db)
            .await;

            if let Err(e) = insert_dl {
                errors.push(format!("Error linking download for track #{}: {}", tid, e));
                continue;
            }

            let link_ts = sqlx::query(
                r#"INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score)
                   VALUES (?, ?, ?, 1, ?)
                   ON CONFLICT(track_id, service_id) DO UPDATE SET
                       service_track_id = excluded.service_track_id,
                       available = 1,
                       quality_score = excluded.quality_score"#,
            )
            .bind(tid)
            .bind(service_id)
            .bind(&track.file_path)
            .bind(track.score as i64)
            .execute(db)
            .await;
            if let Err(e) = link_ts {
                errors.push(format!(
                    "Error linking track source for track #{}: {}",
                    tid, e
                ));
                continue;
            }

            let link_le = sqlx::query(
                "INSERT OR IGNORE INTO library_entries (account_id, track_id, added_at, is_liked) VALUES (?, ?, CURRENT_TIMESTAMP, 0)",
            )
            .bind(account_id)
            .bind(tid)
            .execute(db)
            .await;
            if let Err(e) = link_le {
                errors.push(format!(
                    "Error linking library entry for track #{}: {}",
                    tid, e
                ));
                continue;
            }

            duplicates_matched += 1;
        } else {
            let clean_artist = syncify_core_domain::metadata::sanitize_artist_name(&track.artist);
            let existing_artist =
                sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(name) = LOWER(?) LIMIT 1")
                    .bind(&clean_artist)
                    .fetch_optional(db)
                    .await;
            let artist_id: i64 = match existing_artist {
                Ok(Some(id)) => id,
                Ok(None) => {
                    match sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                        .bind(&clean_artist)
                        .fetch_one(db)
                        .await
                    {
                        Ok(id) => id,
                        Err(e) => {
                            errors
                                .push(format!("Failed to insert artist '{}': {}", clean_artist, e));
                            continue;
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to look up artist '{}' for {:?}: {}",
                        clean_artist, track.file_path, e
                    ));
                    continue;
                }
            };

            let clean_album = syncify_core_domain::metadata::sanitize_album_title(&track.album);
            let existing_album = sqlx::query_scalar(
                "SELECT al.id FROM albums al JOIN album_artists aa ON al.id = aa.album_id WHERE LOWER(al.title) = LOWER(?) AND aa.artist_id = ? LIMIT 1",
            )
            .bind(&clean_album)
            .bind(artist_id)
            .fetch_optional(db)
            .await;
            let album_id: i64 = match existing_album {
                Ok(Some(id)) => id,
                Ok(None) => {
                    match sqlx::query_scalar(
                        "INSERT INTO albums (title, release_date, local_track_count) VALUES (?, ?, 1) RETURNING id",
                    )
                    .bind(&clean_album)
                    .bind(track.release_year.map(|y| y.to_string()))
                    .fetch_one(db)
                    .await
                    {
                        Ok(id) => {
                            let link_aa = sqlx::query(
                                "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)",
                            )
                            .bind(id)
                            .bind(artist_id)
                            .execute(db)
                            .await;
                            if let Err(e) = link_aa {
                                errors.push(format!(
                                    "Failed to link artist to album '{}' (#{}): {}",
                                    clean_album, id, e
                                ));
                                continue;
                            }
                            id
                        }
                        Err(e) => {
                            errors.push(format!("Failed to insert album '{}': {}", clean_album, e));
                            continue;
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to look up album '{}' for {:?}: {}",
                        clean_album, track.file_path, e
                    ));
                    continue;
                }
            };

            let clean_title = syncify_core_domain::metadata::sanitize_track_title(&track.title);
            let new_track_id: i64 = match sqlx::query_scalar(
                r#"INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, musicbrainz_id, genre, release_year)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"#,
            )
            .bind(&clean_title)
            .bind(album_id)
            .bind(track.duration_ms)
            .bind(track.track_number)
            .bind(track.disc_number.unwrap_or(1))
            .bind(&track.isrc)
            .bind(&track.musicbrainz_id)
            .bind(&track.genre)
            .bind(track.release_year)
            .fetch_one(db)
            .await
            {
                Ok(id) => id,
                Err(e) => {
                    errors.push(format!("Failed to insert track '{}': {}", clean_title, e));
                    continue;
                }
            };

            let link_ta = sqlx::query(
                "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
            )
            .bind(new_track_id)
            .bind(artist_id)
            .execute(db)
            .await;
            if let Err(e) = link_ta {
                errors.push(format!(
                    "Failed to link artist '{}' to track #{}: {}",
                    clean_artist, new_track_id, e
                ));
                continue;
            }

            let insert_dl = sqlx::query(
                r#"INSERT INTO downloads (
                       track_id, source_service_id, file_path, file_format,
                       file_size_bytes, file_hash, bit_depth, sample_rate,
                       metadata_completeness, effective_service
                   ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'local')"#,
            )
            .bind(new_track_id)
            .bind(service_id)
            .bind(&track.file_path)
            .bind(&track.file_format)
            .bind(track.file_size_bytes)
            .bind(&track.file_hash)
            .bind(track.bit_depth)
            .bind(track.sample_rate)
            .bind(track.score as i64)
            .execute(db)
            .await;
            if let Err(e) = insert_dl {
                errors.push(format!(
                    "Failed to insert download for {:?}: {}",
                    track.file_path, e
                ));
                continue;
            }

            let link_ts = sqlx::query(
                "INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score) VALUES (?, ?, ?, 1, ?)",
            )
            .bind(new_track_id)
            .bind(service_id)
            .bind(&track.file_path)
            .bind(track.score as i64)
            .execute(db)
            .await;
            if let Err(e) = link_ts {
                errors.push(format!(
                    "Error linking track source for track #{}: {}",
                    new_track_id, e
                ));
                continue;
            }

            let link_le = sqlx::query(
                "INSERT OR IGNORE INTO library_entries (account_id, track_id, added_at, is_liked) VALUES (?, ?, CURRENT_TIMESTAMP, 0)",
            )
            .bind(account_id)
            .bind(new_track_id)
            .execute(db)
            .await;
            if let Err(e) = link_le {
                errors.push(format!(
                    "Error linking library entry for track #{}: {}",
                    new_track_id, e
                ));
                continue;
            }

            imported += 1;
        }
    }

    // Refresh albums local_track_count
    let refresh_counts = sqlx::query(
        "UPDATE albums SET local_track_count = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id)",
    )
    .execute(db)
    .await;
    if let Err(e) = refresh_counts {
        errors.push(format!("Failed to refresh album track counts: {}", e));
    }

    if auto_resolve {
        if let Err(e) = auto_resolve_duplicates_inner(db).await {
            errors.push(format!("Failed to auto-resolve duplicates: {}", e));
        }
    }

    Ok(LocalSyncReport {
        success: errors.is_empty(),
        directory: directory.to_string(),
        total_scanned,
        imported,
        duplicates_matched,
        duplicates_skipped,
        errors,
    })
}

/// Comando Tauri (F-01/F-02): sincroniza una carpeta local con la biblioteca.
/// Registra el servicio 'local' y su cuenta, escanea los archivos de audio del
/// disco (SHA-256 + scoring local, sin duplicar pistas de streaming) y devuelve
/// un informe. Es la unica frontera entre el backend local y la UI.
#[tauri::command]
pub async fn sync_local_library_cmd(
    state: State<'_, AppState>,
    directory: String,
    recursive: bool,
) -> Result<LocalSyncReport, String> {
    tracing::info!(
        "sync_local_library_cmd directory={} recursive={}",
        directory,
        recursive
    );
    let db = state.db.clone();
    // El escaneo lee del disco y puede tardar: se ejecuta en un hilo de bloqueo
    // para no congelar el runtime asincrono.
    let dir = directory.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(sync_local_library(&db, &dir, recursive))
    });
    match handle.await {
        Ok(res) => res,
        Err(e) => Err(format!("Local library sync task failed: {}", e)),
    }
}

/// Public API to synchronize a local music directory.
pub async fn sync_local_library(
    db: &sqlx::SqlitePool,
    directory: &str,
    recursive: bool,
) -> Result<LocalSyncReport, String> {
    sync_local_library_inner(db, directory, recursive, false).await
}

#[cfg(test)]
mod local_library_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::TempDir;

    async fn create_test_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE services (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                supports_download INTEGER DEFAULT 0,
                max_quality TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(service_id, email)
            );
            CREATE TABLE artists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                musicbrainz_id TEXT,
                spotify_id TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            -- PMS-03: the six weight columns the sync path reads back
            -- (migration 0022) so the local sync tests control scoring weights.
            CREATE TABLE metadata_preferences (
                id INTEGER PRIMARY KEY DEFAULT 1,
                overwrite_on_reimport BOOLEAN NOT NULL DEFAULT 0,
                preserve_custom_tags BOOLEAN NOT NULL DEFAULT 1,
                multi_value_separator TEXT NOT NULL DEFAULT ';',
                write_releasetype BOOLEAN NOT NULL DEFAULT 1,
                write_label BOOLEAN NOT NULL DEFAULT 1,
                write_work_composer BOOLEAN NOT NULL DEFAULT 0,
                write_musicbrainz_ids BOOLEAN NOT NULL DEFAULT 1,
                write_download_source BOOLEAN NOT NULL DEFAULT 0,
                write_download_date BOOLEAN NOT NULL DEFAULT 0,
                write_only_available_on BOOLEAN NOT NULL DEFAULT 0,
                write_not_available_streaming BOOLEAN NOT NULL DEFAULT 0,
                write_quality_score BOOLEAN NOT NULL DEFAULT 0,
                write_lyrics_tags BOOLEAN NOT NULL DEFAULT 0,
                enable_musicbrainz INTEGER NOT NULL DEFAULT 1,
                enable_lastfm INTEGER NOT NULL DEFAULT 0,
                enable_acoustid INTEGER NOT NULL DEFAULT 0,
                weight_album INTEGER NOT NULL DEFAULT 1,
                weight_isrc INTEGER NOT NULL DEFAULT 1,
                weight_mb_id INTEGER NOT NULL DEFAULT 1,
                weight_cover INTEGER NOT NULL DEFAULT 1,
                weight_year INTEGER NOT NULL DEFAULT 1,
                weight_genre INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO metadata_preferences (id) VALUES (1);
            CREATE TABLE albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                release_date TEXT,
                musicbrainz_id TEXT UNIQUE,
                upc TEXT,
                total_tracks INTEGER,
                local_track_count INTEGER NOT NULL DEFAULT 0,
                cover_art_url TEXT,
                is_stub INTEGER DEFAULT 0,
                is_compilation INTEGER DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE album_artists (
                album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
                artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
                is_primary INTEGER DEFAULT 1,
                PRIMARY KEY (album_id, artist_id)
            );
            CREATE TABLE tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                album_id INTEGER REFERENCES albums(id) ON DELETE SET NULL,
                duration_ms INTEGER,
                track_number INTEGER,
                disc_number INTEGER DEFAULT 1,
                isrc TEXT UNIQUE,
                musicbrainz_id TEXT,
                genre TEXT,
                release_year INTEGER,
                explicit INTEGER DEFAULT 0,
                play_count INTEGER NOT NULL DEFAULT 0,
                last_played TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE track_artists (
                track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
                role TEXT DEFAULT 'primary',
                PRIMARY KEY (track_id, artist_id)
            );
            CREATE TABLE track_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                service_track_id TEXT NOT NULL,
                available INTEGER DEFAULT 1,
                quality_score INTEGER DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(track_id, service_id)
            );
            CREATE TABLE library_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                added_at TEXT,
                is_liked INTEGER DEFAULT 0,
                play_count INTEGER DEFAULT 0,
                auto_download INTEGER DEFAULT 0,
                UNIQUE(account_id, track_id)
            );
            CREATE TABLE downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,
                source_service_id INTEGER REFERENCES services(id),
                file_path TEXT NOT NULL,
                file_format TEXT,
                file_size_bytes INTEGER,
                file_hash TEXT,
                bit_depth INTEGER,
                sample_rate INTEGER,
                metadata_completeness INTEGER DEFAULT 0,
                downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
                only_available_on TEXT,
                not_streaming INTEGER DEFAULT 0,
                effective_service TEXT,
                effective_service_track_id TEXT
            );
            INSERT INTO services (name, supports_download, max_quality) VALUES ('spotify', 0, 'lossy');
            INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn create_dummy_flac_with_tags(
        path: &std::path::Path,
        title: &str,
        artist: &str,
        album: &str,
        isrc: Option<&str>,
    ) {
        let mut flac_bytes = Vec::new();
        flac_bytes.extend_from_slice(b"fLaC");
        flac_bytes.extend_from_slice(&[
            0x80, 0x00, 0x00, 0x22, 0x10, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        std::fs::write(path, &flac_bytes).expect("Failed to write dummy flac");

        let meta = syncify_flac_writer::FlacMetadata {
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            isrc: isrc.map(|s| s.to_string()),
            track_number: 1,
            track_total: 10,
            disc_number: 1,
            disc_total: 1,
            genre: Some("Rock".to_string()),
            release_year: Some("2024".to_string()),
            ..Default::default()
        };
        syncify_flac_writer::apply_flac_tags(path, &meta).expect("Failed to apply tags");
    }

    #[tokio::test]
    async fn test_ensure_local_service_registered() {
        let db = create_test_db().await;
        let (svc_id, acc_id) = ensure_local_service_registered(&db).await.unwrap();
        assert!(svc_id > 0);
        assert!(acc_id > 0);

        let svc_name: String = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
            .bind(svc_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(svc_name, "local");

        let acc_name: String = sqlx::query_scalar("SELECT display_name FROM accounts WHERE id = ?")
            .bind(acc_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(acc_name, "Local Library");
    }

    #[tokio::test]
    async fn test_sync_local_library_new_tracks_persisted_and_scored() {
        let db = create_test_db().await;
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("track1.flac");
        create_dummy_flac_with_tags(
            &flac_file,
            "Local Anthem",
            "Indie Band",
            "Local EP",
            Some("USLOC2400001"),
        );

        let report = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();

        assert_eq!(report.total_scanned, 1);
        assert_eq!(report.imported, 1);
        assert_eq!(report.duplicates_matched, 0);

        // Verify tracks row
        let track_row: (i64, String, Option<String>) =
            sqlx::query_as("SELECT id, title, isrc FROM tracks WHERE isrc = 'USLOC2400001'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(track_row.1, "Local Anthem");

        // Verify downloads row
        let dl_row: (String, String, i64, Option<String>) = sqlx::query_as(
            "SELECT file_path, file_format, metadata_completeness, effective_service FROM downloads WHERE track_id = ?",
        )
        .bind(track_row.0)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(dl_row.0, flac_file.to_str().unwrap());
        assert_eq!(dl_row.1, "FLAC");
        assert!(dl_row.2 > 0, "SC-03 quality score must be positive");
        assert_eq!(dl_row.3, Some("local".to_string()));

        // Verify track_sources row
        let source_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_sources WHERE track_id = ? AND service_track_id = ?",
        )
        .bind(track_row.0)
        .bind(flac_file.to_str().unwrap())
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(source_count, 1);
    }

    #[tokio::test]
    async fn test_local_scoring_weights_use_persisted_nonuniform_values() {
        use syncify_metadata_domain::scoring::ScoringWeights;

        let db = create_test_db().await; // isolated in-memory database, not the user's data
        sqlx::query(
            "UPDATE metadata_preferences SET weight_album = 2, weight_isrc = 3, \
             weight_mb_id = 9, weight_cover = 4, weight_year = 5, weight_genre = 7 WHERE id = 1",
        )
        .execute(&db)
        .await
        .unwrap();
        assert_eq!(
            load_local_scoring_weights(&db).await.unwrap(),
            ScoringWeights {
                weight_album: 2,
                weight_isrc: 3,
                weight_mb_id: 9,
                weight_cover: 4,
                weight_year: 5,
                weight_genre: 7,
            }
        );

        let temp_dir = TempDir::new().unwrap();
        let flac = temp_dir.path().join("weighted.flac");
        create_dummy_flac_with_tags(&flac, "Weighted", "Artist", "Album", Some("USWGT2400001"));

        // Title, artist, album, ISRC, year, and genre; no MBID or cover.
        // Configured 20 + round(80 * (2+3+5+7) / 30) = 65.
        let default_score = scan_local_audio_files(temp_dir.path(), true).unwrap()[0].score;
        assert_eq!(default_score, 73); // six equal weights: 20 + round(80 * 4/6)
        let report = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();
        assert_eq!(report.imported, 1);
        let stored_score: i64 =
            sqlx::query_scalar("SELECT metadata_completeness FROM downloads WHERE file_path = ?")
                .bind(flac.to_str().unwrap())
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(stored_score, 65);
        assert_ne!(stored_score, default_score as i64);
    }

    #[tokio::test]
    async fn test_local_scoring_weights_fallback_only_when_row_missing() {
        use syncify_metadata_domain::scoring::ScoringWeights;

        let db = create_test_db().await;
        // A present row uses the migration defaults (all i64), not hard-coded scores.
        assert_eq!(
            load_local_scoring_weights(&db).await.unwrap(),
            ScoringWeights::default()
        );
        sqlx::query("DELETE FROM metadata_preferences WHERE id = 1")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(
            load_local_scoring_weights(&db).await.unwrap(),
            ScoringWeights::default()
        );

        // Missing table (or missing column/decoding failure) is an error, never
        // mistaken for an absent singleton row.
        sqlx::query("DROP TABLE metadata_preferences")
            .execute(&db)
            .await
            .unwrap();
        let err = load_local_scoring_weights(&db).await.unwrap_err();
        assert!(
            err.contains("Failed to load metadata scoring weights"),
            "{err}"
        );

        let temp_dir = TempDir::new().unwrap();
        create_dummy_flac_with_tags(
            &temp_dir.path().join("unscored.flac"),
            "Song",
            "Artist",
            "Album",
            None,
        );
        let err = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap_err();
        assert!(
            err.contains("Failed to load metadata scoring weights"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn test_sync_local_library_deduplicates_by_isrc() {
        let db = create_test_db().await;

        // Simulate an existing track previously imported from streaming (e.g. Spotify)
        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Known Artist') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        let album_id: i64 =
            sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Known Album') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        let existing_tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, isrc) VALUES ('Known Hit', ?, 'USKNO2000001') RETURNING id",
        )
        .bind(album_id)
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
            .bind(existing_tid)
            .bind(artist_id)
            .execute(&db)
            .await
            .unwrap();

        // Now local sync has a physical FLAC with identical ISRC
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("local_copy.flac");
        create_dummy_flac_with_tags(
            &flac_file,
            "Known Hit (Remastered)",
            "Known Artist",
            "Known Album",
            Some("USKNO2000001"),
        );

        let report = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();

        assert_eq!(report.total_scanned, 1);
        assert_eq!(
            report.imported, 0,
            "No duplicate track row should be created"
        );
        assert_eq!(
            report.duplicates_matched, 0,
            "ISRC alone must not link a local file"
        );
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("needs_review")));

        // Total tracks in DB must remain 1
        let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(total_tracks, 1);

        // A tentative ISRC match must not attach a physical file.
        let attached: i64 = sqlx::query_scalar("SELECT count(*) FROM downloads WHERE track_id = ?")
            .bind(existing_tid)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(attached, 0);
    }

    #[tokio::test]
    async fn test_sync_local_library_deduplicates_by_title_artist() {
        let db = create_test_db().await;

        // Existing streaming track with no ISRC
        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The Beatles') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        let album_id: i64 =
            sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Abbey Road') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        let existing_tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id) VALUES ('Come Together', ?) RETURNING id",
        )
        .bind(album_id)
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(existing_tid)
        .bind(artist_id)
        .execute(&db)
        .await
        .unwrap();

        // Local flac with exact title and artist
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("come_together.flac");
        create_dummy_flac_with_tags(
            &flac_file,
            "Come Together",
            "The Beatles",
            "Abbey Road",
            None,
        );

        let report = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();

        assert_eq!(report.imported, 0);
        assert_eq!(report.duplicates_matched, 0);
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("needs_review")));

        let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            total_tracks, 1,
            "review cannot create a tentative catalog association"
        );
    }

    #[tokio::test]
    async fn test_sync_local_library_skips_already_imported_file() {
        let db = create_test_db().await;
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("song.flac");
        create_dummy_flac_with_tags(&flac_file, "Song A", "Artist A", "Album A", None);

        // Run 1: import
        let rep1 = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();
        assert_eq!(rep1.imported, 1);
        assert_eq!(rep1.duplicates_skipped, 0);

        // Run 2: rescan
        let rep2 = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();
        assert_eq!(rep2.imported, 0);
        assert_eq!(rep2.duplicates_skipped, 1);
    }

    #[tokio::test]
    async fn test_local_track_ready_for_playback() {
        let db = create_test_db().await;
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("playable.flac");
        create_dummy_flac_with_tags(&flac_file, "Playable Track", "Artist", "Album", None);

        let rep = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();
        assert_eq!(rep.imported, 1);

        // Fetch track id
        let tid: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Playable Track'")
            .fetch_one(&db)
            .await
            .unwrap();

        // Check downloads row for playback resolution (mirrors playback.rs query)
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT file_path, file_format FROM downloads WHERE track_id = ? LIMIT 1",
        )
        .bind(tid)
        .fetch_optional(&db)
        .await
        .unwrap();

        assert!(
            row.is_some(),
            "Downloads row must exist for playback source resolution"
        );
        let (path, format) = row.unwrap();
        assert_eq!(path, flac_file.to_str().unwrap());
        assert_eq!(format, Some("FLAC".to_string()));
        assert!(
            std::path::Path::new(&path).is_file(),
            "File must exist on disk"
        );
    }

    /// Incremental SHA-256 must equal the digest of the whole file, byte for byte.
    #[test]
    fn test_hash_local_audio_file_matches_whole_file_digest() {
        use sha2::Digest;

        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("hash_check.bin");

        // Deterministic payload, larger than the internal 64 KiB buffer, so the
        // hashing loop runs several iterations.
        let mut payload = Vec::new();
        for i in 0..200_000u32 {
            payload.push((i % 251) as u8);
        }
        std::fs::write(&file, &payload).unwrap();

        let mut hasher = sha2::Sha256::new();
        hasher.update(&payload);
        let expected = format!("{:x}", hasher.finalize());

        assert_eq!(hash_local_audio_file(&file).unwrap(), expected);

        // A zero-byte file must hash to the empty input digest, not panic or
        // yield an empty string.
        let empty = temp_dir.path().join("empty.bin");
        std::fs::write(&empty, b"").unwrap();
        let empty_hasher = sha2::Sha256::new();
        let empty_digest = format!("{:x}", empty_hasher.finalize());
        assert_eq!(hash_local_audio_file(&empty).unwrap(), empty_digest);
        assert_eq!(hash_local_audio_file(&empty).unwrap().len(), 64);
    }

    /// A missing or unreadable file must surface an Io error (the caller
    /// propagates it instead of silently storing an unusable hash).
    #[test]
    fn test_hash_local_audio_file_errors_are_not_silenced() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("does_not_exist.flac");
        let err = hash_local_audio_file(&missing).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    /// Traversal failures must abort the scan with a descriptive error rather
    /// than being swallowed by `filter_map(|e| e.ok())`.
    #[test]
    fn test_scan_local_audio_files_reports_traversal_errors() {
        let temp_dir = TempDir::new().unwrap();
        let missing_path = temp_dir.path().join("missing_subtree");
        let entries = walkdir::WalkDir::new(missing_path);

        let result = scan_local_audio_entries(
            entries,
            &syncify_metadata_domain::scoring::ScoringWeights::default(),
        );
        assert!(result.is_err(), "traversal failure must not be hidden");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Failed to traverse local library"),
            "error must name the failing stage, got: {}",
            msg
        );
    }

    /// Unreadable audio files must abort the scan; the previous behaviour of
    /// storing `String::new()` as the hash silently bypassed hash deduplication.
    #[test]
    fn test_scan_local_audio_files_reports_unreadable_file() {
        let temp_dir = TempDir::new().unwrap();
        let flac = temp_dir.path().join("unreadable.flac");
        create_dummy_flac_with_tags(&flac, "Unreadable", "Artist", "Album", None);

        // Chmod 000 makes the file unreadable for non-root users.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o000);
            std::fs::set_permissions(&flac, perms).unwrap();
            let is_root = unsafe { libc::geteuid() } == 0;
            if is_root {
                return;
            }
        }

        let result = scan_local_audio_files(temp_dir.path(), true);
        assert!(result.is_err(), "unreadable file must fail the scan");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Failed to read audio file"),
            "error must name the failing stage, got: {}",
            msg
        );
    }

    /// Non-FLAC formats must fall back to filename/parent-directory guesses, and
    /// the scan must never invent tag-derived values for them (no ISRC from an
    /// MP3 the scanner cannot actually read).
    #[test]
    fn test_scan_local_audio_files_guesses_metadata_for_non_flac_formats() {
        let temp_dir = TempDir::new().unwrap();
        let artist_dir = temp_dir.path().join("Guessed Artist");
        std::fs::create_dir_all(&artist_dir).unwrap();
        let mp3 = artist_dir.join("Guessed Title.mp3");
        std::fs::write(&mp3, b"ID3fake-mp3-payload").unwrap();

        let scanned = scan_local_audio_files(temp_dir.path(), true).unwrap();
        assert_eq!(scanned.len(), 1);
        let track = &scanned[0];
        assert_eq!(track.file_format, "MP3");
        assert_eq!(track.file_size_bytes, 19);
        assert_eq!(track.title, "Guessed Title");
        assert_eq!(track.artist, "Guessed Artist");
        assert_eq!(track.album, "Guessed Artist");
        assert!(
            track.isrc.is_none(),
            "MP3 tags are not read; ISRC must not be invented"
        );
        assert!(track.musicbrainz_id.is_none());
        assert_eq!(track.file_hash.len(), 64);
    }

    /// The same content must produce the same hash across two different paths
    /// (the property hash-based duplicate detection depends on).
    #[test]
    fn test_scan_local_audio_files_hash_is_content_addressed() {
        use std::collections::HashMap;

        let temp_dir = TempDir::new().unwrap();
        let a = temp_dir.path().join("a.flac");
        let b = temp_dir.path().join("b.flac");
        let c = temp_dir.path().join("c.flac");
        create_dummy_flac_with_tags(&a, "Same", "Artist", "Album", None);
        std::fs::copy(&a, &b).unwrap();
        std::fs::write(&c, b"other content entirely").unwrap();

        let scanned = scan_local_audio_files(temp_dir.path(), true).unwrap();
        assert_eq!(scanned.len(), 3);

        let hashes: HashMap<String, usize> = scanned.iter().fold(HashMap::new(), |mut acc, t| {
            *acc.entry(t.file_hash.clone()).or_insert(0) += 1;
            acc
        });
        // Two identical files collapse to one hash group, the third is unique.
        assert_eq!(hashes.len(), 2);
        assert!(hashes.values().any(|count| *count == 2));
    }

    /// A failed look-up must report an error and not create a duplicate track
    /// row. Previously `unwrap_or_default()` turned a DB failure into "no match",
    /// which then inserted a second track for the same recording.
    #[tokio::test]
    async fn test_db_lookup_failure_is_not_treated_as_no_match() {
        let db = create_test_db().await;
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("broken_lookup.flac");
        create_dummy_flac_with_tags(
            &flac_file,
            "Broken Lookup",
            "Broken Artist",
            "Broken Album",
            None,
        );

        // Break the table used by the title+artist de-duplication query so the
        // SELECT fails while the scan itself still succeeds.
        sqlx::query("DROP TABLE track_artists")
            .execute(&db)
            .await
            .unwrap();

        let report = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();

        assert!(
            !report.errors.is_empty(),
            "a failed de-duplication query must be reported"
        );
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("Failed to look up duplicates")),
            "errors must name the de-duplication stage, got: {:?}",
            report.errors
        );
        assert_eq!(
            report.imported, 0,
            "no track may be imported when its duplicate state is unknown"
        );
        assert!(
            !report.success,
            "success must not be claimed when the scan produced errors"
        );
    }

    /// A successful scan with zero errors reports success:true, so the stricter
    /// success flag does not regress the happy path.
    #[tokio::test]
    async fn test_sync_local_library_success_flag_without_errors() {
        let db = create_test_db().await;
        let temp_dir = TempDir::new().unwrap();
        let flac_file = temp_dir.path().join("ok.flac");
        create_dummy_flac_with_tags(
            &flac_file,
            "Clean Track",
            "Clean Artist",
            "Clean Album",
            None,
        );

        let report = sync_local_library(&db, temp_dir.path().to_str().unwrap(), true)
            .await
            .unwrap();

        assert!(report.errors.is_empty());
        assert!(report.success);
        assert_eq!(report.imported, 1);
    }
}
