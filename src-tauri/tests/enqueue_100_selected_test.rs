//! Integration Test: S176Q - 100 Selected -> 100 Enqueued Test Suite
//!
//! Asserts that selecting 100 tracks in Library results in exactly 100 items enqueued
//! in `download_queue` with status 'queued' and zero silent exclusions.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{perform_enqueue_tracks, perform_reconcile_queue};

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

    // Insert baseline accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    // Insert baseline service preferences (Qobuz = 1, Tidal = 2)
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority) VALUES ('qobuz', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority) VALUES ('tidal', 2)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_enqueue_100_selected_tracks_all_enqueued() {
    let db = create_test_db().await;

    let mut selected_track_ids = Vec::with_capacity(100);

    // 1. Setup 50 Qobuz tracks
    for i in 1..=50 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id"
        )
        .bind(format!("Qobuz Track {:03}", i))
        .bind(format!("USQOB100{:04}", i))
        .fetch_one(&db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 150, 1)"
        )
        .bind(tid)
        .bind(format!("qobuz_tr_{:03}", i))
        .execute(&db)
        .await
        .unwrap();

        selected_track_ids.push(tid);
    }

    // 2. Setup 30 Tidal tracks
    for i in 1..=30 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id"
        )
        .bind(format!("Tidal Track {:03}", i))
        .bind(format!("USTID100{:04}", i))
        .fetch_one(&db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'FLAC', 16, 44100, 100, 1)"
        )
        .bind(tid)
        .bind(format!("tidal_tr_{:03}", i))
        .execute(&db)
        .await
        .unwrap();

        selected_track_ids.push(tid);
    }

    // 3. Setup 20 Dual-Provider tracks (available in BOTH Qobuz & Tidal simultaneously)
    for i in 1..=20 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id"
        )
        .bind(format!("Dual Provider Track {:03}", i))
        .bind(format!("USDUAL10{:04}", i))
        .fetch_one(&db)
        .await
        .unwrap();

        // Qobuz source
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 150, 1)"
        )
        .bind(tid)
        .bind(format!("qobuz_dual_{:03}", i))
        .execute(&db)
        .await
        .unwrap();

        // Tidal source
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'FLAC', 16, 44100, 100, 1)"
        )
        .bind(tid)
        .bind(format!("tidal_dual_{:03}", i))
        .execute(&db)
        .await
        .unwrap();

        selected_track_ids.push(tid);
    }

    assert_eq!(selected_track_ids.len(), 100, "Must have exactly 100 selected tracks");

    // Perform Enqueue Operation (S176Q)
    let response = perform_enqueue_tracks(
        &db,
        selected_track_ids.clone(),
        Some(50),
        Some("lossless".to_string()),
        None,
        Some(false),
        Some(true),
        Some(true),
        Some(true),
    )
    .await
    .expect("perform_enqueue_tracks must succeed");

    // Assert 100 Selected -> 100 Enqueued
    assert_eq!(response.selected, 100, "Total selected must be 100");
    assert_eq!(response.eligible, 100, "All 100 valid tracks must be eligible");
    assert_eq!(response.enqueued, 100, "Exactly 100 tracks must be enqueued into download_queue");
    assert_eq!(response.skipped, 0, "No tracks should be skipped");
    assert_eq!(response.deduplicated, 0, "No tracks should be deduplicated");
    assert!(response.excluded_preflight.is_empty(), "Zero silent exclusions");

    // Verify DB state
    let queued_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(queued_count, 100, "Database must contain exactly 100 queued items");

    // Verify Reconciliation Report
    let recon_report = perform_reconcile_queue(&db, Some(selected_track_ids))
        .await
        .expect("perform_reconcile_queue must succeed");

    assert_eq!(recon_report.selected, 100);
    assert_eq!(recon_report.eligible, 100);
    assert_eq!(recon_report.excluded_preflight, 0);
    assert_eq!(recon_report.pending, 100);
    assert_eq!(recon_report.active, 0);
    assert_eq!(recon_report.completed, 0);
    assert_eq!(recon_report.failed, 0);
}
