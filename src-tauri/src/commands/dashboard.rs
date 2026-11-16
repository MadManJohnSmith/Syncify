#[allow(unused_imports)]
use super::*;

// Dashboard Commands - submodule of crate::commands
//
// Dashboard views, library snapshots, album/artist details, diagnostics

// Handlers - remaining commands
//
// Dashboard, migration, enrichment workers, etc.

// ==============================================
// SPRINT 4: DASHBOARD + LIBRARY DETAIL VIEWS
// ==============================================

use crate::models::{LibrarySnapshot, ServiceHealthInfo};

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

/// FE-PERF-04 (#51): los seis recuentos del snapshot y las columnas de la
/// vista `library_stats` se resuelven en UNA lectura. `ls.*` trae la vista
/// verbatim (la misma semántica que devuelve `get_library_stats`) y los seis
/// subselects son los valores que el snapshot necesita y la vista no expone;
/// cada subselect es la consulta exacta que antes corría como sentencia
/// separada, ahora en una sola sentencia.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UnifiedLibraryCounts {
    // Columnas de la vista library_stats (0005_fix_album_stats.sql:13-27,
    // recreada verbatim en 0085_alignment_audit.sql:212-226).
    pub total_tracks: i64,
    pub total_artists: i64,
    pub total_albums: i64,
    pub total_downloads: i64,
    pub queued_downloads: i64,
    pub active_downloads: i64,
    pub library_entries: i64,
    pub playlists: i64,
    pub services_with_data: i64,
    // Valores propios del snapshot que la vista no cubre.
    pub snap_total_albums: i64,
    pub snap_total_artists: i64,
    pub snap_total_size_bytes: i64,
    pub snap_tracks_with_lyrics: i64,
    pub snap_tracks_lossless: i64,
    pub snap_tracks_hires: i64,
}

const UNIFIED_LIBRARY_COUNTS_SQL: &str = r#"
SELECT ls.*,
    (SELECT COUNT(*) FROM albums) as snap_total_albums,
    (SELECT COUNT(*) FROM artists art
     WHERE EXISTS (
         SELECT 1 FROM track_artists ta WHERE ta.artist_id = art.id
         UNION
         SELECT 1 FROM album_artists aa WHERE aa.artist_id = art.id
     ) OR art.is_favorite = 1 OR art.favorite_at IS NOT NULL) as snap_total_artists,
    (SELECT COALESCE(SUM(file_size_bytes), 0) FROM downloads) as snap_total_size_bytes,
    (SELECT COUNT(DISTINCT track_id) FROM lyrics WHERE content IS NOT NULL AND content != '') as snap_tracks_with_lyrics,
    (SELECT COUNT(*) FROM tracks WHERE audio_quality = 'lossless') as snap_tracks_lossless,
    (SELECT COUNT(*) FROM tracks WHERE audio_quality = 'hires') as snap_tracks_hires
FROM library_stats ls
"#;

/// pub para los tests de contrato: ejercitan la lectura unificada directa.
pub async fn read_unified_library_counts(
    db: &crate::db::DbPool,
) -> Result<UnifiedLibraryCounts, String> {
    sqlx::query_as::<_, UnifiedLibraryCounts>(UNIFIED_LIBRARY_COUNTS_SQL)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Unified dashboard counts error: {}", e))
}

impl UnifiedLibraryCounts {
    /// La vista `library_stats`, tal cual la devuelve `get_library_stats`.
    fn to_library_stats(&self) -> LibraryStats {
        LibraryStats {
            total_tracks: self.total_tracks,
            total_artists: self.total_artists,
            total_albums: self.total_albums,
            total_downloads: self.total_downloads,
            queued_downloads: self.queued_downloads,
            active_downloads: self.active_downloads,
            library_entries: self.library_entries,
            playlists: self.playlists,
            services_with_data: self.services_with_data,
        }
    }
}

