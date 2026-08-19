//! Integration test suite for Sprint S148: Settings Persistence Across Application Restart
//!
//! Validates:
//! 1. Settings modified in one lifecycle survive an application shutdown and restart.
//! 2. Authoritative SQLite persistence ensures no regression to static defaults.
//! 3. Effective values (quality caps, preferred format, fallback action, provider priority, concurrency, templates)
//!    are accurately loaded by a brand new AppState instance.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use syncify_tauri_lib::commands::{
    perform_get_effective_download_preferences, perform_save_effective_download_preferences,
    EffectiveDownloadPreferences,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::models::QualityPreference;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tempfile::TempDir;

#[tokio::test]
async fn test_effective_settings_persist_across_restart() {
    let temp_dir = TempDir::new().expect("Failed to create tempdir");
    let db_path = temp_dir.path().join("syncify_test_restart.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy().replace('\\', "/"));

    let custom_download_dir = temp_dir.path().join("MyCustomLibrary");
    std::fs::create_dir_all(&custom_download_dir).expect("Failed to create custom download directory");
    let custom_download_path = custom_download_dir.to_string_lossy().to_string();

    // ==============================================
    // LIFECYCLE 1: INITIALIZE, SEED & SAVE CUSTOM SETTINGS
    // ==============================================
    {
        let opts = SqliteConnectOptions::from_str(&db_url)
            .unwrap()
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("Failed to connect to test DB in lifecycle 1");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Migrations must apply cleanly");

        // Seed baseline services
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

        let state = AppState {
            db: pool.clone(),
            worker_state: DownloadWorkerState::new(2),
            album_lock: Arc::new(tokio::sync::Mutex::new(())),
            enrichment_state: EnrichmentWorkerState::new(),
        };

        // Read initial state
        let initial = perform_get_effective_download_preferences(&state)
            .await
            .expect("Initial get_effective_download_preferences should succeed");

        // Apply customized settings
        let custom_prefs = EffectiveDownloadPreferences {
            download_path: custom_download_path.clone(),
            staging_path: format!("{}/.staging", custom_download_path),
            path_status: "valid".to_string(),
            free_space_bytes: initial.free_space_bytes,
            max_quality: "hires".to_string(),
            preferred_format: "flac".to_string(),
            fallback_action: "skip".to_string(),
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
                    max_quality: "lossless".to_string(),
                    preferred_format: "flac".to_string(),
                    fallback_quality: "high".to_string(),
                    fallback_format: "mp3".to_string(),
                },
            ],
            max_concurrent_downloads: 5,
            rate_limit_delay_ms: 1200,
            max_retries: 4,
            retry_delay_seconds: 10,
            auto_download_favorites: true,
            generate_lyrics_lrc: true,
            generate_cover_art: true,
            generate_animated_cover: false, // Customized: disabled
            generate_booklet: false,        // Customized: disabled
            generate_artist_sidecars: true,
            auto_sync_enabled: true,
            sync_interval_value: 2,
            sync_interval_unit: "days".to_string(),
            sync_on_startup: false,
            background_download: false,
            pause_on_metered: true,
            pause_on_low_battery: true,
            folder_template: "{AlbumArtist}/{Year} - {Album}".to_string(),
            file_template: "{DiscNumber}-{TrackNumber:pad2} {Title}".to_string(),
            artist_separator: " / ".to_string(),
            replace_spaces_with: Some("_".to_string()),
            max_path_length: 220,
        };

        let saved = perform_save_effective_download_preferences(&state, custom_prefs)
            .await
            .expect("Saving custom preferences in lifecycle 1 should succeed");

        assert_eq!(saved.max_concurrent_downloads, 5);
        assert_eq!(saved.download_path, custom_download_path);
        assert_eq!(saved.generate_animated_cover, false);
        assert_eq!(saved.generate_booklet, false);

        pool.close().await;
    }

    // ==============================================
    // LIFECYCLE 2: SIMULATE APPLICATION RESTART
    // ==============================================
    {
        let opts = SqliteConnectOptions::from_str(&db_url)
            .unwrap()
            .create_if_missing(false);

        let restarted_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("Failed to reconnect to test DB in lifecycle 2");

        // Read saved concurrency from sync_settings or advanced_settings
        let saved_concurrency: i32 = sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
            .fetch_one(&restarted_pool)
            .await
            .unwrap();

        let restarted_state = AppState {
            db: restarted_pool.clone(),
            worker_state: DownloadWorkerState::new(saved_concurrency as usize),
            album_lock: Arc::new(tokio::sync::Mutex::new(())),
            enrichment_state: EnrichmentWorkerState::new(),
        };

        // Query effective download preferences on fresh state
        let effective = perform_get_effective_download_preferences(&restarted_state)
            .await
            .expect("get_effective_download_preferences should succeed after restart");

        // Assert all persisted values match custom settings without reverting to defaults
        assert_eq!(effective.download_path, custom_download_path);
        assert_eq!(effective.max_concurrent_downloads, 5);
        assert_eq!(effective.fallback_action, "skip");
        assert_eq!(effective.allow_downgrade, false);
        assert_eq!(effective.strict_quality, true);
        assert_eq!(effective.rate_limit_delay_ms, 1200);
        assert_eq!(effective.max_retries, 4);
        assert_eq!(effective.retry_delay_seconds, 10);
        assert_eq!(effective.auto_download_favorites, true);
        assert_eq!(effective.generate_animated_cover, false);
        assert_eq!(effective.generate_booklet, false);
        assert_eq!(effective.generate_lyrics_lrc, true);
        assert_eq!(effective.folder_template, "{AlbumArtist}/{Year} - {Album}");
        assert_eq!(effective.file_template, "{DiscNumber}-{TrackNumber:pad2} {Title}");
        assert_eq!(effective.artist_separator, " / ");
        assert_eq!(effective.replace_spaces_with, Some("_".to_string()));
        assert_eq!(effective.max_path_length, 220);

        // Verify provider priorities order survived restart
        assert_eq!(effective.preferred_download_service, Some("tidal".to_string()));
        assert_eq!(effective.service_priority_order, vec![
            "tidal".to_string(),
            "qobuz".to_string(),
            "deezer".to_string(),
            "spotify".to_string(),
            "soundcloud".to_string(),
        ]);

        // Verify quality preferences per service survived restart
        let tidal_q = effective.service_qualities.iter().find(|q| q.service_name == "tidal").unwrap();
        assert_eq!(tidal_q.max_quality, "hires");
        assert_eq!(tidal_q.preferred_format, "flac");
        assert_eq!(tidal_q.fallback_quality, "lossless");

        let qobuz_q = effective.service_qualities.iter().find(|q| q.service_name == "qobuz").unwrap();
        assert_eq!(qobuz_q.max_quality, "lossless");
        assert_eq!(qobuz_q.preferred_format, "flac");
        assert_eq!(qobuz_q.fallback_quality, "high");

        restarted_pool.close().await;
    }
}
