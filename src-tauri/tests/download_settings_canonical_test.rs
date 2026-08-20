//! Integration test suite for Sprint S148: Canonical Quality Settings, Sync & Scheduling, and Effective Preferences
//!
//! Validates:
//! 1. Canonical `get_effective_download_preferences` DTO resolution from authoritative SQLite tables.
//! 2. Atomic persistence and multi-table synchronization via `save_effective_download_preferences`.
//! 3. Worker live concurrency synchronization between SQLite (`sync_settings`, `advanced_settings`, `settings`) and `DownloadWorkerState`.
//! 4. Resilient UPSERT on `quality_preferences` table for existing and newly introduced streaming services.
//! 5. Strict rejection and rollback when attempting to save invalid / non-writable download directories.
//! 6. Dynamic priority resolution in `evaluate_track_preflight` fallback matching based on `service_preferences`.
//! 7. Strict quality enforcement (`fallback_action = "skip"`) vs fallback downgrade (`fallback_action = "try_next"`).

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_tauri_lib::commands::{
    evaluate_track_preflight, perform_get_effective_download_preferences,
    perform_reorder_service_priorities, perform_save_effective_download_preferences,
    perform_update_quality_preference, DownloadPreflightStatus,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::models::QualityPreference;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tempfile::TempDir;

/// Helper to create an in-memory test database with full migrations and baseline data
async fn setup_test_app_state(initial_concurrency: usize) -> (AppState, SqlitePool, TempDir) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (4, 'deezer', 1, 'high')")
        .execute(&pool).await.unwrap();

    // Baseline service preferences
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority, auto_import_enabled) VALUES ('qobuz', 1, 1), ('tidal', 2, 1), ('spotify', 3, 1), ('deezer', 4, 1)")
        .execute(&pool).await.unwrap();

    // Baseline accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz Test', 'qobuz@test.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal Test', 'tidal@test.com', 1)")
        .execute(&pool).await.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create tempdir");
    let base_path = temp_dir.path().to_string_lossy().to_string();

    // Seed folder_settings with temp path
    sqlx::query("UPDATE folder_settings SET base_folder = ? WHERE id = 1")
        .bind(&base_path)
        .execute(&pool)
        .await
        .unwrap();

    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(initial_concurrency),
        album_lock: Arc::new(tokio::sync::Mutex::new(())),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };

    (state, pool, temp_dir)
}

