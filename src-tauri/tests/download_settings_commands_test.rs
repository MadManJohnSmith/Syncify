//! Integration test suite for Sprint S120B: Download Commands & CLI Flags Backend Integration
//!
//! Validates:
//! 1. Unified `get_download_settings` and `save_download_settings` roundtrip.
//! 2. Synchronization of `max_concurrent_downloads` between settings, SQLite, and `DownloadWorkerState`.
//! 3. `update_quality_preference` and `update_fallback_action` persistence.
//! 4. `force_redownload_tracks`, `clear_download_history`, and `reset_download_history` lifecycle.
//! 5. Fine-grained `get_sidecar_settings` and `update_sidecar_settings` flags.
//! 6. Configured download directory resolution in library layout and promotion.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_core_domain::{FolderFileTemplateConfig, LibraryLayout, TrackLayoutContext};
use syncify_tauri_lib::commands::{
    perform_batch_health_check, perform_clear_download_history, perform_force_redownload_tracks,
    perform_get_download_settings, perform_get_effective_download_paths, perform_get_folder_settings,
    perform_get_quality_preferences, perform_get_sidecar_settings, perform_reset_download_history,
    perform_save_download_settings, perform_save_setting, perform_set_max_concurrent_downloads,
    perform_update_fallback_action, perform_update_quality_preference,
    perform_update_sidecar_settings, resolve_effective_download_paths, validate_directory_path,
    DownloadSettingsDto, SidecarSettingsDto,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
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

    // Baseline service preferences
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority, auto_import_enabled) VALUES ('qobuz', 1, 1), ('tidal', 2, 1), ('spotify', 3, 1)")
        .execute(&pool).await.unwrap();

    // Baseline accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz Test', 'qobuz@test.com', 1)")
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
async fn test_download_settings_roundtrip_and_worker_synchronization() {
    let (state, pool, temp_dir) = setup_test_app_state(2).await;
    let base_path = temp_dir.path().to_string_lossy().to_string();

    // Verify initial state
    assert_eq!(state.worker_state.max_concurrent(), 2);
    let initial_settings = perform_get_download_settings(&state)
        .await
        .expect("get_download_settings should succeed");

    assert_eq!(initial_settings.max_concurrent_downloads, 2);

    // Update settings with custom configuration
    let new_custom_path = format!("{}/CustomMusicLibrary", base_path);
    let modified_settings = DownloadSettingsDto {
        download_path: new_custom_path.clone(),
        temporary_root: Some(format!("{}/.staging", base_path)),
        folder_template: "{AlbumArtist}/{Year} - {Album}".to_string(),
        file_template: "{DiscNumber}-{TrackNumber:pad2} {Title}".to_string(),
        artist_separator: " / ".to_string(),
        replace_spaces_with: Some("_".to_string()),
        max_path_length: 240,
        fallback_action: "skip".to_string(),
        max_concurrent_downloads: 5,
        retry_failed: true,
        retry_count: 4,
        retry_delay_ms: 3000,
        auto_download_favorites: true,
        organize_by_artist: true,
        organize_by_album: true,
        generate_lyrics_lrc: true,
        generate_cover_art: true,
        generate_animated_cover: false,
        generate_booklet: false,
        generate_artist_sidecars: true,
        library_root: None,
        staging_root: None,
        path_status: None,
        free_space_bytes: None,
    };

    let saved_settings = perform_save_download_settings(&state, modified_settings.clone())
        .await
        .expect("save_download_settings should succeed");

    // 1. Assert worker state was updated live to 5
    assert_eq!(state.worker_state.max_concurrent(), 5);

    // 2. Assert return value matches saved configuration
    assert_eq!(saved_settings.download_path, new_custom_path);
    assert_eq!(saved_settings.max_concurrent_downloads, 5);
    assert_eq!(saved_settings.folder_template, "{AlbumArtist}/{Year} - {Album}");
    assert_eq!(saved_settings.file_template, "{DiscNumber}-{TrackNumber:pad2} {Title}");
    assert_eq!(saved_settings.artist_separator, " / ");
    assert_eq!(saved_settings.replace_spaces_with, Some("_".to_string()));
    assert_eq!(saved_settings.max_path_length, 240);
    assert_eq!(saved_settings.fallback_action, "skip");
    assert_eq!(saved_settings.retry_count, 4);
    assert_eq!(saved_settings.retry_delay_ms, 3000);
    assert_eq!(saved_settings.auto_download_favorites, true);
    assert_eq!(saved_settings.generate_animated_cover, false);
    assert_eq!(saved_settings.generate_booklet, false);

    // 3. Verify SQLite tables persistence directly
    let db_folder_base: String = sqlx::query_scalar("SELECT base_folder FROM folder_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_folder_base, new_custom_path);

    let db_sync_max: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_sync_max, 5);

    let db_kv_concurrent: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_concurrent_downloads'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_kv_concurrent, "5");
}

