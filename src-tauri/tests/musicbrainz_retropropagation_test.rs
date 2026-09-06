//! TASK-84: MusicBrainz Retro-propagation, Corrupt WebP Sidecar Prevention, and Stuck Downloads Sanitization
//!
//! Test Suite verifies:
//! 1. Retro-propagation of physical VorbisComment `MUSICBRAINZ_TRACKID` to `tracks.musicbrainz_id` in SQLite.
//! 2. Rejection and prevention of 0-byte and sub-30-byte corrupt WebP cover sidecars.
//! 3. Deterministic timeout sanitization of downloads stuck in `downloading` state (> 1 hour) to `failed` and purging of `.part` staging files.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::download::qobuz::{is_valid_webp_sidecar, promote_webp_sidecars};
use syncify_tauri_lib::services::musicbrainz::{
    extract_musicbrainz_track_id_from_flac, reconcile_musicbrainz_from_physical_flacs,
    sync_flac_musicbrainz_id_to_track,
};
use syncify_tauri_lib::services::operation_recovery::sanitize_timed_out_downloads;
use tempfile::TempDir;

/// Create a minimal valid FLAC streaminfo container to allow atomic VorbisComment tagging
fn create_minimal_flac(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = File::create(path).expect("Create minimal FLAC file");
    let flac_header: [u8; 8] = [0x66, 0x4C, 0x61, 0x43, 0x80, 0x00, 0x00, 0x22];
    file.write_all(&flac_header).expect("Write header");
    let streaminfo = [0u8; 34];
    file.write_all(&streaminfo).expect("Write streaminfo");
    file.flush().expect("Flush minimal FLAC file");
}

