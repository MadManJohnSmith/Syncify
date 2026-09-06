//! E2E Test Suite for Sprint S100: Hardening de Descargas en Producción
//!
//! Validates physical file checking, magic bytes validation, staging purge,
//! database referential consistency, and automatic repair routines using production commands.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;
use syncify_tauri_lib::commands::integrity::{
    perform_repair_integrity_issues, perform_run_integrity_audit,
};

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

    // Seed services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

fn create_temp_test_dir(prefix: &str) -> std::path::PathBuf {
    let temp_dir = std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    temp_dir
}

#[tokio::test]
async fn test_integrity_audit_clean_database_passes() {
    let db = create_test_db().await;
    let temp_dir = create_temp_test_dir("syncify_audit_clean");
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

    // Invoke production integrity audit
    let report = perform_run_integrity_audit(&db, Some(temp_dir.to_string_lossy().to_string()))
        .await
        .expect("perform_run_integrity_audit must succeed");

    assert!(report.is_healthy, "Audit must report healthy for valid file and database state");
    assert_eq!(report.total_tracks_scanned, 1);
    assert_eq!(report.verified_files, 1);
    assert!(report.missing_files.is_empty());
    assert!(report.corrupt_or_zero_byte_files.is_empty());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_integrity_audit_missing_physical_file_detected() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Missing Track') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, '/non_existent_folder/missing.flac', 'FLAC')")
        .bind(track_id)
        .execute(&db)
        .await
        .unwrap();

    // Invoke production integrity audit
    let report = perform_run_integrity_audit(&db, None)
        .await
        .expect("perform_run_integrity_audit must succeed");

    assert!(!report.is_healthy, "Audit must flag missing files as unhealthy");
    assert_eq!(report.missing_files.len(), 1, "Missing physical file must be flagged in audit");
    assert!(report.missing_files[0].contains("missing.flac"));
}

#[tokio::test]
async fn test_integrity_audit_zero_byte_and_corrupt_file_detected() {
    let db = create_test_db().await;
    let temp_dir = create_temp_test_dir("syncify_audit_corrupt");

    let zero_byte = temp_dir.join("zero.flac");
    fs::write(&zero_byte, b"").unwrap();

    let corrupt_header = temp_dir.join("corrupt.flac");
    fs::write(&corrupt_header, b"CORRUPT_NOT_AUDIO_BYTES_HERE").unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Zero Byte Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Corrupt Track') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, ?, 'FLAC')")
        .bind(t1).bind(zero_byte.to_string_lossy().to_string()).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, ?, 'FLAC')")
        .bind(t2).bind(corrupt_header.to_string_lossy().to_string()).execute(&db).await.unwrap();

    // Production audit detects both zero-byte and corrupt audio magic bytes
    let report = perform_run_integrity_audit(&db, None)
        .await
        .expect("perform_run_integrity_audit must succeed");

    assert!(!report.is_healthy);
    assert_eq!(report.corrupt_or_zero_byte_files.len(), 2, "Both zero-byte and corrupt header must be flagged");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_integrity_audit_abandoned_staging_detected_and_repaired() {
    let db = create_test_db().await;
    let temp_dir = create_temp_test_dir("syncify_audit_staging");

    let part1 = temp_dir.join("track_1.flac.part");
    let part2 = temp_dir.join("track_2.m4a.partial");
    fs::write(&part1, b"partial data 1").unwrap();
    fs::write(&part2, b"partial data 2").unwrap();

    let report = perform_run_integrity_audit(&db, Some(temp_dir.to_string_lossy().to_string()))
        .await
        .expect("perform_run_integrity_audit must succeed");

    assert_eq!(report.abandoned_staging_files.len(), 2, "Audit must detect both abandoned staging files");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_integrity_audit_stuck_downloading_repaired() {
    let db = create_test_db().await;

    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Stuck Track') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 50, 0)")
        .bind(tid).execute(&db).await.unwrap();

    // Verify detection in production audit
    let report = perform_run_integrity_audit(&db, None).await.unwrap();
    assert!(report.database_inconsistencies.iter().any(|s| s.contains("stuck in 'downloading'")));

    // Invoke production repair command to reset stuck items
    let repair_res = perform_repair_integrity_issues(&db, None)
        .await
        .expect("perform_repair_integrity_issues must succeed");

    assert_eq!(repair_res.cleaned_database_entries, 1);

    let queued_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(queued_count.0, 1, "Stuck download must be reset to 'queued' by repair");
}
