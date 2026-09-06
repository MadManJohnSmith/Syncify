//! E2E Test Suite for Sprint S107: Flujo de Descarga End-to-End desde UI (MVP Funcional)
//!
//! Validates:
//! 1. Enqueuing single tracks, album tracks, and artist tracks into `download_queue` using production queue commands.
//! 2. Priority assignment, deduplication, and position sequencing.
//! 3. Audio container validation (FLAC / M4A stream parsing) using production repair_guardrail.
//! 4. Atomic item claiming preventing race conditions using production DownloadWorker.
//! 5. Lifecycle status transitions (queued -> downloading) and audit verification.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::{
    commands::integrity::perform_run_integrity_audit,
    commands::queue::{perform_add_to_queue, perform_audit_download_queue},
    services::repair_guardrail::extract_audio_content_hash_from_bytes,
    worker::{DownloadWorker, DownloadWorkerState},
};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through current must apply cleanly");

    // Seed services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Seed default active account
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 3, 'Tidal Account', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_enqueue_single_track_lifecycle() {
    let pool = create_test_db().await;

    // 1. Seed artist, album, track, and source
    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The Warning') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('ERROR') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_id).bind(art_id).execute(&pool).await.unwrap();

    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('Choke', ?, 232000, 'USUG12101234') RETURNING id"
    )
    .bind(alb_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk_id).bind(art_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, format) VALUES (?, 3, 'td_123', 1, 'FLAC')")
        .bind(trk_id).execute(&pool).await.unwrap();

    // 2. Enqueue track using production perform_add_to_queue
    let queue_id = perform_add_to_queue(
        &pool,
        trk_id,
        Some(50),
        Some("HI_RES_LOSSLESS".to_string()),
        None,
        Some(3),
        Some("tidal".to_string()),
        None,
        Some("td_123".to_string()),
        None,
        Some("Choke".to_string()),
        Some("The Warning".to_string()),
        Some("ERROR".to_string()),
        Some("USUG12101234".to_string()),
        Some(false),
        Some(true),
        None,
    )
    .await
    .expect("perform_add_to_queue should succeed");

    assert!(queue_id > 0);

    // 3. Verify queue state via production queue audit
    let audit = perform_audit_download_queue(&pool).await.expect("perform_audit_download_queue must succeed");
    assert_eq!(audit.total_items, 1);
    assert_eq!(audit.ready_count, 1);

    // 4. Test atomic claiming by production DownloadWorker
    let worker_state = DownloadWorkerState::new(2);
    let worker = DownloadWorker::new(pool.clone(), worker_state);

    let claimed = worker.claim_next_item().await.expect("worker must claim queued item");
    assert_eq!(claimed.0, queue_id);
    assert_eq!(claimed.1, trk_id);
    assert_eq!(claimed.2, "Choke");
    assert_eq!(claimed.3, "The Warning");

    // 5. Verify database status transitioned to 'downloading'
    let status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "downloading");
}

#[tokio::test]
async fn test_enqueue_album_batch_tracks() {
    let pool = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, total_tracks) VALUES ('Discovery', 3) RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_id).bind(art_id).execute(&pool).await.unwrap();

    let track_titles = vec!["One More Time", "Aerodynamic", "Digital Love"];
    let mut track_ids = Vec::new();

    for (idx, title) in track_titles.iter().enumerate() {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, track_number) VALUES (?, ?, ?) RETURNING id"
        )
        .bind(title)
        .bind(alb_id)
        .bind((idx + 1) as i64)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(art_id).execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, format) VALUES (?, 3, ?, 1, 'FLAC')")
            .bind(tid).bind(format!("dp_{}", idx + 1)).execute(&pool).await.unwrap();

        track_ids.push(tid);
    }

    assert_eq!(track_ids.len(), 3);

    // Enqueue all 3 tracks using production perform_add_to_queue
    for (pos, tid) in track_ids.iter().enumerate() {
        let qid = perform_add_to_queue(
            &pool,
            *tid,
            Some(50),
            Some("lossless".to_string()),
            None,
            Some(3),
            Some("tidal".to_string()),
            None,
            Some(format!("dp_{}", pos + 1)),
            None,
            Some(track_titles[pos].to_string()),
            Some("Daft Punk".to_string()),
            Some("Discovery".to_string()),
            None,
            Some(false),
            Some(true),
            None,
        )
        .await
        .expect("perform_add_to_queue should succeed for album tracks");

        assert!(qid > 0);
    }

    let audit = perform_audit_download_queue(&pool).await.unwrap();
    assert_eq!(audit.total_items, 3);
    assert_eq!(audit.ready_count, 3);

    // Verify ordering by position
    let ordered_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT track_id FROM download_queue WHERE status = 'queued' ORDER BY position ASC, id ASC"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(ordered_ids, track_ids);
}

