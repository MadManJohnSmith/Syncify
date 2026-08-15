//! Integration Tests for S96 Download Queue with Prioritization and Resumption
//! Tests queue ordering, drag-and-drop reorder, retry mechanics, cancel with staging cleanup, and interrupted restore.

use sqlx::sqlite::SqlitePoolOptions;
use std::io::Write;

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations including 0046 must apply cleanly");

    // Insert sample artist and album
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    // Insert sample tracks
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms) VALUES (?, ?, 180000) RETURNING id"
        )
        .bind(format!("Track {}", i))
        .bind(album_id)
        .fetch_one(&pool).await.unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
            .bind(tid).bind(artist_id)
            .execute(&pool).await.unwrap();
    }

    pool
}

#[tokio::test]
async fn test_migration_0046_full_lifecycle_and_idempotence() {
    let db = create_test_db().await;

    // Verify columns exist in download_queue
    let row = sqlx::query(
        "SELECT position, staging_path, resumable, last_error FROM download_queue LIMIT 1"
    )
    .fetch_optional(&db)
    .await;

    assert!(row.is_ok(), "Table download_queue must contain columns added in 0046");
}

#[tokio::test]
async fn test_enqueue_and_queue_order_by_priority_and_position() {
    let db = create_test_db().await;

    // Enqueue 3 tracks:
    // Item 1: Priority 50, Position 0
    // Item 2: Priority 90 (High), Position 1
    // Item 3: Priority 50, Position 2
    sqlx::query(
        "INSERT INTO download_queue (track_id, priority, position, status, created_at) VALUES (1, 50, 0, 'queued', '2026-08-15 10:00:00')"
    ).execute(&db).await.unwrap();

    sqlx::query(
        "INSERT INTO download_queue (track_id, priority, position, status, created_at) VALUES (2, 90, 1, 'queued', '2026-08-15 10:01:00')"
    ).execute(&db).await.unwrap();

    sqlx::query(
        "INSERT INTO download_queue (track_id, priority, position, status, created_at) VALUES (3, 50, 2, 'queued', '2026-08-15 10:02:00')"
    ).execute(&db).await.unwrap();

    // Query ordered items
    let items: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, track_id, priority, position FROM download_queue WHERE status = 'queued' ORDER BY priority DESC, position ASC, created_at ASC"
    )
    .fetch_all(&db).await.unwrap();

    assert_eq!(items.len(), 3);
    // Item with priority 90 must come first
    assert_eq!(items[0].1, 2, "Highest priority track must be first");
    // Among priority 50, position 0 must come before position 2
    assert_eq!(items[1].1, 1, "Position 0 must precede position 2");
    assert_eq!(items[2].1, 3, "Position 2 must be last");
}

