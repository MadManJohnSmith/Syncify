//! Deterministic Concurrency Tests: Download, Repair, Queue, and Settings (Tests C, D, E, H, I)

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use syncify_core_domain::LockScope;
use syncify_tauri_lib::services::{get_global_concurrency_manager, ConcurrencyError};

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test SQLite");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            progress_percent REAL DEFAULT 0,
            retry_count INTEGER DEFAULT 0,
            error_message TEXT,
            last_error TEXT,
            started_at TEXT,
            completed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_size_bytes INTEGER,
            downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS sync_settings (
            id INTEGER PRIMARY KEY,
            max_concurrent_downloads INTEGER DEFAULT 2,
            quality_preference TEXT DEFAULT 'LOSSLESS'
        );
        INSERT OR IGNORE INTO sync_settings (id, max_concurrent_downloads, quality_preference)
        VALUES (1, 2, 'LOSSLESS');
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create base tables");

    pool
}

/// Test C: Two downloads of same track are mutually exclusive and prevent duplicate file writes
#[tokio::test]
async fn test_concurrency_c_two_downloads_same_track_serialized() {
    let mgr = get_global_concurrency_manager();
    let track_id = 501;

    let dl1 = mgr
        .acquire(
            LockScope::Download(track_id),
            Some("dl-worker-1"),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("First worker should acquire Download lock");

    // Second download worker attempting the same track
    let dl2_res = mgr
        .acquire(
            LockScope::Download(track_id),
            Some("dl-worker-2"),
            Some(Duration::from_millis(50)),
        )
        .await;

    assert!(
        matches!(dl2_res, Err(ConcurrencyError::Timeout { .. })),
        "Second download on same track must time out due to mutual exclusion"
    );

    drop(dl1);
    tokio::time::sleep(Duration::from_millis(15)).await;

    let dl3 = mgr
        .acquire(
            LockScope::Download(track_id),
            Some("dl-worker-3"),
            Some(Duration::from_millis(100)),
        )
        .await
        .expect("Subsequent download should acquire lock once previous released");

    assert_eq!(dl3.operation_id, "dl-worker-3");
}

/// Test D: Download and repair on the same track are mutually exclusive
#[tokio::test]
async fn test_concurrency_d_download_and_repair_mutual_exclusion() {
    let mgr = get_global_concurrency_manager();
    let track_id = 701;

    // Start download
    let dl_guard = mgr
        .acquire(
            LockScope::Download(track_id),
            Some("dl-active"),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("Acquire Download lock");

    // Concurrent repair should be blocked
    let repair_res = mgr
        .acquire(
            LockScope::Repair(track_id),
            Some("repair-active"),
            Some(Duration::from_millis(50)),
        )
        .await;

    assert!(
        matches!(repair_res, Err(ConcurrencyError::Timeout { .. })),
        "Repair must not run concurrently with active download of the same track"
    );

    drop(dl_guard);
    tokio::time::sleep(Duration::from_millis(15)).await;

    let repair_guard = mgr
        .acquire(
            LockScope::Repair(track_id),
            Some("repair-now-safe"),
            Some(Duration::from_millis(100)),
        )
        .await
        .expect("Repair should succeed after download finishes");

    assert_eq!(repair_guard.operation_id, "repair-now-safe");
}

/// Test E: Filesystem path lock prevents concurrent promotion or tag writes on the exact same physical destination
#[tokio::test]
async fn test_concurrency_e_filesystem_path_lock_prevents_file_collision() {
    let mgr = get_global_concurrency_manager();
    let path = "C:/Music/Artist/Album/01 - Track.flac".to_string();

    let fs_guard1 = mgr
        .acquire(
            LockScope::FilesystemPath(path.clone()),
            Some("promote-file"),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("First operation must acquire FilesystemPath lock");

    // Concurrent tagging or reconciliation on exact same path
    let fs_guard2_res = mgr
        .acquire(
            LockScope::FilesystemPath(path.clone()),
            Some("reconcile-file"),
            Some(Duration::from_millis(50)),
        )
        .await;

    assert!(
        matches!(fs_guard2_res, Err(ConcurrencyError::Timeout { .. })),
        "Colliding operation on same destination path must be excluded"
    );

    drop(fs_guard1);
    tokio::time::sleep(Duration::from_millis(15)).await;

    let fs_guard3 = mgr
        .acquire(
            LockScope::FilesystemPath(path),
            Some("reconcile-file-safe"),
            Some(Duration::from_millis(100)),
        )
        .await
        .expect("Path lock available after release");

    assert_eq!(fs_guard3.operation_id, "reconcile-file-safe");
}

/// Test H: Queue retry and cancel transitions do not leave orphaned intermediate states
#[tokio::test]
async fn test_concurrency_h_queue_retry_and_cancel_race() {
    let db = setup_test_db().await;

    // Seed failed queue item
    let queue_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, error_message, retry_count) VALUES (1, 'failed', 'NetworkTimeout', 1) RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let h_retry = {
        let db = db.clone();
        tokio::spawn(async move {
            sqlx::query(
                "UPDATE download_queue SET status = 'queued', error_message = NULL, retry_count = retry_count + 1 WHERE id = ? AND status = 'failed'"
            )
            .bind(queue_id)
            .execute(&db)
            .await
            .unwrap()
        })
    };

    let h_cancel = {
        let db = db.clone();
        tokio::spawn(async move {
            sqlx::query(
                "UPDATE download_queue SET status = 'cancelled' WHERE id = ? AND status IN ('queued', 'downloading')"
            )
            .bind(queue_id)
            .execute(&db)
            .await
            .unwrap()
        })
    };

    let (r1, r2) = tokio::join!(h_retry, h_cancel);
    r1.unwrap();
    r2.unwrap();

    let final_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert!(
        final_status == "queued" || final_status == "cancelled",
        "Final status must be deterministically queued or cancelled, got: {}",
        final_status
    );
}

/// Test I: Settings snapshot read during preflight is coherent even during settings write
#[tokio::test]
async fn test_concurrency_i_settings_atomic_snapshot_during_preflight() {
    let db = setup_test_db().await;
    let mgr = get_global_concurrency_manager();

    let stop_signal = Arc::new(AtomicBool::new(false));

    // Writer task updating settings repeatedly
    let h_writer = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        let stop = Arc::clone(&stop_signal);
        tokio::spawn(async move {
            let mut i = 0;
            while !stop.load(Ordering::Relaxed) {
                let quality = if i % 2 == 0 { "HI_RES" } else { "LOSSLESS" };
                let concurrency = if i % 2 == 0 { 4 } else { 2 };

                let _guard = mgr
                    .acquire(LockScope::Settings, Some("settings-update"), None)
                    .await
                    .unwrap();

                sqlx::query("UPDATE sync_settings SET max_concurrent_downloads = ?, quality_preference = ? WHERE id = 1")
                    .bind(concurrency)
                    .bind(quality)
                    .execute(&db)
                    .await
                    .unwrap();

                i += 1;
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
    };

    // Reader task performing preflight snapshot reads
    let h_reader = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            for _ in 0..25 {
                let _guard = mgr
                    .acquire(LockScope::Settings, Some("preflight-read"), None)
                    .await
                    .unwrap();

                let (concurrency, quality): (i64, String) = sqlx::query_as(
                    "SELECT max_concurrent_downloads, quality_preference FROM sync_settings WHERE id = 1"
                )
                .fetch_one(&db)
                .await
                .unwrap();

                // Ensure coherent pairs (2, LOSSLESS) or (4, HI_RES)
                if concurrency == 2 {
                    assert_eq!(quality, "LOSSLESS");
                } else if concurrency == 4 {
                    assert_eq!(quality, "HI_RES");
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
    };

    h_reader.await.unwrap();
    stop_signal.store(true, Ordering::Relaxed);
    h_writer.await.unwrap();
}
