#[allow(unused_imports)]
use super::*;

// Unified Search Commands - submodule of crate::commands
// Provides high-performance multi-entity search and filtering across tracks, albums, artists, and playlists

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultTrack {
    pub id: i64,
    pub title: String,
    pub display_title: Option<String>,
    pub source_title: Option<String>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub album_id: Option<i64>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub is_favorite: bool,
    pub services: Option<String>,
    pub quality: Option<String>,
    pub download_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultAlbum {
    pub id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub release_year: Option<i32>,
    pub cover_art_url: Option<String>,
    pub track_count: i64,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultArtist {
    pub id: i64,
    pub name: String,
    pub is_favorite: bool,
    pub track_count: i64,
    pub album_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultPlaylist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub track_count: i64,
    pub service_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchResult {
    pub query: String,
    pub tracks: Vec<SearchResultTrack>,
    pub albums: Vec<SearchResultAlbum>,
    pub artists: Vec<SearchResultArtist>,
    pub playlists: Vec<SearchResultPlaylist>,
    pub total_tracks: i64,
    pub total_albums: i64,
    pub total_artists: i64,
    pub total_playlists: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchLibraryParams {
    pub query: String,
    pub entity_type: Option<String>, // "all", "tracks", "albums", "artists", "playlists"
    pub service: Option<String>,     // "spotify", "tidal", "qobuz", "all"
    pub only_favorites: Option<bool>,
    pub download_status: Option<String>, // "downloaded", "queued", "not_downloaded", "all"
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

/// Unified search across tracks, albums, artists, and playlists with advanced filtering
#[tauri::command]
pub async fn search_library(
    state: State<'_, AppState>,
    params: SearchLibraryParams,
) -> Result<UnifiedSearchResult, String> {
    let trimmed_query = params.query.trim().to_string();
    let pattern = format!("%{}%", trimmed_query);
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50);
    let entity_type = params.entity_type.as_deref().unwrap_or("all");
    let service_filter = params.service.as_deref().unwrap_or("all");
    let only_fav = params.only_favorites.unwrap_or(false);
    let dl_filter = params.download_status.as_deref().unwrap_or("all");

    let mut result = UnifiedSearchResult {
        query: trimmed_query.clone(),
        tracks: Vec::new(),
        albums: Vec::new(),
        artists: Vec::new(),
        playlists: Vec::new(),
        total_tracks: 0,
        total_albums: 0,
        total_artists: 0,
        total_playlists: 0,
    };

    // 1. Search Tracks
    if entity_type == "all" || entity_type == "tracks" {
        let count_row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT t.id)
            FROM tracks t
            LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
            LEFT JOIN artists art ON art.id = ta.artist_id
            LEFT JOIN albums al ON al.id = t.album_id
            LEFT JOIN track_sources ts ON ts.track_id = t.id
            LEFT JOIN services s ON s.id = ts.service_id
            LEFT JOIN downloads d ON d.track_id = t.id
            WHERE (
                t.title LIKE ? OR t.display_title LIKE ? OR art.name LIKE ? OR al.title LIKE ? OR t.isrc LIKE ?
            )
            AND (? = 'all' OR LOWER(s.name) = LOWER(?))
            AND (? = 0 OR t.is_favorite = 1 OR t.favorite_at IS NOT NULL)
            AND (
                ? = 'all' OR 
                (? = 'downloaded' AND d.file_path IS NOT NULL) OR
                (? = 'not_downloaded' AND d.file_path IS NULL)
            )
            "#
        )
        .bind(&pattern).bind(&pattern).bind(&pattern).bind(&pattern).bind(&pattern)
        .bind(service_filter).bind(service_filter)
        .bind(if only_fav { 1 } else { 0 })
        .bind(dl_filter).bind(dl_filter).bind(dl_filter)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        result.total_tracks = count_row.0;

        let tracks_rows: Vec<(
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT 
                t.id,
                COALESCE(t.display_title, t.title) as title,
                t.display_title,
                COALESCE(t.source_title, t.title) as source_title,
                art.name as artist_name,
                al.title as album_name,
                al.id as album_id,
                t.duration_ms,
                t.isrc,
                CASE WHEN t.is_favorite = 1 OR t.favorite_at IS NOT NULL THEN 1 ELSE 0 END as is_fav,
                GROUP_CONCAT(DISTINCT s.name) as services,
                d.file_format as quality,
                CASE 
                    WHEN d.file_path IS NOT NULL THEN 'downloaded'
                    WHEN EXISTS (SELECT 1 FROM download_queue dq WHERE dq.track_id = t.id AND (dq.status = 'queued' OR dq.status = 'downloading')) THEN 'queued'
                    ELSE 'not_downloaded'
                END as download_status
            FROM tracks t
            LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
            LEFT JOIN artists art ON art.id = ta.artist_id
            LEFT JOIN albums al ON al.id = t.album_id
            LEFT JOIN track_sources ts ON ts.track_id = t.id
            LEFT JOIN services s ON s.id = ts.service_id
            LEFT JOIN downloads d ON d.track_id = t.id
            WHERE (
                t.title LIKE ? OR t.display_title LIKE ? OR art.name LIKE ? OR al.title LIKE ? OR t.isrc LIKE ?
            )
            AND (? = 'all' OR LOWER(s.name) = LOWER(?))
            AND (? = 0 OR t.is_favorite = 1 OR t.favorite_at IS NOT NULL)
            AND (
                ? = 'all' OR 
                (? = 'downloaded' AND d.file_path IS NOT NULL) OR
                (? = 'not_downloaded' AND d.file_path IS NULL)
            )
            GROUP BY t.id
            ORDER BY t.title ASC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(&pattern).bind(&pattern).bind(&pattern).bind(&pattern).bind(&pattern)
        .bind(service_filter).bind(service_filter)
        .bind(if only_fav { 1 } else { 0 })
        .bind(dl_filter).bind(dl_filter).bind(dl_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        result.tracks = tracks_rows
            .into_iter()
            .map(|(id, title, display_title, source_title, artist_name, album_name, album_id, duration_ms, isrc, is_fav, services, quality, download_status)| {
                SearchResultTrack {
                    id,
                    title,
                    display_title,
                    source_title,
                    artist_name,
                    album_name,
                    album_id,
                    duration_ms,
                    isrc,
                    is_favorite: is_fav == 1,
                    services,
                    quality,
                    download_status: download_status.unwrap_or_else(|| "not_downloaded".to_string()),
                }
            })
            .collect();
    }

    // 2. Search Albums
    if entity_type == "all" || entity_type == "albums" {
        let count_row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT al.id)
            FROM albums al
            LEFT JOIN album_artists aa ON aa.album_id = al.id
            LEFT JOIN artists art ON art.id = aa.artist_id
            WHERE (al.title LIKE ? OR art.name LIKE ?)
            AND (? = 0 OR al.favorite_at IS NOT NULL)
            "#
        )
        .bind(&pattern).bind(&pattern)
        .bind(if only_fav { 1 } else { 0 })
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        result.total_albums = count_row.0;

        let albums_rows: Vec<(
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            i64,
        )> = sqlx::query_as(
            r#"
            SELECT 
                al.id,
                al.title,
                art.name as artist_name,
                al.release_date,
                al.cover_art_url,
                (SELECT COUNT(*) FROM tracks WHERE album_id = al.id) as track_count,
                CASE WHEN al.favorite_at IS NOT NULL THEN 1 ELSE 0 END as is_fav
            FROM albums al
            LEFT JOIN album_artists aa ON aa.album_id = al.id
            LEFT JOIN artists art ON art.id = aa.artist_id
            WHERE (al.title LIKE ? OR art.name LIKE ?)
            AND (? = 0 OR al.favorite_at IS NOT NULL)
            GROUP BY al.id
            ORDER BY al.title ASC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(&pattern).bind(&pattern)
        .bind(if only_fav { 1 } else { 0 })
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        result.albums = albums_rows
            .into_iter()
            .map(|(id, title, artist_name, release_date, cover_art_url, track_count, is_fav)| {
                let release_year = release_date
                    .and_then(|d| d.chars().take(4).collect::<String>().parse::<i32>().ok());
                SearchResultAlbum {
                    id,
                    title,
                    artist_name,
                    release_year,
                    cover_art_url,
                    track_count,
                    is_favorite: is_fav == 1,
                }
            })
            .collect();
    }

    // 3. Search Artists
    if entity_type == "all" || entity_type == "artists" {
        let count_row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM artists art
            WHERE art.name LIKE ?
            AND (? = 0 OR art.favorite_at IS NOT NULL OR art.is_favorite = 1)
            AND (
                EXISTS (
                    SELECT 1 FROM track_artists ta WHERE ta.artist_id = art.id
                    UNION
                    SELECT 1 FROM album_artists aa WHERE aa.artist_id = art.id
                )
                OR art.is_favorite = 1
                OR art.favorite_at IS NOT NULL
            )
            "#
        )
        .bind(&pattern)
        .bind(if only_fav { 1 } else { 0 })
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        result.total_artists = count_row.0;

        let artists_rows: Vec<(i64, String, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT 
                art.id,
                art.name,
                CASE WHEN art.favorite_at IS NOT NULL OR art.is_favorite = 1 THEN 1 ELSE 0 END as is_fav,
                (SELECT COUNT(*) FROM track_artists WHERE artist_id = art.id) as track_count,
                (SELECT COUNT(*) FROM album_artists WHERE artist_id = art.id) as album_count
            FROM artists art
            WHERE art.name LIKE ?
            AND (? = 0 OR art.favorite_at IS NOT NULL OR art.is_favorite = 1)
            AND (
                EXISTS (
                    SELECT 1 FROM track_artists ta WHERE ta.artist_id = art.id
                    UNION
                    SELECT 1 FROM album_artists aa WHERE aa.artist_id = art.id
                )
                OR art.is_favorite = 1
                OR art.favorite_at IS NOT NULL
            )
            ORDER BY art.name ASC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(&pattern)
        .bind(if only_fav { 1 } else { 0 })
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        result.artists = artists_rows
            .into_iter()
            .map(|(id, name, is_fav, track_count, album_count)| SearchResultArtist {
                id,
                name,
                is_favorite: is_fav == 1,
                track_count,
                album_count,
            })
            .collect();
    }

    // 4. Search Playlists
    if entity_type == "all" || entity_type == "playlists" {
        let count_row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM playlists p
            LEFT JOIN accounts a ON a.id = p.account_id
            LEFT JOIN services s ON s.id = a.service_id
            WHERE (p.name LIKE ? OR p.description LIKE ?)
            AND (? = 'all' OR LOWER(s.name) = LOWER(?))
            "#
        )
        .bind(&pattern).bind(&pattern)
        .bind(service_filter).bind(service_filter)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        result.total_playlists = count_row.0;

        let playlists_rows: Vec<(i64, String, Option<String>, i64, Option<String>)> = sqlx::query_as(
            r#"
            SELECT 
                p.id,
                p.name,
                p.description,
                p.track_count,
                s.name as service_name
            FROM playlists p
            LEFT JOIN accounts a ON a.id = p.account_id
            LEFT JOIN services s ON s.id = a.service_id
            WHERE (p.name LIKE ? OR p.description LIKE ?)
            AND (? = 'all' OR LOWER(s.name) = LOWER(?))
            ORDER BY p.name ASC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(&pattern).bind(&pattern)
        .bind(service_filter).bind(service_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        result.playlists = playlists_rows
            .into_iter()
            .map(|(id, name, description, track_count, service_name)| SearchResultPlaylist {
                id,
                name,
                description,
                track_count,
                service_name,
            })
            .collect();
    }

    Ok(result)
}