#[tokio::test]
async fn test_musicbrainz_retropropagation_from_physical_flac() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test_mb_retroprop.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Seed a track with NULL musicbrainz_id
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, musicbrainz_id) VALUES ('Retro Track', 'USRC12345678', NULL) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let initial_mbid: Option<String> = sqlx::query_scalar("SELECT musicbrainz_id FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(initial_mbid.is_none(), "Track musicbrainz_id must start as NULL");

    // 2. Create physical FLAC file and write VorbisComment tags including MUSICBRAINZ_TRACKID
    let target_flac = temp.path().join("music").join("Retro Track.flac");
    create_minimal_flac(&target_flac);

    let expected_mbid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
    let meta = FlacMetadata {
        title: "Retro Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        isrc: Some("USRC12345678".to_string()),
        musicbrainz_track_id: Some(expected_mbid.to_string()),
        ..Default::default()
    };
    let tag_res = apply_and_verify_flac_tags(&target_flac, &meta);
    assert!(tag_res.is_ok(), "FLAC tagging failed: {:?}", tag_res.err());

    // 3. Test direct VorbisComment extraction
    let extracted = extract_musicbrainz_track_id_from_flac(&target_flac);
    assert_eq!(
        extracted.as_deref(),
        Some(expected_mbid),
        "Extracted VorbisComment MBID mismatch"
    );

    // 4. Test atomic sync function: sync_flac_musicbrainz_id_to_track
    let sync_res = sync_flac_musicbrainz_id_to_track(&pool, track_id, &target_flac)
        .await
        .expect("sync_flac_musicbrainz_id_to_track should succeed");
    assert_eq!(sync_res.as_deref(), Some(expected_mbid));

    let updated_mbid: Option<String> = sqlx::query_scalar("SELECT musicbrainz_id FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        updated_mbid.as_deref(),
        Some(expected_mbid),
        "tracks.musicbrainz_id must be updated in DB"
    );

    // 5. Test reconciliation report via directory walk
    let dir_report = reconcile_musicbrainz_from_physical_flacs(&pool, Some(&temp.path().join("music")))
        .await
        .expect("reconcile_musicbrainz_from_physical_flacs directory walk should succeed");
    assert_eq!(dir_report.scanned_files, 1);
    assert_eq!(dir_report.mbid_found_in_tags, 1);
    assert_eq!(dir_report.already_synchronized, 1, "Should report already synchronized since DB has MBID");
    assert_eq!(dir_report.db_updated, 0);

    // Reset DB to test updating via downloads ledger
    sqlx::query("UPDATE tracks SET musicbrainz_id = NULL WHERE id = ?")
        .bind(track_id)
        .execute(&pool)
        .await
        .unwrap();

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO downloads (track_id, source_service_id, file_path, file_format)
        VALUES (?, ?, ?, 'FLAC')
        "#
    )
    .bind(track_id)
    .bind(service_id)
    .bind(target_flac.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let ledger_report = reconcile_musicbrainz_from_physical_flacs(&pool, None)
        .await
        .expect("reconcile_musicbrainz_from_physical_flacs ledger mode should succeed");
    assert_eq!(ledger_report.scanned_files, 1);
    assert_eq!(ledger_report.mbid_found_in_tags, 1);
    assert_eq!(ledger_report.db_updated, 1, "Should update DB from downloads ledger");

    let final_mbid: Option<String> = sqlx::query_scalar("SELECT musicbrainz_id FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(final_mbid.as_deref(), Some(expected_mbid));
}

#[tokio::test]
async fn test_webp_sidecar_corrupt_and_zero_byte_rejection() {
    let temp = TempDir::new().unwrap();
    let staging_dir = temp.path().join(".staging");
    let target_dir = temp.path().join("album_dir");
    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();

    // 1. Test 0-byte WebP candidate
    let zero_byte_file = staging_dir.join("zero.cover.webp");
    File::create(&zero_byte_file).unwrap();
    assert_eq!(std::fs::metadata(&zero_byte_file).unwrap().len(), 0);

    assert!(!is_valid_webp_sidecar(&zero_byte_file));
    let zero_res = promote_webp_sidecars(&zero_byte_file, &target_dir).await;
    assert!(zero_res.is_ok());
    assert_eq!(zero_res.unwrap(), false, "0-byte WebP must be rejected and not promoted");
    assert!(!target_dir.join("cover.webp").exists(), "0-byte cover.webp must not be created");
    assert!(!zero_byte_file.exists(), "Rejected staging file must be removed");

    // 2. Test truncated 15-byte WebP candidate (< 30 bytes)
    let fifteen_byte_file = staging_dir.join("fifteen.cover.webp");
    let mut f15 = File::create(&fifteen_byte_file).unwrap();
    f15.write_all(b"RIFF\x00\x00\x00\x00WEBPVP8X").unwrap();
    f15.flush().unwrap();
    assert_eq!(std::fs::metadata(&fifteen_byte_file).unwrap().len(), 16);

    assert!(!is_valid_webp_sidecar(&fifteen_byte_file));
    let f15_res = promote_webp_sidecars(&fifteen_byte_file, &target_dir).await;
    assert!(f15_res.is_ok());
    assert_eq!(f15_res.unwrap(), false, "16-byte WebP (< 30) must be rejected");
    assert!(!target_dir.join("cover.webp").exists());

    // 3. Test 29-byte WebP candidate (boundary condition)
    let twenty_nine_byte_file = staging_dir.join("twentynine.cover.webp");
    let mut f29 = File::create(&twenty_nine_byte_file).unwrap();
    f29.write_all(&[0xAA; 29]).unwrap();
    f29.flush().unwrap();
    assert_eq!(std::fs::metadata(&twenty_nine_byte_file).unwrap().len(), 29);

    assert!(!is_valid_webp_sidecar(&twenty_nine_byte_file));
    let f29_res = promote_webp_sidecars(&twenty_nine_byte_file, &target_dir).await;
    assert!(f29_res.is_ok());
    assert_eq!(f29_res.unwrap(), false, "29-byte WebP (< 30) must be rejected");
    assert!(!target_dir.join("cover.webp").exists());

    // 4. Test valid WebP candidate (>= 30 bytes)
    let valid_file = staging_dir.join("valid.cover.webp");
    let mut f_valid = File::create(&valid_file).unwrap();
    // 34-byte buffer representing a valid-sized payload
    f_valid.write_all(&[0xBB; 34]).unwrap();
    f_valid.flush().unwrap();
    assert_eq!(std::fs::metadata(&valid_file).unwrap().len(), 34);

    assert!(is_valid_webp_sidecar(&valid_file));
    let valid_res = promote_webp_sidecars(&valid_file, &target_dir).await;
    assert!(valid_res.is_ok());
    assert_eq!(valid_res.unwrap(), true, "Valid WebP (>= 30 bytes) must be promoted");
    assert!(target_dir.join("cover.webp").exists(), "cover.webp sidecar must exist");
    assert!(target_dir.join("folder.webp").exists(), "folder.webp sidecar must exist");
    assert!(target_dir.join("animated.webp").exists(), "animated.webp sidecar must exist");
    assert_eq!(std::fs::metadata(target_dir.join("cover.webp")).unwrap().len(), 34);
}

#[tokio::test]
async fn test_stuck_downloads_timeout_sanitized_to_failed() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test_stuck_timeout.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let staging_dir = temp.path().join(".staging");
    std::fs::create_dir_all(&staging_dir).unwrap();

    // Seed dummy tracks
    let tid1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, duration_ms) VALUES ('Stuck Track 1', 200000) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let tid2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, duration_ms) VALUES ('Stuck Track 2', 210000) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let tid3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, duration_ms) VALUES ('Active In-Flight', 180000) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let tid4: i64 = sqlx::query_scalar("INSERT INTO tracks (title, duration_ms) VALUES ('Queued Track', 190000) RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // 1. Stuck item 101: started 2 hours ago (> 1 hour)
    let part1 = staging_dir.join("101.part");
    File::create(&part1).unwrap().write_all(b"PARTIAL_PAYLOAD_101").unwrap();
    assert!(part1.exists());

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, status, started_at, created_at, staging_path)
        VALUES (101, ?, 'downloading', datetime('now', '-2 hours'), datetime('now', '-2 hours'), ?)
        "#
    )
    .bind(tid1)
    .bind(part1.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 2. Stuck item 102: started_at NULL, but created 90 minutes ago (> 1 hour)
    let part2 = staging_dir.join("102.part");
    File::create(&part2).unwrap().write_all(b"PARTIAL_PAYLOAD_102").unwrap();
    assert!(part2.exists());

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, status, started_at, created_at, staging_path)
        VALUES (102, ?, 'downloading', NULL, datetime('now', '-90 minutes'), ?)
        "#
    )
    .bind(tid2)
    .bind(part2.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 3. Active in-flight item 103: started 5 minutes ago (< 1 hour)
    let part3 = staging_dir.join("103.part");
    File::create(&part3).unwrap().write_all(b"PARTIAL_PAYLOAD_103").unwrap();
    assert!(part3.exists());

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, status, started_at, created_at, staging_path)
        VALUES (103, ?, 'downloading', datetime('now', '-5 minutes'), datetime('now', '-5 minutes'), ?)
        "#
    )
    .bind(tid3)
    .bind(part3.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 4. Queued item 104
    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, status, started_at, created_at)
        VALUES (104, ?, 'queued', NULL, datetime('now', '-3 hours'))
        "#
    )
    .bind(tid4)
    .execute(&pool)
    .await
    .unwrap();

    // Execute sanitization
    let sanitized_count = sanitize_timed_out_downloads(&pool, Some(&staging_dir))
        .await
        .expect("sanitize_timed_out_downloads should succeed");

    assert_eq!(sanitized_count, 2, "Exactly 2 stuck items (> 1 hour) should be sanitized");

    // Check stuck item 101 -> failed and .part purged
    let (s1, err1): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM download_queue WHERE id = 101"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(s1, "failed");
    assert!(err1.unwrap_or_default().contains("timed out"));
    assert!(!part1.exists(), "Staging .part file for 101 must be purged");

    // Check stuck item 102 -> failed and .part purged
    let (s2, err2): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM download_queue WHERE id = 102"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(s2, "failed");
    assert!(err2.unwrap_or_default().contains("timed out"));
    assert!(!part2.exists(), "Staging .part file for 102 must be purged");

    // Check active item 103 -> preserved untouched
    let (s3, _): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM download_queue WHERE id = 103"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(s3, "downloading", "In-flight download < 1h must NOT be marked failed");
    assert!(part3.exists(), "In-flight download .part file must NOT be purged");

    // Check queued item 104 -> preserved untouched
    let (s4, _): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM download_queue WHERE id = 104"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(s4, "queued", "Queued item must remain queued");
}