#[tokio::test]
async fn test_quality_preferences_and_fallback_action_commands() {
    let (state, pool, _temp) = setup_test_app_state(3).await;

    // 1. Update quality preference for Qobuz
    let updated_qobuz = perform_update_quality_preference(
        &state.db,
        "qobuz".to_string(),
        "24_96".to_string(),
        "flac".to_string(),
        "16_44".to_string(),
        "flac".to_string(),
    )
    .await
    .expect("update_quality_preference should succeed");

    assert_eq!(updated_qobuz.service_name, "qobuz");
    assert_eq!(updated_qobuz.max_quality, "24_96");
    assert_eq!(updated_qobuz.preferred_format, "flac");

    let all_prefs = perform_get_quality_preferences(&state.db)
        .await
        .expect("get_quality_preferences should succeed");
    assert!(!all_prefs.is_empty());
    let qobuz_entry = all_prefs.iter().find(|p| p.service_name == "qobuz").unwrap();
    assert_eq!(qobuz_entry.max_quality, "24_96");

    // 2. Update fallback action
    let fallback = perform_update_fallback_action(&state.db, "strict".to_string())
        .await
        .expect("update_fallback_action should succeed");
    assert_eq!(fallback, "strict");

    let current_folder_settings = perform_get_folder_settings(&state.db)
        .await
        .unwrap();
    assert_eq!(current_folder_settings.fallback_action, "strict");

    let db_action: String = sqlx::query_scalar("SELECT fallback_action FROM folder_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_action, "strict");
}

#[tokio::test]
async fn test_set_max_concurrent_downloads_command_persists() {
    let (state, pool, _temp) = setup_test_app_state(3).await;

    let res = perform_set_max_concurrent_downloads(&state, 4)
        .await
        .expect("set_max_concurrent_downloads should succeed");
    assert_eq!(res, 4);

    // Verify worker in memory
    assert_eq!(state.worker_state.max_concurrent(), 4);

    // Verify sync_settings in DB
    let sync_max: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sync_max, 4);

    // Verify settings KV in DB
    let kv_val: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_concurrent_downloads'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kv_val, "4");
}

