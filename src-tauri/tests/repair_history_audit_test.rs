//! S163: Repair History and Audit Integration Tests
//!
//! Tests:
//! 1. append on successful repair (all 16 minimum fields recorded)
//! 2. no append on dry-run (dry-run leaves repair_history empty)
//! 3. append on failed/rollback repair records failed audit event with rollback state
//! 4. history ordering (chronologically descending timestamp, id)
//! 5. hash & provenance integrity (sanitization, no secrets, no streaming URLs)
//! 6. historical 918/919 import if verifiable (only when verified downloads exist, otherwise no unverified records)

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use tempfile::TempDir;
use syncify_tauri_lib::services::repair_history::{
    fetch_repair_history, record_applied_repair, import_historical_verified_repairs,
    sanitize_audit_text,
};
use syncify_tauri_lib::services::tidal_pipeline::reenrich_download_file;

async fn write_test_flac(path: &Path, audio_payload: &[u8]) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // Last metadata block, STREAMINFO, 34 bytes
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(audio_payload);
    tokio::fs::write(path, &flac_bytes).await.expect("Failed to write test flac");
}

async fn create_test_db() -> (sqlx::Pool<sqlx::Sqlite>, TempDir) {
    let _ = syncify_tauri_lib::crypto::init_keychain_crypto()
        .or_else(|_| syncify_tauri_lib::crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_repair_history.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool)
        .await
        .unwrap();

    (pool, temp_dir)
}

#[tokio::test]
async fn test_no_append_on_dry_run() {
    let (pool, temp) = create_test_db().await;

    let audio_dir = temp.path().join("Unknown Artist").join("Unknown Album");
    tokio::fs::create_dir_all(&audio_dir).await.unwrap();
    let flac_path = audio_dir.join("01 - Tidal Track 134683067.flac");
    write_test_flac(&flac_path, b"DRY_RUN_AUDIO").await;

    sqlx::query("INSERT OR REPLACE INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    let ghost_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, track_number) VALUES ('Tidal Track 134683067', 1) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let dl_id: i64 = sqlx::query_scalar("INSERT INTO downloads (track_id, file_path, file_format, metadata_completeness) VALUES (?, ?, 'FLAC', 0) RETURNING id")
        .bind(ghost_id).bind(flac_path.to_string_lossy().to_string()).fetch_one(&pool).await.unwrap();

    // Execute DRY RUN
    let dry_res = reenrich_download_file(&pool, dl_id, true).await.unwrap();
    assert!(dry_res.dry_run);

    // Verify 0 rows in repair_history
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repair_history").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0, "Dry-run must NEVER append records to repair_history");
}

#[tokio::test]
async fn test_append_on_successful_repair() {
    let (pool, temp) = create_test_db().await;

    let audio_dir = temp.path().join("Unknown Artist").join("Unknown Album");
    tokio::fs::create_dir_all(&audio_dir).await.unwrap();
    let flac_path = audio_dir.join("01 - Tidal Track 134683067.flac");
    let audio_payload = b"\xFF\xF8\x18\x00_TEST_APPEND_SUCCESS_AUDIO";
    write_test_flac(&flac_path, audio_payload).await;

    sqlx::query("INSERT OR REPLACE INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(temp.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Canonical target
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Radiohead') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('OK Computer', '1997-05-21') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();
    let real_track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (album_id, title, track_number, isrc) VALUES (?, 'Airbag', 1, 'GBAYE9700001') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(real_track_id).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, '134683067')")
        .bind(real_track_id).execute(&pool).await.unwrap();

    let ghost_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, track_number) VALUES ('Tidal Track 134683067', 1) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let dl_id: i64 = sqlx::query_scalar("INSERT INTO downloads (track_id, file_path, file_format, metadata_completeness) VALUES (?, ?, 'FLAC', 0) RETURNING id")
        .bind(ghost_id).bind(flac_path.to_string_lossy().to_string()).fetch_one(&pool).await.unwrap();

    // Execute Apply
    let apply_res = reenrich_download_file(&pool, dl_id, false).await.unwrap();
    assert!(apply_res.success);

    // Query repair_history
    let history = fetch_repair_history(&pool, None, None).await.unwrap();
    assert_eq!(history.len(), 1);

    let rec = &history[0];
    assert!(!rec.repair_id.is_empty());
    assert_eq!(rec.download_id, Some(dl_id));
    assert_eq!(rec.old_track_id, Some(ghost_id));
    assert_eq!(rec.new_track_id, Some(real_track_id));
    assert_eq!(rec.old_path, flac_path.to_string_lossy().to_string());
    assert!(rec.new_path.contains("01 - Airbag.flac"));
    assert!(!rec.input_file_hash.is_empty());
    assert!(rec.output_file_hash.is_some());
    assert!(rec.audio_payload_hash_before.is_some());
    assert!(rec.audio_payload_hash_after.is_some());
    assert_eq!(rec.audio_payload_hash_before, rec.audio_payload_hash_after);
    assert_eq!(rec.baseline_validation, "valid");
    assert!(rec.actions.contains(&"validated_baseline".to_string()));
    assert!(rec.actions.contains(&"tags_applied".to_string()));
    assert!(rec.actions.contains(&"database_updated".to_string()));
    assert_eq!(rec.result, "success");
    assert_eq!(rec.provenance, "tidal_pipeline.re_enrich");
    assert!(rec.rollback_state.is_none());
}

