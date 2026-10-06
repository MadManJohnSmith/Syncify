//! E2E Test Suite for Sprint S104: Dashboard de Estadísticas y Health Checks en Tiempo Real
//!
//! Validates real-time dashboard statistics aggregation, service breakdown,
//! audio quality distribution, lyrics & metadata enrichment coverage, and system health checks
//! using production commands and services.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_tauri_lib::{
    commands::dashboard::{get_dashboard_stats, get_health_checks, perform_batch_health_check},
    worker::DownloadWorkerState,
    AppState, EnrichmentWorkerState,
};
use tauri::Manager;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through current must apply cleanly");

    // Seed services & default accounts
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active, credentials_invalid) VALUES (1, 1, 'Spotify User', 1, 0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active, credentials_invalid) VALUES (2, 3, 'Tidal User', 1, 0)")
        .execute(&pool).await.unwrap();

    pool
}

fn create_test_app(pool: SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);
    app
}

#[tokio::test]
async fn test_get_dashboard_stats_empty_and_populated() {
    let db = create_test_db().await;
    let app = create_test_app(db.clone());

    // 1. Empty state verification through production command
    let empty_stats = get_dashboard_stats(app.state::<AppState>())
        .await
        .expect("get_dashboard_stats should succeed on empty DB");
    assert_eq!(empty_stats.total_tracks, 0);
    assert_eq!(empty_stats.total_downloads, 0);
    assert_eq!(empty_stats.lyrics_coverage_percentage, 0.0);

    // 2. Populate 4 tracks
    // Track 1: With lyrics + enriched + downloaded + favorite
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, musicbrainz_id, is_favorite, favorite_at) VALUES ('Track 1', 'mb-1', 1, '2026-08-15T12:00:00Z') RETURNING id"
    ).fetch_one(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO lyrics (track_id, format, content) VALUES (?, 'plain', 'Some lyrics')",
    )
    .bind(t1)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, '/path/1.flac', 'FLAC')")
        .bind(t1).execute(&db).await.unwrap();

    // Track 2: With lyrics only
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Track 2') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO lyrics (track_id, format, content) VALUES (?, 'plain', 'More lyrics')",
    )
    .bind(t2)
    .execute(&db)
    .await
    .unwrap();

    // Track 3: Enriched only
    let _t3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, musicbrainz_id) VALUES ('Track 3', 'mb-3') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();

    // Track 4: Plain
    let _t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Track 4') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();

    // 3. Re-query production stats command and assert calculated percentages
    let populated_stats = get_dashboard_stats(app.state::<AppState>())
        .await
        .expect("get_dashboard_stats should succeed on populated DB");

    assert_eq!(populated_stats.total_tracks, 4);
    assert_eq!(populated_stats.total_downloads, 1);
    assert_eq!(populated_stats.total_favorites, 1);
    assert!((populated_stats.lyrics_coverage_percentage - 50.0).abs() < f64::EPSILON);
    assert!((populated_stats.enriched_metadata_percentage - 50.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_dashboard_services_breakdown() {
    let db = create_test_db().await;

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Song 1') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Song 2') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp_1')",
    )
    .bind(t1)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, 'td_2')",
    )
    .bind(t2)
    .execute(&db)
    .await
    .unwrap();

    let app = create_test_app(db);
    let stats = get_dashboard_stats(app.state::<AppState>())
        .await
        .expect("get_dashboard_stats should succeed");

    let sp_item = stats.services.iter().find(|s| s.service_name == "spotify");
    assert!(
        sp_item.is_some(),
        "Spotify must be present in services breakdown"
    );
    assert_eq!(sp_item.unwrap().track_count, 1);

    let td_item = stats.services.iter().find(|s| s.service_name == "tidal");
    assert!(
        td_item.is_some(),
        "Tidal must be present in services breakdown"
    );
    assert_eq!(td_item.unwrap().track_count, 1);
}