#[tokio::test]
async fn test_force_redownload_and_clear_history_commands() {
    let (state, pool, temp_dir) = setup_test_app_state(3).await;

    // Create test artists, albums, tracks
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Redownload Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('Redownload Album', '123456789012') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    let track_id_1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Track 1', ?, 'USRC1001') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    let track_id_2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Track 2', ?, 'USRC1002') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary'), (?, ?, 'primary')")
        .bind(track_id_1).bind(artist_id)
        .bind(track_id_2).bind(artist_id)
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_1', 'FLAC', 24, 96000, 100, 1), (?, 2, 'qobuz_2', 'FLAC', 24, 96000, 100, 1)")
        .bind(track_id_1)
        .bind(track_id_2)
        .execute(&pool).await.unwrap();

    let fake_file_1 = temp_dir.path().join("Track1.flac");
    std::fs::write(&fake_file_1, b"fLaCdummy1").unwrap();
    let fake_file_2 = temp_dir.path().join("Track2.flac");
    std::fs::write(&fake_file_2, b"fLaCdummy2").unwrap();

    // Insert history in downloads table
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_size_bytes, bit_depth, sample_rate, downloaded_at) VALUES (?, 2, ?, 1024, 24, 96000, CURRENT_TIMESTAMP)")
        .bind(track_id_1)
        .bind(fake_file_1.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_size_bytes, bit_depth, sample_rate, downloaded_at) VALUES (?, 2, ?, 1024, 24, 96000, CURRENT_TIMESTAMP)")
        .bind(track_id_2)
        .bind(fake_file_2.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Insert completed queue entries
    sqlx::query("INSERT INTO download_queue (track_id, status, progress_percent) VALUES (?, 'complete', 100.0), (?, 'complete', 100.0)")
        .bind(track_id_1)
        .bind(track_id_2)
        .execute(&pool)
        .await
        .unwrap();

    let initial_download_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(initial_download_count, 2);

    // 1. Force re-download track 1
    let requeued = perform_force_redownload_tracks(
        &state,
        vec![track_id_1],
        Some(70),
        Some("hires".to_string()),
    )
    .await
    .expect("force_redownload_tracks should succeed");

    assert_eq!(requeued, 1);

    // Track 1 should be removed from downloads table
    let count_t1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
        .bind(track_id_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_t1, 0);

    // Track 1 should now be queued in download_queue with priority 70
    let queue_status: (String, i64) = sqlx::query_as("SELECT status, priority FROM download_queue WHERE track_id = ?")
        .bind(track_id_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queue_status.0, "queued");
    assert_eq!(queue_status.1, 70);

    // Track 2 is still in downloads table
    let count_t2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
        .bind(track_id_2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_t2, 1);

    // 2. Clear specific download history
    let cleared_count = perform_clear_download_history(&state.db, Some(vec![track_id_2]))
        .await
        .expect("clear_download_history should succeed");
    assert_eq!(cleared_count, 1);

    let total_downloads_after_clear: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(total_downloads_after_clear, 0);

    // 3. Reset download history
    let reset_msg = perform_reset_download_history(&state.db)
        .await
        .expect("reset_download_history should succeed");
    assert!(reset_msg.contains("reset successfully"));
}

#[tokio::test]
async fn test_sidecar_settings_commands_toggle() {
    let (state, pool, _temp) = setup_test_app_state(3).await;

    let initial = perform_get_sidecar_settings(&state.db)
        .await
        .expect("get_sidecar_settings should succeed");

    assert_eq!(initial.generate_lyrics_lrc, true);
    assert_eq!(initial.generate_cover_art, true);
    assert_eq!(initial.generate_animated_cover, true);
    assert_eq!(initial.generate_booklet, true);
    assert_eq!(initial.generate_artist_sidecars, true);

    // Update flags
    let updated = perform_update_sidecar_settings(
        &state.db,
        SidecarSettingsDto {
            generate_lyrics_lrc: true,
            generate_cover_art: true,
            generate_animated_cover: false,
            generate_booklet: false,
            generate_artist_sidecars: false,
        },
    )
    .await
    .expect("update_sidecar_settings should succeed");

    assert_eq!(updated.generate_lyrics_lrc, true);
    assert_eq!(updated.generate_cover_art, true);
    assert_eq!(updated.generate_animated_cover, false);
    assert_eq!(updated.generate_booklet, false);
    assert_eq!(updated.generate_artist_sidecars, false);

    // Assert persistence in settings table
    let db_booklet: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_generate_booklet'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_booklet, "false");
}

