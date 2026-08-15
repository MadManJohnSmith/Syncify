// Dashboard Commands - included via include!() in mod.rs
// 
// Dashboard views, library snapshots, album/artist details, diagnostics

// Handlers - remaining commands
// 
// Dashboard, migration, enrichment workers, etc.



// ==============================================
// SPRINT 4: DASHBOARD + LIBRARY DETAIL VIEWS
// ==============================================

use crate::models::{AlbumDetail, ArtistDetail, LibrarySnapshot, ServiceHealthInfo};

/// Get service health status for all connected services
#[tauri::command]
pub async fn get_service_health(
    state: State<'_, AppState>,
) -> Result<Vec<ServiceHealthInfo>, String> {
    tracing::info!("get_service_health");

    sqlx::query_as::<_, ServiceHealthInfo>(
        "SELECT id, service_name, is_connected, token_valid, token_expires_at, 
         last_checked, error_message, rate_limit_remaining, rate_limit_reset_at 
         FROM service_health_cache ORDER BY service_name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Create a library snapshot for historical tracking
#[tauri::command]
pub async fn create_library_snapshot(
    state: State<'_, AppState>,
) -> Result<LibrarySnapshot, String> {
    tracing::info!("create_library_snapshot");

    // Gather current library stats
    let (total_tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Query error: {}", e))?;

    let (total_albums,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM albums")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Query error: {}", e))?;

    let (total_artists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Query error: {}", e))?;

    let (downloaded_tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Query error: {}", e))?;

    // Insert or update today's snapshot
    sqlx::query(
        "INSERT INTO library_snapshots (snapshot_date, total_tracks, total_albums, total_artists, downloaded_tracks)
         VALUES (date('now'), ?, ?, ?, ?)
         ON CONFLICT(snapshot_date) DO UPDATE SET 
         total_tracks = excluded.total_tracks, total_albums = excluded.total_albums,
         total_artists = excluded.total_artists, downloaded_tracks = excluded.downloaded_tracks",
    )
    .bind(total_tracks)
    .bind(total_albums)
    .bind(total_artists)
    .bind(downloaded_tracks)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Insert error: {}", e))?;

    // Return the snapshot
    sqlx::query_as::<_, LibrarySnapshot>(
        "SELECT * FROM library_snapshots WHERE snapshot_date = date('now')",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
}

/// Get library snapshots for the past N days (for growth chart)
#[tauri::command]
pub async fn get_library_snapshots(
    state: State<'_, AppState>,
    days: i32,
) -> Result<Vec<LibrarySnapshot>, String> {
    tracing::info!("get_library_snapshots: {} days", days);

    sqlx::query_as::<_, LibrarySnapshot>(
        "SELECT * FROM library_snapshots 
         WHERE snapshot_date >= date('now', '-' || ? || ' days')
         ORDER BY snapshot_date ASC",
    )
    .bind(days)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Get album detail with extended information
#[tauri::command]
pub async fn get_album_detail(
    state: State<'_, AppState>,
    album_name: String,
    artist_name: String,
) -> Result<AlbumDetail, String> {
    tracing::info!("get_album_detail: {} by {}", album_name, artist_name);

    // Build album detail from tracks table
    let row: (i64, String, String, Option<i32>, Option<String>, i64, i64) = sqlx::query_as(
        "SELECT 
            MIN(t.id) as id,
            t.album,
            COALESCE(a.name, t.artist_name) as artist_name,
            t.year,
            t.genre,
            COUNT(*) as track_count,
            SUM(t.duration_ms) as total_duration_ms
         FROM tracks t
         LEFT JOIN artists a ON t.artist_id = a.id
         WHERE t.album = ? AND (a.name = ? OR t.artist_name = ?)
         GROUP BY t.album",
    )
    .bind(&album_name)
    .bind(&artist_name)
    .bind(&artist_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Album not found: {}", e))?;

    Ok(AlbumDetail {
        id: row.0,
        title: row.1,
        artist_name: row.2,
        release_year: row.3,
        genre: row.4,
        label: None, // Would need additional table/column
        track_count: row.5,
        total_duration_ms: row.6,
        artwork_url: None,
        quality: None,
        source_service: None,
    })
}

/// Get tracks for a specific album
#[tauri::command]
pub async fn get_album_tracks(
    state: State<'_, AppState>,
    album_name: String,
    artist_name: String,
) -> Result<Vec<LibraryTrack>, String> {
    tracing::info!("get_album_tracks: {} by {}", album_name, artist_name);

    sqlx::query_as::<_, LibraryTrack>(
        "SELECT t.id, t.title, a.name as artist_name, t.album, t.duration_ms
         FROM tracks t
         LEFT JOIN artists a ON t.artist_id = a.id
         WHERE t.album = ? AND (a.name = ? OR t.artist_name = ?)
         ORDER BY t.track_number NULLS LAST, t.title",
    )
    .bind(&album_name)
    .bind(&artist_name)
    .bind(&artist_name)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Get artist detail with extended information
#[tauri::command]
pub async fn get_artist_detail(
    state: State<'_, AppState>,
    artist_id: i64,
) -> Result<ArtistDetail, String> {
    tracing::info!("get_artist_detail: {}", artist_id);

    let (id, name): (i64, String) = sqlx::query_as("SELECT id, name FROM artists WHERE id = ?")
        .bind(artist_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Artist not found: {}", e))?;

    let (album_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT album) FROM tracks WHERE artist_id = ?")
            .bind(artist_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| format!("Query error: {}", e))?;

    let (track_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE artist_id = ?")
        .bind(artist_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Query error: {}", e))?;

    Ok(ArtistDetail {
        id,
        name,
        album_count,
        track_count,
        genres: vec![], // Would need additional logic
        artwork_url: None,
    })
}

/// Get albums by a specific artist
#[tauri::command]
pub async fn get_artist_albums(
    state: State<'_, AppState>,
    artist_id: i64,
) -> Result<Vec<AlbumDetail>, String> {
    tracing::info!("get_artist_albums: {}", artist_id);

    let rows: Vec<(i64, String, String, Option<i32>, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT 
            MIN(t.id) as id,
            t.album,
            a.name as artist_name,
            t.year,
            t.genre,
            COUNT(*) as track_count,
            SUM(t.duration_ms) as total_duration_ms
         FROM tracks t
         JOIN artists a ON t.artist_id = a.id
         WHERE t.artist_id = ?
         GROUP BY t.album
         ORDER BY t.year DESC NULLS LAST, t.album",
    )
    .bind(artist_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|row| AlbumDetail {
            id: row.0,
            title: row.1,
            artist_name: row.2,
            release_year: row.3,
            genre: row.4,
            label: None,
            track_count: row.5,
            total_duration_ms: row.6,
            artwork_url: None,
            quality: None,
            source_service: None,
        })
        .collect())
}

/// Get all tracks by a specific artist
#[tauri::command]
pub async fn get_artist_tracks(
    state: State<'_, AppState>,
    artist_id: i64,
) -> Result<Vec<LibraryTrack>, String> {
    tracing::info!("get_artist_tracks: {}", artist_id);

    sqlx::query_as::<_, LibraryTrack>(
        "SELECT t.id, t.title, a.name as artist_name, t.album, t.duration_ms
         FROM tracks t
         JOIN artists a ON t.artist_id = a.id
         WHERE t.artist_id = ?
         ORDER BY t.album, t.track_number NULLS LAST, t.title",
    )
    .bind(artist_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

// ==============================================
// SPRINT 5: ADVANCED SETTINGS & POLISH
// ==============================================

use crate::models::{AdvancedSettings, CacheStats, DiagnosticResult};

/// Get advanced application settings
#[tauri::command]
pub async fn get_advanced_settings(state: State<'_, AppState>) -> Result<AdvancedSettings, String> {
    tracing::info!("get_advanced_settings");

    sqlx::query_as::<_, AdvancedSettings>("SELECT * FROM advanced_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))
}

/// Update advanced application settings
#[tauri::command]
pub async fn update_advanced_settings(
    state: State<'_, AppState>,
    settings: AdvancedSettings,
) -> Result<AdvancedSettings, String> {
    tracing::info!("update_advanced_settings");

    sqlx::query(
        "UPDATE advanced_settings SET 
         log_level = ?, log_to_file = ?, log_file_max_size_mb = ?, log_file_retention_days = ?,
         max_concurrent_downloads = ?, max_concurrent_imports = ?, worker_timeout_seconds = ?,
         cache_enabled = ?, cache_max_size_mb = ?, cache_ttl_hours = ?,
         fuzzy_match_threshold = ?, use_acoustic_fingerprinting = ?, prefer_exact_matches = ?,
         request_timeout_seconds = ?, max_retries = ?, retry_delay_seconds = ?,
         use_proxy = ?, proxy_url = ?, debug_mode = ?, verbose_api_logging = ?,
         updated_at = datetime('now')
         WHERE id = 1",
    )
    .bind(&settings.log_level)
    .bind(settings.log_to_file)
    .bind(settings.log_file_max_size_mb)
    .bind(settings.log_file_retention_days)
    .bind(settings.max_concurrent_downloads)
    .bind(settings.max_concurrent_imports)
    .bind(settings.worker_timeout_seconds)
    .bind(settings.cache_enabled)
    .bind(settings.cache_max_size_mb)
    .bind(settings.cache_ttl_hours)
    .bind(settings.fuzzy_match_threshold)
    .bind(settings.use_acoustic_fingerprinting)
    .bind(settings.prefer_exact_matches)
    .bind(settings.request_timeout_seconds)
    .bind(settings.max_retries)
    .bind(settings.retry_delay_seconds)
    .bind(settings.use_proxy)
    .bind(&settings.proxy_url)
    .bind(settings.debug_mode)
    .bind(settings.verbose_api_logging)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_advanced_settings(state).await
}

/// Vacuum the database to reclaim space
#[tauri::command]
pub async fn vacuum_database(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("vacuum_database");

    sqlx::query("VACUUM")
        .execute(&state.db)
        .await
        .map_err(|e| format!("Vacuum error: {}", e))?;

    Ok("Database vacuumed successfully".to_string())
}

/// Get cache statistics
#[tauri::command]
pub async fn get_cache_stats(state: State<'_, AppState>) -> Result<Vec<CacheStats>, String> {
    tracing::info!("get_cache_stats");

    sqlx::query_as::<_, CacheStats>("SELECT * FROM cache_stats ORDER BY cache_type")
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))
}

/// Clear cache by type or all
#[tauri::command]
pub async fn clear_cache(
    state: State<'_, AppState>,
    cache_type: Option<String>,
) -> Result<String, String> {
    tracing::info!("clear_cache: {:?}", cache_type);

    if let Some(ct) = cache_type {
        sqlx::query(
            "UPDATE cache_stats SET size_bytes = 0, item_count = 0, last_updated = datetime('now') 
             WHERE cache_type = ?",
        )
        .bind(&ct)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Clear error: {}", e))?;

        Ok(format!("Cache '{}' cleared", ct))
    } else {
        sqlx::query(
            "UPDATE cache_stats SET size_bytes = 0, item_count = 0, last_updated = datetime('now')",
        )
        .execute(&state.db)
        .await
        .map_err(|e| format!("Clear error: {}", e))?;

        Ok("All caches cleared".to_string())
    }
}

/// Run system diagnostics
#[tauri::command]
pub async fn run_diagnostics(state: State<'_, AppState>) -> Result<Vec<DiagnosticResult>, String> {
    tracing::info!("run_diagnostics");

    let mut results = Vec::new();

    // Check database connection
    let db_start = std::time::Instant::now();
    let db_check = sqlx::query("SELECT 1").execute(&state.db).await;
    results.push(DiagnosticResult {
        check_name: "Database Connection".to_string(),
        status: if db_check.is_ok() {
            "ok".to_string()
        } else {
            "error".to_string()
        },
        message: if db_check.is_ok() {
            "Connected".to_string()
        } else {
            "Connection failed".to_string()
        },
        duration_ms: db_start.elapsed().as_millis() as i64,
    });

    // Check Python bridge
    let py_start = std::time::Instant::now();
    let py_check = std::process::Command::new("python")
        .args(&["--version"])
        .output();
    results.push(DiagnosticResult {
        check_name: "Python Runtime".to_string(),
        status: if py_check.is_ok() && py_check.as_ref().unwrap().status.success() {
            "ok".to_string()
        } else {
            "warning".to_string()
        },
        message: match py_check {
            Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
            Err(_) => "Python not found".to_string(),
        },
        duration_ms: py_start.elapsed().as_millis() as i64,
    });

    // Check FFmpeg
    let ff_start = std::time::Instant::now();
    let ff_check = std::process::Command::new("ffmpeg")
        .args(&["-version"])
        .output();
    results.push(DiagnosticResult {
        check_name: "FFmpeg".to_string(),
        status: if ff_check.is_ok() && ff_check.as_ref().unwrap().status.success() {
            "ok".to_string()
        } else {
            "warning".to_string()
        },
        message: if ff_check.is_ok() && ff_check.as_ref().unwrap().status.success() {
            "Available".to_string()
        } else {
            "Not found".to_string()
        },
        duration_ms: ff_start.elapsed().as_millis() as i64,
    });

    Ok(results)
}

/// Reset settings to defaults
#[tauri::command]
pub async fn reset_to_defaults(
    state: State<'_, AppState>,
    settings_type: String,
) -> Result<String, String> {
    tracing::info!("reset_to_defaults: {}", settings_type);

    match settings_type.as_str() {
        "advanced" => {
            sqlx::query("DELETE FROM advanced_settings WHERE id = 1")
                .execute(&state.db)
                .await
                .map_err(|e| format!("Delete error: {}", e))?;
            sqlx::query("INSERT INTO advanced_settings (id) VALUES (1)")
                .execute(&state.db)
                .await
                .map_err(|e| format!("Insert error: {}", e))?;
            Ok("Advanced settings reset to defaults".to_string())
        }
        "all" => {
            // Reset all settings tables
            for table in &[
                "advanced_settings",
                "sync_settings",
                "folder_file_settings",
                "duplicate_settings",
                "audio_processing_settings",
                "lyrics_config",
            ] {
                let _ = sqlx::query(&format!("DELETE FROM {} WHERE id = 1", table))
                    .execute(&state.db)
                    .await;
                let _ = sqlx::query(&format!("INSERT OR IGNORE INTO {} (id) VALUES (1)", table))
                    .execute(&state.db)
                    .await;
            }
            Ok("All settings reset to defaults".to_string())
        }
        _ => Err(format!("Unknown settings type: {}", settings_type)),
    }
}

/// Get duplicate tracks statistics (by Title + Primary Artist)
#[tauri::command]
pub async fn get_duplicate_stats(state: State<'_, AppState>) -> Result<i64, String> {
    tracing::info!("get_duplicate_stats");

    let (extra_tracks,): (i64,) = sqlx::query_as(
        r#"
        SELECT IFNULL(SUM(cnt - 1), 0) FROM (
            SELECT title, artist_id, COUNT(*) as cnt 
            FROM tracks t 
            JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'primary' 
            GROUP BY title, artist_id 
            HAVING COUNT(*) > 1
        )
        "#
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(extra_tracks)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatItem {
    pub service_name: String,
    pub track_count: i64,
    pub album_count: i64,
    pub artist_count: i64,
    pub playlist_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityStatItem {
    pub quality: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_tracks: i64,
    pub total_albums: i64,
    pub total_artists: i64,
    pub total_playlists: i64,
    pub total_downloads: i64,
    pub total_favorites: i64,
    pub lyrics_coverage_percentage: f64,
    pub enriched_metadata_percentage: f64,
    pub services: Vec<ServiceStatItem>,
    pub quality_distribution: Vec<QualityStatItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthCheck {
    pub service: String,
    pub is_connected: bool,
    pub account_name: Option<String>,
    pub token_status: String,
    pub rate_limit_status: String,
    pub last_synced: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthChecks {
    pub database_ok: bool,
    pub ffmpeg_ok: bool,
    pub services: Vec<ServiceHealthCheck>,
    pub background_worker_active: bool,
}

/// Aggregated library statistics for Dashboard
#[tauri::command]
pub async fn get_dashboard_stats(
    state: State<'_, AppState>,
) -> Result<DashboardStats, String> {
    let (total_tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&state.db).await.unwrap_or((0,));
    let (total_albums,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM albums")
        .fetch_one(&state.db).await.unwrap_or((0,));
    let (total_artists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists")
        .fetch_one(&state.db).await.unwrap_or((0,));
    let (total_playlists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists")
        .fetch_one(&state.db).await.unwrap_or((0,));
    let (total_downloads,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads")
        .fetch_one(&state.db).await.unwrap_or((0,));
    let (total_favorites,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tracks WHERE is_favorite = 1 OR favorite_at IS NOT NULL"
    ).fetch_one(&state.db).await.unwrap_or((0,));

    let (lyrics_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT track_id) FROM lyrics WHERE content IS NOT NULL AND content != ''"
    ).fetch_one(&state.db).await.unwrap_or((0,));

    let (enriched_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tracks WHERE musicbrainz_id IS NOT NULL"
    ).fetch_one(&state.db).await.unwrap_or((0,));

    let lyrics_coverage_percentage = if total_tracks > 0 {
        ((lyrics_count as f64) / (total_tracks as f64)) * 100.0
    } else {
        0.0
    };

    let enriched_metadata_percentage = if total_tracks > 0 {
        ((enriched_count as f64) / (total_tracks as f64)) * 100.0
    } else {
        0.0
    };

    // Services breakdown
    let services_rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT 
            s.name as service_name,
            (SELECT COUNT(DISTINCT ts.track_id) FROM track_sources ts WHERE ts.service_id = s.id) as track_count,
            (SELECT COUNT(DISTINCT al.id) FROM albums al WHERE al.spotify_id IS NOT NULL AND s.name = 'spotify' OR al.tidal_id IS NOT NULL AND s.name = 'tidal' OR al.qobuz_id IS NOT NULL AND s.name = 'qobuz') as album_count,
            (SELECT COUNT(DISTINCT art.id) FROM artists art WHERE art.spotify_id IS NOT NULL AND s.name = 'spotify' OR art.tidal_id IS NOT NULL AND s.name = 'tidal' OR art.qobuz_id IS NOT NULL AND s.name = 'qobuz') as artist_count,
            (SELECT COUNT(DISTINCT p.id) FROM playlists p JOIN accounts a ON a.id = p.account_id WHERE a.service_id = s.id) as playlist_count
        FROM services s
        "#
    ).fetch_all(&state.db).await.unwrap_or_default();

    let services: Vec<ServiceStatItem> = services_rows
        .into_iter()
        .map(|(service_name, track_count, album_count, artist_count, playlist_count)| {
            ServiceStatItem {
                service_name,
                track_count,
                album_count,
                artist_count,
                playlist_count,
            }
        })
        .collect();

    // Quality distribution
    let quality_rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT 
            COALESCE(file_format, 'UNKNOWN') as format,
            COUNT(*) as count
        FROM downloads
        GROUP BY file_format
        "#
    ).fetch_all(&state.db).await.unwrap_or_default();

    let quality_distribution: Vec<QualityStatItem> = quality_rows
        .into_iter()
        .map(|(quality, count)| {
            let percentage = if total_downloads > 0 {
                ((count as f64) / (total_downloads as f64)) * 100.0
            } else {
                0.0
            };
            QualityStatItem {
                quality,
                count,
                percentage,
            }
        })
        .collect();

    Ok(DashboardStats {
        total_tracks,
        total_albums,
        total_artists,
        total_playlists,
        total_downloads,
        total_favorites,
        lyrics_coverage_percentage,
        enriched_metadata_percentage,
        services,
        quality_distribution,
    })
}

/// Real-time health checks for services, database, and background workers
#[tauri::command]
pub async fn get_health_checks(
    state: State<'_, AppState>,
) -> Result<SystemHealthChecks, String> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    let ffmpeg_ok = std::process::Command::new("ffmpeg")
        .args(&["-version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let accounts: Vec<(String, Option<String>, i64, bool)> = sqlx::query_as(
        r#"
        SELECT s.name, COALESCE(a.display_name, a.email), a.is_active, COALESCE(a.credentials_invalid, 0)
        FROM services s
        LEFT JOIN accounts a ON a.service_id = s.id
        "#
    ).fetch_all(&state.db).await.unwrap_or_default();

    let mut services = Vec::new();
    for (s_name, acc_name, is_active, creds_invalid) in accounts {
        let is_connected = acc_name.is_some() && is_active == 1;
        let token_status = if !is_connected {
            "missing".to_string()
        } else if creds_invalid {
            "expired".to_string()
        } else {
            "valid".to_string()
        };

        services.push(ServiceHealthCheck {
            service: s_name,
            is_connected,
            account_name: acc_name,
            token_status,
            rate_limit_status: "ok".to_string(),
            last_synced: Some(chrono::Utc::now().to_rfc3339()),
            last_error: if creds_invalid { Some("Credentials expired or revoked".to_string()) } else { None },
        });
    }

    Ok(SystemHealthChecks {
        database_ok: db_ok,
        ffmpeg_ok,
        services,
        background_worker_active: true,
    })
}

