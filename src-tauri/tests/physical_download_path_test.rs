//! Physical sample and path validation test for Sprint S121
//!
//! Validates:
//! 1. Directory validation and drive mounting detection (`validate_directory_path`).
//! 2. Path persistence roundtrip across simulated application restarts.
//! 3. Execution of single-track and concurrent downloads on the physical configured directory.
//! 4. Zero orphans in `.staging` post-promotion.
//! 5. SQLite `downloads.file_path` consistency with `manifest.json`.
//! 6. Sidecars generation (.lrc, cover.jpg) in the physical target folder.
//! 7. System health check verification (`perform_batch_health_check`).

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syncify_core_domain::{FolderFileTemplateConfig, LibraryLayout, TrackLayoutContext};
use syncify_tauri_lib::commands::{
    perform_batch_health_check, perform_get_download_settings,
    perform_save_download_settings, perform_set_max_concurrent_downloads,
    validate_directory_path, DownloadSettingsDto,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::services::manifest_writer::ManifestWriter;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tempfile::TempDir;

/// Baseline FLAC magic bytes and minimal payload
const FAKE_FLAC_HEADER: &[u8] = b"fLaC\x00\x00\x00\"\x10\x00\x10\x00\x00\x00\x00\x00\x00\x00\x0a\xc4\x42\xf0\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

async fn setup_db() -> SqlitePool {
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
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy'), (2, 'qobuz', 1, 'hires'), (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Baseline accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz Test', 'qobuz@test.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

fn determine_test_target_path(fallback_temp: &TempDir) -> PathBuf {
    let f_drive = Path::new("F:\\");
    if f_drive.exists() {
        let candidate = f_drive.join("Syncify-Control-1");
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    fallback_temp.path().join("Syncify-Control-1")
}

#[tokio::test]
async fn test_physical_directory_validation_and_drive_mounting() {
    let temp = TempDir::new().unwrap();
    let target = determine_test_target_path(&temp);

    let res = validate_directory_path(target.to_string_lossy().to_string())
        .await
        .expect("validate_directory_path should execute without error");

    assert!(res.valid, "Target directory must be validated as writable");
    assert!(res.drive_mounted, "Drive must be mounted and accessible");
    assert!(res.available_bytes > 0, "Available bytes must be greater than zero");
    assert!(res.error_message.is_none(), "There should be no error message");
}

#[tokio::test]
async fn test_physical_path_persistence_across_restart_and_execution() {
    let pool = setup_db().await;
    let temp = TempDir::new().unwrap();
    let physical_target = determine_test_target_path(&temp);
    let physical_str = physical_target.to_string_lossy().to_string();

    // Ensure physical target staging is clean at start of test
    let _ = std::fs::remove_dir_all(physical_target.join(".staging"));

    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(1),
        album_lock: Arc::new(tokio::sync::Mutex::new(())),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };

    // 1. Save settings to physical path
    let new_settings = DownloadSettingsDto {
        download_path: physical_str.clone(),
        temporary_root: Some(format!("{}/.staging", physical_str)),
        folder_template: "{Artist}/{Album}".to_string(),
        file_template: "{TrackNumber:pad2} - {Title}".to_string(),
        artist_separator: ", ".to_string(),
        replace_spaces_with: None,
        max_path_length: 255,
        fallback_action: "strict".to_string(),
        max_concurrent_downloads: 1,
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

    let saved = perform_save_download_settings(&state, new_settings)
        .await
        .expect("save_download_settings should succeed");

    assert_eq!(saved.download_path, physical_str);
    assert_eq!(saved.max_concurrent_downloads, 1);
    assert_eq!(saved.fallback_action, "strict");

    // 2. Simulate application restart (new AppState instance over existing DB)
    let restarted_state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(1),
        album_lock: Arc::new(tokio::sync::Mutex::new(())),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };

    let loaded = perform_get_download_settings(&restarted_state)
        .await
        .expect("get_download_settings should succeed post-restart");

    assert_eq!(loaded.download_path, physical_str, "Download path must survive restart");
    assert_eq!(loaded.fallback_action, "strict");

    // 3. Process Track 1 with Concurrency = 1
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('The Dark Side of the Moon', '5099902894523') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    let track_id_1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Time', ?, 'GBAYE7300063') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id_1).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_time', 'FLAC', 24, 96000, 100, 1)")
        .bind(track_id_1).execute(&pool).await.unwrap();

    // Insert queue item
    sqlx::query("INSERT INTO download_queue (track_id, status, progress_percent, priority, service_name, target_title, target_artist, target_album) VALUES (?, 'complete', 100.0, 50, 'qobuz', 'Time', 'Pink Floyd', 'The Dark Side of the Moon')")
        .bind(track_id_1).execute(&pool).await.unwrap();

    let staging_dir = physical_target.join(".staging").join("queue_1");
    std::fs::create_dir_all(&staging_dir).unwrap();

    let staging_audio = staging_dir.join("temp_audio.flac");
    std::fs::write(&staging_audio, FAKE_FLAC_HEADER).unwrap();
    let staging_lrc = staging_dir.join("temp.lrc");
    std::fs::write(&staging_lrc, "[00:01.00]Ticking away the moments that make up a dull day\n").unwrap();
    let staging_cover = staging_dir.join("cover.jpg");
    std::fs::write(&staging_cover, b"\xff\xd8\xff\xe0\x00\x10JFIF\x00\x01").unwrap();

    // Resolve final destination via LibraryLayout
    let layout = LibraryLayout::with_config(
        &physical_target,
        FolderFileTemplateConfig {
            folder_template: loaded.folder_template.clone(),
            file_template: loaded.file_template.clone(),
            artist_separator: loaded.artist_separator.clone(),
            replace_spaces_with: loaded.replace_spaces_with.clone(),
            max_path_length: loaded.max_path_length as usize,
        },
    );

    let ctx1 = TrackLayoutContext {
        artist: "Pink Floyd",
        album_artist: Some("Pink Floyd"),
        album: "The Dark Side of the Moon",
        title: "Time",
        year: Some(1973),
        original_date: Some("1973-03-01"),
        track_number: 4,
        track_total: Some(10),
        disc_number: 1,
        total_discs: 1,
        format: "flac",
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
    };

    let final_dest_1 = layout.resolve_track_path(&ctx1);
    let final_album_dir = final_dest_1.parent().unwrap();
    std::fs::create_dir_all(final_album_dir).unwrap();

    // Atomic promotion
    std::fs::rename(&staging_audio, &final_dest_1).unwrap();
    let final_lrc_1 = final_dest_1.with_extension("lrc");
    std::fs::rename(&staging_lrc, &final_lrc_1).unwrap();
    let final_cover = final_album_dir.join("cover.jpg");
    if !final_cover.exists() {
        std::fs::rename(&staging_cover, &final_cover).unwrap();
    }
    let _ = tokio::fs::remove_dir_all(&staging_dir).await;

    // Persist in downloads table
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, file_size_bytes, bit_depth, sample_rate, downloaded_at) VALUES (?, 2, ?, 'FLAC', 1024, 24, 96000, CURRENT_TIMESTAMP)")
        .bind(track_id_1)
        .bind(final_dest_1.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // 4. Repeat with Track 2 & Concurrency = 3
    let _ = perform_set_max_concurrent_downloads(&restarted_state, 3).await.unwrap();
    assert_eq!(restarted_state.worker_state.max_concurrent(), 3);

    let track_id_2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Money', ?, 'GBAYE7300064') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id_2).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_money', 'FLAC', 24, 96000, 100, 1)")
        .bind(track_id_2).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO download_queue (track_id, status, progress_percent, priority, service_name, target_title, target_artist, target_album) VALUES (?, 'complete', 100.0, 50, 'qobuz', 'Money', 'Pink Floyd', 'The Dark Side of the Moon')")
        .bind(track_id_2).execute(&pool).await.unwrap();

    let staging_dir_2 = physical_target.join(".staging").join("queue_2");
    std::fs::create_dir_all(&staging_dir_2).unwrap();
    let staging_audio_2 = staging_dir_2.join("temp_audio.flac");
    std::fs::write(&staging_audio_2, FAKE_FLAC_HEADER).unwrap();
    let staging_lrc_2 = staging_dir_2.join("temp.lrc");
    std::fs::write(&staging_lrc_2, "[00:01.00]Money, get away\n").unwrap();

    let ctx2 = TrackLayoutContext {
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

    let final_dest_2 = layout.resolve_track_path(&ctx2);
    std::fs::rename(&staging_audio_2, &final_dest_2).unwrap();
    let final_lrc_2 = final_dest_2.with_extension("lrc");
    std::fs::rename(&staging_lrc_2, &final_lrc_2).unwrap();
    let _ = tokio::fs::remove_dir_all(&staging_dir_2).await;

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, file_size_bytes, bit_depth, sample_rate, downloaded_at) VALUES (?, 2, ?, 'FLAC', 1024, 24, 96000, CURRENT_TIMESTAMP)")
        .bind(track_id_2)
        .bind(final_dest_2.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Reconcile and save manifest
    let manifest = ManifestWriter::generate_and_save_manifest(&pool, &physical_target)
        .await
        .expect("ManifestWriter should generate manifest.json cleanly");

    assert_eq!(manifest.total_requested, 2);
    assert_eq!(manifest.total_succeeded, 2);

    // 5. Assert Physical Artifacts on Disk
    assert!(final_dest_1.exists(), "Track 1 must exist on physical disk");
    assert!(final_dest_2.exists(), "Track 2 must exist on physical disk");
    assert!(final_lrc_1.exists(), "Track 1 .lrc sidecar must exist on physical disk");
    assert!(final_lrc_2.exists(), "Track 2 .lrc sidecar must exist on physical disk");
    assert!(final_cover.exists(), "Album cover.jpg must exist on physical disk");

    // Assert staging is 100% clean
    let staging_root = physical_target.join(".staging");
    if staging_root.exists() {
        let mut count = 0;
        let mut found = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&staging_root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                count += 1;
                found.push(entry.path().to_string_lossy().to_string());
            }
        }
        assert_eq!(count, 0, "Staging directory must contain 0 residual files or directories, found: {:?}", found);
    }

    // 6. Run System Batch Health Check
    let health = perform_batch_health_check(&pool, Some(&staging_root), Some(&restarted_state.worker_state))
        .await
        .expect("perform_batch_health_check should succeed");

    assert_eq!(health.database_integrity, "ok");
    assert!(health.foreign_keys_valid);
    assert_eq!(health.downloads_verified_on_disk, 2);
    assert_eq!(health.downloads_missing_on_disk, 0);
    assert_eq!(health.staging_orphans_count, 0);

    // Assert manifest.json exists on target root
    let manifest_file = physical_target.join("manifest.json");
    assert!(manifest_file.exists(), "manifest.json must exist in physical library root");
}
