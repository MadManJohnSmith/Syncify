#[allow(unused_imports)]
use super::*;

// Migration Commands - submodule of crate::commands
//
// Service-to-service migration, templates, matching

// ==============================================
// SPRINT 6: MIGRATION COMMANDS
// ==============================================

use crate::models::{
    DestinationTrackMatch, MigrationItem, MigrationJob, MigrationOptions, MigrationPreviewResult,
    MigrationProgress, MigrationReport, MigrationTemplate, PlaylistPreview,
};

const REQUIRED_AUDIT_TABLES: [&str; 5] = [
    "services",
    "accounts",
    "migration_jobs",
    "migration_items",
    "migration_templates",
];

async fn detect_schema_version(db: &sqlx::SqlitePool) -> Result<i64, String> {
    match sqlx::query_as::<_, (Option<i64>,)>("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(db)
        .await
    {
        Ok((Some(version),)) => Ok(version),
        Ok((None,)) | Err(_) => sqlx::query_as::<_, (i64,)>("PRAGMA user_version")
            .fetch_one(db)
            .await
            .map(|(version,)| version)
            .map_err(|e| format!("Failed to read schema version: {}", e)),
    }
}

async fn collect_migration_audit(db: &sqlx::SqlitePool) -> Result<MigrationReport, String> {
    let schema_version = detect_schema_version(db).await?;

    let existing_tables: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table'")
            .fetch_all(db)
            .await
            .map_err(|e| format!("Failed to inspect schema tables: {}", e))?;

    let existing_table_set: std::collections::HashSet<String> = existing_tables
        .into_iter()
        .map(|(table_name,)| table_name)
        .collect();

    let missing_tables: Vec<String> = REQUIRED_AUDIT_TABLES
        .iter()
        .filter(|table_name| !existing_table_set.contains(**table_name))
        .map(|table_name| (*table_name).to_string())
        .collect();

    let has_credentials_invalid_column = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name = 'credentials_invalid'",
    )
    .fetch_one(db)
    .await
    .map(|(count,)| count > 0)
    .unwrap_or(false);

    let legacy_services_detected = if has_credentials_invalid_column {
        sqlx::query_as::<_, (String,)>(
            "SELECT s.name FROM accounts a JOIN services s ON s.id = a.service_id WHERE COALESCE(a.credentials_invalid, 0) = 1 GROUP BY s.name ORDER BY s.name",
        )
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to inspect legacy credentials: {}", e))?
        .into_iter()
        .map(|(service_name,)| service_name)
        .collect()
    } else {
        Vec::new()
    };

    let schema_ok = missing_tables.is_empty();
    let summary = format!(
        "Schema v{} {}, {}.",
        schema_version,
        if schema_ok { "OK" } else { "incomplete" },
        if legacy_services_detected.is_empty() {
            "no legacy creds detected"
        } else {
            "legacy creds detected"
        }
    );

    Ok(MigrationReport {
        schema_version,
        schema_ok,
        missing_tables,
        legacy_services_detected,
        summary,
    })
}

/// Run migration schema/state audit for MigrationView
#[tauri::command]
pub async fn run_migration_audit(state: State<'_, AppState>) -> Result<MigrationReport, String> {
    collect_migration_audit(&state.db).await
}

