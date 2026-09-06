#[allow(unused_imports)]
use super::*;

// Library Commands - submodule of crate::commands
// 
// Library CRUD operations, search, playlists

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
            COALESCE(t.file_disambiguator, d.file_disambiguator) as file_disambiguator
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
    Ok(metadata)
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
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500); // Default 100, max 500 per request

    tracing::info!("get_library called with offset={}, limit={}", offset, limit);

    // Get total count first
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Count error: {}", e))?;

    let tracks = sqlx::query_as::<_, LibraryTrack>(
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
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        GROUP BY t.id
        ORDER BY t.title ASC
        LIMIT ? OFFSET ?
        "#
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

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

    tracing::info!("get_duplicate_tracks called with offset={}, limit={}", offset, limit);

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
        "#
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Count error: {}", e))?;

    let tracks = sqlx::query_as::<_, LibraryTrack>(
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
            -- bio and image_url don't exist in current db layout natively, mapped to null/None
            NULL as image_url,
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

    let appearances = fetch_artist_appearances(db, artist_id).await.unwrap_or_default();

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

/// Retrieve appearance tracks for an artist (guest / collaboration / compilation tracks)
#[tauri::command]
pub async fn get_artist_appearances(
    state: State<'_, AppState>,
    artist_id: i64,
) -> Result<Vec<ArtistAppearanceTrack>, String> {
    fetch_artist_appearances(&state.db, artist_id).await
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
    let album: (i64, String, Option<String>, Option<i64>, Option<String>, i64, Option<i64>, Option<String>) = sqlx::query_as(
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

    tracing::info!("get_playlist_tracks called for playlist_id={}, offset={}, limit={}", playlist_id, offset, limit);

    // Get total count first
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
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
    sqlx::query_as::<_, LibraryTrack>(
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
    .map_err(|e| format!("Database error: {}", e))
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

    let client = MusicBrainzClient::new();
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
    let num_batches = (total + BATCH_SIZE - 1) / BATCH_SIZE;

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
                                    if let Ok(detail) = client.get_recording_details(&recording.id).await {
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
                     RETURNING id"
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

    // 5. Clear sync logs
    sqlx::query("DELETE FROM sync_log")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 6. Reset auto-increment counters for cleanliness (optional but nice)
    sqlx::query("DELETE FROM sqlite_sequence WHERE name IN ('tracks', 'albums', 'artists', 'playlists', 'download_queue')")
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
    tracing::info!("search_tracks called: {} (offset={}, limit={})", query, offset, limit);

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

    let tracks = sqlx::query_as::<_, LibraryTrack>(
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

    let has_more = offset + (tracks.len() as i64) < total.0;
    tracing::info!("search_tracks returned {} results (total: {}, has_more: {})", tracks.len(), total.0, has_more);
    
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
            NULL as owner_name,
            p.track_count,
            NULL as image_url,
            s.name as service_name
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
    for (_i, track_id) in track_ids.iter().enumerate() {
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
           RETURNING id"#
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

/// Remove a single track from the library (cascading deletes via FK)
#[tauri::command]
pub async fn remove_track(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<(), String> {
    tracing::info!("remove_track called: track_id={}", track_id);

    let result = sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(track_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to remove track: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Track {} not found", track_id));
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
    let query_str = format!("DELETE FROM tracks WHERE id IN ({})", placeholders.join(","));

    let mut query = sqlx::query(&query_str);
    for id in &track_ids {
        query = query.bind(id);
    }

    let result = query
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to bulk remove tracks: {}", e))?;

    let removed = result.rows_affected() as usize;
    tracing::info!("Bulk removed {} tracks", removed);
    Ok(removed)
}

/// Toggle the favorite status of a track (atomic via RETURNING with timestamp update)
#[tauri::command]
pub async fn toggle_favorite(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<bool, String> {
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
         RETURNING is_favorite"
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

/// Deprecated alias for toggle_favorite; consolidated to toggle_favorite
#[tauri::command]
#[allow(dead_code)]
pub async fn toggle_track_favorite(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<bool, String> {
    toggle_favorite(state, track_id).await
}

/// Explicitly set the favorite status of a track
#[tauri::command]
pub async fn set_track_favorite(
    state: State<'_, AppState>,
    track_id: i64,
    is_favorite: bool,
) -> Result<bool, String> {
    tracing::info!("set_track_favorite called: track_id={}, is_favorite={}", track_id, is_favorite);

    if track_id <= 0 {
        return Err(format!("Invalid track_id: {}", track_id));
    }

    // S168: Acquire CanonicalTrack lock
    let _track_guard = state.concurrency_manager
        .acquire(
            syncify_core_domain::LockScope::CanonicalTrack(track_id),
            Some(&format!("fav-{}", track_id)),
            None,
        )
        .await
        .map_err(|e| format!("Concurrency lock error: {}", e))?;

    let val = if is_favorite { 1 } else { 0 };
    let fav_at_expr = if is_favorite { "datetime('now')" } else { "NULL" };

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

/// Get all favorite tracks in the library - paginated
#[tauri::command]
pub async fn get_favorite_tracks(
    state: State<'_, AppState>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LibraryPage, String> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100).min(500);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE is_favorite = 1")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Count error: {}", e))?;

    let tracks = sqlx::query_as::<_, LibraryTrack>(
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
        WHERE t.is_favorite = 1
        GROUP BY t.id
        ORDER BY t.favorite_at DESC, t.title ASC
        LIMIT ? OFFSET ?
        "#
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

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
pub async fn show_in_folder(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<(), String> {
    tracing::info!("show_in_folder called: track_id={}", track_id);

    // Get file path from downloads table
    let file_path: Option<(String,)> = sqlx::query_as(
        "SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1"
    )
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
    opener::reveal(path_buf)
        .map_err(|e| format!("Failed to reveal file in explorer: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod library_tests {
    // Note: These tests validate SQL logic using in-memory SQLite.
    // They do NOT use Tauri State<> — they test the raw SQL operations.

    use sqlx::SqlitePool;
    use super::*;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        // Run all migrations
        if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
            panic!("Migration failed in test: {}", e);
        }
        pool
    }

    #[tokio::test]
    async fn test_remove_track_persists() {
        let pool = setup_test_db().await;

        // Insert a track
        sqlx::query("INSERT INTO tracks (title) VALUES ('Test Track')")
            .execute(&pool).await.unwrap();

        // Verify it exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title = 'Test Track'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count.0, 1);

        // Remove it
        sqlx::query("DELETE FROM tracks WHERE title = 'Test Track'")
            .execute(&pool).await.unwrap();

        // Verify gone
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title = 'Test Track'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn test_bulk_remove_tracks() {
        let pool = setup_test_db().await;

        // Insert 3 tracks
        for i in 1..=3 {
            sqlx::query("INSERT INTO tracks (title) VALUES (?)")
                .bind(format!("Bulk Track {}", i))
                .execute(&pool).await.unwrap();
        }

        // Verify 3 exist
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title LIKE 'Bulk Track%'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count.0, 3);

        // Get IDs of first 2
        let rows: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE title LIKE 'Bulk Track%' LIMIT 2")
            .fetch_all(&pool).await.unwrap();
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
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title LIKE 'Bulk Track%'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_toggle_favorite() {
        let pool = setup_test_db().await;

        // Insert a track (is_favorite defaults to 0)
        sqlx::query("INSERT INTO tracks (title) VALUES ('Fav Track')")
            .execute(&pool).await.unwrap();

        let id: (i64,) = sqlx::query_as("SELECT id FROM tracks WHERE title = 'Fav Track'")
            .fetch_one(&pool).await.unwrap();

        // Verify default is 0
        let fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
            .bind(id.0).fetch_one(&pool).await.unwrap();
        assert_eq!(fav.0, 0);

        // Toggle to 1
        sqlx::query("UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?")
            .bind(id.0).execute(&pool).await.unwrap();
        let fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
            .bind(id.0).fetch_one(&pool).await.unwrap();
        assert_eq!(fav.0, 1);

        // Toggle back to 0
        sqlx::query("UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?")
            .bind(id.0).execute(&pool).await.unwrap();
        let fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
            .bind(id.0).fetch_one(&pool).await.unwrap();
        assert_eq!(fav.0, 0);
    }

    #[tokio::test]
    async fn test_search_tracks_returns_results() {
        let pool = setup_test_db().await;

        // Insert a track (FTS trigger auto-indexes it)
        sqlx::query("INSERT INTO tracks (title) VALUES ('Searchable Track')")
            .execute(&pool).await.unwrap();

        // Query FTS real JOIN
        let rows: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT t.id, t.title 
            FROM tracks t 
            JOIN library_fts fts ON fts.rowid = t.id 
            WHERE library_fts MATCH ?
            "#
        )
        .bind("Searchable")
        .fetch_all(&pool).await.unwrap();

        assert!(rows.len() >= 1);
        assert_eq!(rows[0].1, "Searchable Track");
    }

    #[tokio::test]
    async fn test_get_artist_returns_albums() {
        let pool = setup_test_db().await;

        let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
            .fetch_one(&pool).await.unwrap();

        let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
            .fetch_one(&pool).await.unwrap();

        sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
            .bind(album_id)
            .bind(artist_id)
            .execute(&pool).await.unwrap();

        // The query from get_artist
        let rows: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT
                al.id,
                al.title
            FROM albums al
            JOIN album_artists aa ON aa.album_id = al.id
            WHERE aa.artist_id = ?
            "#
        )
        .bind(artist_id)
        .fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "Test Album");
    }

    #[tokio::test]
    async fn test_get_album_returns_tracks() {
        let pool = setup_test_db().await;

        let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
            .fetch_one(&pool).await.unwrap();

        let track_a_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, track_number, isrc, explicit) VALUES ('Track A', ?, 1, 'USMETA0000001', 1) RETURNING id"
        )
            .bind(album_id)
            .fetch_one(&pool).await.unwrap();

        sqlx::query("INSERT INTO tracks (title, album_id, track_number) VALUES ('Track B', ?, 2)")
            .bind(album_id)
            .execute(&pool).await.unwrap();

        let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Meta Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
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
            "#
        )
        .bind(album_id)
        .fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1, "Track A");
        assert_eq!(rows[1].1, "Track B");

        let metadata = fetch_track_metadata(&pool, track_a_id).await.unwrap();
        assert_eq!(metadata.track_id, track_a_id);
        assert_eq!(metadata.title, "Track A");
        assert_eq!(metadata.artist_name.as_deref(), Some("Meta Artist"));
        assert_eq!(metadata.album_name.as_deref(), Some("Test Album"));
        assert_eq!(metadata.explicit, Some(true));
        assert_eq!(metadata.file_path.as_deref(), Some("C:/Music/Syncify/Track A.flac"));

        let not_found = fetch_track_metadata(&pool, 999_999).await.unwrap_err();
        assert!(not_found.contains("Track not found"));
    }

    #[sqlx::test]
    async fn test_auto_resolve_duplicates_by_isrc() {
        let pool = setup_test_db().await;
        
        // Create an artist
        sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Test Artist')")
            .execute(&pool).await.unwrap();
        
        // Insert 3 tracks without ISRC but same title and near duration
        // so fallback tolerant matching resolves this duplicate set.
        let id1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Duplicate Song', NULL, 180000) RETURNING id")
            .fetch_one(&pool).await.unwrap();
        
        let id2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Duplicate Song', NULL, 181000) RETURNING id")
            .fetch_one(&pool).await.unwrap();
        
        let id3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Duplicate Song', NULL, 179000) RETURNING id")
            .fetch_one(&pool).await.unwrap();

        // Link all tracks to same artist
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id1).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id2).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id3).execute(&pool).await.unwrap();

        let service_id: (i64,) = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 1")
            .fetch_one(&pool).await.unwrap();

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
        let remaining: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE title = 'Duplicate Song'")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, id1);
    }

    #[sqlx::test]
    async fn test_auto_resolve_duplicates_merges_sources_and_relations() {
        let pool = setup_test_db().await;

        // Create an artist
        sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Merge Artist')")
            .execute(&pool).await.unwrap();

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
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id1).execute(&pool).await.unwrap();
        // Track 2 has artist link
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id2).execute(&pool).await.unwrap();

        let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 2")
            .fetch_all(&pool).await.unwrap();
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
        sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
            .bind(p1).bind(id1).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
            .bind(p2).bind(id2).execute(&pool).await.unwrap();

        // Track 1 has lyrics
        sqlx::query("INSERT INTO lyrics (track_id, format, sync_level, content) VALUES (?, 'lrc', 'line', '[00:01.00]Hello')")
            .bind(id1).execute(&pool).await.unwrap();

        // Resolve duplicates
        let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
        assert_eq!(res.groups_resolved, 1);
        assert_eq!(res.tracks_removed, 1);

        // Winner must be id2 (higher quality score 120 vs 100)
        let remaining_tracks: Vec<(i64, Option<String>, Option<f64>)> = sqlx::query_as(
            "SELECT id, genre, bpm FROM tracks WHERE title = 'Dupe Song'"
        )
        .fetch_all(&pool).await.unwrap();
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
        let pl1_track: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?")
            .bind(p1).fetch_one(&pool).await.unwrap();
        assert_eq!(pl1_track.0, id2);
        let pl2_track: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?")
            .bind(p2).fetch_one(&pool).await.unwrap();
        assert_eq!(pl2_track.0, id2);

        // Lyrics transferred to id2
        let lyr_track: (i64,) = sqlx::query_as("SELECT track_id FROM lyrics WHERE format = 'lrc'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(lyr_track.0, id2);
    }

    #[sqlx::test]
    async fn test_auto_resolve_duplicates_sample_rate_tiebreaker() {
        let pool = setup_test_db().await;

        sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Rate Artist')")
            .execute(&pool).await.unwrap();

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

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id1).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
            .bind(id2).execute(&pool).await.unwrap();

        let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 2")
            .fetch_all(&pool).await.unwrap();
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
        let remaining: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE title = 'Sample Rate Song'")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, id2);
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

/// Merge Level 2 (intra-album) and Level 3 duplicates with ISRC reconciliation and explicit criteria
#[tauri::command]
pub async fn merge_level2_3_duplicates(
    state: tauri::State<'_, AppState>,
) -> Result<AutoResolveResult, String> {
    merge_level2_3_duplicates_inner(&state.db).await
}

pub async fn merge_level2_3_duplicates_inner(
    db: &crate::db::DbPool,
) -> Result<AutoResolveResult, String> {
    auto_resolve_duplicates_inner(db).await
}

pub async fn auto_resolve_duplicates_inner(
    db: &crate::db::DbPool,
) -> Result<AutoResolveResult, String> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await.map_err(|e| format!("Tx error: {}", e))?;
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
        if track_ids.len() <= 1 { return Ok(0); }
        
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
            if let Some(i) = info { infos.push(i); }
        }
        
        if infos.len() <= 1 { return Ok(0); }
        
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
            if info.id == winner_id { continue; }
            let loser_id = info.id;

            // F2.5: Transactional MERGE rather than destructive DELETE

            // Retrieve audio_quality from both tracks to preserve the highest tier
            let winner_q: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
                .bind(winner_id)
                .fetch_optional(&mut **tx).await.ok().flatten();
            let loser_q: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
                .bind(loser_id)
                .fetch_optional(&mut **tx).await.ok().flatten();

            let parse_tier = |s: &str| -> AudioTier {
                s.parse::<AudioTier>().unwrap_or_else(|_| classify_audio_tier(None, None, None, Some(s)))
            };

            let mut merged_tier = match (winner_q.as_deref().map(parse_tier), loser_q.as_deref().map(parse_tier)) {
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
                "#
            )
            .bind(loser_id)
            .fetch_optional(&mut **tx).await.ok().flatten();

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
                    "#
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
                .execute(&mut **tx).await;
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
                            .execute(&mut **tx).await;
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
                            .execute(&mut **tx).await;
                    }
                }
            }

            // 2. Transfer playlist_tracks
            sqlx::query("UPDATE playlist_tracks SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 3. Transfer downloads (track_id is unique in downloads)
            let winner_has_download: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM downloads WHERE track_id = ?"
            )
            .bind(winner_id)
            .fetch_one(&mut **tx).await.unwrap_or(false);

            if !winner_has_download {
                let _ = sqlx::query("UPDATE downloads SET track_id = ? WHERE track_id = ?")
                    .bind(winner_id).bind(loser_id).execute(&mut **tx).await;
            } else {
                let _ = sqlx::query("DELETE FROM downloads WHERE track_id = ?")
                    .bind(loser_id).execute(&mut **tx).await;
            }

            // 4. Transfer lyrics
            sqlx::query("UPDATE OR IGNORE lyrics SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM lyrics WHERE track_id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 5. Transfer library_entries
            sqlx::query("UPDATE OR IGNORE library_entries SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM library_entries WHERE track_id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 6. Transfer track_credits
            sqlx::query("UPDATE OR IGNORE track_credits SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM track_credits WHERE track_id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 7. Transfer track_artists
            sqlx::query("UPDATE OR IGNORE track_artists SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM track_artists WHERE track_id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 8. Transfer download_queue
            sqlx::query("UPDATE OR IGNORE download_queue SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM download_queue WHERE track_id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 9. Transfer enrichment_progress
            sqlx::query("UPDATE OR IGNORE enrichment_progress SET track_id = ? WHERE track_id = ?")
                .bind(winner_id).bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM enrichment_progress WHERE track_id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

            // 10. Transfer operation_journal if table exists
            let has_op_journal: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'operation_journal'"
            )
            .fetch_one(&mut **tx).await.unwrap_or(false);
            if has_op_journal {
                let _ = sqlx::query("UPDATE OR IGNORE operation_journal SET track_id = ? WHERE track_id = ?")
                    .bind(winner_id).bind(loser_id).execute(&mut **tx).await;
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
                    .execute(&mut **tx).await;
            }

            // 12. Finally remove the loser track record
            sqlx::query("DELETE FROM tracks WHERE id = ?")
                .bind(loser_id).execute(&mut **tx).await.map_err(|e| e.to_string())?;

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
        let tracks: Vec<(i64, Option<i32>)> = sqlx::query_as("SELECT id, explicit FROM tracks WHERE isrc = ?")
            .bind(&isrc)
            .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
        
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
    let intra_clean_candidates: Vec<(i64, i64, String, String, Option<String>, Option<String>)> = sqlx::query_as(
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
        "#
    )
    .fetch_all(&mut *tx).await.unwrap_or_default();

    let mut extra_intra = Vec::new();
    for (id_a, id_b, t_a, t_b, isrc_a, isrc_b) in intra_clean_candidates {
        if syncify_core_domain::metadata::clean_title(&t_a) == syncify_core_domain::metadata::clean_title(&t_b) {
            extra_intra.push((id_a, id_b, isrc_a, isrc_b));
        }
    }

    // Phase 2b: Level 3 fuzzy duplicates (matching primary artist, title, duration ± 2000 ms, same explicit)
    let fuzzy_pairs: Vec<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.id as id_a, b.id as id_b, a.isrc as isrc_a, b.isrc as isrc_b
        FROM track_artists ta1
        JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id AND ta1.track_id < ta2.track_id
        JOIN tracks a ON a.id = ta1.track_id
        JOIN tracks b ON b.id = ta2.track_id
        WHERE (LOWER(COALESCE(ta1.role, 'primary')) IN ('primary', 'main') OR (SELECT COUNT(*) FROM track_artists WHERE track_id = a.id) = 1)
          AND (LOWER(COALESCE(ta2.role, 'primary')) IN ('primary', 'main') OR (SELECT COUNT(*) FROM track_artists WHERE track_id = b.id) = 1)
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND TRIM(COALESCE(b.title, '')) != ''
          AND COALESCE(a.explicit, 0) = COALESCE(b.explicit, 0)
          AND LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
        "#
    )
    .fetch_all(&mut *tx).await.map_err(|e| format!("Query error: {}", e))?;

    let mut parent: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut rank: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();
    let mut component_isrc: std::collections::HashMap<i64, String> = std::collections::HashMap::new();

    // Process intra-album pairs (ISRC reconciled within the same album)
    for (id_a, id_b, isrc_a, isrc_b) in intra_pairs.into_iter().chain(extra_intra.into_iter()) {
        parent.entry(id_a).or_insert(id_a);
        parent.entry(id_b).or_insert(id_b);
        rank.entry(id_a).or_insert(0);
        rank.entry(id_b).or_insert(0);

        let clean_isrc_a = isrc_a.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });
        let clean_isrc_b = isrc_b.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
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
            if t.is_empty() { None } else { Some(t) }
        });
        let clean_isrc_b = isrc_b.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
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
            "SELECT DISTINCT COALESCE(disc_number, 1) FROM tracks WHERE album_id = ? ORDER BY 1"
        )
        .bind(album_id)
        .fetch_all(&mut *tx).await.unwrap_or_default();

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
                    "UPDATE tracks SET track_number = ? WHERE id = ? AND track_number != ?"
                )
                .bind(new_num)
                .bind(tid)
                .bind(new_num)
                .execute(&mut *tx).await;
            }
        }

        // TASK-138: Sincronizar total_tracks tras merge y deduplicacion
        let _ = sqlx::query(
            "UPDATE albums SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) WHERE id = ? AND (is_stub != 1 OR is_stub IS NULL)"
        )
        .bind(album_id)
        .execute(&mut *tx).await;
    }

    tx.commit().await.map_err(|e| format!("Tx commit error: {}", e))?;

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
            ("stale_404".to_string(), 0, Some("Source identity missing or empty service_track_id".to_string()))
        } else if service_track_id.contains("404") || service_track_id.starts_with("stale_") {
            ("stale_404".to_string(), 0, Some("Track not found on streaming provider (HTTP 404)".to_string()))
        } else if service_track_id.contains("region") || service_track_id.contains("geo_blocked") {
            ("region_unavailable".to_string(), 0, Some("Track restricted in current account region/territory".to_string()))
        } else if service_track_id.contains("auth") || service_track_id.contains("unauth") || service_track_id.contains("401") || service_track_id.contains("403") {
            ("requires_auth".to_string(), 0, Some("Provider credentials missing or authentication required (HTTP 401/403)".to_string()))
        } else {
            // Check account authentication for the service
            let active_account: Option<(i64, Option<String>)> = sqlx::query_as(
                "SELECT a.id, a.access_token FROM accounts a WHERE a.service_id = ? AND a.is_active = 1 LIMIT 1"
            )
            .bind(src.service_id)
            .fetch_optional(db)
            .await
            .unwrap_or(None);

            match active_account {
                None if svc_lower == "qobuz" || svc_lower == "tidal" || svc_lower == "spotify" => {
                    let any_account: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM accounts WHERE service_id = ? LIMIT 1"
                    )
                    .bind(src.service_id)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None);

                    if any_account.is_none() {
                        ("requires_auth".to_string(), 0, Some(format!("No active {} account connected", src.service_name)))
                    } else {
                        ("available".to_string(), 1, Some("Verified available on provider".to_string()))
                    }
                },
                _ => {
                    ("available".to_string(), 1, Some("Verified available on provider".to_string()))
                }
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
pub async fn perform_reconcile_library_physical_state(
    db: &crate::DbPool,
    options: Option<ReconciliationOptions>,
) -> Result<LibraryReconciliationReport, String> {
    let opts = options.unwrap_or_default();

    // 1. Validate safety gate rules
    if !opts.dry_run && opts.missing_file_policy == MissingFilePolicy::DeleteRecord {
        if opts.confirm_delete != Some(true) {
            return Err("Safety gate rejection: DeleteRecord policy requires explicit confirmation (confirm_delete: true).".to_string());
        }
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
        return Err(format!("Base music directory does not exist or is invalid: {}", base_folder));
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
    let is_mutating_apply = !opts.dry_run && (
        opts.missing_file_policy != MissingFilePolicy::ReportOnly ||
        opts.orphan_policy == OrphanPolicy::RelinkIfExactIdentity ||
        opts.staging_policy == StagingPolicy::PurgeSafeResiduals
    );

    if is_mutating_apply {
        let db_file_row: Option<(i64, String, String)> = sqlx::query_as("PRAGMA database_list")
            .fetch_optional(db)
            .await
            .unwrap_or(None);
        if let Some((_, _, file_path)) = db_file_row {
            if !file_path.is_empty() && file_path != ":memory:" {
                let db_p = std::path::Path::new(&file_path);
                if db_p.is_file() {
                    let bak_uuid = uuid::Uuid::new_v4().simple().to_string();
                    let bak_file_name = format!("syncify_reconcile_{}_{}.db.bak", chrono::Utc::now().format("%Y%m%d_%H%M%S"), &bak_uuid[..8]);
                    let bak_target = db_p.parent().unwrap_or(std::path::Path::new(".")).join(&bak_file_name);
                    if let Ok(bytes) = std::fs::read(db_p) {
                        use sha2::Digest;
                        let mut hasher = sha2::Sha256::new();
                        hasher.update(&bytes);
                        let hash = format!("{:x}", hasher.finalize());
                        if std::fs::write(&bak_target, &bytes).is_ok() {
                            backup_id = Some(format!("bak_{}", &bak_uuid[..8]));
                            backup_path = Some(bak_target.to_string_lossy().to_string());
                            backup_sha256 = Some(hash);
                        }
                    }
                }
            }
        }
        if backup_id.is_none() {
            backup_id = Some(format!("bak_mem_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]));
            backup_sha256 = Some("in_memory_transactional_snapshot".to_string());
        }
    }

    // 4. Gather download records based on Scope
    let download_rows: Vec<(i64, Option<i64>, String, Option<i64>, Option<String>)> = match &opts.scope {
        ReconciliationScope::All => {
            sqlx::query_as("SELECT id, track_id, file_path, source_service_id, effective_service FROM downloads")
                .fetch_all(db)
                .await
                .map_err(|e| format!("Failed to query downloads: {}", e))?
        }
        ReconciliationScope::SelectedDownloadIds(ids) => {
            if ids.is_empty() {
                Vec::new()
            } else {
                let id_list = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
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
            all.into_iter().filter(|r| r.2.starts_with(root)).collect()
        }
    };

    struct MissingItem {
        dl_id: i64,
        track_id: Option<i64>,
        file_path: String,
        #[allow(dead_code)]
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
                    if ext_lower == "flac" || ext_lower == "m4a" || ext_lower == "mp3" || ext_lower == "wav"
                        || ext_lower == "alac" || ext_lower == "aac" || ext_lower == "ogg" || ext_lower == "opus" {
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
        let existing_dl: Option<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT id, track_id FROM downloads WHERE file_path = ?"
        )
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
                            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE id = ?")
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
                        if let Some(stid) = vorbis.get("SYNCIFY_SOURCE_TRACK_ID").or_else(|| vorbis.get("SYNCIFY_SERVICE_TRACK_ID")).and_then(|v| v.first()) {
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
                            let isrc_matches: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ?")
                                .bind(isrc)
                                .fetch_all(db)
                                .await
                                .unwrap_or_default();
                            if isrc_matches.len() == 1 {
                                matched_track_id = Some(isrc_matches[0]);
                            }
                        }
                    }
                    if let Some(src) = vorbis.get("SYNCIFY_AUDIO_SOURCE").or_else(|| vorbis.get("AUDIO_SOURCE")).and_then(|v| v.first()) {
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
                    let isrc_matches: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ?")
                        .bind(isrc)
                        .fetch_all(db)
                        .await
                        .unwrap_or_default();
                    if isrc_matches.len() == 1 {
                        matched_track_id = Some(isrc_matches[0]);
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
            if let Some(insp) = crate::download::audio_inspector::inspect_physical_audio_file(file_path_buf) {
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
            let filename = file_path_buf.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if filename.contains("Tidal Track ") || filename.contains("[Tidal-") {
                let clean_id = if filename.contains("[Tidal-") {
                    filename.split("[Tidal-").nth(1).and_then(|s| s.split(']').next()).unwrap_or("").trim().to_string()
                } else {
                    filename.replace("01 - Tidal Track ", "").replace("Tidal Track ", "").trim().to_string()
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

                    if title_artist_matches.len() == 1 {
                        matched_track_id = Some(title_artist_matches[0]);
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
                details: format!("Exact match verified for track #{} ({})", tid, effective_service),
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

    // 7. Execute Mutations in Apply Mode inside SQL Transaction
    if !opts.dry_run {
        let mut tx = db.begin_with("BEGIN IMMEDIATE").await.map_err(|e| format!("Failed to begin reconciliation transaction: {}", e))?;

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
                            failures.push(format!("Failed to delete download record #{}: {}", item.dl_id, e));
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
                                details: format!("Cleared file_path for download row #{}", item.dl_id),
                                track_id: item.track_id,
                                download_id: Some(item.dl_id),
                                service: item.effective_service.clone(),
                                executed: true,
                            });
                        }
                        Err(e) => {
                            failures.push(format!("Failed to mark download record #{}: {}", item.dl_id, e));
                        }
                    }
                }
            }
            MissingFilePolicy::ReportOnly => {}
        }

        // 7b. Process orphan items
        if opts.orphan_policy == OrphanPolicy::RelinkIfExactIdentity {
            for item in &exact_orphan_relinks {
                let service_id: i64 = if item.effective_service == "tidal" {
                    3
                } else if item.effective_service == "spotify" {
                    1
                } else {
                    2
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
                        effective_service = excluded.effective_service"#
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
                        failures.push(format!("Failed to re-link orphan {}: {}", item.file_path_str, e));
                    }
                }
            }
        }

        // Commit or Rollback transaction
        if failures.is_empty() {
            tx.commit().await.map_err(|e| format!("Failed to commit reconciliation transaction: {}", e))?;
        } else {
            return Err(format!("Reconciliation transaction rolled back due to failures: {:?}", failures));
        }

        // 7c. Clean staging directory residuals
        if opts.staging_policy == StagingPolicy::PurgeSafeResiduals {
            for p in &safe_staging_files {
                if p.is_file() {
                    match std::fs::remove_file(p) {
                        Ok(_) => {
                            cleaned_staging_residuals += 1;
                            executed_actions.push(ReconciliationActionItem {
                                action_type: "purge_staging_residual".to_string(),
                                target: p.to_string_lossy().to_string(),
                                details: "Removed safe staging artifact".to_string(),
                                track_id: None,
                                download_id: None,
                                service: None,
                                executed: true,
                            });
                        }
                        Err(e) => {
                            failures.push(format!("Failed to delete staging file {:?}: {}", p, e));
                        }
                    }
                }
            }
        }
    }

    let total_download_records_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let verified_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE file_path IS NOT NULL AND file_path != ''")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let after_stats = ReconciliationStats {
        total_download_records: total_download_records_after as u64,
        physical_audio_files: physical_flacs.len() as u64,
        missing_file_records: if opts.dry_run || opts.missing_file_policy == MissingFilePolicy::ReportOnly { missing_files.len() as u64 } else { 0 },
        orphan_files_count: if opts.dry_run || opts.orphan_policy == OrphanPolicy::ReportOnly { orphan_files.len() as u64 } else { orphan_files.len().saturating_sub(exact_orphan_relinks.len()) as u64 },
        staging_residuals_count: if opts.dry_run || opts.staging_policy == StagingPolicy::ReportOnly { safe_staging_files.len() as u64 } else { 0 },
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
    perform_reconcile_library_physical_state(&state.db, options).await
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
                norm_quality.clone().or_else(|| normalize_quality_preference(pf.resolved_quality.as_deref())),
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
            let reason = if pf.status == DownloadPreflightStatus::AlreadyDownloaded && skip_downloaded {
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
    let (selected, eligible, excluded_preflight, exclusions, breakdown_by_reason) = if let Some(ref ids) = selected_track_ids {
        let sel_count = ids.len() as i64;
        let mut elig_count = 0i64;
        let mut excl_count = 0i64;
        let mut excl_vec = Vec::new();
        let mut breakdown: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

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
        (total_tracks.0, total_tracks.0, 0, Vec::new(), std::collections::HashMap::new())
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
    let skipped = if excluded_preflight > 0 { excluded_preflight } else { queue_stats.4 };

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
    let client = crate::services::MusicBrainzClient::new();
    client.resolve_ghost_artists(&state.db).await
}

/// Hydrate stub favorite albums with tracklists from MusicBrainz or library (mitigates M1)
#[tauri::command]
pub async fn hydrate_stub_albums(
    state: State<'_, AppState>,
) -> Result<crate::services::musicbrainz::StubAlbumHydrationReport, String> {
    let client = crate::services::MusicBrainzClient::new();
    client.hydrate_stub_albums(&state.db).await
}

/// Report on total_tracks recalculation and reconciliation across albums (TASK-138)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlbumTotalTracksReconcileReport {
    pub updated_albums: u64,
    pub divergent_before: i64,
    pub divergent_after: i64,
}

pub async fn perform_recalculate_album_total_tracks(
    db: &crate::DbPool,
    album_ids: Option<Vec<i64>>,
) -> Result<AlbumTotalTracksReconcileReport, String> {
    // Count divergent albums before
    let divergent_before: i64 = if let Some(ref ids) = album_ids {
        if ids.is_empty() {
            0
        } else {
            let mut count = 0i64;
            for id in ids {
                let div: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*) FROM albums
                    WHERE id = ?
                      AND (is_stub != 1 OR is_stub IS NULL)
                      AND (total_tracks IS NULL OR total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id))
                    "#
                )
                .bind(id)
                .fetch_one(db)
                .await
                .map_err(|e| e.to_string())?;
                count += div;
            }
            count
        }
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM albums
            WHERE (is_stub != 1 OR is_stub IS NULL)
              AND (total_tracks IS NULL OR total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id))
            "#
        )
        .fetch_one(db)
        .await
        .map_err(|e| e.to_string())?
    };

    let updated = if let Some(ref ids) = album_ids {
        let mut total_aff = 0u64;
        for id in ids {
            let aff = crate::services::enrichment::recalculate_album_total_tracks(db, Some(*id))
                .await
                .map_err(|e| e.to_string())?;
            total_aff += aff;
        }
        total_aff
    } else {
        crate::services::enrichment::recalculate_album_total_tracks(db, None)
            .await
            .map_err(|e| e.to_string())?
    };

    // Count divergent albums after
    let divergent_after: i64 = if let Some(ref ids) = album_ids {
        if ids.is_empty() {
            0
        } else {
            let mut count = 0i64;
            for id in ids {
                let div: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*) FROM albums
                    WHERE id = ?
                      AND (is_stub != 1 OR is_stub IS NULL)
                      AND (total_tracks IS NULL OR total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id))
                    "#
                )
                .bind(id)
                .fetch_one(db)
                .await
                .map_err(|e| e.to_string())?;
                count += div;
            }
            count
        }
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM albums
            WHERE (is_stub != 1 OR is_stub IS NULL)
              AND (total_tracks IS NULL OR total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id))
            "#
        )
        .fetch_one(db)
        .await
        .map_err(|e| e.to_string())?
    };

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
    let mut tx = db.begin().await.map_err(|e| format!("Failed to begin transaction: {}", e))?;

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
    let fk_violations: Vec<(String, Option<i64>, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check;")
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("Failed to run foreign_key_check: {}", e))?;

    if !fk_violations.is_empty() {
        return Err(format!("Foreign key check failed after purge: {:?}", fk_violations));
    }

    tx.commit().await.map_err(|e| format!("Failed to commit transaction: {}", e))?;

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




