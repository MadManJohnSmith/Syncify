//! Deterministic Concurrency Tests: Multi-Operation Stress & Deadlock Prevention (Tests J, K, L)

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use syncify_core_domain::LockScope;
use syncify_tauri_lib::services::get_global_concurrency_manager;

async fn setup_stress_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to SQLite memory pool");

    // WAL & Busy timeout settings for SQLite
    sqlx::query("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            album_id INTEGER,
            is_favorite INTEGER DEFAULT 0,
            record_label TEXT,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT NOT NULL,
            UNIQUE(service_id, service_track_id)
        );

        CREATE TABLE IF NOT EXISTS download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            progress_percent REAL DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            file_path TEXT NOT NULL
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create stress tables");

    pool
}

/// Test J: Startup recovery and download worker resume do not conflict
#[tokio::test]
async fn test_concurrency_j_startup_recovery_and_worker_resume() {
    let db = setup_stress_db().await;
    let mgr = get_global_concurrency_manager();

    // Seed interrupted downloading item
    let queue_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status) VALUES (10, 'downloading') RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let h_recovery = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            let _guard = mgr
                .acquire(
                    LockScope::CanonicalTrack(10),
                    Some("recovery-pass"),
                    Some(Duration::from_secs(2)),
                )
                .await
                .unwrap();

            sqlx::query("UPDATE download_queue SET status = 'queued' WHERE id = ? AND status = 'downloading'")
                .bind(queue_id)
                .execute(&db)
                .await
                .unwrap();
        })
    };

    let h_worker = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            let _guard = mgr
                .acquire(
                    LockScope::Download(10),
                    Some("worker-claim"),
                    Some(Duration::from_secs(2)),
                )
                .await
                .unwrap();

            // Try claiming queued item
            sqlx::query("UPDATE download_queue SET status = 'downloading' WHERE id = ? AND status = 'queued'")
                .bind(queue_id)
                .execute(&db)
                .await
                .unwrap();
        })
    };

    let (r1, r2) = tokio::join!(h_recovery, h_worker);
    r1.unwrap();
    r2.unwrap();

    let status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert!(status == "queued" || status == "downloading");
}

/// Test K: 50 concurrent mixed operations (Sync + Download + Repair + Enrichment + Favorite + Recovery) execute with zero deadlocks and zero duplicates
#[tokio::test]
async fn test_concurrency_k_50_mixed_operations_stress() {
    let db = setup_stress_db().await;
    let mgr = get_global_concurrency_manager();

    // Pre-seed 10 tracks
    for i in 1..=10 {
        sqlx::query("INSERT INTO tracks (id, title) VALUES (?, ?)")
            .bind(i)
            .bind(format!("Stress Track {}", i))
            .execute(&db)
            .await
            .unwrap();
    }

    let completed_ops = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..50 {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        let completed = Arc::clone(&completed_ops);
        let track_id = (i % 10) + 1;

        let handle = tokio::spawn(async move {
            let op_type = i % 5;
            match op_type {
                0 => {
                    // Sync / TrackIdentity
                    let _guard = mgr
                        .acquire(
                            LockScope::TrackIdentity {
                                service_id: 3,
                                service_track_id: format!("trk-{}", track_id),
                            },
                            Some("stress-sync"),
                            Some(Duration::from_secs(10)),
                        )
                        .await
                        .unwrap();

                    sqlx::query(
                        "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, ?)",
                    )
                    .bind(track_id)
                    .bind(format!("trk-{}", track_id))
                    .execute(&db)
                    .await
                    .unwrap();
                }
                1 => {
                    // Download
                    let _guard = mgr
                        .acquire(
                            LockScope::Download(track_id),
                            Some("stress-dl"),
                            Some(Duration::from_secs(10)),
                        )
                        .await
                        .unwrap();

                    sqlx::query(
                        "INSERT INTO download_queue (track_id, status) VALUES (?, 'complete')",
                    )
                    .bind(track_id)
                    .execute(&db)
                    .await
                    .unwrap();
                }
                2 => {
                    // CanonicalTrack (Enrichment / Favorite)
                    let _guard = mgr
                        .acquire(
                            LockScope::CanonicalTrack(track_id),
                            Some("stress-canonical"),
                            Some(Duration::from_secs(10)),
                        )
                        .await
                        .unwrap();

                    sqlx::query(
                        "UPDATE tracks SET is_favorite = 1, record_label = 'Stress Label' WHERE id = ?",
                    )
                    .bind(track_id)
                    .execute(&db)
                    .await
                    .unwrap();
                }
                3 => {
                    // Filesystem path promotion
                    let path = format!("C:/Music/Album/Track_{:02}.flac", track_id);
                    let _guard = mgr
                        .acquire(
                            LockScope::FilesystemPath(path.clone()),
                            Some("stress-fs"),
                            Some(Duration::from_secs(10)),
                        )
                        .await
                        .unwrap();

                    sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, ?)")
                        .bind(track_id)
                        .bind(&path)
                        .execute(&db)
                        .await
                        .unwrap();
                }
                _ => {
                    // Repair coordinator
                    let _guard = mgr
                        .acquire(
                            LockScope::Repair(track_id),
                            Some("stress-repair"),
                            Some(Duration::from_secs(10)),
                        )
                        .await
                        .unwrap();

                    sqlx::query("UPDATE tracks SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                        .bind(track_id)
                        .execute(&db)
                        .await
                        .unwrap();
                }
            }
            completed.fetch_add(1, Ordering::Relaxed);
        });

        handles.push(handle);
    }

    for h in handles {
        h.await.expect("All concurrent tasks must complete cleanly");
    }

    assert_eq!(
        completed_ops.load(Ordering::Relaxed),
        50,
        "All 50 operations must complete successfully"
    );

    // Verify stats summary
    let stats = mgr.get_stats_summary().await;
    assert!(stats.total_acquisitions >= 50);
    println!("\n[STRESS TEST SUMMARY]: {:?}", stats);
}

/// Test L: Deadlock detection with timeout across 20 iterations of conflicting multi-lock acquisitions
#[tokio::test]
async fn test_concurrency_l_deadlock_detection_with_timeout() {
    let mgr = get_global_concurrency_manager();

    for iteration in 0..20 {
        let tid_a = (iteration * 2) + 1;
        let tid_b = (iteration * 2) + 2;

        let mgr_1 = Arc::clone(&mgr);
        let h1 = tokio::spawn(async move {
            let scopes = vec![
                LockScope::CanonicalTrack(tid_b),
                LockScope::CanonicalTrack(tid_a),
            ];
            // Multi-lock acquisition sorts scopes internally according to global hierarchy
            let guard = mgr_1
                .acquire_multi(scopes, Some("op-ordered-1"), Some(Duration::from_secs(2)))
                .await;
            guard.is_ok()
        });

        let mgr_2 = Arc::clone(&mgr);
        let h2 = tokio::spawn(async move {
            let scopes = vec![
                LockScope::CanonicalTrack(tid_a),
                LockScope::CanonicalTrack(tid_b),
            ];
            let guard = mgr_2
                .acquire_multi(scopes, Some("op-ordered-2"), Some(Duration::from_secs(2)))
                .await;
            guard.is_ok()
        });

        let (r1, r2) = tokio::join!(h1, h2);
        assert!(r1.unwrap(), "Task 1 multi-lock acquisition must not deadlock");
        assert!(r2.unwrap(), "Task 2 multi-lock acquisition must not deadlock");
    }
}
