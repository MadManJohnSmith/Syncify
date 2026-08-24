//! Integration Test: S176Q - Dual Provider No Exclusion Test Suite
//!
//! Asserts that:
//! 1. Tracks with simultaneous active sources on Qobuz and Tidal are NEVER excluded as AmbiguousSource.
//! 2. Dual-provider tracks are classified as ReadyExactSource with is_eligible = true.
//! 3. Provider decision is resolved via service_preferences (or quality) without blocking preflight or enqueueing.
//! 4. 100% of dual-provider tracks selected are enqueued successfully.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{evaluate_track_preflight, perform_enqueue_tracks, DownloadPreflightStatus};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Insert baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Insert active accounts for both Qobuz and Tidal
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz Active', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal Active', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    // Set Qobuz as priority 1, Tidal as priority 2
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority) VALUES ('qobuz', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority) VALUES ('tidal', 2)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_dual_provider_tracks_never_excluded_and_enqueued_cleanly() {
    let db = create_test_db().await;

    let mut track_ids = Vec::new();

    // Setup 10 dual-provider tracks
    for i in 1..=10 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id"
        )
        .bind(format!("Dual Provider Song {:02}", i))
        .bind(format!("USDUAL99{:04}", i))
        .fetch_one(&db)
        .await
        .unwrap();

        // Qobuz candidate: 24-bit Hi-Res
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 150, 1)"
        )
        .bind(tid)
        .bind(format!("q_dual_track_{:02}", i))
        .execute(&db)
        .await
        .unwrap();

        // Tidal candidate: 16-bit Lossless
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'FLAC', 16, 44100, 100, 1)"
        )
        .bind(tid)
        .bind(format!("t_dual_track_{:02}", i))
        .execute(&db)
        .await
        .unwrap();

        track_ids.push(tid);
    }

    // 1. Verify preflight on each dual-provider track
    for &tid in &track_ids {
        let pf = evaluate_track_preflight(&db, tid, None, None, false, true)
            .await
            .expect("Preflight evaluation must succeed");

        assert_eq!(pf.status, DownloadPreflightStatus::ReadyExactSource);
        assert!(pf.is_eligible, "Dual provider track must be eligible for download");
        assert_eq!(pf.resolved_service_name.as_deref(), Some("qobuz"), "Preferred provider must be chosen");
    }

    // 2. Perform Enqueue of all 10 dual-provider tracks
    let enq_res = perform_enqueue_tracks(
        &db,
        track_ids.clone(),
        Some(10),
        Some("hires".to_string()),
        None,
        Some(false),
        Some(true),
        Some(true),
        Some(true),
    )
    .await
    .expect("perform_enqueue_tracks must succeed");

    assert_eq!(enq_res.selected, 10);
    assert_eq!(enq_res.eligible, 10);
    assert_eq!(enq_res.enqueued, 10);
    assert_eq!(enq_res.skipped, 0);
    assert!(enq_res.excluded_preflight.is_empty());

    // 3. Verify all 10 items exist in download_queue with Qobuz service chosen
    let queue_items: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT s.name, dq.service_track_id
        FROM download_queue dq
        JOIN services s ON s.id = dq.service_id
        ORDER BY dq.id ASC
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(queue_items.len(), 10);
    for (idx, item) in queue_items.iter().enumerate() {
        assert_eq!(item.0, "qobuz");
        assert_eq!(item.1, format!("q_dual_track_{:02}", idx + 1));
    }
}
