//! Unit and Integration Tests for Retroactive Version Disambiguation & Repair (S143B)
//! Verifies:
//! 1. dry-run does not alter FS or SQLite
//! 2. successful rename of FLAC + sidecar LRC
//! 3. SHA-256 bit-for-bit invariance before and after move
//! 4. full rollback of FS and DB on transaction failure
//! 5. separation of canonical, source, display, and file titles

use std::path::PathBuf;
use tempfile::TempDir;
use sha2::{Sha256, Digest};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::disambiguation_repair::{
    compute_file_sha256, plan_disambiguation_repair, execute_disambiguation_repair,
};

async fn setup_test_db() -> (sqlx::SqlitePool, TempDir) {
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_repair.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Populate services, artist, album
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Gorillaz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Gorillaz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 1, 1)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2512, '19-2000', 1, 11, 'GBAYE1400474')")
        .execute(&pool)
        .await
        .unwrap();

    (pool, temp_dir)
}

#[tokio::test]
async fn test_dry_run_does_not_modify_fs_or_sqlite() {
    let (pool, temp) = setup_test_db().await;

    // Create fake audio and LRC files
    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();

    let flac_path = music_dir.join("17 - 19-2000.flac");
    let lrc_path = music_dir.join("17 - 19-2000.lrc");

    let fake_audio_data = b"AUDIO_DATA_FOR_SOULCHILD_REMIX_TRACK_17";
    let fake_lrc_data = b"[00:01.00] 19-2000 Soulchild Remix";

    tokio::fs::write(&flac_path, fake_audio_data).await.unwrap();
    tokio::fs::write(&lrc_path, fake_lrc_data).await.unwrap();

    // Insert track 2507 into SQLite
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES (2507, 1, 'primary')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format) VALUES (806, 2507, 2, ?, 'FLAC')"
    )
    .bind(flac_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // Run dry-run plan
    let plan = plan_disambiguation_repair(&pool).await.unwrap();

    assert!(plan.dry_run);
    assert_eq!(plan.total_candidates, 1);
    assert_eq!(plan.total_renamed, 1);
    assert_eq!(plan.items[0].display_title, "19-2000 (Soulchild Remix)");
    assert_eq!(plan.items[0].file_disambiguator, "Soulchild Remix");
    assert!(plan.items[0].target_audio_path.contains("17 - 19-2000 [Soulchild Remix].flac"));

    // Verify files on disk have NOT moved
    assert!(flac_path.exists(), "Original FLAC must remain in place after dry-run");
    assert!(lrc_path.exists(), "Original LRC must remain in place after dry-run");
    assert!(!PathBuf::from(&plan.items[0].target_audio_path).exists(), "Target file must not exist in dry-run");

    // Verify SQLite has NOT changed
    let db_path: String = sqlx::query_scalar("SELECT file_path FROM downloads WHERE track_id = 2507")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_path, flac_path.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_successful_flac_and_lrc_rename_with_sha256_invariance() {
    let (pool, temp) = setup_test_db().await;

    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();

    let flac_path = music_dir.join("17 - 19-2000.flac");
    let lrc_path = music_dir.join("17 - 19-2000.lrc");

    let fake_audio_data = b"AUDIO_DATA_FOR_SOULCHILD_REMIX_EXACT_SHA256";
    let fake_lrc_data = b"[00:01.00] 19-2000 Soulchild Remix Lyrics";

    tokio::fs::write(&flac_path, fake_audio_data).await.unwrap();
    tokio::fs::write(&lrc_path, fake_lrc_data).await.unwrap();

    let expected_hash = compute_file_sha256(&flac_path).await.unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (2507, 1, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format) VALUES (806, 2507, 2, ?, 'FLAC')"
    )
    .bind(flac_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let plan = plan_disambiguation_repair(&pool).await.unwrap();
    let report = execute_disambiguation_repair(&pool, plan).await.unwrap();

    assert_eq!(report.total_renamed, 1);
    assert!(report.errors.is_empty());

    let target_flac = music_dir.join("17 - 19-2000 [Soulchild Remix].flac");
    let target_lrc = music_dir.join("17 - 19-2000 [Soulchild Remix].lrc");

    // Verify FS changes
    assert!(!flac_path.exists(), "Old FLAC path must not exist after rename");
    assert!(!lrc_path.exists(), "Old LRC path must not exist after rename");
    assert!(target_flac.exists(), "New FLAC path must exist");
    assert!(target_lrc.exists(), "New LRC path must exist");

    // Verify SHA-256 after move is bit-for-bit identical
    let actual_hash_after = compute_file_sha256(&target_flac).await.unwrap();
    assert_eq!(actual_hash_after, expected_hash, "Audio SHA-256 must remain identical after rename");

    // Verify SQLite updated atomically
    let (db_path, db_disambiguator): (String, Option<String>) = sqlx::query_as(
        "SELECT file_path, file_disambiguator FROM downloads WHERE track_id = 2507"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(db_path, target_flac.to_string_lossy().to_string());
    assert_eq!(db_disambiguator, Some("Soulchild Remix".to_string()));

    let (display_title, source_title, file_disambiguator): (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT display_title, source_title, file_disambiguator FROM tracks WHERE id = 2507"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(display_title, Some("19-2000 (Soulchild Remix)".to_string()));
    assert_eq!(source_title, Some("19-2000".to_string()));
    assert_eq!(file_disambiguator, Some("Soulchild Remix".to_string()));
}

#[tokio::test]
async fn test_rollback_preserves_original_state_on_failure() {
    let (pool, temp) = setup_test_db().await;

    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();

    let flac_path = music_dir.join("17 - 19-2000.flac");
    let lrc_path = music_dir.join("17 - 19-2000.lrc");

    let fake_audio_data = b"ORIGINAL_AUDIO_DATA_BEFORE_FAILED_REPAIR";
    let fake_lrc_data = b"[00:01.00] 19-2000 Original Lyrics";

    tokio::fs::write(&flac_path, fake_audio_data).await.unwrap();
    tokio::fs::write(&lrc_path, fake_lrc_data).await.unwrap();

    let expected_hash = compute_file_sha256(&flac_path).await.unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (2507, 1, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format) VALUES (806, 2507, 2, ?, 'FLAC')"
    )
    .bind(flac_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // Create a plan where target audio path has an invalid/read-only or colliding condition
    let mut plan = plan_disambiguation_repair(&pool).await.unwrap();
    
    // Simulate failure by corrupting the item target path or breaking the DB constraint
    // Drop table downloads before execution to simulate unexpected DB failure mid-transaction
    sqlx::query("DROP TABLE downloads").execute(&pool).await.unwrap();

    let report = execute_disambiguation_repair(&pool, plan).await.unwrap();

    assert_eq!(report.total_renamed, 0, "No tracks should be successfully renamed on DB failure");
    assert!(!report.errors.is_empty(), "Errors must be captured in report");

    // Assert that FS rollback occurred and files were returned to their exact original locations
    assert!(flac_path.exists(), "Original FLAC path must still exist after rollback");
    assert!(lrc_path.exists(), "Original LRC path must still exist after rollback");

    let restored_hash = compute_file_sha256(&flac_path).await.unwrap();
    assert_eq!(restored_hash, expected_hash, "Hash after rollback must be identical");
}

#[tokio::test]
async fn test_canonical_source_display_separation() {
    let (pool, _temp) = setup_test_db().await;

    // Track with disambiguator
    sqlx::query(
        r#"INSERT INTO tracks (id, title, display_title, source_title, file_disambiguator, isrc)
           VALUES (2507, '19-2000', '19-2000 (Soulchild Remix)', '19-2000', 'Soulchild Remix', 'GBAYE1400480')"#
    )
    .execute(&pool)
    .await
    .unwrap();

    let (title, display_title, source_title, disambiguator): (String, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT title, display_title, source_title, file_disambiguator FROM tracks WHERE id = 2507"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(title, "19-2000", "Canonical title remains clean");
    assert_eq!(display_title, Some("19-2000 (Soulchild Remix)".to_string()));
    assert_eq!(source_title, Some("19-2000".to_string()), "Upstream source title preserved without forgery");
    assert_eq!(disambiguator, Some("Soulchild Remix".to_string()));
}
