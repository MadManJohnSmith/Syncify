//! Integration test suite for Sprint S148A:
//! Runtime Validation & Canonical Settings Audit (Preflight Control, Atomic Rollback, Cross-Tab Sync)
//!
//! Validates:
//! 1. Settings Quality mutation (strict quality, max quality, allow lossy fallback, provider priorities).
//! 2. Cross-tab synchronization without reload (Downloads & Sync tabs receive identical canonical effective values).
//! 3. Persistence across application lifecycle restart.
//! 4. Preflight execution over tracks with Qobuz/Tidal/Deezer (dynamic provider priority, strict quality excluding lossy, permitted fallback).
//! 5. Atomic rollback on invalid download directory (zero partial writes across all 6 tables + in-memory worker).
//! 6. Full Before/After audit dumps of:
//!    - `quality_preferences`
//!    - `service_preferences`
//!    - `folder_settings`
//!    - `sync_settings`
//!    - `advanced_settings`
//!    - `settings`
//!    - worker concurrency

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use syncify_tauri_lib::commands::{
    evaluate_track_preflight, perform_get_effective_download_preferences,
    perform_reorder_service_priorities, perform_save_effective_download_preferences,
    perform_update_quality_preference, DownloadPreflightStatus, EffectiveDownloadPreferences,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::models::QualityPreference;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tempfile::TempDir;

#[derive(Debug, Clone, PartialEq)]
pub struct TableAuditSnapshot {
    pub quality_preferences_count: i64,
    pub service_priorities: Vec<(String, i32)>,
    pub folder_base: String,
    pub folder_fallback: String,
    pub sync_concurrency: i32,
    pub sync_auto: bool,
    pub advanced_concurrency: i32,
    pub advanced_retries: i32,
    pub settings_path: Option<String>,
    pub settings_concurrency: Option<String>,
    pub worker_concurrency: usize,
}

async fn capture_audit_snapshot(pool: &SqlitePool, state: &AppState) -> TableAuditSnapshot {
    let q_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quality_preferences")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let sp_rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT service_name, priority FROM service_preferences ORDER BY priority ASC"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let (folder_base, folder_fallback): (String, String) = sqlx::query_as(
        "SELECT base_folder, fallback_action FROM folder_settings WHERE id = 1"
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| ("".to_string(), "".to_string()));

    let (sync_concurrency, sync_auto): (i32, bool) = sqlx::query_as(
        "SELECT max_concurrent_downloads, auto_sync_enabled FROM sync_settings WHERE id = 1"
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0, false));

    let (adv_concurrency, adv_retries): (i32, i32) = sqlx::query_as(
        "SELECT max_concurrent_downloads, max_retries FROM advanced_settings WHERE id = 1"
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let settings_path: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'download_path'")
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    let settings_concurrency: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_concurrent_downloads'")
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    TableAuditSnapshot {
        quality_preferences_count: q_count,
        service_priorities: sp_rows,
        folder_base,
        folder_fallback,
        sync_concurrency,
        sync_auto,
        advanced_concurrency: adv_concurrency,
        advanced_retries: adv_retries,
        settings_path,
        settings_concurrency,
        worker_concurrency: state.worker_state.max_concurrent(),
    }
}

async fn setup_runtime_test_db(temp_dir: &TempDir) -> (SqlitePool, String) {
    let db_path = temp_dir.path().join("syncify_runtime_audit.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy().replace('\\', "/"));

    let opts = SqliteConnectOptions::from_str(&db_url)
        .unwrap()
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("Failed to connect to test runtime DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Seed services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (4, 'deezer', 1, 'high')")
        .execute(&pool).await.unwrap();

    // Accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify Active', 'spotify@test.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz Active', 'qobuz@test.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal Active', 'tidal@test.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (4, 4, 'Deezer Active', 'deezer@test.com', 1)")
        .execute(&pool).await.unwrap();

    (pool, db_url)
}

#[tokio::test]
async fn test_runtime_validation_suite_s148a() {
    let temp_dir = TempDir::new().expect("Failed to create tempdir");
    let (pool, db_url) = setup_runtime_test_db(&temp_dir).await;

    let valid_custom_download_dir = temp_dir.path().join("AuditedMusicLibrary");
    std::fs::create_dir_all(&valid_custom_download_dir).expect("Failed to create valid download dir");
    let valid_path = valid_custom_download_dir.to_string_lossy().to_string();

    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(3),
        album_lock: Arc::new(tokio::sync::Mutex::new(())),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };

    // =========================================================================
    // 0. CAPTURE BEFORE AUDIT SNAPSHOT
    // =========================================================================
    let snapshot_before = capture_audit_snapshot(&pool, &state).await;
    println!("\n[AUDIT BEFORE S148A]: {:#?}", snapshot_before);

    // =========================================================================
    // 1. CHANGE FROM SETTINGS QUALITY (Strict, Max Quality, Fallback, Priorities)
    // =========================================================================
    // Update Quality Preference for Tidal & Qobuz
    perform_update_quality_preference(
        &pool,
        "tidal".to_string(),
        "hires".to_string(),
        "flac".to_string(),
        "lossless".to_string(),
        "flac".to_string(),
    )
    .await
    .unwrap();

    perform_update_quality_preference(
        &pool,
        "qobuz".to_string(),
        "hires".to_string(),
        "flac".to_string(),
        "lossless".to_string(),
        "flac".to_string(),
    )
    .await
    .unwrap();

    // Reorder provider priority: Tidal (1) -> Qobuz (2) -> Deezer (3) -> Spotify (4) -> SoundCloud (5)
    perform_reorder_service_priorities(
        &pool,
        vec![
            "tidal".to_string(),
            "qobuz".to_string(),
            "deezer".to_string(),
            "spotify".to_string(),
            "soundcloud".to_string(),
        ],
    )
    .await
    .unwrap();

    // Save consolidated preferences (Settings Quality + Downloads + Sync)
    let custom_prefs = EffectiveDownloadPreferences {
        download_path: valid_path.clone(),
        staging_path: format!("{}/.staging", valid_path),
        path_status: "valid".to_string(),
        free_space_bytes: 100_000_000_000,
        max_quality: "hires".to_string(),
        preferred_format: "flac".to_string(),
        fallback_action: "skip".to_string(), // Strict Quality = true (no lossy fallback)
        allow_downgrade: false,
        strict_quality: true,
        preferred_download_service: Some("tidal".to_string()),
        service_priority_order: vec![
            "tidal".to_string(),
            "qobuz".to_string(),
            "deezer".to_string(),
            "spotify".to_string(),
            "soundcloud".to_string(),
        ],
        service_qualities: vec![
            QualityPreference {
                id: 1,
                service_name: "tidal".to_string(),
                max_quality: "hires".to_string(),
                preferred_format: "flac".to_string(),
                fallback_quality: "lossless".to_string(),
                fallback_format: "flac".to_string(),
            },
            QualityPreference {
                id: 2,
                service_name: "qobuz".to_string(),
                max_quality: "hires".to_string(),
                preferred_format: "flac".to_string(),
                fallback_quality: "lossless".to_string(),
                fallback_format: "flac".to_string(),
            },
        ],
        max_concurrent_downloads: 4,
        rate_limit_delay_ms: 800,
        max_retries: 4,
        retry_delay_seconds: 8,
        auto_download_favorites: true,
        generate_lyrics_lrc: true,
        generate_cover_art: true,
        generate_animated_cover: true,
        generate_booklet: false,
        generate_artist_sidecars: true,
        auto_sync_enabled: true,
        sync_interval_value: 6,
        sync_interval_unit: "hours".to_string(),
        sync_on_startup: true,
        background_download: true,
        pause_on_metered: false,
        pause_on_low_battery: false,
        folder_template: "{AlbumArtist}/{Album}".to_string(),
        file_template: "{TrackNumber:pad2} - {Title}".to_string(),
        artist_separator: " / ".to_string(),
        replace_spaces_with: None,
        max_path_length: 255,
    };

    let saved = perform_save_effective_download_preferences(&state, custom_prefs)
        .await
        .expect("Saving consolidated settings must succeed");

    assert_eq!(saved.strict_quality, true);
    assert_eq!(saved.fallback_action, "skip");
    assert_eq!(saved.allow_downgrade, false);
    assert_eq!(saved.preferred_download_service, Some("tidal".to_string()));
    assert_eq!(state.worker_state.max_concurrent(), 4);

    // =========================================================================
    // 2. OPEN SETTINGS DOWNLOADS & SYNC (Confirm identical effective value)
    // =========================================================================
    let effective_downloads = perform_get_effective_download_preferences(&state)
        .await
        .expect("Fetching effective settings from Downloads view");
    let effective_sync = perform_get_effective_download_preferences(&state)
        .await
        .expect("Fetching effective settings from Sync view");

    assert_eq!(effective_downloads, effective_sync);
    assert_eq!(effective_downloads.max_concurrent_downloads, 4);
    assert_eq!(effective_downloads.download_path, valid_path);
    assert_eq!(effective_downloads.fallback_action, "skip");
    assert_eq!(effective_downloads.preferred_download_service, Some("tidal".to_string()));

    // =========================================================================
    // 3. CLOSE/REOPEN APPLICATION (Confirm persisted values)
    // =========================================================================
    pool.close().await;

    let opts = SqliteConnectOptions::from_str(&db_url)
        .unwrap()
        .create_if_missing(false);

    let restarted_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("Failed to reconnect after restart");

    let saved_concurrency: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
        .fetch_one(&restarted_pool)
        .await
        .unwrap();

    let restarted_state = AppState {
        db: restarted_pool.clone(),
        worker_state: DownloadWorkerState::new(saved_concurrency as usize),
        album_lock: Arc::new(tokio::sync::Mutex::new(())),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };

    let post_restart_effective = perform_get_effective_download_preferences(&restarted_state)
        .await
        .expect("Post restart get_effective_download_preferences");

    assert_eq!(post_restart_effective.download_path, valid_path);
    assert_eq!(post_restart_effective.max_concurrent_downloads, 4);
    assert_eq!(post_restart_effective.fallback_action, "skip");
    assert_eq!(post_restart_effective.strict_quality, true);
    assert_eq!(post_restart_effective.allow_downgrade, false);
    assert_eq!(post_restart_effective.preferred_download_service, Some("tidal".to_string()));
    assert_eq!(restarted_state.worker_state.max_concurrent(), 4);

    // =========================================================================
    // 4. PREFLIGHT EXECUTION OVER TRACKS (Qobuz/Tidal/Fallback/Lossless/AAC)
    // =========================================================================
    // Track 100: Spotify import with Tidal (Hi-Res FLAC 24/96) and Qobuz (Hi-Res FLAC 24/96) sources
    sqlx::query("INSERT INTO tracks (id, title, isrc, duration_ms) VALUES (100, 'Bohemian Rhapsody', 'GBUM71029604', 354000)")
        .execute(&restarted_pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) VALUES (100, 1, 'spotify-100', 'OGG', 1)")
        .execute(&restarted_pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (100, 2, 'qobuz-100', 'FLAC', 24, 96000, 150, 1)")
        .execute(&restarted_pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (100, 3, 'tidal-100', 'FLAC', 24, 96000, 150, 1)")
        .execute(&restarted_pool).await.unwrap();

    // 4A: Order Tidal (1) -> Qobuz (2) => Preflight resolves Tidal
    let preflight_tidal = evaluate_track_preflight(
        &restarted_pool,
        100,
        Some("spotify"),
        Some("hires"),
        false,
        true,
    )
    .await
    .unwrap();
    assert_eq!(preflight_tidal.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(preflight_tidal.resolved_service_name, Some("tidal".to_string()));
    assert_eq!(preflight_tidal.resolved_service_track_id, Some("tidal-100".to_string()));

    // 4B: Reorder Qobuz (1) -> Tidal (2) => Preflight dynamically resolves Qobuz
    perform_reorder_service_priorities(
        &restarted_pool,
        vec![
            "qobuz".to_string(),
            "tidal".to_string(),
            "deezer".to_string(),
            "spotify".to_string(),
            "soundcloud".to_string(),
        ],
    )
    .await
    .unwrap();

    let preflight_qobuz = evaluate_track_preflight(
        &restarted_pool,
        100,
        Some("spotify"),
        Some("hires"),
        false,
        true,
    )
    .await
    .unwrap();
    assert_eq!(preflight_qobuz.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(preflight_qobuz.resolved_service_name, Some("qobuz".to_string()));
    assert_eq!(preflight_qobuz.resolved_service_track_id, Some("qobuz-100".to_string()));

    // Track 200: Spotify import with Deezer (Lossy AAC/MP3 320k) source
    sqlx::query("INSERT INTO tracks (id, title, isrc, duration_ms) VALUES (200, 'Under Pressure', 'GBUM71029605', 248000)")
        .execute(&restarted_pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) VALUES (200, 1, 'spotify-200', 'OGG', 1)")
        .execute(&restarted_pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (200, 4, 'deezer-200', 'MP3', 16, 44100, 50, 1)")
        .execute(&restarted_pool).await.unwrap();

    // 4C: Strict quality = true (fallback_action = "skip") EXCLUDES Deezer lossy candidate
    let preflight_strict_deezer = evaluate_track_preflight(
        &restarted_pool,
        200,
        Some("spotify"),
        Some("hires"),
        true, // strict_quality = true
        true,
    )
    .await
    .unwrap();
    assert_eq!(preflight_strict_deezer.status, DownloadPreflightStatus::RejectedQuality);
    assert_eq!(preflight_strict_deezer.is_eligible, false);

    // 4D: Strict quality = false (fallback_action = "try_next") ACCEPTS lossy fallback
    let preflight_permissive_deezer = evaluate_track_preflight(
        &restarted_pool,
        200,
        Some("spotify"),
        Some("hires"),
        false, // strict_quality = false
        true,
    )
    .await
    .unwrap();
    assert_eq!(preflight_permissive_deezer.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(preflight_permissive_deezer.is_eligible, true);
    assert_eq!(preflight_permissive_deezer.resolved_service_name, Some("deezer".to_string()));

    // =========================================================================
    // 5. ATOMIC ROLLBACK ON INVALID DOWNLOAD PATH (Zero partial writes)
    // =========================================================================
    let snapshot_pre_invalid = capture_audit_snapshot(&restarted_pool, &restarted_state).await;

    let mut invalid_prefs = post_restart_effective.clone();
    invalid_prefs.download_path = if cfg!(windows) {
        "Z:\\NonExistentDrive\\CorruptedPath".to_string()
    } else {
        "/mnt/nonexistent_drive_unmounted_volume/CorruptedPath".to_string()
    };
    invalid_prefs.max_concurrent_downloads = 10; // Would be an illegal partial update if not transactional
    invalid_prefs.fallback_action = "try_next".to_string();

    let save_err = perform_save_effective_download_preferences(&restarted_state, invalid_prefs).await;
    assert!(save_err.is_err(), "Invalid download path must fail validation");

    let snapshot_post_invalid = capture_audit_snapshot(&restarted_pool, &restarted_state).await;
    assert_eq!(
        snapshot_pre_invalid, snapshot_post_invalid,
        "All tables and worker state must remain completely untouched after failed save"
    );

    // UI restores previous effective values
    let restored_effective = perform_get_effective_download_preferences(&restarted_state)
        .await
        .expect("Querying effective settings after failed save");
    assert_eq!(restored_effective.download_path, valid_path);
    assert_eq!(restored_effective.max_concurrent_downloads, 4);
    assert_eq!(restored_effective.fallback_action, "skip");

    // =========================================================================
    // 6. CAPTURE FINAL AFTER AUDIT SNAPSHOT
    // =========================================================================
    let snapshot_after = capture_audit_snapshot(&restarted_pool, &restarted_state).await;
    println!("\n[AUDIT AFTER S148A]: {:#?}", snapshot_after);

    restarted_pool.close().await;
}
