//! Synthetic Integration Test Suite for TASK-135:
//! Canonical Multi-Disc and Track Ordering in get_album
//!
//! Validates:
//! 1. Multi-disc albums have all Disc 1 tracks strictly before Disc 2 tracks.
//! 2. Tracks with track_number NULL are ordered at the end of their respective disc, sorted by title.
//! 3. Tracks with disc_number NULL are treated as Disc 1 via COALESCE(disc_number, 1).
//! 4. Interleaved database insertion order does not corrupt the canonical output order.
//! 5. Both `fetch_album` and Tauri command `get_album` return the exact expected order.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;
use syncify_tauri_lib::commands::{fetch_album, get_album};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::services::ConcurrencyManager;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tauri::Manager;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    pool
}

fn create_test_app_state(pool: SqlitePool) -> AppState {
    AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(ConcurrencyManager::new()),
    }
}

#[tokio::test]
async fn test_multidisc_album_canonical_order_e2e() {
    let pool = setup_test_db().await;

    // Create artist and album
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Symphony Orchestra') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('Double Concertos', '2023-05-12') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert tracks in intentionally randomized / interleaved order
    // Disc 2 tracks, Disc 1 tracks, and NULL track_numbers
    let tracks_to_insert = vec![
        // (title, disc_number, track_number)
        ("Disc 2 - Track 2 - Allegro", Some(2), Some(2)),
        ("Disc 1 - Track 2 - Adagio", Some(1), Some(2)),
        ("Disc 2 - Track 1 - Moderato", Some(2), Some(1)),
        ("Disc 1 - Track 1 - Prelude", Some(1), Some(1)),
        ("Disc 1 - Bonus Z", Some(1), None),
        ("Disc 1 - Bonus A", Some(1), None),
        ("Disc 2 - Bonus B", Some(2), None),
        ("Disc 2 - Bonus A", Some(2), None),
        ("Disc NULL (treated as Disc 1) - Track 3", None, Some(3)),
        ("Disc NULL (treated as Disc 1) - Bonus Middle", None, None),
    ];

    for (title, disc_num, track_num) in tracks_to_insert {
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, disc_number, track_number, duration_ms) VALUES (?, ?, ?, ?, 180000) RETURNING id"
        )
        .bind(title)
        .bind(album_id)
        .bind(disc_num)
        .bind(track_num)
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(track_id)
            .bind(artist_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    let app = tauri::test::mock_app();
    let state = create_test_app_state(pool.clone());
    app.manage(state);
    let app_state = app.state::<AppState>();

    // Test Tauri command get_album
    let album_detail = get_album(app_state, album_id).await.expect("get_album should succeed");

    assert_eq!(album_detail.title, "Double Concertos");
    assert_eq!(album_detail.track_count, 10);
    assert_eq!(album_detail.tracks.len(), 10);

    let titles: Vec<&str> = album_detail.tracks.iter().map(|t| t.title.as_str()).collect();

    // Canonical expected order:
    // Disc 1 (or COALESCE(NULL, 1) = 1):
    // - Track 1: Prelude
    // - Track 2: Adagio
    // - Track 3: Disc NULL - Track 3
    // - Track NULL (999), sorted by title ASC:
    //     - "Disc 1 - Bonus A"
    //     - "Disc 1 - Bonus Z"
    //     - "Disc NULL (treated as Disc 1) - Bonus Middle"
    // Disc 2:
    // - Track 1: Moderato
    // - Track 2: Allegro
    // - Track NULL (999), sorted by title ASC:
    //     - "Disc 2 - Bonus A"
    //     - "Disc 2 - Bonus B"
    let expected_titles = vec![
        "Disc 1 - Track 1 - Prelude",
        "Disc 1 - Track 2 - Adagio",
        "Disc NULL (treated as Disc 1) - Track 3",
        "Disc 1 - Bonus A",
        "Disc 1 - Bonus Z",
        "Disc NULL (treated as Disc 1) - Bonus Middle",
        "Disc 2 - Track 1 - Moderato",
        "Disc 2 - Track 2 - Allegro",
        "Disc 2 - Bonus A",
        "Disc 2 - Bonus B",
    ];

    assert_eq!(titles, expected_titles, "Pistas must follow canonical disc and track order");

    // Also assert disc_number field populated correctly
    assert_eq!(album_detail.tracks[0].disc_number, Some(1));
    assert_eq!(album_detail.tracks[0].track_number, Some(1));
    assert_eq!(album_detail.tracks[6].disc_number, Some(2));
    assert_eq!(album_detail.tracks[6].track_number, Some(1));
}

#[tokio::test]
async fn test_multidisc_interleaved_regression_prevented() {
    let pool = setup_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Wall of Sound (Deluxe)') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Insert identical track numbers across 2 discs interleaved
    // Disc 2 Track 1, Disc 1 Track 1, Disc 2 Track 2, Disc 1 Track 2, Disc 2 Track 3, Disc 1 Track 3
    let tracks = vec![
        ("D2T1", 2, 1),
        ("D1T1", 1, 1),
        ("D2T2", 2, 2),
        ("D1T2", 1, 2),
        ("D2T3", 2, 3),
        ("D1T3", 1, 3),
    ];

    for (title, disc_num, track_num) in tracks {
        sqlx::query("INSERT INTO tracks (title, album_id, disc_number, track_number) VALUES (?, ?, ?, ?)")
            .bind(title)
            .bind(album_id)
            .bind(disc_num)
            .bind(track_num)
            .execute(&pool)
            .await
            .unwrap();
    }

    let detail = fetch_album(&pool, album_id).await.expect("fetch_album should succeed");
    let titles: Vec<&str> = detail.tracks.iter().map(|t| t.title.as_str()).collect();

    // Must NOT be interleaved [D1T1, D2T1, D1T2, D2T2, D1T3, D2T3]
    // Must be strictly Disc 1 then Disc 2:
    assert_eq!(titles, vec!["D1T1", "D1T2", "D1T3", "D2T1", "D2T2", "D2T3"]);
}

#[tokio::test]
async fn test_tracks_with_all_null_track_numbers_ordered_by_title() {
    let pool = setup_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Unindexed Vinyl Rip') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // All track numbers are NULL
    let tracks = vec![
        "Zeta Movement",
        "Alpha Movement",
        "Gamma Movement",
        "Beta Movement",
    ];

    for title in tracks {
        sqlx::query("INSERT INTO tracks (title, album_id, disc_number, track_number) VALUES (?, ?, 1, NULL)")
            .bind(title)
            .bind(album_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    let detail = fetch_album(&pool, album_id).await.expect("fetch_album should succeed");
    let titles: Vec<&str> = detail.tracks.iter().map(|t| t.title.as_str()).collect();

    assert_eq!(titles, vec![
        "Alpha Movement",
        "Beta Movement",
        "Gamma Movement",
        "Zeta Movement",
    ]);
}