#[tokio::test]
async fn test_history_ordering_desc() {
    let (pool, _temp) = create_test_db().await;

    // Manually insert 3 audit records with different timestamps
    record_applied_repair(
        &pool, "rep_1", Some(101), Some(1), Some(2),
        "/old/1.flac", "/new/1.flac", "hash_1", Some("hash_out_1"),
        Some("audio_1"), Some("audio_1"), "valid", &["action1".to_string()],
        None, "test.prov", "success", None
    ).await.unwrap();

    record_applied_repair(
        &pool, "rep_2", Some(102), Some(3), Some(4),
        "/old/2.flac", "/new/2.flac", "hash_2", Some("hash_out_2"),
        Some("audio_2"), Some("audio_2"), "valid", &["action2".to_string()],
        None, "test.prov", "success", None
    ).await.unwrap();

    record_applied_repair(
        &pool, "rep_3", Some(103), Some(5), Some(6),
        "/old/3.flac", "/new/3.flac", "hash_3", None,
        Some("audio_3"), None, "repair_input_changed", &["action3".to_string()],
        Some("RollbackExecuted"), "test.prov", "failed", None
    ).await.unwrap();

    let list = fetch_repair_history(&pool, None, None).await.unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].repair_id, "rep_3", "Newest record must be first");
    assert_eq!(list[1].repair_id, "rep_2");
    assert_eq!(list[2].repair_id, "rep_1");
}

#[tokio::test]
async fn test_hash_and_provenance_integrity_and_sanitization() {
    let (pool, _temp) = create_test_db().await;

    let dirty_path = "https://streaming.tidal.com/audio/secret_token=xyz12345/01.flac";
    let sanitized = sanitize_audit_text(dirty_path);
    assert!(!sanitized.contains("secret_token=xyz12345"));
    assert_eq!(sanitized, "[REDACTED_STREAM_URL]");

    let rep_id = "rep_sanitized_test";
    record_applied_repair(
        &pool, rep_id, Some(999), None, None,
        dirty_path, "/clean/path.flac", "input_sha256", Some("output_sha256"),
        None, None, "valid", &[], None,
        "tidal_pipeline.re_enrich https://api.tidal.com/v1/auth_token=abc", "success", None
    ).await.unwrap();

    let list = fetch_repair_history(&pool, None, None).await.unwrap();
    let r = list.into_iter().find(|item| item.repair_id == rep_id).unwrap();
    assert!(!r.old_path.contains("secret_token"));
    assert!(!r.provenance.contains("auth_token=abc"));
}