#[tokio::test]
async fn test_dashboard_quality_distribution() {
    let db = create_test_db().await;

    let t1: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('FLAC Track') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();
    let t2: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('MP3 Track') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, '/path/1.flac', 'FLAC')")
        .bind(t1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 1, '/path/2.mp3', 'MP3')")
        .bind(t2).execute(&db).await.unwrap();

    let app = create_test_app(db);
    let stats = get_dashboard_stats(app.state::<AppState>())
        .await
        .expect("get_dashboard_stats should succeed");

    assert_eq!(stats.total_downloads, 2);
    let flac_entry = stats
        .quality_distribution
        .iter()
        .find(|q| q.quality.to_uppercase() == "FLAC");
    assert!(
        flac_entry.is_some(),
        "FLAC quality must be present in distribution"
    );
    assert_eq!(flac_entry.unwrap().count, 1);

    let mp3_entry = stats
        .quality_distribution
        .iter()
        .find(|q| q.quality.to_uppercase() == "MP3");
    assert!(
        mp3_entry.is_some(),
        "MP3 quality must be present in distribution"
    );
    assert_eq!(mp3_entry.unwrap().count, 1);
}

#[tokio::test]
async fn test_dashboard_system_health_checks() {
    let db = create_test_db().await;
    let app = create_test_app(db);

    let health = get_health_checks(app.state::<AppState>())
        .await
        .expect("get_health_checks should succeed");

    assert!(health.database_ok, "Database health must be ok");
    assert!(
        health.background_worker_active,
        "Background worker must be active"
    );
    assert!(
        !health.services.is_empty(),
        "Services health check list must not be empty"
    );

    let spotify_check = health.services.iter().find(|s| s.service == "spotify");
    assert!(spotify_check.is_some());
    assert_eq!(spotify_check.unwrap().token_status, "valid");
}

#[tokio::test]
async fn test_dashboard_batch_health_report() {
    let db = create_test_db().await;
    let worker_state = DownloadWorkerState::new(2);
    // A clean in-memory DB must not inherit orphan files from the user's real
    // default download directory (other tests can also write there in parallel).
    let download_root = tempfile::tempdir().expect("isolated download root must be creatable");
    sqlx::query("UPDATE folder_settings SET base_folder = ? WHERE id = 1")
        .bind(download_root.path().to_string_lossy().as_ref())
        .execute(&db)
        .await
        .expect("test download root must be configured");

    let batch_report = perform_batch_health_check(&db, None, Some(&worker_state))
        .await
        .expect("perform_batch_health_check should succeed");

    assert!(
        batch_report.database_healthy,
        "Batch health must report healthy DB"
    );
    assert_eq!(batch_report.database_integrity, "ok");
    assert!(
        batch_report.foreign_keys_valid,
        "Foreign keys must be valid"
    );
    assert!(
        batch_report.healthy,
        "Overall batch health must be true on clean DB"
    );
}

#[tokio::test]
async fn test_dashboard_stats_propagates_sql_errors_instead_of_zeroes() {
    let db = create_test_db().await;
    let app = create_test_app(db.clone());

    sqlx::query("DROP TABLE tracks")
        .execute(&db)
        .await
        .expect("dropping tracks must succeed");

    let error = get_dashboard_stats(app.state::<AppState>())
        .await
        .expect_err("a broken counter query must surface as an error, not as zero tracks");

    assert!(
        error.contains("tracks"),
        "error must name the failing counter, got: {}",
        error
    );
}

#[tokio::test]
async fn test_dashboard_stats_propagates_missing_services_breakdown() {
    let db = create_test_db().await;
    let app = create_test_app(db.clone());

    sqlx::query("DROP TABLE services")
        .execute(&db)
        .await
        .expect("dropping services must succeed");

    let error = get_dashboard_stats(app.state::<AppState>())
        .await
        .expect_err("a failing services breakdown must not be reported as an empty list");

    assert!(
        error.contains("services"),
        "error must name the failing breakdown, got: {}",
        error
    );
}

