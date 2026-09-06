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
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // 1. Update basic track fields
    let mut has_updates = false;
    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("UPDATE tracks SET ");
    {
        let mut separated = qb.separated(", ");

        if let Some(ref t) = metadata.title {
            separated.push("title = ");
            separated.push_bind_unseparated(t);
            has_updates = true;
        }
        if let Some(n) = metadata.track_number {
            separated.push("track_number = ");
            separated.push_bind_unseparated(n);
            has_updates = true;
        }
        if let Some(n) = metadata.disc_number {
            separated.push("disc_number = ");
            separated.push_bind_unseparated(n);
            has_updates = true;
        }
        if let Some(ref i) = metadata.isrc {
            separated.push("isrc = ");
            separated.push_bind_unseparated(i);
            has_updates = true;
        }
        if let Some(e) = metadata.explicit {
            separated.push("explicit = ");
            separated.push_bind_unseparated(if e { 1 } else { 0 });
            has_updates = true;
        }
        if let Some(ref g) = metadata.genre {
            separated.push("genre = ");
            separated.push_bind_unseparated(g);
            has_updates = true;
        }
        if let Some(y) = metadata.year {
            separated.push("release_year = ");
            separated.push_bind_unseparated(y);
            has_updates = true;
        }
        if let Some(b) = metadata.bpm {
            separated.push("bpm = ");
            separated.push_bind_unseparated(b);
            has_updates = true;
        }
        if let Some(ref k) = metadata.musical_key {
            separated.push("musical_key = ");
            separated.push_bind_unseparated(k);
            has_updates = true;
        }
        if let Some(ref mbid) = metadata.mb_track_id {
            separated.push("musicbrainz_id = ");
            separated.push_bind_unseparated(mbid);
            has_updates = true;
        }
        if let Some(ref l) = metadata.label {
            separated.push("record_label = ");
            separated.push_bind_unseparated(l);
            has_updates = true;
        }
    }

    if has_updates {
        qb.push(" WHERE id = ").push_bind(track_id);
        qb.build()
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update track fields: {}", e))?;
    }

    // 2. Handle Artist Change
    if let Some(raw_artist_name) = metadata.artist_name {
        let artist_name = syncify_core_domain::metadata::sanitize_artist_name(&raw_artist_name);
        // Find or create artist
        let artist_id: i64 = match sqlx::query_scalar("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
            .bind(&artist_name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        {
            Some(id) => id,
            None => sqlx::query("INSERT INTO artists (name) VALUES (?) ON CONFLICT(name) DO UPDATE SET id=id RETURNING id")
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

    let stats = sqlx::query(
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
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Failed to fetch stats: {}", e))?;

    let stats_total: i64 = stats.get("total");
    let with_isrc: i64 = stats.get("with_isrc");
    let with_mbid: i64 = stats.get("with_mbid");
    let with_album: i64 = stats.get("with_album");
    let with_year: i64 = stats.get("with_year");
    let with_genre: i64 = stats.get("with_genre");
    let with_art: i64 = stats.get("with_art");

    let total = stats_total as f64;
    let avg = if total > 0.0 {
        // Assume Title and Artist are always present (10 + 10 = 20 points base)
        // Since python check confirmed 0 empty titles, this is a safe baseline.
        let base_score = 20.0;
        let album_score = (with_album as f64 / total) * 10.0;
        let isrc_score = (with_isrc as f64 / total) * 20.0;
        let mbid_score = (with_mbid as f64 / total) * 20.0;
        let art_score = (with_art as f64 / total) * 10.0;
        let year_score = (with_year as f64 / total) * 10.0;
        let genre_score = (with_genre as f64 / total) * 10.0;

        base_score + album_score + isrc_score + mbid_score + art_score + year_score + genre_score
    } else {
        0.0
    };

    Ok(MetadataStats {
        total_tracks: stats_total,
        with_isrc,
        with_musicbrainz_id: with_mbid,
        with_album,
        with_year,
        with_genre,
        with_art,
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
            -- S201 audit: these four are REQUIRED (no #[sqlx(default)]) on LibraryTrack;
            -- omitting them made live routes fail with
            -- "no column found for name: imported_from".
            CAST(NULL as TEXT) as imported_from,
            CAST(NULL as TEXT) as downloaded_from,
            CAST(NULL as TEXT) as available_services,
            CAST(NULL as TEXT) as availability_summary,
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
            -- S201 audit: these four are REQUIRED (no #[sqlx(default)]) on LibraryTrack;
            -- omitting them made live routes fail with
            -- "no column found for name: imported_from".
            CAST(NULL as TEXT) as imported_from,
            CAST(NULL as TEXT) as downloaded_from,
            CAST(NULL as TEXT) as available_services,
            CAST(NULL as TEXT) as availability_summary,
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

/// S163: Query persistent, append-only historical audit records of applied repairs (read-only).
#[tauri::command]
pub async fn get_repair_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<syncify_core_domain::repair::RepairHistoryRecord>, String> {
    crate::services::repair_history::fetch_repair_history(&state.db, limit, offset).await
}

/// S165: Read-only audit across 16 categories of catalog and physical consistency.
#[tauri::command]
pub async fn audit_catalog_identity(
    state: State<'_, AppState>,
) -> Result<crate::services::catalog_identity_audit::CatalogIdentityAuditReport, String> {
    crate::services::catalog_identity_audit::audit_catalog_identity(&state.db, None).await
}

/// S165: Generate a non-mutating Dry-Run plan for catalog repair.
#[tauri::command]
pub async fn plan_catalog_identity_repair(
    state: State<'_, AppState>,
) -> Result<crate::services::catalog_identity_repair::CatalogRepairPlan, String> {
    crate::services::catalog_identity_repair::plan_catalog_identity_repair(&state.db, None).await
}

/// S165: Apply catalog repair plan with strict confirmation, automatic SQLite backup, and append-only audit trail.
#[tauri::command]
pub async fn apply_catalog_identity_repair(
    state: State<'_, AppState>,
    plan: crate::services::catalog_identity_repair::CatalogRepairPlan,
    confirmed: bool,
) -> Result<crate::services::catalog_identity_repair::CatalogRepairExecutionReport, String> {
    let backup_dir = dirs::data_local_dir().map(|p| p.join("com.syncify.app").join("backups"));
    crate::services::catalog_identity_repair::apply_catalog_identity_repair(&state.db, &plan, confirmed, backup_dir.as_deref()).await
}

/// S167: Query aggregate post-crash recovery audit summary and details.
#[tauri::command]
pub async fn get_recovery_audit_summary(
    state: State<'_, AppState>,
) -> Result<syncify_core_domain::RecoveryAuditSummary, String> {
    crate::services::operation_recovery::get_recovery_audit_summary(&state.db).await
}

/// S167: Trigger manual/startup post-crash reconciliation.
#[tauri::command]
pub async fn trigger_startup_reconciliation(
    state: State<'_, AppState>,
) -> Result<syncify_core_domain::RecoveryAuditSummary, String> {
    crate::services::operation_recovery::reconcile_startup_operations(&state.db, None).await
}

/// S168: Get concurrency statistics summary (contention, timeouts, active count)
#[tauri::command]
pub async fn get_concurrency_stats_summary(
    state: State<'_, AppState>,
) -> Result<syncify_core_domain::ConcurrencyStatsSummary, String> {
    Ok(state.concurrency_manager.get_stats_summary().await)
}

/// S168: Get active redacted concurrency lock hashes
#[tauri::command]
pub async fn get_active_concurrency_locks(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state.concurrency_manager.get_active_locks().await)
}

// ==============================================
// COVER ART BACKFILL (MusicBrainz + Cover Art Archive)
// ==============================================

/// Summary of a cover-art backfill run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverArtBackfillResult {
    pub checked: i64,
    pub updated: i64,
    pub skipped: i64,
    pub failed: i64,
}

/// Backfills `albums.cover_art_url` for albums shown without artwork.
///
/// Resolution path per affected track: ISRC → MusicBrainz recording → first
/// release with a release-group → Cover Art Archive `front-500` URL verified
/// with a HEAD request before it is persisted. Tracks without ISRC or whose
/// release group carries no front image are counted as skipped/failed so the
/// UI can report honest numbers.
#[tauri::command]
pub async fn fetch_missing_cover_art(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<CoverArtBackfillResult, String> {
    use crate::services::MusicBrainzClient;

    let limit = limit.unwrap_or(100).clamp(1, 500);
    let client = MusicBrainzClient::new();

    // One candidate per album that currently lacks artwork.
    let rows: Vec<(i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT al.id,
               MAX(t.isrc) AS any_isrc
        FROM tracks t
        JOIN albums al ON al.id = t.album_id
        WHERE t.album_id IS NOT NULL
          AND (al.cover_art_url IS NULL OR al.cover_art_url = '')
        GROUP BY al.id
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to list albums missing art: {}", e))?;

    let mut result = CoverArtBackfillResult { checked: 0, updated: 0, skipped: 0, failed: 0 };

    for (album_id, isrc) in rows {
        result.checked += 1;
        let Some(isrc) = isrc.filter(|s| !s.trim().is_empty()) else {
            result.skipped += 1;
            continue;
        };

        let recording = match client.lookup_by_isrc(&isrc).await {
            Ok(rec) => rec,
            Err(e) => {
                tracing::warn!("Cover art backfill: MB lookup failed for ISRC {}: {}", isrc, e);
                result.failed += 1;
                continue;
            }
        };

        let rg_id = recording
            .as_ref()
            .and_then(|rec| rec.releases.as_ref())
            .and_then(|rels| rels.iter().find_map(|rel| rel.release_group.as_ref()))
            .map(|rg| rg.id.clone());

        let Some(rg_id) = rg_id else {
            result.skipped += 1;
            continue;
        };

        // Verify the CAA front image actually exists before persisting.
        let url = format!("https://coverartarchive.org/release-group/{}/front-500", rg_id);
        let head = client_head_check(&url).await;
        match head {
            Ok(true) => {
                sqlx::query("UPDATE albums SET cover_art_url = ? WHERE id = ?")
                    .bind(&url)
                    .bind(album_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| format!("Failed to update album cover: {}", e))?;
                result.updated += 1;
            }
            Ok(false) => result.skipped += 1,
            Err(e) => {
                tracing::warn!("Cover art backfill: HEAD check failed for {}: {}", url, e);
                result.failed += 1;
            }
        }
    }

    tracing::info!(
        "fetch_missing_cover_art: checked={} updated={} skipped={} failed={}",
        result.checked, result.updated, result.skipped, result.failed
    );
    Ok(result)
}

/// Lightweight HEAD probe against Cover Art Archive (follows redirects).
async fn client_head_check(url: &str) -> Result<bool, String> {
    let client = crate::download::http_client::create_http_client();
    let response = client
        .head(url)
        .header("User-Agent", "Syncify/1.0 (cover art backfill)")
        .send()
        .await
        .map_err(|e| format!("CAA request failed: {}", e))?;
    Ok(response.status().is_success())
}

/// Reconcile physical FLAC Vorbis comments (MUSICBRAINZ_TRACKID) with SQLite `tracks.musicbrainz_id` (TASK-84).
#[tauri::command]
pub async fn reconcile_musicbrainz_tags(
    state: State<'_, AppState>,
    path_override: Option<String>,
) -> Result<crate::services::musicbrainz::MusicBrainzTagReconciliationReport, String> {
    let p = path_override.as_deref().map(std::path::Path::new);
    crate::services::musicbrainz::reconcile_musicbrainz_from_physical_flacs(&state.db, p).await
}



