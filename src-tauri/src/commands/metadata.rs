// Metadata Commands - included via include!() in mod.rs

use sqlx::Row; // Required for .get()

// ==============================================
// STRUCTS
// ==============================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTrackMetadata {
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artist_name: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub isrc: Option<String>,
    pub explicit: Option<bool>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub bpm: Option<f64>,
    pub musical_key: Option<String>,
    // MBIDs
    pub mb_track_id: Option<String>,
    pub _mb_release_id: Option<String>,
    pub _upc: Option<String>,
    pub _copyright: Option<String>,
    pub _composer: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataStats {
    pub total_tracks: i64,
    pub with_isrc: i64,
    pub with_musicbrainz_id: i64,
    pub with_album: i64,
    pub with_year: i64,
    pub with_genre: i64,
    pub with_art: i64,
    pub average_completeness: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataMatch {
    pub recording_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub release_date: Option<String>,
    pub score: i32,
    pub source: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSearchParams {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub _duration_ms: Option<i64>,
    pub _isrc: Option<String>,
}

// ==============================================
// COMMANDS
// ==============================================

#[tauri::command]
pub async fn update_track_metadata(
    state: State<'_, AppState>,
    track_id: i64,
    metadata: UpdateTrackMetadata,
) -> Result<LibraryTrack, String> {
    tracing::info!("Updating metadata for track {}: {:?}", track_id, metadata);

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // 1. Update basic track fields
    let query = "UPDATE tracks SET ".to_string();
    let mut updates = Vec::new();

    if let Some(ref t) = metadata.title {
        updates.push(format!("title = '{}'", t.replace("'", "''")));
    }
    if let Some(n) = metadata.track_number {
        updates.push(format!("track_number = {}", n));
    }
    if let Some(n) = metadata.disc_number {
        updates.push(format!("disc_number = {}", n));
    }
    if let Some(ref i) = metadata.isrc {
        updates.push(format!("isrc = '{}'", i));
    }
    if let Some(e) = metadata.explicit {
        updates.push(format!("explicit = {}", if e { 1 } else { 0 }));
    }
    if let Some(ref g) = metadata.genre {
        updates.push(format!("genre = '{}'", g.replace("'", "''")));
    }
    if let Some(y) = metadata.year {
        updates.push(format!("release_year = {}", y));
    }
    if let Some(b) = metadata.bpm {
        updates.push(format!("bpm = {}", b));
    }
    if let Some(ref k) = metadata.musical_key {
        updates.push(format!("musical_key = '{}'", k));
    }
    if let Some(ref mbid) = metadata.mb_track_id {
        updates.push(format!("musicbrainz_id = '{}'", mbid));
    }
    if let Some(ref l) = metadata.label {
        updates.push(format!("record_label = '{}'", l.replace("'", "''")));
    }

    if !updates.is_empty() {
        let sql = format!("{} {} WHERE id = {}", query, updates.join(", "), track_id);
        sqlx::query(&sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update track fields: {}", e))?;
    }

    // 2. Handle Artist Change
    if let Some(artist_name) = metadata.artist_name {
        // Find or create artist
        let artist_id: i64 = match sqlx::query_scalar("SELECT id FROM artists WHERE name = ?")
            .bind(&artist_name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        {
            Some(id) => id,
            None => sqlx::query("INSERT INTO artists (name) VALUES (?) RETURNING id")
                .bind(&artist_name)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert artist: {}", e))?
                .get(0),
        };

        // Update link
        sqlx::query("DELETE FROM track_artists WHERE track_id = ?")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'main')")
            .bind(track_id)
            .bind(artist_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // 3. Handle Album Change
    if let Some(album_name) = metadata.album_name {
        let album_id: i64 = match sqlx::query_scalar("SELECT id FROM albums WHERE title = ?")
            .bind(&album_name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
        {
            Some(id) => id,
            None => {
                let release_year = metadata.year.or(Some(2024));
                sqlx::query(
                    "INSERT INTO albums (title, release_date) VALUES (?, ?) RETURNING id",
                )
                .bind(&album_name)
                .bind(format!("{}-01-01", release_year.unwrap_or(2024)))
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?
                .get(0)
            }
        };

        sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ?")
            .bind(album_id)
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit changes: {}", e))?;

    // Return updated track
    get_track_details(&state.db, track_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_metadata_stats(state: State<'_, AppState>) -> Result<MetadataStats, String> {
    tracing::info!("Fetching metadata stats");

    let stats = sqlx::query!(
        r#"
        SELECT 
            COUNT(*) as total,
            COUNT(CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 1 END) as with_isrc,
            COUNT(CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 1 END) as with_mbid,
            COUNT(t.album_id) as with_album,
            COUNT(CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 1 END) as with_year,
            COUNT(CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 1 END) as with_genre,
            COUNT(CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 1 END) as with_art
        FROM tracks t
        LEFT JOIN albums al ON t.album_id = al.id
        "#
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch stats: {}", e))?;

    let total = stats.total as f64;
    let avg = if total > 0.0 {
        // Assume Title and Artist are always present (10 + 10 = 20 points base)
        // Since python check confirmed 0 empty titles, this is a safe baseline.
        let base_score = 20.0;
        let album_score = (stats.with_album as f64 / total) * 10.0;
        let isrc_score = (stats.with_isrc as f64 / total) * 20.0;
        let mbid_score = (stats.with_mbid as f64 / total) * 20.0;
        let art_score = (stats.with_art as f64 / total) * 10.0;
        let year_score = (stats.with_year as f64 / total) * 10.0;
        let genre_score = (stats.with_genre as f64 / total) * 10.0;
        
        base_score + album_score + isrc_score + mbid_score + art_score + year_score + genre_score
    } else {
        0.0
    };

    Ok(MetadataStats {
        total_tracks: stats.total,
        with_isrc: stats.with_isrc,
        with_musicbrainz_id: stats.with_mbid,
        with_album: stats.with_album,
        with_year: stats.with_year,
        with_genre: stats.with_genre,
        with_art: stats.with_art,
        average_completeness: avg,
    })
}

#[tauri::command]
pub async fn get_tracks_needing_metadata(
    state: State<'_, AppState>,
    limit: i32,
) -> Result<Vec<LibraryTrack>, String> {
    let limit = limit.min(100);
    let tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT
            t.id, t.title,
            ar.name as artist_name,
            ar.id as artist_id,
            al.title as album_name,
            t.album_id,
            t.duration_ms,
            t.isrc,
            CAST(NULL as TEXT) as services,
            ts.format as quality,
            CAST(NULL as TEXT) as download_status,
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            CAST(NULL as TEXT) as lyrics_type,
            al.cover_art_url,
            CAST(NULL as TEXT) as spotify_track_id,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            CAST(NULL as TEXT) as file_path
        FROM tracks t
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'main'
        LEFT JOIN artists ar ON ta.artist_id = ar.id
        LEFT JOIN track_sources ts ON t.id = ts.track_id
        WHERE (t.musicbrainz_id IS NULL OR t.musicbrainz_id = 'NOT_FOUND')
        ORDER BY t.id DESC
        LIMIT ?
        "#
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch tracks: {}", e))?;

    Ok(tracks)
}

#[tauri::command]
pub async fn match_musicbrainz(
    params: MetadataSearchParams,
) -> Result<Vec<MetadataMatch>, String> {
    let client = crate::services::musicbrainz::MusicBrainzClient::new();

    // Use title and artist for search
    let results = client.search_recordings(
        &params.title,
        &params.artist,
        params.album.as_deref(),
        5
    ).await.map_err(|e| e.to_string())?;

    let matches: Vec<MetadataMatch> = results.into_iter().map(|r| {
        let artist_credit = r.artist_credit.clone().unwrap_or_default();
        let artist_name = artist_credit.first().map(|ac| ac.name.clone()).unwrap_or_default();

        let release = r.releases.as_ref().and_then(|rel| rel.first());
        let album_title = release.map(|rel| rel.title.clone());

        MetadataMatch {
            recording_id: r.id,
            title: r.title,
            artist: artist_name,
            album: album_title,
            release_date: None,
            score: 90,
            source: "musicbrainz".to_string(),
        }
    }).collect();

    Ok(matches)
}

#[tauri::command]
pub async fn apply_musicbrainz_match(
    state: State<'_, AppState>,
    track_id: i64,
    recording_id: String,
) -> Result<bool, String> {
    sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
        .bind(recording_id)
        .bind(track_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to update track: {}", e))?;

    Ok(true)
}

// ==============================================
// HELPERS
// ==============================================

async fn get_track_details(db: &sqlx::SqlitePool, track_id: i64) -> Result<LibraryTrack, sqlx::Error> {
    let track: Option<LibraryTrack> = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT
            t.id, t.title,
            ar.name as artist_name,
            ar.id as artist_id,
            al.title as album_name,
            t.album_id,
            t.duration_ms,
            t.isrc,
            CAST(NULL as TEXT) as services,
            ts.format as quality,
            CAST(NULL as TEXT) as download_status,
            (
                CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                CASE WHEN al.title IS NOT NULL AND al.title != '' THEN 10 ELSE 0 END +
                CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                CASE WHEN t.musicbrainz_id IS NOT NULL THEN 20 ELSE 0 END +
                CASE WHEN al.cover_art_url IS NOT NULL AND al.cover_art_url != '' THEN 10 ELSE 0 END +
                CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
            ) as metadata_score,
            CAST(NULL as TEXT) as lyrics_type,
            al.cover_art_url,
            CAST(NULL as TEXT) as spotify_track_id,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            CAST(NULL as TEXT) as file_path
        FROM tracks t
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'main'
        LEFT JOIN artists ar ON ta.artist_id = ar.id
        LEFT JOIN track_sources ts ON t.id = ts.track_id
        WHERE t.id = ?
        LIMIT 1
        "#
    )
    .bind(track_id)
    .fetch_optional(db)
    .await?;

    track.ok_or(sqlx::Error::RowNotFound)
}

/// S158: Query rich non-mutating dry-run repair audit items for all corrupt Tidal downloads.
#[tauri::command]
pub async fn get_tidal_repair_dry_run(
    state: State<'_, AppState>,
) -> Result<Vec<crate::services::tidal_pipeline::DownloadRepairDryRunItem>, String> {
    crate::services::tidal_pipeline::compute_download_repair_dry_run(&state.db).await
}

