// Library Commands - included via include!() in mod.rs
// 
// Library CRUD operations, search, playlists

#[derive(Debug, Serialize, sqlx::FromRow)]
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
}

async fn fetch_track_metadata(
    db: &sqlx::SqlitePool,
    track_id: i64,
) -> Result<TrackMetadata, String> {
    let metadata = sqlx::query_as::<_, TrackMetadata>(
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
            d.file_path
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
    .map_err(|e| format!("Failed to fetch track metadata: {}", e))?;

    metadata.ok_or_else(|| format!("Track not found: {}", track_id))
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
            d.file_path
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
pub async fn _get_top_genres(
    state: tauri::State<'_, AppState>,
    limit: i64,
) -> Result<Vec<TopGenre>, String> {
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LibraryArtistDetail {
    pub id: i64,
    pub name: String,
    pub bio: Option<String>,
    pub image_url: Option<String>,
    pub album_count: i64,
    pub track_count: i64,
    pub albums: Vec<ArtistAlbum>,
    pub top_tracks: Vec<ArtistTrack>,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ArtistAlbum {
    pub id: i64,
    pub title: String,
    pub cover_url: Option<String>,
    pub release_year: Option<i64>,
    pub track_count: i64,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
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
    tracing::info!("get_artist called for {}", artist_id);

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
    .fetch_optional(&state.db)
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
    .fetch_all(&state.db)
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
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error fetching top tracks: {}", e))?;

    Ok(LibraryArtistDetail {
        id: artist.0,
        name: artist.1,
        bio: None,
        image_url: artist.2,
        album_count: artist.3,
        track_count: artist.4,
        albums,
        top_tracks,
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
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

#[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct AlbumTrack {
    pub id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub duration_ms: Option<i64>,
    pub track_number: Option<i64>,
}

/// Retrieve album details and tracks
#[tauri::command]
pub async fn get_album(
    state: State<'_, AppState>,
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
    .fetch_optional(&state.db)
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
            t.track_number
        FROM tracks t
        WHERE t.album_id = ?
        ORDER BY t.track_number ASC, t.title ASC
        "#,
    )
    .bind(album_id)
    .fetch_all(&state.db)
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
pub async fn get_local_playlist_tracks(
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
            d.file_path
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
        GROUP BY t.id
        ORDER BY pt.position ASC, t.title ASC
        LIMIT ? OFFSET ?
        "#
    )
    .bind(playlist_id)
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

/// Reset/delete the entire database (requires app restart)
#[tauri::command]
pub async fn reset_database(state: State<'_, AppState>) -> Result<String, String> {
    tracing::warn!("reset_database called - clearing library data!");

    // Execute deletions in a transaction to ensure atomicity
    let mut tx = state
        .db
        .begin()
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
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tracks WHERE id IN (SELECT rowid FROM library_fts WHERE library_fts MATCH ?)"
    )
    .bind(&query)
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
            d.file_path
        FROM tracks t
        LEFT JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify')
        LEFT JOIN downloads d ON d.track_id = t.id
        LEFT JOIN download_queue dq ON dq.track_id = t.id AND dq.status IN ('queued', 'downloading')
        LEFT JOIN lyrics l ON l.track_id = t.id
        WHERE t.id IN (SELECT rowid FROM library_fts WHERE library_fts MATCH ?)
        GROUP BY t.id
        ORDER BY t.title ASC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(&query)
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
    account_id: i64,
    name: String,
    description: Option<String>,
) -> Result<i64, String> {
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

/// Alias for toggle_favorite
#[tauri::command]
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
            ORDER BY t.track_number ASC
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

pub async fn auto_resolve_duplicates_inner(
    db: &crate::db::DbPool,
) -> Result<AutoResolveResult, String> {
    let mut tx = db.begin().await.map_err(|e| format!("Tx error: {}", e))?;
    let mut groups_resolved = 0;
    let mut tracks_removed = 0;

    let isrc_groups: Vec<(String,)> = sqlx::query_as(
        "SELECT isrc FROM tracks WHERE isrc IS NOT NULL GROUP BY isrc HAVING COUNT(id) > 1"
    )
    .fetch_all(&mut *tx).await.map_err(|e| format!("Query error: {}", e))?;

    let fallback_pairs: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT a.id as id_a, b.id as id_b
        FROM tracks a
        JOIN tracks b ON (
            a.id < b.id
            AND a.isrc IS NULL
            AND b.isrc IS NULL
            AND LOWER(a.title) = LOWER(b.title)
            AND ABS(
                COALESCE(a.duration_ms, 0) -
                COALESCE(b.duration_ms, 0)
            ) <= 2000
        )
        "#
    )
    .fetch_all(&mut *tx).await.map_err(|e| format!("Query error: {}", e))?;

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

    async fn resolve_group(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, track_ids: Vec<i64>) -> Result<u32, String> {
        if track_ids.len() <= 1 { return Ok(0); }
        
        #[derive(sqlx::FromRow)]
        struct TrackInfo {
            id: i64,
            quality_score: Option<i32>,
            bit_depth: Option<i32>,
            bitrate: Option<i32>,
            file_size_bytes: Option<i64>,
            file_path: Option<String>,
        }
        
        let mut infos = Vec::new();
        for &id in &track_ids {
            let info: Option<TrackInfo> = sqlx::query_as(
                r#"
                SELECT 
                    t.id,
                    ts.quality_score,
                    ts.bit_depth,
                    ts.bitrate,
                    d.file_size_bytes,
                    d.file_path
                FROM tracks t
                LEFT JOIN track_sources ts ON t.id = ts.track_id
                LEFT JOIN downloads d ON t.id = d.track_id
                WHERE t.id = ?
                ORDER BY ts.quality_score DESC NULLS LAST
                LIMIT 1
                "#
            )
            .bind(id)
            .fetch_optional(&mut **tx).await.map_err(|e| e.to_string())?;
            if let Some(i) = info { infos.push(i); }
        }
        
        if infos.len() <= 1 { return Ok(0); }
        
        infos.sort_by(|a, b| {
            let a_has_file = a.file_path.is_some();
            let b_has_file = b.file_path.is_some();
            if a_has_file != b_has_file {
                return a_has_file.cmp(&b_has_file);
            }
            let a_qs = a.quality_score.unwrap_or(0);
            let b_qs = b.quality_score.unwrap_or(0);
            if a_qs != b_qs { return a_qs.cmp(&b_qs); }
            let a_bd = a.bit_depth.unwrap_or(0);
            let b_bd = b.bit_depth.unwrap_or(0);
            if a_bd != b_bd { return a_bd.cmp(&b_bd); }
            let a_br = a.bitrate.unwrap_or(0);
            let b_br = b.bitrate.unwrap_or(0);
            if a_br != b_br { return a_br.cmp(&b_br); }
            let a_fs = a.file_size_bytes.unwrap_or(0);
            let b_fs = b.file_size_bytes.unwrap_or(0);
            a_fs.cmp(&b_fs)
        });
        
        let winner_id = infos.last().unwrap().id;
        let mut removed = 0;
        
        for info in &infos {
            if info.id == winner_id { continue; }
            sqlx::query("DELETE FROM library_entries WHERE track_id = ?").bind(info.id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM track_sources WHERE track_id = ?").bind(info.id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM tracks WHERE id = ?").bind(info.id).execute(&mut **tx).await.map_err(|e| e.to_string())?;
            removed += 1;
        }
        Ok(removed)
    }

    for (isrc,) in isrc_groups {
        let tracks: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE isrc = ?")
            .bind(&isrc)
            .fetch_all(&mut *tx).await.map_err(|e| e.to_string())?;
        let track_ids: Vec<i64> = tracks.into_iter().map(|t| t.0).collect();
        let rem = resolve_group(&mut tx, track_ids).await?;
        if rem > 0 {
            groups_resolved += 1;
            tracks_removed += rem;
        }
    }

    let mut parent: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut rank: std::collections::HashMap<i64, u8> = std::collections::HashMap::new();

    for (id_a, id_b) in fallback_pairs {
        parent.entry(id_a).or_insert(id_a);
        parent.entry(id_b).or_insert(id_b);
        rank.entry(id_a).or_insert(0);
        rank.entry(id_b).or_insert(0);
        union_nodes(&mut parent, &mut rank, id_a, id_b);
    }

    let nodes: Vec<i64> = parent.keys().copied().collect();
    let mut components: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for node in nodes {
        let root = find_root(&mut parent, node);
        components.entry(root).or_default().push(node);
    }

    for track_ids in components.into_values() {
        let rem = resolve_group(&mut tx, track_ids).await?;
        if rem > 0 {
            groups_resolved += 1;
            tracks_removed += rem;
        }
    }

    tx.commit().await.map_err(|e| format!("Tx commit error: {}", e))?;

    Ok(AutoResolveResult {
        groups_resolved,
        tracks_removed,
    })
}