/// Get migration history
#[tauri::command]
pub async fn get_migration_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<MigrationJob>, String> {
    let limit = limit.unwrap_or(50);
    sqlx::query_as::<_, MigrationJob>(
        r#"SELECT id, source_service, destination_service, source_playlist_ids, options, status,
            total_items, completed_items, failed_items, skipped_items, started_at, completed_at,
            error_message, created_at FROM migration_jobs ORDER BY created_at DESC LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to get migration history: {}", e))
}

/// Get migration job details
#[tauri::command]
pub async fn get_migration_details(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<MigrationJob, String> {
    sqlx::query_as::<_, MigrationJob>("SELECT * FROM migration_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| format!("Migration job {} not found", job_id))
}

/// Get migration items by status
#[tauri::command]
pub async fn get_migration_items_by_status(
    state: State<'_, AppState>,
    job_id: String,
    status: Option<String>,
) -> Result<Vec<MigrationItem>, String> {
    let query = if let Some(s) = status {
        sqlx::query_as::<_, MigrationItem>(
            "SELECT * FROM migration_items WHERE job_id = ? AND status = ? ORDER BY id",
        )
        .bind(&job_id)
        .bind(&s)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, MigrationItem>(
            "SELECT * FROM migration_items WHERE job_id = ? ORDER BY id",
        )
        .bind(&job_id)
        .fetch_all(&state.db)
        .await
    };
    query.map_err(|e| format!("Failed to get migration items: {}", e))
}

/// Preview a migration before starting.
///
/// The counts are real: every track the job would process (the same
/// `fetch_migration_source_tracks` selection start_migration uses) is matched
/// against the destination service with the engine's ISRC/metadata search —
/// in a side-effect free dry run, so nothing is added to the destination.
#[tauri::command]
pub async fn preview_migration(
    state: State<'_, AppState>,
    source_service: String,
    destination_service: String,
    playlist_ids: Option<Vec<String>>,
    options: MigrationOptions,
) -> Result<MigrationPreviewResult, String> {
    let clients = DestinationClients::for_destination(&state.db, &destination_service).await;
    if !clients.available() {
        // Without a destination account no real match count exists; refusing
        // beats reporting a fabricated estimate.
        return Err(format!(
            "No connected {} account is available to preview matches; connect the destination service first",
            destination_service
        ));
    }

    let tracks =
        fetch_migration_source_tracks(&state.db, &source_service, playlist_ids.as_deref()).await?;

    let mut matched_tracks = 0i64;
    // Per-playlist aggregation: (external id, real name, track_count, matched_count)
    let mut playlist_stats: Vec<(String, String, i64, i64)> = Vec::new();

    for track in &tracks {
        let isrc: Option<(String,)> = sqlx::query_as(
            "SELECT t.isrc FROM tracks t
             JOIN track_sources ts ON ts.track_id = t.id
             WHERE ts.service_track_id = ? AND t.isrc IS NOT NULL LIMIT 1",
        )
        .bind(&track.external_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let (match_confidence, _method, dest_track_id, _error) = clients
            .search_match(
                isrc.as_ref().map(|(value,)| value.as_str()),
                &track.title,
                &track.artist,
            )
            .await;
        let is_match = match_confidence >= options.match_threshold && dest_track_id.is_some();
        if is_match {
            matched_tracks += 1;
        }

        if let (Some(playlist_id), Some(playlist_name)) = (&track.playlist_id, &track.playlist_name)
        {
            match playlist_stats.iter_mut().find(|s| s.0 == *playlist_id) {
                Some(stats) => {
                    stats.2 += 1;
                    if is_match {
                        stats.3 += 1;
                    }
                }
                None => playlist_stats.push((
                    playlist_id.clone(),
                    playlist_name.clone(),
                    1,
                    if is_match { 1 } else { 0 },
                )),
            }
        }

        // Same pacing as start_migration to stay within the destination's
        // rate limits while matching.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    let total_tracks = tracks.len() as i64;
    Ok(MigrationPreviewResult {
        total_tracks,
        matched_tracks,
        unmatched_tracks: total_tracks - matched_tracks,
        playlists: playlist_stats
            .into_iter()
            .map(|(id, name, track_count, matched_count)| PlaylistPreview {
                id,
                name,
                track_count,
                matched_count,
            })
            .collect(),
    })
}

/// Services the wizard may offer as migration destinations (data-driven:
/// derived from MIGRATION_DESTINATION_SERVICES, so a new backend destination
/// appears in the UI without any frontend change).
#[tauri::command]
pub async fn get_migration_destinations() -> Result<Vec<String>, String> {
    Ok(MIGRATION_DESTINATION_SERVICES
        .iter()
        .map(|service| service.to_string())
        .collect())
}

/// Format a remaining-time estimate (in seconds) in a human readable form.
fn format_eta(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as i64;
    if total_seconds < 60 {
        format!("{} s", total_seconds)
    } else if total_seconds < 3600 {
        format!("{} min {} s", total_seconds / 60, total_seconds % 60)
    } else {
        format!(
            "{} h {} min",
            total_seconds / 3600,
            (total_seconds % 3600) / 60
        )
    }
}

/// Build the `migration-progress` payload, deriving `percent`, `speed` and `eta`
/// from the items processed so far and the time elapsed since the job started.
fn build_migration_progress(
    elapsed_secs: f64,
    job_id: &str,
    current_item: i64,
    total_items: i64,
    current_track: String,
    current_action: String,
    status: &str,
    completed_count: i64,
    failed_count: i64,
    skipped_count: i64,
) -> MigrationProgress {
    let processed = completed_count + failed_count + skipped_count;

    let percent = if total_items > 0 {
        ((current_item as f64 / total_items as f64) * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let speed = if elapsed_secs > 0.0 {
        ((processed as f64 / elapsed_secs) * 60.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let eta = if total_items > 0 && processed >= total_items {
        "0 s".to_string()
    } else if speed > 0.0 {
        let remaining_secs = (total_items - processed).max(0) as f64 / speed * 60.0;
        format_eta(remaining_secs)
    } else {
        "calculating...".to_string()
    };

    MigrationProgress {
        job_id: job_id.to_string(),
        current_item,
        total_items,
        current_track,
        status: status.to_string(),
        completed_count,
        failed_count,
        skipped_count,
        percent,
        speed,
        eta,
        current_action,
    }
}

/// A source track selected for a migration job, plus the playlist it was
/// pulled from (both playlist fields `None` when the job covers the source
/// service's favorites/likes instead of specific playlists).
#[derive(Debug, Clone)]
pub struct MigrationSourceTrack {
    pub external_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub playlist_id: Option<String>,
    pub playlist_name: Option<String>,
}

/// Rows read per page while the migration source set is walked.
///
/// BD-3: both source queries ended in a hard `LIMIT 1000`, so a service library
/// (or playlist) with more tracks migrated only its first thousand — silently,
/// because the job counted the truncated result and closed as `completed`.
const MIGRATION_SOURCE_PAGE_SIZE: i64 = 500;

/// Resolve the tracks a migration job will process.
///
/// With `playlist_ids` (the external playlist ids the user selected, the same
/// ids stored in `migration_jobs.source_playlist_ids`) only the tracks of
/// those playlists are returned — one deduplicated row per (track, playlist),
/// so a track living in several selected playlists yields one item per
/// playlist with its own attribution. With `None` the whole source-service
/// library (favorites view) is used. BD-6: the selection is actually applied
/// here — before this filter, a playlist-scoped job silently migrated every
/// playlist of the source service.
///
/// BD-1: `library_items` mirrors one row per canonical track and keeps only the
/// track's *best* source, so `library_items.external_id` belongs to whichever
/// service won that ranking — reading it for a job whose source service is a
/// different one either dropped the track (`source_service` filter) or sent an
/// id that means nothing to the source API. The external id comes from the
/// `track_sources` row of the source service instead, which also keeps a track
/// shared between two providers migratable from both.
///
/// BD-3: the result set is walked in pages of `MIGRATION_SOURCE_PAGE_SIZE`
/// until a short page arrives, so the total the job reports is the real one.
pub async fn fetch_migration_source_tracks(
    db: &sqlx::SqlitePool,
    source_service: &str,
    playlist_ids: Option<&[String]>,
) -> Result<Vec<MigrationSourceTrack>, String> {
    let playlist_scoped = playlist_ids.is_some();
    let ids: &[String] = playlist_ids.unwrap_or(&[]);
    if playlist_scoped && ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut tracks: Vec<MigrationSourceTrack> = Vec::new();
    let mut offset = 0i64;

    loop {
        let page = if playlist_scoped {
            let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                r#"SELECT DISTINCT ts.service_track_id AS external_id, li.title, li.artist, li.album,
                          COALESCE(p.external_id, CAST(p.id AS TEXT)) AS playlist_id,
                          p.name AS playlist_name
                   FROM library_items li
                   JOIN track_sources ts ON ts.track_id = li.id
                   JOIN services s ON s.id = ts.service_id
                   JOIN playlist_tracks pt ON pt.track_id = li.id
                   JOIN playlists p ON p.id = pt.playlist_id
                   WHERE s.name = "#,
            );
            query.push_bind(source_service);
            query.push(" AND p.source_service = ");
            query.push_bind(source_service);
            query.push(" AND p.external_id IN (");
            let mut separated = query.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
            query
                .push(")")
                // Ordering by every selected column gives the pages a total
                // order, so no row is read twice or skipped across page bounds.
                .push(
                    " ORDER BY external_id, title, artist, album, playlist_id, playlist_name
                      LIMIT ",
                )
                .push_bind(MIGRATION_SOURCE_PAGE_SIZE)
                .push(" OFFSET ")
                .push_bind(offset);

            let rows: Vec<(
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            )> = query
                .build_query_as()
                .fetch_all(db)
                .await
                .map_err(|e| format!("Failed to load migration tracks: {}", e))?;

            rows.into_iter()
                .map(
                    |(external_id, title, artist, album, playlist_id, playlist_name)| {
                        MigrationSourceTrack {
                            external_id,
                            title,
                            artist,
                            album,
                            playlist_id,
                            playlist_name,
                        }
                    },
                )
                .collect::<Vec<_>>()
        } else {
            let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                r#"SELECT ts.service_track_id, li.title, li.artist, li.album
                   FROM library_items li
                   JOIN track_sources ts ON ts.track_id = li.id
                   JOIN services s ON s.id = ts.service_id
                   WHERE s.name = "#,
            );
            query
                .push_bind(source_service)
                // `track_sources` is UNIQUE on (service_id, service_track_id),
                // so the service_track_id order is already a total order.
                .push(" ORDER BY ts.service_track_id LIMIT ")
                .push_bind(MIGRATION_SOURCE_PAGE_SIZE)
                .push(" OFFSET ")
                .push_bind(offset);

            let rows: Vec<(String, String, String, Option<String>)> = query
                .build_query_as()
                .fetch_all(db)
                .await
                .map_err(|e| format!("Failed to load migration tracks: {}", e))?;

            rows.into_iter()
                .map(|(external_id, title, artist, album)| MigrationSourceTrack {
                    external_id,
                    title,
                    artist,
                    album,
                    playlist_id: None,
                    playlist_name: None,
                })
                .collect::<Vec<_>>()
        };

        let fetched = page.len();
        tracks.extend(page);
        if (fetched as i64) < MIGRATION_SOURCE_PAGE_SIZE {
            break;
        }
        offset += MIGRATION_SOURCE_PAGE_SIZE;
    }

    Ok(tracks)
}

/// Insert one pending migration item and return its row id.
///
/// BD-7: the row id is what the per-item result update addresses —
/// `source_track_id` is the source service's external id and may repeat within
/// a job (the same track in several selected playlists), and the playlist
/// columns (0017) are written here instead of staying NULL forever.
pub async fn insert_migration_item(
    db: &sqlx::SqlitePool,
    job_id: &str,
    track: &MigrationSourceTrack,
) -> Result<Option<i64>, String> {
    sqlx::query_scalar(
        r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist,
                                        source_track_album, source_playlist_id, source_playlist_name, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')
           RETURNING id"#,
    )
    .bind(job_id)
    .bind(&track.external_id)
    .bind(&track.title)
    .bind(&track.artist)
    .bind(&track.album)
    .bind(&track.playlist_id)
    .bind(&track.playlist_name)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to insert migration item: {}", e))
}

/// Destination services the migration engine can transfer to.
///
/// This list is the single source of truth for what the wizard may offer as a
/// destination (served to the UI by `get_migration_destinations`): adding a
/// destination means adding its client support to `DestinationClients` (and to
/// `search_destination_track`) plus one entry here — no UI change required.
pub const MIGRATION_DESTINATION_SERVICES: &[&str] = &[
    "qobuz",
    "tidal",
    "spotify",
    "deezer",
    "soundcloud",
    "apple_music",
];

/// Destination clients used by the migration engine to match and transfer
/// tracks. Every field stays `None` unless the destination service maps to it
/// and a usable account exists; `for_destination` mirrors exactly what
/// `start_migration` used to build inline.
struct DestinationClients {
    qobuz: Option<crate::services::QobuzClient>,
    tidal: Option<crate::services::TidalClient>,
    spotify: Option<crate::services::SpotifyClient>,
    deezer: Option<crate::services::DeezerClient>,
    soundcloud: Option<crate::services::SoundCloudClient>,
    apple_music: Option<crate::services::AppleMusicClient>,
}

impl DestinationClients {
    /// True when at least one destination client could be constructed.
    fn available(&self) -> bool {
        self.qobuz.is_some()
            || self.tidal.is_some()
            || self.spotify.is_some()
            || self.deezer.is_some()
            || self.soundcloud.is_some()
            || self.apple_music.is_some()
    }

    /// Build the clients for `destination_service` from the account stored in
    /// the database (code moved verbatim from start_migration).
    async fn for_destination(db: &sqlx::SqlitePool, destination_service: &str) -> Self {
        let service = destination_service.to_lowercase();

        let qobuz: Option<crate::services::QobuzClient> = if service == "qobuz" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'qobuz' AND a.is_active = 1",
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    if let Some(token) = creds.get("user_auth_token").and_then(|v| v.as_str()) {
                        let app_id = std::env::var("QOBUZ_APP_ID")
                            .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
                        let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_default();
                        Some(crate::services::QobuzClient::new_with_token(
                            app_id,
                            app_secret,
                            token.to_string(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let tidal: Option<crate::services::TidalClient> = if service == "tidal" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'tidal' AND a.is_active = 1",
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    let access_token = creds.get("access_token").and_then(|v| v.as_str());
                    let user_id = creds.get("user_id").and_then(|v| v.as_str());
                    let country_code = creds
                        .get("country_code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("US");

                    if let (Some(token), Some(uid)) = (access_token, user_id) {
                        Some(
                            crate::services::TidalClient::new(token.to_string())
                                .with_user(uid.to_string(), country_code.to_string()),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let spotify: Option<crate::services::SpotifyClient> = if service == "spotify" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'spotify' AND a.is_active = 1",
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    creds
                        .get("access_token")
                        .and_then(|v| v.as_str())
                        .map(|token| {
                            crate::services::SpotifyClient::new(token.to_string(), None, 0)
                        })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let deezer: Option<crate::services::DeezerClient> = if service == "deezer" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'deezer' AND a.is_active = 1",
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    if let Some(arl) = creds.get("arl").and_then(|v| v.as_str()) {
                        let mut client = crate::services::DeezerClient::new(arl.to_string());
                        // Initialize the client to get API token
                        if client.init().await.is_ok() {
                            Some(client)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let soundcloud: Option<crate::services::SoundCloudClient> = if service == "soundcloud" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'soundcloud' AND a.is_active = 1",
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    let oauth_token = creds.get("oauth_token").and_then(|v| v.as_str());
                    let user_id = creds.get("user_id").and_then(|v| v.as_i64());

                    if let (Some(token), Some(uid)) = (oauth_token, user_id) {
                        Some(
                            crate::services::SoundCloudClient::new(token.to_string())
                                .with_user_id(uid),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let apple_music: Option<crate::services::AppleMusicClient> = if service == "apple_music" {
            let creds: Option<(String,)> = sqlx::query_as(
                "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'apple_music' AND a.is_active = 1",
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

            if let Some((creds_json,)) = creds {
                let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                    let developer_token = creds
                        .get("developer_token")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|t| !t.is_empty());
                    let music_user_token = creds
                        .get("music_user_token")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .filter(|t| !t.is_empty());

                    match (developer_token, music_user_token) {
                        (Some(dev), Some(user)) => {
                            Some(crate::services::AppleMusicClient::from_credentials(
                                dev.to_string(),
                                user.to_string(),
                                &creds,
                            ))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Self {
            qobuz,
            tidal,
            spotify,
            deezer,
            soundcloud,
            apple_music,
        }
    }

    /// Search one source track on the destination service with the engine's
    /// ISRC-first / metadata-fallback strategy. No side effects: nothing is
    /// written to the destination, so the preview can count real matches.
    ///
    /// Returns `(confidence, method, destination_track_id, error)` exactly as
    /// the matching arms of start_migration used to produce them.
    async fn search_match(
        &self,
        isrc: Option<&str>,
        title: &str,
        artist: &str,
    ) -> (f64, &'static str, Option<String>, Option<String>) {
        if let Some(ref client) = self.qobuz {
            // Real Qobuz matching
            if let Some(track_isrc) = isrc {
                // Try ISRC match first (most reliable)
                match client.search_by_isrc(track_isrc).await {
                    Ok(Some(result)) => (1.0, "isrc", Some(result.track_id), None),
                    Ok(None) => {
                        // ISRC not found, try metadata
                        match client.match_by_metadata(title, artist).await {
                            Ok(Some(result)) => (0.85, "metadata", Some(result.track_id), None),
                            _ => (0.0, "none", None, None),
                        }
                    }
                    Err(e) => {
                        tracing::warn!("ISRC search failed: {}", e);
                        (
                            0.0,
                            "none",
                            None,
                            Some(format!("ISRC search failed: {}", e)),
                        )
                    }
                }
            } else {
                // No ISRC, try metadata matching
                match client.match_by_metadata(title, artist).await {
                    Ok(Some(result)) => (0.80, "metadata", Some(result.track_id), None),
                    _ => (0.0, "none", None, None),
                }
            }
        } else if let Some(ref client) = self.tidal {
            // Real Tidal matching
            if let Some(track_isrc) = isrc {
                match client.search_by_isrc(track_isrc).await {
                    Ok(Some(result)) => (1.0, "isrc", Some(result.track_id), None),
                    Ok(None) => match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => (0.85, "metadata", Some(result.track_id), None),
                        _ => (0.0, "none", None, None),
                    },
                    Err(e) => {
                        tracing::warn!("Tidal ISRC search failed: {}", e);
                        (
                            0.0,
                            "none",
                            None,
                            Some(format!("ISRC search failed: {}", e)),
                        )
                    }
                }
            } else {
                match client.match_by_metadata(title, artist).await {
                    Ok(Some(result)) => (0.80, "metadata", Some(result.track_id), None),
                    _ => (0.0, "none", None, None),
                }
            }
        } else if let Some(ref client) = self.spotify {
            // Real Spotify matching
            if let Some(track_isrc) = isrc {
                match client.search_by_isrc(track_isrc).await {
                    Ok(Some(result)) => (1.0, "isrc", Some(result.track_id), None),
                    Ok(None) => match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => (0.85, "metadata", Some(result.track_id), None),
                        _ => (0.0, "none", None, None),
                    },
                    Err(e) => {
                        tracing::warn!("Spotify ISRC search failed: {}", e);
                        (
                            0.0,
                            "none",
                            None,
                            Some(format!("ISRC search failed: {}", e)),
                        )
                    }
                }
            } else {
                match client.match_by_metadata(title, artist).await {
                    Ok(Some(result)) => (0.80, "metadata", Some(result.track_id), None),
                    _ => (0.0, "none", None, None),
                }
            }
        } else if let Some(ref client) = self.deezer {
            // Real Deezer matching
            if let Some(track_isrc) = isrc {
                match client.search_by_isrc(track_isrc).await {
                    Ok(Some(result)) => (1.0, "isrc", Some(result.track_id), None),
                    Ok(None) => match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => (0.85, "metadata", Some(result.track_id), None),
                        _ => (0.0, "none", None, None),
                    },
                    Err(e) => {
                        tracing::warn!("Deezer ISRC search failed: {}", e);
                        (
                            0.0,
                            "none",
                            None,
                            Some(format!("ISRC search failed: {}", e)),
                        )
                    }
                }
            } else {
                match client.match_by_metadata(title, artist).await {
                    Ok(Some(result)) => (0.80, "metadata", Some(result.track_id), None),
                    _ => (0.0, "none", None, None),
                }
            }
        } else if let Some(ref client) = self.soundcloud {
            // Real SoundCloud matching (no ISRC support)
            match client.match_by_metadata(title, artist).await {
                Ok(Some(result)) => (0.75, "metadata", Some(result.track_id), None),
                _ => (0.0, "none", None, None),
            }
        } else if let Some(ref client) = self.apple_music {
            // Real Apple Music matching (ISRC-first, text search fallback)
            if let Some(track_isrc) = isrc {
                match client.search_by_isrc(track_isrc).await {
                    Ok(Some(result)) => (1.0, "isrc", Some(result.track_id), None),
                    Ok(None) => match client.match_by_metadata(title, artist).await {
                        Ok(Some(result)) => (0.85, "metadata", Some(result.track_id), None),
                        _ => (0.0, "none", None, None),
                    },
                    Err(e) => {
                        tracing::warn!("Apple Music ISRC search failed: {}", e);
                        (
                            0.0,
                            "none",
                            None,
                            Some(format!("ISRC search failed: {}", e)),
                        )
                    }
                }
            } else {
                match client.match_by_metadata(title, artist).await {
                    Ok(Some(result)) => (0.80, "metadata", Some(result.track_id), None),
                    _ => (0.0, "none", None, None),
                }
            }
        } else {
            // No destination client available, use simulated matching
            (
                0.85,
                "simulated",
                None,
                Some(
                    "Destination service client unavailable; the track could not be transferred"
                        .to_string(),
                ),
            )
        }
    }

    /// Add one destination track to the destination account's favorites — the
    /// engine's transfer step. Fails when no destination client is available.
    async fn add_favorite(&self, dest_track_id: &str) -> Result<(), String> {
        if let Some(ref client) = self.qobuz {
            client.add_to_favorites(dest_track_id).await
        } else if let Some(ref client) = self.tidal {
            client.add_to_favorites(dest_track_id).await
        } else if let Some(ref client) = self.spotify {
            client.add_to_favorites(dest_track_id).await
        } else if let Some(ref client) = self.deezer {
            client.add_to_favorites(dest_track_id).await
        } else if let Some(ref client) = self.soundcloud {
            client.add_to_favorites(dest_track_id).await
        } else if let Some(ref client) = self.apple_music {
            client.add_to_favorites(dest_track_id).await
        } else {
            Err(
                "Destination service client unavailable; the track could not be transferred"
                    .to_string(),
            )
        }
    }

    /// Match one source track and, when a destination track was found, add it
    /// to the destination favorites. This is start_migration's per-track
    /// behavior, expressed over `search_match` + `add_favorite`.
    async fn match_and_transfer(
        &self,
        isrc: Option<&str>,
        title: &str,
        artist: &str,
    ) -> (f64, &'static str, Option<String>, Option<String>) {
        let (confidence, method, dest_track_id, error) =
            self.search_match(isrc, title, artist).await;
        match &dest_track_id {
            Some(dest_id) => match self.add_favorite(dest_id).await {
                Ok(_) => (confidence, method, dest_track_id, None),
                Err(e) => {
                    tracing::warn!("Failed to add to favorites: {}", e);
                    // Match found but transfer failed
                    (
                        confidence,
                        method,
                        None,
                        Some(format!("Failed to add to favorites: {}", e)),
                    )
                }
            },
            None => (confidence, method, dest_track_id, error),
        }
    }
}

/// Most recent match a user attached by hand to this source track on any
/// previous migration job toward the same destination service. Reviewed
/// matches must be effective: the next run applies them instead of silently
/// recomputing (and possibly undoing) the user's decision.
pub async fn find_manual_match(
    db: &sqlx::SqlitePool,
    source_track_id: &str,
    destination_service: &str,
) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT mi.destination_track_id
           FROM migration_items mi
           JOIN migration_jobs mj ON mj.id = mi.job_id
           WHERE mi.source_track_id = ?
             AND mi.match_method = 'manual'
             AND mi.destination_track_id IS NOT NULL
             AND mj.destination_service = ?
           ORDER BY mi.id DESC
           LIMIT 1"#,
    )
    .bind(source_track_id)
    .bind(destination_service)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to look up manual matches: {}", e))?;
    Ok(row.map(|(dest_track_id,)| dest_track_id))
}

/// Record one item's match outcome on its exact row (by id).
///
/// BD-7: matching by `job_id AND source_track_id` updated every row sharing
/// the external id in one blow and left no room for a per-item error message;
/// `error_message` is written here for failed items (and cleared otherwise).
pub async fn record_migration_item_result(
    db: &sqlx::SqlitePool,
    item_id: i64,
    status: &str,
    match_confidence: f64,
    match_method: &str,
    dest_track_id: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"UPDATE migration_items
           SET status = ?, match_confidence = ?, match_method = ?,
               dest_track_id = ?, destination_track_id = ?,
               error_message = ?, processed_at = CURRENT_TIMESTAMP
           WHERE id = ?"#,
    )
    .bind(status)
    .bind(match_confidence)
    .bind(match_method)
    .bind(dest_track_id)
    .bind(dest_track_id)
    .bind(error_message)
    .bind(item_id)
    .execute(db)
    .await
    .map(|_| ())
    .map_err(|e| format!("Failed to update migration item: {}", e))
}

/// Start a new migration
#[tauri::command]
pub async fn start_migration<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    source_service: String,
    destination_service: String,
    playlist_ids: Option<Vec<String>>,
    options: MigrationOptions,
) -> Result<String, String> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let options_json = serde_json::to_string(&options).map_err(|e| e.to_string())?;
    let playlist_ids_json = playlist_ids
        .as_ref()
        .and_then(|ids| serde_json::to_string(ids).ok());

    // Create migration job
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, source_playlist_ids, options, status)
           VALUES (?, ?, ?, ?, ?, 'pending')"#
    )
    .bind(&job_id)
    .bind(&source_service)
    .bind(&destination_service)
    .bind(&playlist_ids_json)
    .bind(&options_json)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to create migration job: {}", e))?;

    // Get tracks to migrate. BD-6: when the user selected playlists, filter by
    // them (external ids, same ids saved in source_playlist_ids) instead of
    // migrating every playlist of the source service.
    // BD-4: a failed read used to become an empty list, so the job stored
    // total_items = 0 and closed as `completed`, telling the user everything
    // migrated when not a single track had been read.
    let tracks = match fetch_migration_source_tracks(
        &state.db,
        &source_service,
        playlist_ids.as_deref(),
    )
    .await
    {
        Ok(tracks) => tracks,
        Err(e) => {
            tracing::error!("Migration {} failed reading source tracks: {}", job_id, e);
            let _ = sqlx::query(
                "UPDATE migration_jobs SET status = 'failed', error_message = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&e)
            .bind(&job_id)
            .execute(&state.db)
            .await;
            return Err(e);
        }
    };

    let total_items = tracks.len() as i64;

    // Update job with total items
    sqlx::query("UPDATE migration_jobs SET total_items = ?, status = 'running', started_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(total_items)
        .bind(&job_id)
        .execute(&state.db)
        .await
        .ok();

    // Emit initial progress before the per-item rows are written: the job id
    // reaches the UI (and becomes cancellable) as soon as the job is running,
    // not after the whole library has been inserted.
    let started_at = std::time::Instant::now();
    let _ = app.emit(
        "migration-progress",
        build_migration_progress(
            started_at.elapsed().as_secs_f64(),
            &job_id,
            0,
            total_items,
            "Starting migration...".to_string(),
            "Starting migration...".to_string(),
            "running",
            0,
            0,
            0,
        ),
    );

    // Insert migration items up front (status 'pending') and keep each row's id
    // so the per-item result update addresses exactly that row (BD-7).
    let mut item_ids: Vec<Option<i64>> = Vec::with_capacity(tracks.len());
    for track in &tracks {
        let item_id = match insert_migration_item(&state.db, &job_id, track).await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "Failed to insert migration item for {}: {}",
                    track.external_id,
                    e
                );
                None
            }
        };
        item_ids.push(item_id);
    }

    // Destination clients for matching/transferring, built from the stored
    // account credentials of the destination service.
    let clients = DestinationClients::for_destination(&state.db, &destination_service).await;

    // Process tracks with real matching
    let mut completed = 0i64;
    let mut failed = 0i64;
    let mut skipped = 0i64;
    // Set when cancel_migration flipped the job while the loop was running:
    // the loop stops early, so the job must not be reported as completed.
    let mut cancelled = false;

    for (i, (track, item_id)) in tracks.iter().zip(item_ids.iter()).enumerate() {
        let ext_id = track.external_id.as_str();
        let title = track.title.as_str();
        let artist = track.artist.as_str();

        // Check if cancelled
        let job: Option<(String,)> =
            sqlx::query_as("SELECT status FROM migration_jobs WHERE id = ?")
                .bind(&job_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

        if job.as_ref().map(|j| j.0.as_str()) == Some("cancelled") {
            cancelled = true;
            break;
        }

        // Try to get ISRC for this track from our database
        let isrc: Option<(String,)> = sqlx::query_as(
            "SELECT t.isrc FROM tracks t
             JOIN track_sources ts ON ts.track_id = t.id
             WHERE ts.service_track_id = ? AND t.isrc IS NOT NULL LIMIT 1",
        )
        .bind(ext_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        // A match the user attached by hand on a previous run of this route is
        // applied as-is (4.2: reviewed matches are effective); the transfer
        // still runs so the favorite really lands on the destination.
        let manual_dest_id = find_manual_match(&state.db, ext_id, &destination_service)
            .await
            .ok()
            .flatten();

        // The fourth tuple element carries the underlying API error when one
        // occurred, so failed items can record a real error_message (BD-7).
        let (match_confidence, match_method, dest_track_id, match_error): (
            f64,
            &str,
            Option<String>,
            Option<String>,
        ) = if let Some(dest_track_id) = &manual_dest_id {
            match clients.add_favorite(dest_track_id).await {
                Ok(_) => (1.0, "manual", Some(dest_track_id.clone()), None),
                Err(e) => (
                    1.0,
                    "manual",
                    None,
                    Some(format!("Failed to add to favorites: {}", e)),
                ),
            }
        } else {
            clients
                .match_and_transfer(isrc.as_ref().map(|(value,)| value.as_str()), title, artist)
                .await
        };

        // Determine status based on match
        let status = if match_confidence >= options.match_threshold && dest_track_id.is_some() {
            completed += 1;
            "transferred"
        } else if match_confidence >= options.match_threshold && dest_track_id.is_none() {
            // Match found but transfer failed
            failed += 1;
            "failed"
        } else if options.skip_unmatched {
            skipped += 1;
            "skipped"
        } else {
            failed += 1;
            "failed"
        };

        // BD-7: failed items carry a real error message — the API failure when
        // one occurred, a truthful note about the missing match otherwise.
        let error_message = if status == "failed" {
            Some(match match_error {
                Some(err) => err,
                None => format!("No match found on {}", destination_service),
            })
        } else {
            None
        };

        // Record the outcome on the item's own row: matching by the external id
        // would update every row of the job sharing it in one blow (BD-7).
        if let Some(item_id) = item_id {
            if let Err(e) = record_migration_item_result(
                &state.db,
                *item_id,
                status,
                match_confidence,
                match_method,
                dest_track_id.as_deref(),
                error_message.as_deref(),
            )
            .await
            {
                tracing::warn!("Failed to record migration item result: {}", e);
            }
        }

        // Emit progress every 5 items (more frequent for real API calls)
        if i % 5 == 0 {
            let current_action = if status == "transferred" {
                format!("Transferring {}", title)
            } else {
                "Searching for match".to_string()
            };
            let _ = app.emit(
                "migration-progress",
                build_migration_progress(
                    started_at.elapsed().as_secs_f64(),
                    &job_id,
                    i as i64 + 1,
                    total_items,
                    format!("{} - {}", artist, title),
                    current_action,
                    "running",
                    completed,
                    failed,
                    skipped,
                ),
            );
        }

        // Small delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Close the job with the status it really ended in: a job the user
    // cancelled stopped early, so writing 'completed' here would claim a
    // transfer that never finished (and the UI would show it as finished).
    let processed = completed + failed + skipped;
    let (final_status, final_message) = if cancelled {
        ("cancelled", "Migration cancelled")
    } else {
        ("completed", "Migration complete")
    };
    sqlx::query(
        "UPDATE migration_jobs SET status = ?, completed_items = ?, failed_items = ?, skipped_items = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(final_status)
    .bind(completed)
    .bind(failed)
    .bind(skipped)
    .bind(&job_id)
    .execute(&state.db)
    .await
    .ok();

    // Emit final progress
    let _ = app.emit(
        "migration-progress",
        build_migration_progress(
            started_at.elapsed().as_secs_f64(),
            &job_id,
            processed,
            total_items,
            final_message.to_string(),
            final_message.to_string(),
            final_status,
            completed,
            failed,
            skipped,
        ),
    );

    Ok(job_id)
}

/// Cancel a running migration
#[tauri::command]
pub async fn cancel_migration(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    sqlx::query("UPDATE migration_jobs SET status = 'cancelled', completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'running'")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to cancel migration: {}", e))?;
    Ok("Migration cancelled".to_string())
}

/// Retry failed items in a migration
#[tauri::command]
pub async fn retry_failed_items(state: State<'_, AppState>, job_id: String) -> Result<i64, String> {
    let result = sqlx::query("UPDATE migration_items SET status = 'pending', error_message = NULL WHERE job_id = ? AND status = 'failed'")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to retry items: {}", e))?;

    // Reset job status to running
    sqlx::query("UPDATE migration_jobs SET status = 'running' WHERE id = ?")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .ok();

    Ok(result.rows_affected() as i64)
}

/// Delete a migration job
#[tauri::command]
pub async fn delete_migration(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<String, String> {
    // Items are deleted via CASCADE
    sqlx::query("DELETE FROM migration_jobs WHERE id = ?")
        .bind(&job_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to delete migration: {}", e))?;
    Ok("Migration deleted".to_string())
}

/// Get all migration templates
#[tauri::command]
pub async fn get_migration_templates(
    state: State<'_, AppState>,
) -> Result<Vec<MigrationTemplate>, String> {
    sqlx::query_as::<_, MigrationTemplate>("SELECT * FROM migration_templates ORDER BY name")
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("Failed to get templates: {}", e))
}

/// Save a migration template
#[tauri::command]
pub async fn save_migration_template(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    source_service: String,
    destination_service: String,
    options: MigrationOptions,
) -> Result<i64, String> {
    let options_json = serde_json::to_string(&options).map_err(|e| e.to_string())?;

    let id: i64 = sqlx::query_scalar(
        r#"INSERT INTO migration_templates (name, description, source_service, destination_service, options)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT(name) DO UPDATE SET
           description = excluded.description,
           source_service = excluded.source_service,
           destination_service = excluded.destination_service,
           options = excluded.options,
           updated_at = CURRENT_TIMESTAMP
           RETURNING id"#
    )
    .bind(&name)
    .bind(&description)
    .bind(&source_service)
    .bind(&destination_service)
    .bind(&options_json)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Failed to save template: {}", e))?;

    Ok(id)
}

/// Delete a migration template
#[tauri::command]
pub async fn delete_migration_template(
    state: State<'_, AppState>,
    template_id: i64,
) -> Result<String, String> {
    sqlx::query("DELETE FROM migration_templates WHERE id = ?")
        .bind(template_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to delete template: {}", e))?;
    Ok("Template deleted".to_string())
}

/// Use a migration template (returns template details)
#[tauri::command]
pub async fn use_migration_template(
    state: State<'_, AppState>,
    template_id: i64,
) -> Result<MigrationTemplate, String> {
    sqlx::query_as::<_, MigrationTemplate>("SELECT * FROM migration_templates WHERE id = ?")
        .bind(template_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| format!("Template {} not found", template_id))
}

/// Search for tracks in destination service for manual matching
#[tauri::command]
pub async fn search_destination_track(
    state: State<'_, AppState>,
    service: String,
    query: String,
) -> Result<Vec<DestinationTrackMatch>, String> {
    // If destination is Qobuz, try real API search first
    if service.to_lowercase() == "qobuz" {
        // Get Qobuz credentials from database
        let creds: Option<(String,)> = sqlx::query_as(
            "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'qobuz' AND a.is_active = 1",
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((creds_json,)) = creds {
            let decrypted_json = crate::crypto::decrypt(&creds_json).unwrap_or(creds_json);
            if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&decrypted_json) {
                if let Some(token) = creds.get("user_auth_token").and_then(|v| v.as_str()) {
                    // Create authenticated Qobuz client
                    let app_id = std::env::var("QOBUZ_APP_ID")
                        .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
                    let app_secret = std::env::var("QOBUZ_APP_SECRET").unwrap_or_default();
                    let client = crate::services::QobuzClient::new_with_token(
                        app_id,
                        app_secret,
                        token.to_string(),
                    );

                    // Search real Qobuz API
                    match client.search_track(&query, 20).await {
                        Ok(results) => {
                            tracing::info!(
                                "Qobuz search for '{}' returned {} results",
                                query,
                                results.len()
                            );
                            return Ok(results
                                .into_iter()
                                .map(|r| {
                                    let quality = r
                                        .bit_depth
                                        .map(|d| format!("{}-bit", d))
                                        .or(r.sample_rate.map(|s| format!("{:.1}kHz", s / 1000.0)));
                                    DestinationTrackMatch {
                                        track_id: r.track_id,
                                        title: r.title,
                                        artist: r.artist,
                                        album: r.album,
                                        duration_ms: r.duration_ms,
                                        quality,
                                        confidence: if r.isrc.is_some() { 0.95 } else { 0.75 },
                                    }
                                })
                                .collect());
                        }
                        Err(e) => {
                            tracing::warn!("Qobuz search failed, falling back to local: {}", e);
                        }
                    }
                }
            }
        }
    }

    // Fallback: Search our local library for tracks from the destination service
    let results: Vec<(String, String, String, Option<String>, i64, Option<String>)> =
        sqlx::query_as(
            // BD-1: the mirror keeps the track's best source, so the id returned
            // here has to be the destination service's own one, resolved from
            // track_sources instead of read off the mirrored row.
            r#"SELECT ts.service_track_id, li.title, li.artist, li.album, li.duration_ms, li.quality
           FROM library_items li
           JOIN track_sources ts ON ts.track_id = li.id
           JOIN services s ON s.id = ts.service_id
           WHERE s.name = ? AND (li.title LIKE ? OR li.artist LIKE ?)
           ORDER BY li.title LIMIT 20"#,
        )
        .bind(&service)
        .bind(format!("%{}%", query))
        .bind(format!("%{}%", query))
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Ok(results
        .into_iter()
        .map(
            |(id, title, artist, album, duration, quality)| DestinationTrackMatch {
                track_id: id,
                title,
                artist,
                album,
                duration_ms: duration,
                quality,
                confidence: 0.80,
            },
        )
        .collect())
}

/// Manually match a migration item to a destination track
#[tauri::command]
pub async fn manual_match_item(
    state: State<'_, AppState>,
    item_id: i64,
    destination_track_id: String,
) -> Result<String, String> {
    sqlx::query(
        "UPDATE migration_items SET destination_track_id = ?, dest_track_id = ?, match_method = 'manual', match_confidence = 1.0, status = 'matched' WHERE id = ?"
    )
    .bind(&destination_track_id)
    .bind(&destination_track_id)
    .bind(item_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to match item: {}", e))?;
    Ok("Item matched".to_string())
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Minimal schema for `DestinationClients::for_destination`: only the two
    /// tables it reads (an active account of a service and its credentials).
    async fn setup_destination_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)")
            .execute(&pool)
            .await
            .expect("Failed to create services table");

        sqlx::query(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL,
                credentials_json TEXT,
                is_active INTEGER DEFAULT 1
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create accounts table");

        pool
    }

    #[tokio::test]
    async fn test_apple_music_is_a_migration_destination_and_keeps_its_storefront() {
        assert!(
            MIGRATION_DESTINATION_SERVICES.contains(&"apple_music"),
            "the wizard only offers what MIGRATION_DESTINATION_SERVICES declares, got {:?}",
            MIGRATION_DESTINATION_SERVICES
        );

        let pool = setup_destination_db().await;
        sqlx::query("INSERT INTO services (id, name) VALUES (7, 'apple_music')")
            .execute(&pool)
            .await
            .expect("insert service");
        sqlx::query(
            "INSERT INTO accounts (service_id, credentials_json, is_active)
             VALUES (7, ?, 1)",
        )
        .bind(
            serde_json::json!({
                "developer_token": "dev-token",
                "music_user_token": "user-token",
                "storefront": "mx"
            })
            .to_string(),
        )
        .execute(&pool)
        .await
        .expect("insert account");

        let clients = DestinationClients::for_destination(&pool, "apple_music").await;
        assert!(clients.available(), "the Apple Music client must be usable");
        let client = clients
            .apple_music
            .as_ref()
            .expect("apple_music destination client");
        assert_eq!(
            client.storefront(),
            "mx",
            "the storefront saved with the credentials must drive the catalog"
        );
    }

    #[tokio::test]
    async fn test_apple_music_destination_requires_both_tokens() {
        let pool = setup_destination_db().await;
        sqlx::query("INSERT INTO services (id, name) VALUES (7, 'apple_music')")
            .execute(&pool)
            .await
            .expect("insert service");
        // developer_token only: not enough to talk to the API.
        sqlx::query(
            "INSERT INTO accounts (service_id, credentials_json, is_active)
             VALUES (7, ?, 1)",
        )
        .bind(serde_json::json!({ "developer_token": "dev-token" }).to_string())
        .execute(&pool)
        .await
        .expect("insert account");

        let clients = DestinationClients::for_destination(&pool, "apple_music").await;
        assert!(
            clients.apple_music.is_none(),
            "an account without music_user_token must not produce a client"
        );
        assert!(!clients.available());
    }

    async fn setup_test_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)")
            .execute(&pool)
            .await
            .expect("Failed to create services table");

        sqlx::query(
            "CREATE TABLE accounts (id INTEGER PRIMARY KEY AUTOINCREMENT, service_id INTEGER NOT NULL, credentials_invalid INTEGER DEFAULT 0)",
        )
        .execute(&pool)
        .await
        .expect("Failed to create accounts table");

        sqlx::query("CREATE TABLE migration_jobs (id TEXT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("Failed to create migration_jobs table");

        sqlx::query(
            "CREATE TABLE migration_items (id INTEGER PRIMARY KEY AUTOINCREMENT, job_id TEXT)",
        )
        .execute(&pool)
        .await
        .expect("Failed to create migration_items table");

        sqlx::query("CREATE TABLE migration_templates (id INTEGER PRIMARY KEY AUTOINCREMENT)")
            .execute(&pool)
            .await
            .expect("Failed to create migration_templates table");

        sqlx::query("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("Failed to create _sqlx_migrations table");

        sqlx::query("INSERT INTO _sqlx_migrations (version) VALUES (30)")
            .execute(&pool)
            .await
            .expect("Failed to insert migration version");

        sqlx::query("INSERT INTO services (id, name) VALUES (1, 'spotify')")
            .execute(&pool)
            .await
            .expect("Failed to insert service");

        sqlx::query("INSERT INTO accounts (service_id, credentials_invalid) VALUES (1, 1)")
            .execute(&pool)
            .await
            .expect("Failed to insert account");

        pool
    }

    #[tokio::test]
    async fn test_collect_migration_audit_reports_schema_and_legacy_credentials() {
        let pool = setup_test_db().await;

        let report = collect_migration_audit(&pool)
            .await
            .expect("Expected migration audit to succeed");

        assert_eq!(report.schema_version, 30);
        assert!(report.schema_ok);
        assert!(report.missing_tables.is_empty());
        assert_eq!(report.legacy_services_detected, vec!["spotify".to_string()]);
        assert!(report.summary.contains("Schema v30 OK"));
        assert!(report.summary.contains("legacy creds detected"));
    }

    #[tokio::test]
    async fn test_collect_migration_audit_reports_missing_tables() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        sqlx::query("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("Failed to create _sqlx_migrations table");

        sqlx::query("INSERT INTO _sqlx_migrations (version) VALUES (30)")
            .execute(&pool)
            .await
            .expect("Failed to insert migration version");

        let report = collect_migration_audit(&pool)
            .await
            .expect("Expected migration audit to succeed with missing tables");

        assert_eq!(report.schema_version, 30);
        assert!(!report.schema_ok);
        assert!(report.missing_tables.contains(&"services".to_string()));
        assert!(report.missing_tables.contains(&"accounts".to_string()));
        assert!(report.summary.contains("incomplete"));
    }

    #[test]
    fn test_build_migration_progress_derives_percent_speed_and_eta() {
        // 25 of 100 items processed in 60 s -> 25 %, 25 items/min, 75 items left
        // -> 75 / 25 * 60 = 180 s remaining.
        let progress = build_migration_progress(
            60.0,
            "job-1",
            25,
            100,
            "Queen - Bohemian Rhapsody".to_string(),
            "Transferring Bohemian Rhapsody".to_string(),
            "running",
            20,
            3,
            2,
        );

        assert_eq!(progress.current_item, 25);
        assert_eq!(progress.total_items, 100);
        assert_eq!(progress.status, "running");
        assert_eq!(progress.percent, 25.0);
        assert_eq!(progress.speed, 25.0);
        assert_eq!(progress.eta, "3 min 0 s");
        assert_eq!(progress.current_action, "Transferring Bohemian Rhapsody");
    }

    #[test]
    fn test_build_migration_progress_search_action_and_eta_units() {
        // 10 items in 30 s -> 20 items/min; 90 remaining -> 270 s -> "4 min 30 s".
        let progress = build_migration_progress(
            30.0,
            "job-1",
            10,
            100,
            "Artist - Song".to_string(),
            "Searching for match".to_string(),
            "running",
            5,
            2,
            3,
        );

        assert_eq!(progress.percent, 10.0);
        assert_eq!(progress.speed, 20.0);
        assert_eq!(progress.eta, "4 min 30 s");
        assert_eq!(progress.current_action, "Searching for match");
    }

    #[test]
    fn test_build_migration_progress_initial_and_completed_states() {
        // Initial emit: nothing processed yet, no rate available.
        let initial = build_migration_progress(
            0.0,
            "job-1",
            0,
            100,
            "Starting migration...".to_string(),
            "Starting migration...".to_string(),
            "running",
            0,
            0,
            0,
        );
        assert_eq!(initial.percent, 0.0);
        assert_eq!(initial.speed, 0.0);
        assert_eq!(initial.eta, "calculating...");

        // Final emit: all items processed -> 100 %, no time remaining.
        let completed = build_migration_progress(
            120.0,
            "job-1",
            100,
            100,
            "Migration complete".to_string(),
            "Migration complete".to_string(),
            "completed",
            97,
            2,
            1,
        );
        assert_eq!(completed.percent, 100.0);
        assert_eq!(completed.speed, 50.0);
        assert_eq!(completed.eta, "0 s");
    }

    #[test]
    fn test_migration_progress_serializes_derived_payload_fields() {
        // Regression: the event payload must carry the derived fields the
        // migration view consumes (percent, speed, eta, current_action).
        let progress = build_migration_progress(
            60.0,
            "job-1",
            25,
            100,
            "Queen - Bohemian Rhapsody".to_string(),
            "Transferring Bohemian Rhapsody".to_string(),
            "running",
            20,
            3,
            2,
        );

        let json = serde_json::to_value(&progress).expect("payload must serialize");
        assert_eq!(json["percent"], serde_json::json!(25.0));
        assert_eq!(json["speed"], serde_json::json!(25.0));
        assert_eq!(json["eta"], serde_json::json!("3 min 0 s"));
        assert_eq!(
            json["current_action"],
            serde_json::json!("Transferring Bohemian Rhapsody")
        );
    }
}