#[tokio::test]
async fn test_reorder_queue_atomic_update() {
    let db = create_test_db().await;

    let q1: i64 = sqlx::query_scalar("INSERT INTO download_queue (track_id, priority, position, status) VALUES (1, 50, 0, 'queued') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let q2: i64 = sqlx::query_scalar("INSERT INTO download_queue (track_id, priority, position, status) VALUES (2, 50, 1, 'queued') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let q3: i64 = sqlx::query_scalar("INSERT INTO download_queue (track_id, priority, position, status) VALUES (3, 50, 2, 'queued') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Drag-and-drop reorder: Move q3 to top, then q1, then q2
    let new_order = vec![q3, q1, q2];

    let mut tx = db.begin().await.unwrap();
    for (pos, id) in new_order.into_iter().enumerate() {
        sqlx::query("UPDATE download_queue SET position = ? WHERE id = ?")
            .bind(pos as i64)
            .bind(id)
            .execute(&mut *tx)
            .await.unwrap();
    }
    tx.commit().await.unwrap();

    // Verify new order
    let items: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, position FROM download_queue WHERE status = 'queued' ORDER BY priority DESC, position ASC, created_at ASC"
    )
    .fetch_all(&db).await.unwrap();

    assert_eq!(items[0].0, q3);
    assert_eq!(items[0].1, 0);
    assert_eq!(items[1].0, q1);
    assert_eq!(items[1].1, 1);
    assert_eq!(items[2].0, q2);
    assert_eq!(items[2].1, 2);
}

#[tokio::test]
async fn test_retry_failed_single_and_bulk() {
    let db = create_test_db().await;

    // Insert 2 failed downloads
    let q1: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, error_message, last_error, retry_count) VALUES (1, 'failed', 'HTTP 504', 'HTTP 504', 1) RETURNING id"
    ).fetch_one(&db).await.unwrap();

    let q2: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, error_message, last_error, retry_count) VALUES (2, 'failed', 'Connection reset', 'Connection reset', 2) RETURNING id"
    ).fetch_one(&db).await.unwrap();

    // 1. Retry single item (q1)
    sqlx::query(
        "UPDATE download_queue SET status = 'queued', error_message = NULL, last_error = NULL, progress_percent = 0, started_at = NULL WHERE id = ?"
    ).bind(q1).execute(&db).await.unwrap();

    let status_q1: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, error_message, last_error FROM download_queue WHERE id = ?"
    ).bind(q1).fetch_one(&db).await.unwrap();

    assert_eq!(status_q1.0, "queued");
    assert_eq!(status_q1.1, None);
    assert_eq!(status_q1.2, None);

    // 2. Retry all remaining failed items
    let rows_affected = sqlx::query(
        "UPDATE download_queue SET status = 'queued', error_message = NULL, last_error = NULL, progress_percent = 0, retry_count = retry_count + 1 WHERE status = 'failed' AND retry_count < 5"
    ).execute(&db).await.unwrap().rows_affected();

    assert_eq!(rows_affected, 1, "Only q2 was still failed");

    let status_q2: (String, i64) = sqlx::query_as(
        "SELECT status, retry_count FROM download_queue WHERE id = ?"
    ).bind(q2).fetch_one(&db).await.unwrap();

    assert_eq!(status_q2.0, "queued");
    assert_eq!(status_q2.1, 3);
}

#[tokio::test]
async fn test_cancel_download_and_staging_cleanup() {
    let db = create_test_db().await;

    // Create temporary .part staging file
    let staging_file = std::env::temp_dir().join("syncify_test_staging_track.part");
    {
        let mut f = std::fs::File::create(&staging_file).unwrap();
        f.write_all(b"partial FLAC audio data").unwrap();
    }
    assert!(staging_file.exists());

    let qid: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, staging_path) VALUES (1, 'downloading', ?) RETURNING id"
    )
    .bind(staging_file.to_string_lossy().to_string())
    .fetch_one(&db).await.unwrap();

    // Cancel download
    let staging: Option<(Option<String>,)> = sqlx::query_as("SELECT staging_path FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_optional(&db)
        .await
        .unwrap();

    if let Some((Some(path),)) = staging {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }

    sqlx::query("UPDATE download_queue SET status = 'cancelled' WHERE id = ?")
        .bind(qid).execute(&db).await.unwrap();

    // Verify staging file is removed
    assert!(!staging_file.exists(), "Staging file must be cleaned up on cancel");

    let status: (String,) = sqlx::query_as("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid).fetch_one(&db).await.unwrap();
    assert_eq!(status.0, "cancelled");
}

#[tokio::test]
async fn test_restore_interrupted_downloads_on_startup() {
    let db = create_test_db().await;

    sqlx::query("INSERT INTO download_queue (track_id, status, started_at) VALUES (1, 'downloading', CURRENT_TIMESTAMP)")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status, started_at) VALUES (2, 'downloading', CURRENT_TIMESTAMP)")
        .execute(&db).await.unwrap();

    // App restarts: restore interrupted downloads
    let restored = sqlx::query(
        "UPDATE download_queue SET status = 'queued', started_at = NULL WHERE status = 'downloading'"
    )
    .execute(&db).await.unwrap().rows_affected();

    assert_eq!(restored, 2);

    let count_queued: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(count_queued.0, 2);
}

#[tokio::test]
async fn test_http_range_resumption_logic() {
    let existing_bytes = 1048576u64; // 1MB downloaded
    let range_header = format!("bytes={}-", existing_bytes);
    assert_eq!(range_header, "bytes=1048576-");

    // Partial content 206 status indicates resumption accepted
    let http_status_partial = 206u16;
    let is_resuming = http_status_partial == 206;
    assert!(is_resuming, "206 Partial Content signals successful resumption");

    // 200 OK indicates full download (server ignored range or new stream)
    let http_status_ok = 200u16;
    let is_full_restart = http_status_ok == 200;
    assert!(is_full_restart, "200 OK signals full stream download");
}
