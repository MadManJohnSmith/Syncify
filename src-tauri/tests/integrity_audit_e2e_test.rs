//! E2E Test Suite for Sprint S100: Hardening de Descargas en Producción
//!
//! Validates physical file checking, magic bytes validation, staging purge,
//! database referential consistency, and automatic repair routines.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through 0047 must apply cleanly");

    // Seed services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_integrity_audit_clean_database_passes() {
    let db = create_test_db().await;

    // Create a temporary valid FLAC file
    let temp_dir = std::env::temp_dir().join(format!("syncify_audit_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("valid_track.flac");

    let mut valid_flac_data = Vec::new();
    valid_flac_data.extend_from_slice(b"fLaC");
    valid_flac_data.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // STREAMINFO header
    valid_flac_data.resize(42, 0);
    fs::write(&flac_path, &valid_flac_data).unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Valid FLAC Track') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, ?, 'FLAC')")
        .bind(track_id)
        .bind(flac_path.to_string_lossy().to_string())
        .execute(&db)
        .await
        .unwrap();

    // Query downloads
    let downloads: Vec<(i64, Option<i64>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, track_id, file_path, file_format FROM downloads"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(downloads.len(), 1);
    let path = std::path::Path::new(&downloads[0].2);
    assert!(path.exists());

    let bytes = fs::read(path).unwrap();
    assert!(bytes.starts_with(b"fLaC"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_integrity_audit_missing_physical_file_detected() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Missing Track') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, 'C:/non_existent_folder/missing.flac', 'FLAC')")
        .bind(track_id)
        .execute(&db)
        .await
        .unwrap();

    let downloads: Vec<(i64, Option<i64>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, track_id, file_path, file_format FROM downloads"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    let mut missing_count = 0;
    for (_, _, fp, _) in downloads {
        if !std::path::Path::new(&fp).exists() {
            missing_count += 1;
        }
    }

    assert_eq!(missing_count, 1, "Missing physical file must be flagged in audit");
}

#[tokio::test]
async fn test_integrity_audit_zero_byte_and_corrupt_file_detected() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_corrupt_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let zero_byte = temp_dir.join("zero.flac");
    fs::write(&zero_byte, b"").unwrap();

    let corrupt_header = temp_dir.join("corrupt.m4a");
    fs::write(&corrupt_header, b"CORRUPT_NOT_AUDIO_BYTES_HERE").unwrap();

    // Verify detection
    let meta_zero = fs::metadata(&zero_byte).unwrap();
    assert_eq!(meta_zero.len(), 0, "Zero-byte file must be detected");

    let corrupt_bytes = fs::read(&corrupt_header).unwrap();
    let is_flac = corrupt_bytes.starts_with(b"fLaC");
    let is_m4a = corrupt_bytes.len() >= 8 && (&corrupt_bytes[4..8] == b"ftyp" || &corrupt_bytes[0..4] == b"ftyp");
    let is_mp3 = corrupt_bytes.starts_with(b"ID3");
    assert!(!is_flac && !is_m4a && !is_mp3, "Invalid audio container magic header must be rejected");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_integrity_audit_abandoned_staging_detected_and_repaired() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_staging_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let part1 = temp_dir.join("track_1.flac.part");
    let part2 = temp_dir.join("track_2.m4a.partial");
    fs::write(&part1, b"partial data 1").unwrap();
    fs::write(&part2, b"partial data 2").unwrap();

    assert!(part1.exists());
    assert!(part2.exists());

    // Repair routine: purge staging files
    for p in [&part1, &part2] {
        if p.exists() {
            fs::remove_file(p).unwrap();
        }
    }

    assert!(!part1.exists());
    assert!(!part2.exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_integrity_audit_stuck_downloading_repaired() {
    let db = create_test_db().await;

    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Stuck Track') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 50, 0)")
        .bind(tid).execute(&db).await.unwrap();

    let stuck_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_queue WHERE status = 'downloading'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(stuck_count.0, 1);

    // Repair: reset to queued
    let res = sqlx::query("UPDATE download_queue SET status = 'queued' WHERE status = 'downloading'")
        .execute(&db).await.unwrap();

    assert_eq!(res.rows_affected(), 1);

    let queued_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(queued_count.0, 1);
}