#[tokio::test]
async fn test_configured_download_path_respected_in_library_layout() {
    let (state, _pool, temp_dir) = setup_test_app_state(3).await;
    let custom_library_dir = temp_dir.path().join("CustomAudioTarget");
    std::fs::create_dir_all(&custom_library_dir).unwrap();

    let custom_path_str = custom_library_dir.to_string_lossy().to_string();

    let settings_to_save = DownloadSettingsDto {
        download_path: custom_path_str.clone(),
        temporary_root: Some(temp_dir.path().join(".staging").to_string_lossy().to_string()),
        folder_template: "{Artist}/{Album}".to_string(),
        file_template: "{TrackNumber:pad2} - {Title}".to_string(),
        artist_separator: ", ".to_string(),
        replace_spaces_with: None,
        max_path_length: 255,
        fallback_action: "try_next".to_string(),
        max_concurrent_downloads: 3,
        retry_failed: true,
        retry_count: 3,
        retry_delay_ms: 1000,
        auto_download_favorites: false,
        organize_by_artist: true,
        organize_by_album: true,
        generate_lyrics_lrc: true,
        generate_cover_art: true,
        generate_animated_cover: true,
        generate_booklet: true,
        generate_artist_sidecars: true,
        library_root: None,
        staging_root: None,
        path_status: None,
        free_space_bytes: None,
    };

    let active_settings = perform_save_download_settings(&state, settings_to_save)
        .await
        .unwrap();

    assert_eq!(active_settings.download_path, custom_path_str);

    // Verify LibraryLayout uses configured base folder
    let layout_config = FolderFileTemplateConfig {
        folder_template: active_settings.folder_template,
        file_template: active_settings.file_template,
        artist_separator: active_settings.artist_separator,
        replace_spaces_with: active_settings.replace_spaces_with,
        max_path_length: active_settings.max_path_length as usize,
    };

    let layout = LibraryLayout::with_config(std::path::Path::new(&active_settings.download_path), layout_config);

    let ctx = TrackLayoutContext {
        artist: "Pink Floyd",
        album_artist: Some("Pink Floyd"),
        album: "The Dark Side of the Moon",
        title: "Money",
        year: Some(1973),
        original_date: Some("1973-03-01"),
        track_number: 6,
        track_total: Some(10),
        disc_number: 1,
        total_discs: 1,
        format: "flac",
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
    };

    let resolved_path = layout.resolve_track_path(&ctx);
    assert!(resolved_path.starts_with(&custom_library_dir), "Track path must be inside configured custom directory");
    assert!(resolved_path.to_string_lossy().contains("Pink Floyd"));
    assert!(resolved_path.to_string_lossy().contains("The Dark Side of the Moon"));
    assert!(resolved_path.to_string_lossy().contains("06 - Money.flac"));
}

#[tokio::test]
async fn test_effective_paths_deterministic_priority_and_compatibility() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Initial State: folder_settings is blank, settings table has no paths -> OS default
    let eff_default = resolve_effective_download_paths(&pool)
        .await
        .expect("Default resolution must succeed");
    assert!(!eff_default.library_root.is_empty());
    assert!(eff_default.staging_root.ends_with(".staging"));
    assert_eq!(eff_default.path_status, "valid");

    // 2. Compatibility Layer: Legacy `download_path` key in settings table
    sqlx::query("INSERT INTO settings (key, value) VALUES ('download_path', 'C:/LegacyMusicPath')")
        .execute(&pool)
        .await
        .unwrap();
    let eff_legacy_1 = resolve_effective_download_paths(&pool).await.unwrap();
    assert_eq!(eff_legacy_1.library_root, "C:/LegacyMusicPath");
    assert_eq!(eff_legacy_1.staging_root, "C:/LegacyMusicPath\\.staging");

    // 3. Priority: `dl_download_path` takes precedence over legacy `download_path`
    sqlx::query("INSERT INTO settings (key, value) VALUES ('dl_download_path', 'C:/NewerDlPath')")
        .execute(&pool)
        .await
        .unwrap();
    let eff_legacy_2 = resolve_effective_download_paths(&pool).await.unwrap();
    assert_eq!(eff_legacy_2.library_root, "C:/NewerDlPath");

    // 4. Canonical Authority: `folder_settings.base_folder` has absolute priority over settings table
    sqlx::query("UPDATE folder_settings SET base_folder = 'D:/CanonicalLibrary' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let eff_canonical = resolve_effective_download_paths(&pool).await.unwrap();
    assert_eq!(eff_canonical.library_root, "D:/CanonicalLibrary");
    assert_eq!(eff_canonical.staging_root, "D:/CanonicalLibrary\\.staging");

    // 5. Anti-drift Guard: Saving blank or stale legacy key does NOT wipe canonical configured path
    perform_save_setting(&pool, "dl_download_path".to_string(), "".to_string()).await.unwrap();
    let eff_preserved = resolve_effective_download_paths(&pool).await.unwrap();
    assert_eq!(eff_preserved.library_root, "D:/CanonicalLibrary");
}