#[tokio::test]
async fn test_historical_918_919_import_if_verifiable() {
    let (pool, _temp) = create_test_db().await;

    // 1. Without downloads in DB, no unverified records are invented
    let imported_empty = import_historical_verified_repairs(&pool).await.unwrap();
    assert_eq!(imported_empty, 0);

    let hist_empty = fetch_repair_history(&pool, None, None).await.unwrap();
    assert_eq!(hist_empty.len(), 0);

    // 2. Insert verified downloads 918 & 919 in canonical state (completeness = 100)
    sqlx::query("INSERT INTO artists (id, name) VALUES (10, 'Radiohead')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (20, 'OK Computer')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (20, 10, 1)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (id, album_id, title) VALUES (50, 20, 'Airbag'), (43, 20, 'Paranoid Android')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO downloads (id, track_id, file_path, file_format, metadata_completeness) VALUES (918, 50, '/Music/Radiohead/1997 - OK Computer/01 - Airbag.flac', 'FLAC', 100), (919, 43, '/Music/Radiohead/1997 - OK Computer/02 - Paranoid Android.flac', 'FLAC', 100)").execute(&pool).await.unwrap();

    // 3. Import verified historical repairs
    let imported_count = import_historical_verified_repairs(&pool).await.unwrap();
    assert_eq!(imported_count, 2);

    let hist = fetch_repair_history(&pool, None, None).await.unwrap();
    assert_eq!(hist.len(), 2);

    let rec_918 = hist.iter().find(|r| r.download_id == Some(918)).unwrap();
    assert_eq!(rec_918.new_track_id, Some(50));
    assert_eq!(rec_918.result, "success");
    assert_eq!(rec_918.provenance, "historical_verified_import");

    let rec_919 = hist.iter().find(|r| r.download_id == Some(919)).unwrap();
    assert_eq!(rec_919.new_track_id, Some(43));
    assert_eq!(rec_919.result, "success");
    assert_eq!(rec_919.provenance, "historical_verified_import");

    // 4. Idempotency: re-running import doesn't duplicate
    let reimport_count = import_historical_verified_repairs(&pool).await.unwrap();
    assert_eq!(reimport_count, 0);
    let hist_after = fetch_repair_history(&pool, None, None).await.unwrap();
    assert_eq!(hist_after.len(), 2);
}

#[tokio::test]
async fn test_append_only_failure_records_never_overwrite_success() {
    let (pool, _temp) = create_test_db().await;

    // 1. Record a successful repair for download 918
    let rep_success_id = "rep_dl_918_success_1";
    record_applied_repair(
        &pool, rep_success_id, Some(918), Some(19495), Some(50),
        "/old/airbag.flac", "/new/airbag.flac", "sha_in_1", Some("sha_out_1"),
        Some("audio_payload_1"), Some("audio_payload_1"), "valid", &["tags_applied".to_string()],
        None, "tidal_pipeline.re_enrich", "success", None
    ).await.unwrap();

    // 2. Simulate subsequent failed repair attempt for the same download 918 (e.g. concurrent conflict or baseline failure)
    let rep_failed_id = "rep_dl_918_failed_2";
    record_applied_repair(
        &pool, rep_failed_id, Some(918), Some(19495), Some(50),
        "/old/airbag.flac", "/new/airbag.flac", "sha_in_1", None,
        Some("audio_payload_1"), None, "repair_input_changed", &["validated_baseline".to_string()],
        Some("RollbackExecuted"), "tidal_pipeline.re_enrich", "failed", None
    ).await.unwrap();

    // 3. Query all records for download 918
    let list = fetch_repair_history(&pool, None, None).await.unwrap();
    assert_eq!(list.len(), 2, "Both success and failed audit events must be preserved (append-only)");

    let success_rec = list.iter().find(|r| r.repair_id == rep_success_id).unwrap();
    assert_eq!(success_rec.result, "success");
    assert!(success_rec.output_file_hash.is_some());

    let failed_rec = list.iter().find(|r| r.repair_id == rep_failed_id).unwrap();
    assert_eq!(failed_rec.result, "failed");
    assert_eq!(failed_rec.rollback_state, Some("RollbackExecuted".to_string()));
}

