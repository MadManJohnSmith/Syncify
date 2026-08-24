//! Integration Test: Quality Preference Normalization & CHECK Constraint Safety
//!
//! Asserts that raw quality preference strings (HI_RES_LOSSLESS, LOSSLESS, HIGH, ANY, None,
//! and unknown garbage strings) are normalized safely to canonical SQLite CHECK constraint values
//! ('hires', 'lossless', 'high', 'any', or NULL) with zero exclusions.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{
    normalize_quality_preference, perform_add_to_queue, perform_enqueue_tracks,
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

    // Insert baseline service preferences
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority) VALUES ('qobuz', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority) VALUES ('tidal', 2)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_quality_preference_normalization_canonical_values() {
    let db = create_test_db().await;

    // Direct unit assertions on normalization function
    assert_eq!(normalize_quality_preference(Some("HI_RES_LOSSLESS")), Some("hires".to_string()));
    assert_eq!(normalize_quality_preference(Some("HI_RES")), Some("hires".to_string()));
    assert_eq!(normalize_quality_preference(Some("hires")), Some("hires".to_string()));
    assert_eq!(normalize_quality_preference(Some("LOSSLESS")), Some("lossless".to_string()));
    assert_eq!(normalize_quality_preference(Some("lossless")), Some("lossless".to_string()));
    assert_eq!(normalize_quality_preference(Some("HIGH")), Some("high".to_string()));
    assert_eq!(normalize_quality_preference(Some("high")), Some("high".to_string()));
    assert_eq!(normalize_quality_preference(Some("ANY")), Some("any".to_string()));
    assert_eq!(normalize_quality_preference(Some("any")), Some("any".to_string()));
    assert_eq!(normalize_quality_preference(None), None);
    assert_eq!(normalize_quality_preference(Some("")), None);
    assert_eq!(normalize_quality_preference(Some("UNKNOWN_GARBAGE_STRING")), None);

    // Test database insertion via perform_add_to_queue for various raw inputs
    let test_cases = vec![
        ("Track HiRes", "HI_RES_LOSSLESS", Some("hires")),
        ("Track Lossless", "LOSSLESS", Some("lossless")),
        ("Track High", "HIGH", Some("high")),
        ("Track Any", "ANY", Some("any")),
        ("Track None", "", None),
        ("Track Garbage", "INVALID_QUALITY_PARAM_123", None),
    ];

    for (idx, (title, raw_quality, expected_db_quality)) in test_cases.into_iter().enumerate() {
        let isrc_code = format!("US123456{:04}", idx);
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id"
        )
        .bind(title)
        .bind(isrc_code)
        .fetch_one(&db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 100, 1)"
        )
        .bind(tid)
        .bind(format!("q_{}", tid))
        .execute(&db)
        .await
        .unwrap();

        let qual_opt = if raw_quality.is_empty() { None } else { Some(raw_quality.to_string()) };

        let q_id = perform_add_to_queue(
            &db,
            tid,
            Some(50),
            qual_opt,
            None,
            Some(2),
            Some("qobuz".to_string()),
            None,
            Some(format!("q_{}", tid)),
            None,
            Some(title.to_string()),
            None,
            None,
            None,
            Some(false),
            Some(true),
            None,
        )
        .await
        .expect("perform_add_to_queue must succeed without CHECK constraint failure");

        let db_quality: Option<String> = sqlx::query_scalar(
            "SELECT quality_preference FROM download_queue WHERE id = ?"
        )
        .bind(q_id)
        .fetch_one(&db)
        .await
        .unwrap();

        assert_eq!(
            db_quality.as_deref(),
            expected_db_quality,
            "Quality for '{}' must match expected DB quality",
            title
        );
    }
}

#[tokio::test]
async fn test_100_tracks_with_hi_res_lossless_all_enqueued() {
    let db = create_test_db().await;

    let mut selected_track_ids = Vec::with_capacity(100);

    for i in 1..=100 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id"
        )
        .bind(format!("HiRes Batch Track {:03}", i))
        .bind(format!("USHIRES{:05}", i))
        .fetch_one(&db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 150, 1)"
        )
        .bind(tid)
        .bind(format!("q_hires_{:03}", i))
        .execute(&db)
        .await
        .unwrap();

        selected_track_ids.push(tid);
    }

    // Execute perform_enqueue_tracks with raw "HI_RES_LOSSLESS" quality preference
    let response = perform_enqueue_tracks(
        &db,
        selected_track_ids.clone(),
        Some(50),
        Some("HI_RES_LOSSLESS".to_string()),
        None,
        Some(false),
        Some(true),
        Some(true),
        Some(true),
    )
    .await
    .expect("perform_enqueue_tracks must succeed");

    assert_eq!(response.selected, 100, "100 tracks selected");
    assert_eq!(response.eligible, 100, "100 tracks eligible");
    assert_eq!(response.enqueued, 100, "All 100 tracks must be enqueued");
    assert_eq!(response.skipped, 0, "0 tracks skipped");
    assert!(response.excluded_preflight.is_empty(), "No preflight exclusions");

    let queued_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM download_queue WHERE status = 'queued' AND quality_preference = 'hires'"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(
        queued_count, 100,
        "Database must have 100 queued items with normalized 'hires' quality"
    );
}