#[tokio::test]
async fn test_microsd_or_custom_path_staging_derivation_and_space() {
    let (state, pool, temp_dir) = setup_test_app_state(2).await;
    let custom_target = temp_dir.path().join("SDCard_Music");
    std::fs::create_dir_all(&custom_target).unwrap();
    let custom_target_str = custom_target.to_string_lossy().to_string();

    let settings = DownloadSettingsDto {
        download_path: custom_target_str.clone(),
        temporary_root: None,
        folder_template: "{Artist}/{Album}".to_string(),
        file_template: "{Title}".to_string(),
        artist_separator: ", ".to_string(),
        replace_spaces_with: None,
        max_path_length: 255,
        fallback_action: "skip".to_string(),
        max_concurrent_downloads: 2,
        retry_failed: true,
        retry_count: 3,
        retry_delay_ms: 1000,
        auto_download_favorites: false,
        organize_by_artist: true,
        organize_by_album: true,
        generate_lyrics_lrc: true,
        generate_cover_art: true,
        generate_animated_cover: false,
        generate_booklet: false,
        generate_artist_sidecars: true,
        library_root: None,
        staging_root: None,
        path_status: None,
        free_space_bytes: None,
    };

    let saved = perform_save_download_settings(&state, settings).await.unwrap();
    assert_eq!(saved.download_path, custom_target_str);
    assert_eq!(saved.library_root, Some(custom_target_str.clone()));
    let expected_staging = custom_target.join(".staging").to_string_lossy().to_string();
    assert_eq!(saved.staging_root, Some(expected_staging.clone()));
    assert_eq!(saved.path_status, Some("valid".to_string()));
    assert!(saved.free_space_bytes.unwrap_or(0) > 0);

    // Verify dedicated command get_effective_download_paths
    let eff = perform_get_effective_download_paths(&pool).await.unwrap();
    assert_eq!(eff.library_root, custom_target_str);
    assert_eq!(eff.staging_root, expected_staging);
    assert_eq!(eff.path_status, "valid");
    assert!(eff.is_writable);
    assert!(eff.drive_mounted);
    assert!(eff.free_space_bytes > 0);
}

#[tokio::test]
async fn test_unmounted_drive_path_status_detection() {
    let unmounted_path = "Z:\\NonExistentVolume\\MusicLibrary".to_string();
    let res = validate_directory_path(unmounted_path.clone()).await.unwrap();
    assert!(!res.valid);
    assert!(!res.drive_mounted);
    assert!(res.error_message.is_some());
}

#[tokio::test]
async fn test_batch_health_check_reports_effective_download_and_staging_paths() {
    let (state, pool, temp_dir) = setup_test_app_state(2).await;
    let custom_target = temp_dir.path().join("HealthCheckLib");
    std::fs::create_dir_all(&custom_target).unwrap();
    let custom_target_str = custom_target.to_string_lossy().to_string();

    sqlx::query("UPDATE folder_settings SET base_folder = ? WHERE id = 1")
        .bind(&custom_target_str)
        .execute(&pool)
        .await
        .unwrap();

    let health = perform_batch_health_check(&pool, None, Some(&state.worker_state))
        .await
        .expect("Health check must succeed");

    assert_eq!(health.effective_download_path, custom_target_str);
    assert_eq!(health.effective_staging_path, custom_target.join(".staging").to_string_lossy().to_string());
    assert!(health.healthy);
}