#[tokio::test]
async fn test_health_checks_report_real_last_sync_instead_of_now() {
    let db = create_test_db().await;
    let app = create_test_app(db.clone());

    let before = get_health_checks(app.state::<AppState>())
        .await
        .expect("get_health_checks should succeed");
    let spotify_before = before
        .services
        .iter()
        .find(|s| s.service == "spotify")
        .expect("spotify health entry must exist");
    assert!(
        spotify_before.last_synced.is_none(),
        "an account that never synced must not report a last-synced timestamp"
    );

    sqlx::query("UPDATE accounts SET last_synced = '2026-01-02T03:04:05Z' WHERE id = 1")
        .execute(&db)
        .await
        .expect("updating last_synced must succeed");

    let after = get_health_checks(app.state::<AppState>())
        .await
        .expect("get_health_checks should succeed");
    let spotify_after = after
        .services
        .iter()
        .find(|s| s.service == "spotify")
        .expect("spotify health entry must exist");
    assert_eq!(
        spotify_after.last_synced.as_deref(),
        Some("2026-01-02T03:04:05Z"),
        "health checks must expose the stored last-synced value"
    );
}

#[tokio::test]
async fn test_batch_health_flags_invalid_download_path() {
    let db = create_test_db().await;
    let worker_state = DownloadWorkerState::new(2);

    // The configured path points at a file: it exists but is not a writable
    // directory, so the report cannot call it healthy.
    let file_path = std::env::temp_dir().join("syncify_not_a_folder_test");
    std::fs::write(&file_path, b"not a folder").expect("temp file must be writable");

    sqlx::query("INSERT OR REPLACE INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(file_path.to_string_lossy().to_string())
        .execute(&db)
        .await
        .expect("seeding folder_settings must succeed");

    let report = perform_batch_health_check(&db, None, Some(&worker_state))
        .await
        .expect("perform_batch_health_check should succeed");

    assert!(
        !report.healthy,
        "an unusable download path must not be reported as healthy"
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("Download path is not healthy")),
        "issues must flag the unusable path, got: {:?}",
        report.issues
    );

    let _ = std::fs::remove_file(&file_path);
}

#[tokio::test]
async fn test_batch_health_names_configured_path_when_resolution_fails() {
    let db = create_test_db().await;
    let worker_state = DownloadWorkerState::new(2);

    let configured = tempfile::tempdir().expect("temp dir must be creatable");
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('dl_download_path', ?)")
        .bind(configured.path().to_string_lossy().to_string())
        .execute(&db)
        .await
        .expect("seeding settings must succeed");

    // Without `folder_settings` the canonical resolution fails, and the report must
    // name the configured path instead of vouching for the default one.
    sqlx::query("DROP TABLE folder_settings")
        .execute(&db)
        .await
        .expect("dropping folder_settings must succeed");

    let report = perform_batch_health_check(&db, None, Some(&worker_state))
        .await
        .expect("perform_batch_health_check should succeed");

    assert_eq!(
        report.effective_download_path,
        configured.path().to_string_lossy().to_string(),
        "the report must name the configured path that failed to resolve"
    );
    assert!(
        !report.healthy,
        "an unresolved download path must not be reported as healthy"
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("could not be resolved")),
        "issues must record the resolution failure, got: {:?}",
        report.issues
    );
}

#[tokio::test]
async fn test_batch_health_ignores_hidden_files_in_staging() {
    let db = create_test_db().await;
    let worker_state = DownloadWorkerState::new(2);

    let staging = tempfile::tempdir().expect("temp dir must be creatable");
    std::fs::write(staging.path().join(".nomedia"), b"").expect(".nomedia must be writable");
    std::fs::write(staging.path().join("partial.flac"), &[0u8; 2048])
        .expect("file must be writable");

    let report = perform_batch_health_check(&db, Some(staging.path()), Some(&worker_state))
        .await
        .expect("perform_batch_health_check should succeed");

    assert_eq!(
        report.staging_orphans_count, 1,
        "only the real leftover file counts as an orphan"
    );
    assert_eq!(report.staging_orphans_bytes, 2048);
}