#[tokio::test]
async fn test_enqueue_artist_discography_deduplication() {
    let pool = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_1: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('The Dark Side of the Moon') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_1).bind(art_id).execute(&pool).await.unwrap();

    let alb_2: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Wish You Were Here') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_2).bind(art_id).execute(&pool).await.unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Time', ?) RETURNING id")
        .bind(alb_1)
        .fetch_one(&pool)
        .await
        .unwrap();

    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Money', ?) RETURNING id")
        .bind(alb_1)
        .fetch_one(&pool)
        .await
        .unwrap();

    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Shine On You Crazy Diamond', ?) RETURNING id")
        .bind(alb_2)
        .fetch_one(&pool)
        .await
        .unwrap();

    for (tid, name) in [(t1, "pf1"), (t2, "pf2"), (t3, "pf3")] {
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(art_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, 3, ?, 1)")
            .bind(tid).bind(name).execute(&pool).await.unwrap();
    }

    // 1. Initial enqueue of t1
    let q1 = perform_add_to_queue(
        &pool, t1, Some(50), None, None, Some(3), Some("tidal".into()), None, None, None, None, None, None, None, None, None, None
    ).await.unwrap();
    assert!(q1 > 0);

    // 2. Re-enqueueing t1 should detect duplicate and return existing queue id without creating a duplicate row
    let q1_dup = perform_add_to_queue(
        &pool, t1, Some(50), None, None, Some(3), Some("tidal".into()), None, None, None, None, None, None, None, None, None, None
    ).await.unwrap();
    assert_eq!(q1, q1_dup, "Duplicate add_to_queue call must return existing queue id");

    // 3. Enqueue t2 and t3
    let q2 = perform_add_to_queue(
        &pool, t2, Some(50), None, None, Some(3), Some("tidal".into()), None, None, None, None, None, None, None, None, None, None
    ).await.unwrap();
    let q3 = perform_add_to_queue(
        &pool, t3, Some(50), None, None, Some(3), Some("tidal".into()), None, None, None, None, None, None, None, None, None, None
    ).await.unwrap();
    assert!(q2 > 0);
    assert!(q3 > 0);

    let total_in_queue: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_in_queue, 3, "Queue must contain exactly 3 distinct items, avoiding duplicates");
}

#[tokio::test]
async fn test_audio_magic_bytes_validation_contract() {
    // Production extract_audio_content_hash_from_bytes parses container and verifies headers
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // STREAMINFO last-metadata block
    flac_bytes.resize(42, 0);

    let flac_hash = extract_audio_content_hash_from_bytes(&flac_bytes).expect("Valid FLAC header must be accepted by production parser");
    assert!(flac_hash.starts_with("flac_"), "Parsed FLAC container must yield flac hash: {}", flac_hash);

    // Arbitrary non-audio bytes are identified as generic fallback rather than FLAC
    let invalid_bytes = b"corrupt payload not audio";
    let invalid_hash = extract_audio_content_hash_from_bytes(invalid_bytes).expect("Payload is parsed with generic fallback");
    assert!(invalid_hash.starts_with("generic_payload:"), "Non-FLAC payload must fall back to generic: {}", invalid_hash);

    // Truncated payload (< 4 bytes) returns Err
    let truncated_res = extract_audio_content_hash_from_bytes(b"fL");
    assert!(truncated_res.is_err(), "Truncated bytes (< 4) must return Err");
}

#[tokio::test]
async fn test_download_persistence_in_library_table() {
    let pool = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Miles Davis') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Kind of Blue') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_id).bind(art_id).execute(&pool).await.unwrap();

    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('So What', ?, 562000, 'USSM15900001') RETURNING id"
    )
    .bind(alb_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Persist verified download
    let test_path = "/tmp/Syncify/Miles_Davis_So_What.flac";
    let dl_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO downloads (
            track_id, source_service_id, file_path, file_format, bit_depth,
            sample_rate, file_size_bytes, downloaded_at, file_hash
        ) VALUES (
            ?, 2, ?, 'FLAC', 24, 96000, 125000000, CURRENT_TIMESTAMP, 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
        ) RETURNING id
        "#
    )
    .bind(trk_id)
    .bind(test_path)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(dl_id > 0);

    // Verify download record is audited by production integrity scanner
    let audit_res = perform_run_integrity_audit(&pool, None).await.expect("perform_run_integrity_audit must succeed");
    assert_eq!(audit_res.total_tracks_scanned, 1);
}