/// Upsert del snapshot del día a partir de una lectura unificada ya hecha.
/// pub para los tests de schema (0085/BD-11), que ejercitan la escritura.
pub async fn upsert_library_snapshot(
    db: &crate::db::DbPool,
    counts: &UnifiedLibraryCounts,
) -> Result<LibrarySnapshot, String> {
    // Insert or update today's snapshot
    sqlx::query(
        "INSERT INTO library_snapshots (snapshot_date, total_tracks, total_albums, total_artists, downloaded_tracks, total_size_bytes, tracks_with_lyrics, tracks_lossless, tracks_hires)
         VALUES (date('now'), ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(snapshot_date) DO UPDATE SET
         total_tracks = excluded.total_tracks, total_albums = excluded.total_albums,
         total_artists = excluded.total_artists, downloaded_tracks = excluded.downloaded_tracks,
         total_size_bytes = excluded.total_size_bytes, tracks_with_lyrics = excluded.tracks_with_lyrics,
         tracks_lossless = excluded.tracks_lossless, tracks_hires = excluded.tracks_hires",
    )
    .bind(counts.total_tracks)
    .bind(counts.snap_total_albums)
    .bind(counts.snap_total_artists)
    .bind(counts.total_downloads) // == COUNT(*) FROM downloads, como antes
    .bind(counts.snap_total_size_bytes)
    .bind(counts.snap_tracks_with_lyrics)
    .bind(counts.snap_tracks_lossless)
    .bind(counts.snap_tracks_hires)
    .execute(db)
    .await
    .map_err(|e| format!("Insert error: {}", e))?;

    // Return the snapshot
    sqlx::query_as::<_, LibrarySnapshot>(
        "SELECT * FROM library_snapshots WHERE snapshot_date = date('now')",
    )
    .fetch_one(db)
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

/// Payload unificado del Dashboard (FE-PERF-04, #51): los recuentos
/// equivalentes llegan en una sola llamada en vez de por separado.
#[derive(Debug, Serialize)]
pub struct DashboardSnapshotRead {
    /// Vista `library_stats` — idéntica a `get_library_stats`.
    pub stats: LibraryStats,
    /// Snapshot del día, ya escrito — idéntico a `create_library_snapshot`.
    pub snapshot: LibrarySnapshot,
    /// Historial para la gráfica de crecimiento — idéntico a
    /// `get_library_snapshots(days)`.
    pub snapshots: Vec<LibrarySnapshot>,
    /// Cobertura de metadatos — misma consulta que `get_metadata_stats`.
    pub metadata: MetadataStats,
    /// Cobertura de letras — misma consulta que `get_lyrics_stats`.
    pub lyrics: LyricsStats,
}

/// Una llamada resuelve los agregados que el Dashboard pedía por separado:
/// los seis recuentos del snapshot y la vista `library_stats` en UNA lectura,
/// y en el mismo payload el historial de snapshots, la cobertura de
/// metadatos y la de letras (reutilizando las consultas exactas de sus
/// comandos, sin re-cálculos). `get_library_stats`, `get_metadata_stats` y
/// `get_lyrics_stats` siguen existiendo para sus otros consumidores
/// (AccountsView, MetadataView, LyricsView).
#[tauri::command]
pub async fn get_dashboard_snapshot(
    state: State<'_, AppState>,
    days: i32,
) -> Result<DashboardSnapshotRead, String> {
    tracing::info!("get_dashboard_snapshot: {} days", days);

    let counts = read_unified_library_counts(&state.db).await?;
    let stats = counts.to_library_stats();
    let snapshot = upsert_library_snapshot(&state.db, &counts).await?;
    let snapshots = get_library_snapshots(state.clone(), days).await?;
    let metadata = get_metadata_stats(state.clone()).await?;
    let lyrics = get_lyrics_stats(state.clone()).await?;

    Ok(DashboardSnapshotRead {
        stats,
        snapshot,
        snapshots,
        metadata,
        lyrics,
    })
}

// ==============================================
// SPRINT 5: ADVANCED SETTINGS & POLISH
// ==============================================

use crate::models::{AdvancedSettings, DiagnosticResult};

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
    let python_cmd = crate::commands::get_python_executable();
    let py_check = crate::cmd_utils::create_std_command(&python_cmd)
        .args(["--version"])
        .output();
    // A4: no unwrap — success is derived from the Result directly.
    let py_ok = matches!(&py_check, Ok(output) if output.status.success());
    results.push(DiagnosticResult {
        check_name: "Python Runtime".to_string(),
        status: if py_ok {
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
    let ff_check = crate::cmd_utils::create_std_command("ffmpeg")
        .args(["-version"])
        .output();
    // A4: no unwrap — single match drives both status and message.
    let ff_ok = matches!(&ff_check, Ok(output) if output.status.success());
    results.push(DiagnosticResult {
        check_name: "FFmpeg".to_string(),
        status: if ff_ok {
            "ok".to_string()
        } else {
            "warning".to_string()
        },
        message: if ff_ok {
            "Available".to_string()
        } else {
            "Not found".to_string()
        },
        duration_ms: ff_start.elapsed().as_millis() as i64,
    });

    Ok(results)
}

/// Reset settings to defaults. SB00 (#196): el reset completo vive en
/// `settings_defaults` (una sola fuente de defaults, una transacción, todos
/// los errores propagados). Las dos escrituras silenciosas que había aquí
/// (D-06) se retiraron con esta reescritura.
#[tauri::command]
pub async fn reset_to_defaults(
    state: State<'_, AppState>,
    settings_type: String,
) -> Result<String, String> {
    tracing::info!("reset_to_defaults: {}", settings_type);

    match settings_type.as_str() {
        "advanced" => {
            crate::settings_defaults::reset_advanced_settings(&state.db).await?;
            Ok("Advanced settings reset to defaults".to_string())
        }
        "all" => {
            crate::settings_defaults::reset_all_settings(&state.db).await?;
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
            SELECT t.title, ta.artist_id, COUNT(*) as cnt
            FROM tracks t
            JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'primary'
            GROUP BY t.title, ta.artist_id
            HAVING COUNT(*) > 1
        )
        "#,
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
pub async fn get_dashboard_stats(state: State<'_, AppState>) -> Result<DashboardStats, String> {
    // A counter swallowed into `Err` would read as 0, indistinguishable from an
    // empty library, so a broken query surfaces as an error and the view shows
    // its failure state instead of a plausible-looking empty panel.
    let (total_tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Dashboard stats error (tracks): {}", e))?;
    let (total_albums,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM albums")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Dashboard stats error (albums): {}", e))?;
    let (total_artists,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM artists art
        WHERE EXISTS (
            SELECT 1 FROM track_artists ta WHERE ta.artist_id = art.id
            UNION
            SELECT 1 FROM album_artists aa WHERE aa.artist_id = art.id
        ) OR art.is_favorite = 1 OR art.favorite_at IS NOT NULL
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Dashboard stats error (artists): {}", e))?;
    let (total_playlists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Dashboard stats error (playlists): {}", e))?;
    let (total_downloads,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads")
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Dashboard stats error (downloads): {}", e))?;
    let (total_favorites,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tracks WHERE is_favorite = 1 OR favorite_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Dashboard stats error (favorites): {}", e))?;

    let (lyrics_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT track_id) FROM lyrics WHERE content IS NOT NULL AND content != ''",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Dashboard stats error (lyrics): {}", e))?;

    let (enriched_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE musicbrainz_id IS NOT NULL")
            .fetch_one(&state.db)
            .await
            .map_err(|e| format!("Dashboard stats error (enriched metadata): {}", e))?;

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
            (SELECT COUNT(DISTINCT art.id) FROM artists art
             WHERE ((art.spotify_id IS NOT NULL AND s.name = 'spotify')
                 OR (art.tidal_id IS NOT NULL AND s.name = 'tidal')
                 OR (art.qobuz_id IS NOT NULL AND s.name = 'qobuz'))
               AND (EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = art.id UNION SELECT 1 FROM album_artists aa WHERE aa.artist_id = art.id) OR art.is_favorite = 1 OR art.favorite_at IS NOT NULL)) as artist_count,
            (SELECT COUNT(DISTINCT p.id) FROM playlists p JOIN accounts a ON a.id = p.account_id WHERE a.service_id = s.id) as playlist_count
        FROM services s
        "#
    ).fetch_all(&state.db).await.map_err(|e| format!("Dashboard stats error (services breakdown): {}", e))?;

    let services: Vec<ServiceStatItem> = services_rows
        .into_iter()
        .map(
            |(service_name, track_count, album_count, artist_count, playlist_count)| {
                ServiceStatItem {
                    service_name,
                    track_count,
                    album_count,
                    artist_count,
                    playlist_count,
                }
            },
        )
        .collect();

    // Quality distribution
    let quality_rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(file_format, 'UNKNOWN') as format,
            COUNT(*) as count
        FROM downloads
        GROUP BY file_format
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Dashboard stats error (quality distribution): {}", e))?;

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
pub async fn get_health_checks(state: State<'_, AppState>) -> Result<SystemHealthChecks, String> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    let ffmpeg_ok = crate::cmd_utils::create_std_command("ffmpeg")
        .args(["-version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let accounts: Vec<(String, Option<String>, i64, bool, Option<String>)> = sqlx::query_as(
        r#"
        SELECT s.name, COALESCE(a.display_name, a.email), a.is_active, COALESCE(a.credentials_invalid, 0), a.last_synced
        FROM services s
        LEFT JOIN accounts a ON a.service_id = s.id
        "#
    ).fetch_all(&state.db).await.map_err(|e| format!("Health checks error (accounts): {}", e))?;

    let mut services = Vec::new();
    for (s_name, acc_name, is_active, creds_invalid, last_synced) in accounts {
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
            // The account's stored timestamp; stamping the current time made the panel
            // announce "All synced" for a service that had never synced.
            last_synced,
            last_error: if creds_invalid {
                Some("Credentials expired or revoked".to_string())
            } else {
                None
            },
        });
    }

    Ok(SystemHealthChecks {
        database_ok: db_ok,
        ffmpeg_ok,
        services,
        background_worker_active: true,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchHealthReport {
    pub timestamp: String,
    pub database_healthy: bool,
    pub database_integrity: String,
    pub foreign_keys_valid: bool,
    pub queue_total: i64,
    pub queue_queued: i64,
    pub queue_downloading: i64,
    pub queue_completed: i64,
    pub queue_failed: i64,
    pub downloads_total: i64,
    pub downloads_verified_on_disk: i64,
    pub downloads_missing_on_disk: i64,
    pub staging_orphans_count: usize,
    pub staging_orphans_bytes: u64,
    pub worker_active_downloads: usize,
    pub worker_max_concurrent: usize,
    pub worker_paused: bool,
    pub healthy: bool,
    pub issues: Vec<String>,
    pub effective_download_path: String,
    pub effective_staging_path: String,
}

/// Configured download root, read for diagnostics only.
///
/// Canonical resolution belongs to `settings::resolve_effective_download_paths`;
/// the configured row is re-read here because that function's error does not
/// carry the path, and the report has to name the failing one instead of
/// falling back to the default.
async fn configured_download_root(db: &crate::db::DbPool) -> Option<String> {
    let from_folder_settings: Option<String> = sqlx::query_scalar(
        "SELECT base_folder FROM folder_settings WHERE id = 1 AND base_folder IS NOT NULL AND TRIM(base_folder) != ''",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    if let Some(path) = from_folder_settings {
        return Some(path);
    }

    sqlx::query_scalar(
        "SELECT value FROM settings WHERE key IN ('dl_download_path', 'download_dir', 'download_path') AND value IS NOT NULL AND TRIM(value) != '' ORDER BY CASE key WHEN 'dl_download_path' THEN 1 WHEN 'download_dir' THEN 2 ELSE 3 END LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Perform a comprehensive batch health check on SQLite, download queue, downloads table, and staging directory
pub async fn perform_batch_health_check(
    db: &crate::db::DbPool,
    staging_override: Option<&std::path::Path>,
    worker_state: Option<&crate::worker::DownloadWorkerState>,
) -> Result<BatchHealthReport, String> {
    let mut issues = Vec::new();

    // 1. Database connection & integrity check
    let (db_integrity,): (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(db)
        .await
        .unwrap_or_else(|e| (format!("error: {}", e),));

    let database_healthy = db_integrity.trim() == "ok";
    if !database_healthy {
        issues.push(format!("SQLite integrity check failed: {}", db_integrity));
    }

    // 2. Foreign key validity
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(db)
        .await
        .unwrap_or_default();

    let foreign_keys_valid = fk_violations.is_empty();
    if !foreign_keys_valid {
        issues.push(format!(
            "Found {} foreign key constraint violation(s)",
            fk_violations.len()
        ));
    }

    // 3. Queue state breakdown
    let queue_rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT status, COUNT(*) FROM download_queue GROUP BY status")
            .fetch_all(db)
            .await
            .unwrap_or_default();

    let mut queue_queued = 0i64;
    let mut queue_downloading = 0i64;
    let mut queue_completed = 0i64;
    let mut queue_failed = 0i64;

    for (status, count) in queue_rows {
        match status.as_str() {
            "queued" | "pending" => queue_queued += count,
            "downloading" | "in_progress" => queue_downloading += count,
            "complete" | "completed" => queue_completed += count,
            "error" | "failed" => queue_failed += count,
            _ => {}
        }
    }

    let queue_total = queue_queued + queue_downloading + queue_completed + queue_failed;
    if queue_failed > 0 {
        issues.push(format!("Queue contains {} failed item(s)", queue_failed));
    }

    // 4. Downloads audit & physical file verification
    let download_paths: Vec<(String,)> =
        sqlx::query_as("SELECT file_path FROM downloads WHERE file_path IS NOT NULL")
            .fetch_all(db)
            .await
            .unwrap_or_default();

    let downloads_total = download_paths.len() as i64;
    let mut downloads_verified_on_disk = 0i64;
    let mut downloads_missing_on_disk = 0i64;

    for (path_str,) in &download_paths {
        if std::path::Path::new(path_str).exists() {
            downloads_verified_on_disk += 1;
        } else {
            downloads_missing_on_disk += 1;
        }
    }

    if downloads_missing_on_disk > 0 {
        issues.push(format!(
            "{} downloaded track file(s) missing from filesystem",
            downloads_missing_on_disk
        ));
    }

    // 5. Staging directory audit & effective paths
    let effective = match resolve_effective_download_paths(db).await {
        Ok(paths) => paths,
        Err(resolve_error) => {
            // Rescuing with the default path and flagging it `valid` reported the panel as
            // healthy exactly when the configured path is the failing one: the
            // report names the configured path and its real state, and the
            // failure is listed as an issue.
            let root = configured_download_root(db)
                .await
                .unwrap_or_else(default_download_path);
            let staging_root = std::path::Path::new(&root)
                .join(".staging")
                .to_string_lossy()
                .into_owned();
            let validation = super::settings::validate_directory_path(root.clone())
                .await
                .ok();
            issues.push(format!(
                "Download path could not be resolved ({}): {}",
                root, resolve_error
            ));
            EffectiveDownloadPaths {
                library_root: root,
                staging_root,
                path_status: "unresolved".to_string(),
                // `free_space_bytes` cannot be null, so the configured path is
                // measured instead of hardcoding a 0 the view would read as a
                // full disk.
                free_space_bytes: validation.as_ref().map(|v| v.available_bytes).unwrap_or(0),
                is_writable: validation.as_ref().is_some_and(|v| v.is_writable),
                drive_mounted: validation.as_ref().is_some_and(|v| v.drive_mounted),
                exists: validation.as_ref().is_some_and(|v| v.exists),
                error_message: validation
                    .as_ref()
                    .and_then(|v| v.error_message.clone())
                    .or(Some(resolve_error)),
            }
        }
    };

    let effective_dl_path = effective.library_root.clone();
    let effective_path_status = effective.path_status.clone();
    if effective_path_status != "valid" {
        issues.push(format!(
            "Download path is not healthy (status: {}){}",
            effective_path_status,
            effective
                .error_message
                .as_ref()
                .map(|m| format!(": {}", m))
                .unwrap_or_default()
        ));
    }
    let staging_path = if let Some(p) = staging_override {
        Some(p.to_path_buf())
    } else {
        Some(std::path::PathBuf::from(&effective.staging_root))
    };
    let effective_staging_str = staging_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| effective.staging_root.clone());

    let mut staging_orphans_count = 0usize;
    let mut staging_orphans_bytes = 0u64;

    if let Some(ref staging_dir) = staging_path {
        if staging_dir.exists() && staging_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(staging_dir) {
                for entry in entries.flatten() {
                    // Los ficheros ocultos no son huérfanos: la propia app escribe
                    // `.nomedia` en staging (ver qobuz.rs y tidal_pipeline.rs), así
                    // que contarlos hacía que el panel diese "no sano" para siempre
                    // en cuanto el usuario descargaba su primera pista.
                    if entry
                        .file_name()
                        .to_str()
                        .map(|n| n.starts_with('.'))
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            staging_orphans_count += 1;
                            staging_orphans_bytes += meta.len();
                        }
                    }
                }
            }
        }
    }

    if staging_orphans_count > 0 {
        issues.push(format!(
            "Staging directory contains {} orphan file(s) ({:.2} KB)",
            staging_orphans_count,
            staging_orphans_bytes as f64 / 1024.0
        ));
    }

    // 6. Worker status
    let (worker_active_downloads, worker_max_concurrent, worker_paused) =
        if let Some(ws) = worker_state {
            (ws.active_downloads(), ws.max_concurrent(), ws.is_paused())
        } else {
            (0, 3, false)
        };

    let healthy = database_healthy
        && foreign_keys_valid
        && downloads_missing_on_disk == 0
        && staging_orphans_count == 0
        && queue_failed == 0
        && effective_path_status == "valid";

    Ok(BatchHealthReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        database_healthy,
        database_integrity: db_integrity,
        foreign_keys_valid,
        queue_total,
        queue_queued,
        queue_downloading,
        queue_completed,
        queue_failed,
        downloads_total,
        downloads_verified_on_disk,
        downloads_missing_on_disk,
        staging_orphans_count,
        staging_orphans_bytes,
        worker_active_downloads,
        worker_max_concurrent,
        worker_paused,
        healthy,
        issues,
        effective_download_path: effective_dl_path,
        effective_staging_path: effective_staging_str,
    })
}

/// Diagnostic command to audit batch processing health, database integrity, queue state, and staging directory
#[tauri::command]
pub async fn run_batch_health_check(
    state: State<'_, AppState>,
) -> Result<BatchHealthReport, String> {
    tracing::info!("run_batch_health_check");
    perform_batch_health_check(&state.db, None, Some(&state.worker_state)).await
}