#[tokio::test]
async fn test_canonical_effective_preferences_get_and_atomic_save() {
    let (state, pool, temp_dir) = setup_test_app_state(2).await;
    let base_path = temp_dir.path().to_string_lossy().to_string();

    // 1. Initial canonical DTO read
    let initial = perform_get_effective_download_preferences(&state)
        .await
        .expect("get_effective_download_preferences should succeed");

    assert_eq!(initial.download_path, base_path);
    assert_eq!(initial.max_concurrent_downloads, 2);
    assert_eq!(initial.fallback_action, "try_next");
    assert_eq!(initial.allow_downgrade, true);
    assert_eq!(initial.strict_quality, false);
    assert_eq!(initial.preferred_download_service, Some("qobuz".to_string()));

    // 2. Mutate settings and save atomically
    let mut updated_prefs = initial.clone();
    updated_prefs.max_concurrent_downloads = 4;
    updated_prefs.fallback_action = "skip".to_string();
    updated_prefs.allow_downgrade = false;
    updated_prefs.strict_quality = true;
    updated_prefs.rate_limit_delay_ms = 750;
    updated_prefs.max_retries = 5;
    updated_prefs.retry_delay_seconds = 8;
    updated_prefs.folder_template = "{AlbumArtist}/{Year} - {Album}".to_string();
    updated_prefs.file_template = "{DiscNumber}-{TrackNumber:pad2} {Title}".to_string();
    updated_prefs.service_priority_order = vec![
        "tidal".to_string(),
        "qobuz".to_string(),
        "spotify".to_string(),
        "deezer".to_string(),
    ];
    updated_prefs.service_qualities = vec![
        QualityPreference {
            id: 1,
            service_name: "qobuz".to_string(),
            max_quality: "hires".to_string(),
            preferred_format: "flac".to_string(),
            fallback_quality: "lossless".to_string(),
            fallback_format: "flac".to_string(),
        },
        QualityPreference {
            id: 2,
            service_name: "tidal".to_string(),
            max_quality: "hires".to_string(),
            preferred_format: "flac".to_string(),
            fallback_quality: "high".to_string(),
            fallback_format: "mp3".to_string(),
        },
    ];

    let saved = perform_save_effective_download_preferences(&state, updated_prefs)
        .await
        .expect("save_effective_download_preferences should succeed");

    // 3. Verify returned DTO
    assert_eq!(saved.max_concurrent_downloads, 4);
    assert_eq!(saved.fallback_action, "skip");
    assert_eq!(saved.allow_downgrade, false);
    assert_eq!(saved.strict_quality, true);
    assert_eq!(saved.preferred_download_service, Some("tidal".to_string()));
    assert_eq!(saved.service_priority_order[0], "tidal");
    assert_eq!(saved.service_priority_order[1], "qobuz");

    // 4. Verify runtime DownloadWorkerState was updated in memory
    assert_eq!(state.worker_state.max_concurrent(), 4);

    // 5. Verify SQLite persistence in underlying tables
    let sync_concurrency: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sync_concurrency, 4);

    let adv_retries: i32 = sqlx::query_scalar("SELECT max_retries FROM advanced_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(adv_retries, 5);

    let folder_fallback: String = sqlx::query_scalar("SELECT fallback_action FROM folder_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(folder_fallback, "skip");

    let tidal_priority: i32 = sqlx::query_scalar("SELECT priority FROM service_preferences WHERE service_name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tidal_priority, 1);
}

#[tokio::test]
async fn test_quality_preferences_resilient_upsert() {
    let (_state, pool, _temp_dir) = setup_test_app_state(2).await;

    // 1. Insert a preference for a brand new service that was not pre-seeded
    let res = perform_update_quality_preference(
        &pool,
        "soundcloud".to_string(),
        "high".to_string(),
        "mp3".to_string(),
        "normal".to_string(),
        "mp3".to_string(),
    )
    .await
    .expect("UPSERT should succeed for new service");

    assert_eq!(res.service_name, "soundcloud");
    assert_eq!(res.max_quality, "high");
    assert_eq!(res.preferred_format, "mp3");

    // 2. Update existing preference for soundcloud
    let updated = perform_update_quality_preference(
        &pool,
        "soundcloud".to_string(),
        "lossless".to_string(),
        "flac".to_string(),
        "high".to_string(),
        "mp3".to_string(),
    )
    .await
    .expect("UPSERT should update existing row without error");

    assert_eq!(updated.service_name, "soundcloud");
    assert_eq!(updated.max_quality, "lossless");
    assert_eq!(updated.preferred_format, "flac");
}

#[tokio::test]
async fn test_invalid_download_path_rejection() {
    let (state, _pool, temp_dir) = setup_test_app_state(2).await;
    let base_path = temp_dir.path().to_string_lossy().to_string();

    let initial = perform_get_effective_download_preferences(&state)
        .await
        .unwrap();

    // Attempt to save empty path
    let mut invalid_prefs = initial.clone();
    invalid_prefs.download_path = "".to_string();

    let err = perform_save_effective_download_preferences(&state, invalid_prefs)
        .await
        .unwrap_err();

    assert!(err.contains("empty") || err.contains("required") || err.contains("invalid"));

    // Verify initial valid path was preserved
    let current = perform_get_effective_download_preferences(&state)
        .await
        .unwrap();
    assert_eq!(current.download_path, base_path);
}

#[tokio::test]
async fn test_dynamic_service_priority_fallback_matching_in_preflight() {
    let (_state, pool, _temp_dir) = setup_test_app_state(2).await;

    // Track 100: Imported from Spotify (non-downloadable) with matching Qobuz and Tidal sources
    sqlx::query(
        r#"INSERT INTO tracks (id, title, isrc, duration_ms) 
           VALUES (100, 'Bohemian Rhapsody', 'GBUM71029604', 354000)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) 
           VALUES (100, 1, 'spotify-100', 'OGG', 1)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) 
           VALUES (100, 2, 'qobuz-100', 'FLAC', 24, 96000, 150, 1)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) 
           VALUES (100, 3, 'tidal-100', 'FLAC', 24, 96000, 150, 1)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    // Test Case A: Priority is Qobuz (1) -> Tidal (2)
    perform_reorder_service_priorities(&pool, vec!["qobuz".to_string(), "tidal".to_string(), "spotify".to_string()])
        .await
        .unwrap();

    let preflight_qobuz = evaluate_track_preflight(
        &pool,
        100,
        Some("spotify"),
        Some("hires"),
        false, // strict_quality = false
        true,  // allow_fallback = true
    )
    .await
    .unwrap();

    assert_eq!(preflight_qobuz.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(preflight_qobuz.resolved_service_name, Some("qobuz".to_string()));
    assert_eq!(preflight_qobuz.resolved_service_track_id, Some("qobuz-100".to_string()));

    // Test Case B: Reorder priorities: Tidal (1) -> Qobuz (2)
    perform_reorder_service_priorities(&pool, vec!["tidal".to_string(), "qobuz".to_string(), "spotify".to_string()])
        .await
        .unwrap();

    let preflight_tidal = evaluate_track_preflight(
        &pool,
        100,
        Some("spotify"),
        Some("hires"),
        false, // strict_quality = false
        true,  // allow_fallback = true
    )
    .await
    .unwrap();

    assert_eq!(preflight_tidal.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(preflight_tidal.resolved_service_name, Some("tidal".to_string()));
    assert_eq!(preflight_tidal.resolved_service_track_id, Some("tidal-100".to_string()));
}

#[tokio::test]
async fn test_strict_quality_vs_allow_downgrade_policy_in_preflight() {
    let (_state, pool, _temp_dir) = setup_test_app_state(2).await;

    // Track 200: Spotify track with lossy MP3 Deezer source
    sqlx::query(
        r#"INSERT INTO tracks (id, title, isrc, duration_ms) 
           VALUES (200, 'Under Pressure', 'GBUM71029605', 248000)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) 
           VALUES (200, 1, 'spotify-200', 'OGG', 1)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (4, 4, 'Deezer Test', 'deezer@test.com', 1)")
        .execute(&pool).await.unwrap();

    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) 
           VALUES (200, 4, 'deezer-200', 'MP3', 16, 44100, 50, 1)"#
    )
    .execute(&pool)
    .await
    .unwrap();

    // 1. Strict quality policy (fallback_action = "skip") rejects lossy candidate when Hi-Res is requested
    let strict_result = evaluate_track_preflight(
        &pool,
        200,
        Some("spotify"),
        Some("hires"),
        true, // strict_quality = true
        true, // allow_fallback = true
    )
    .await
    .unwrap();

    assert_eq!(strict_result.status, DownloadPreflightStatus::RejectedQuality);
    assert_eq!(strict_result.is_eligible, false);

    // 2. Permissive fallback policy (fallback_action = "try_next") accepts downgrade
    let permissive_result = evaluate_track_preflight(
        &pool,
        200,
        Some("spotify"),
        Some("hires"),
        false, // strict_quality = false
        true,  // allow_fallback = true
    )
    .await
    .unwrap();

    assert_eq!(permissive_result.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(permissive_result.is_eligible, true);
    assert_eq!(permissive_result.resolved_service_name, Some("deezer".to_string()));
}