#[tokio::test]
async fn test_save_download_settings_synchronizes_all_legacy_keys() {
    let (state, pool, temp_dir) = setup_test_app_state(2).await;
    let new_path = temp_dir.path().join("SyncedTarget").to_string_lossy().to_string();

    let settings = DownloadSettingsDto {
        download_path: new_path.clone(),
        temporary_root: None,
        folder_template: "{Artist}/{Album}".to_string(),
        file_template: "{Title}".to_string(),
        artist_separator: ", ".to_string(),
        replace_spaces_with: None,
        max_path_length: 255,
        fallback_action: "skip".to_string(),
        max_concurrent_downloads: 3,
        retry_failed: true,
        retry_count: 3,
        retry_delay_ms: 1000,
        auto_download_favorites: false,
        organize_by_artist: true,
        organize_by_album: true,
        generate_lyrics_lrc: true,
        generate_cover_art: true,
        generate_animated_cover: false,
        generate_booklet: false,
        generate_artist_sidecars: true,
        library_root: None,
        staging_root: None,
        path_status: None,
        free_space_bytes: None,
    };

    let _ = perform_save_download_settings(&state, settings).await.unwrap();

    // Verify all keys in SQLite directly
    let base_folder: String = sqlx::query_scalar("SELECT base_folder FROM folder_settings WHERE id = 1")
        .fetch_one(&pool).await.unwrap();
    let dl_download_path: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_download_path'")
        .fetch_one(&pool).await.unwrap();
    let download_dir: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'download_dir'")
        .fetch_one(&pool).await.unwrap();
    let download_path: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'download_path'")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(base_folder, new_path);
    assert_eq!(dl_download_path, new_path);
    assert_eq!(download_dir, new_path);
    assert_eq!(download_path, new_path);
}

#[tokio::test]
async fn test_concurrency_persistence_and_restart_simulation() {
    let (state, pool, _temp_dir) = setup_test_app_state(2).await;

    for target_concurrency in [1usize, 3usize, 5usize] {
        // 1. Set concurrency via perform_set_max_concurrent_downloads
        let res = perform_set_max_concurrent_downloads(&state, target_concurrency).await.unwrap();
        assert_eq!(res, target_concurrency);
        assert_eq!(state.worker_state.max_concurrent(), target_concurrency);

        // 2. Verify all DB persistence tables match
        let sync_val: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        let adv_val: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM advanced_settings WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        let kv_val: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_concurrent_downloads'")
            .fetch_one(&pool).await.unwrap();

        assert_eq!(sync_val as usize, target_concurrency);
        assert_eq!(adv_val as usize, target_concurrency);
        assert_eq!(kv_val.parse::<usize>().unwrap(), target_concurrency);

        // 3. Verify get_download_settings matches
        let settings = perform_get_download_settings(&state).await.unwrap();
        assert_eq!(settings.max_concurrent_downloads as usize, target_concurrency);

        // 4. Simulate App Restart (re-loading persisted max_concurrent from SQLite)
        let loaded_after_restart: usize = {
            let val: Option<i64> = sqlx::query_scalar(
                "SELECT COALESCE(
                    (SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1),
                    (SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'dl_concurrent_downloads'),
                    2
                )"
            )
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();

            val.map(|v| v.max(1) as usize).unwrap_or(2)
        };

        assert_eq!(loaded_after_restart, target_concurrency, "Restored concurrency after simulated restart must match target");

        let restarted_worker = DownloadWorkerState::new(loaded_after_restart);
        assert_eq!(restarted_worker.max_concurrent(), target_concurrency);
    }
}