#[tokio::test]
async fn test_worker_mark_complete_retropropagates_mbid() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test_worker_retroprop.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Seed track with NULL musicbrainz_id
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, musicbrainz_id) VALUES ('Worker MBID Track', 220000, NULL) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 2. Seed download_queue item
    let qid: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (track_id, status, priority, position)
        VALUES (?, 'downloading', 50, 1)
        RETURNING id
        "#
    )
    .bind(tid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 3. Create physical FLAC file with embedded MUSICBRAINZ_TRACKID
    let target_flac = temp.path().join("Worker MBID Track.flac");
    create_minimal_flac(&target_flac);

    let expected_mbid = "99887766-5544-3322-1100-aabbccddeeff";
    let meta = FlacMetadata {
        title: "Worker MBID Track".to_string(),
        artist: "Worker Artist".to_string(),
        album: "Worker Album".to_string(),
        musicbrainz_track_id: Some(expected_mbid.to_string()),
        ..Default::default()
    };
    apply_and_verify_flac_tags(&target_flac, &meta).expect("FLAC tagging must succeed");

    // 4. Instantiate DownloadWorker and invoke mark_complete
    let worker_state = syncify_tauri_lib::worker::DownloadWorkerState::new(2);
    let worker = syncify_tauri_lib::worker::DownloadWorker::new(pool.clone(), worker_state);

    let download_res = syncify_tauri_lib::download::DownloadResult {
        file_path: target_flac.to_string_lossy().to_string(),
        bit_depth: 16,
        sample_rate: 44100,
        title: "Worker MBID Track".to_string(),
        artist: "Worker Artist".to_string(),
        album: "Worker Album".to_string(),
        service: "qobuz".to_string(),
        ..Default::default()
    };

    worker.mark_complete(qid, &download_res).await;

    // 5. Verify tracks.musicbrainz_id was retro-propagated
    let updated_mbid: Option<String> = sqlx::query_scalar("SELECT musicbrainz_id FROM tracks WHERE id = ?")
        .bind(tid)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(
        updated_mbid.as_deref(),
        Some(expected_mbid),
        "Worker mark_complete must retro-propagate physical MUSICBRAINZ_TRACKID to tracks table"
    );
}
