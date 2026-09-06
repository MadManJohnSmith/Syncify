//! TASK-80: Integration tests for preserving duplicate tracks in local playlist queries.
//!
//! Verifies:
//! 1. When a playlist contains the same track in multiple positions (e.g. pos 1 and pos 5),
//!    `fetch_local_playlist_tracks_page` does NOT collapse them via `GROUP BY t.id`.
//! 2. Both occurrences are returned with their distinct positions preserved and sorted by `pt.position ASC`.
//! 3. `get_local_playlist_tracks` and `get_playlist_tracks` report `total == 2` and `has_more` correctly.
//! 4. Pagination (offset / limit) works accurately across duplicate occurrences.
//! 5. Multiple `track_sources` for a duplicate track aggregate cleanly without creating cartesian artifacts.

use std::sync::Arc;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tauri::Manager;
use syncify_tauri_lib::{
    commands::{fetch_local_playlist_tracks_page, get_local_playlist_tracks, get_playlist_tracks},
    enrichment_worker::EnrichmentWorkerState,
    services::ConcurrencyManager,
    worker::DownloadWorkerState,
    AppState,
};

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Insert standard services and a test account for playlist foreign key integrity
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'tidal'), (3, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO accounts (id, service_id, email) VALUES (1, 1, 'tester@example.com')")
        .execute(&pool)
        .await
        .unwrap();

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
async fn test_duplicate_tracks_preservation_in_playlist_query() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Discovery') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Insert track 100 ("One More Time") and track 200 ("Aerodynamic")
    let track_id_100: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, album_id, duration_ms, track_number) VALUES (100, 'One More Time', ?, 320000, 1) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let track_id_200: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, album_id, duration_ms, track_number) VALUES (200, 'Aerodynamic', ?, 212000, 2) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary'), (?, ?, 'primary')")
        .bind(track_id_100)
        .bind(artist_id)
        .bind(track_id_200)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // Create playlist
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'daft_punk_pl', 'Daft Punk Loop', 3) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert track 100 at position 1, track 200 at position 3, and track 100 AGAIN at position 5
    sqlx::query(
        r#"
        INSERT INTO playlist_tracks (playlist_id, track_id, position)
        VALUES
            (?, ?, 1),
            (?, ?, 3),
            (?, ?, 5);
        "#
    )
    .bind(playlist_id)
    .bind(track_id_100)
    .bind(playlist_id)
    .bind(track_id_200)
    .bind(playlist_id)
    .bind(track_id_100)
    .execute(&pool)
    .await
    .unwrap();

    // Query via fetch_local_playlist_tracks_page directly
    let raw_tracks = fetch_local_playlist_tracks_page(&pool, playlist_id, 0, 100)
        .await
        .expect("fetch_local_playlist_tracks_page should succeed");

    assert_eq!(
        raw_tracks.len(),
        3,
        "All 3 entries must be returned, including duplicate track_id 100 at distinct positions"
    );

    assert_eq!(raw_tracks[0].id, track_id_100);
    assert_eq!(raw_tracks[0].position, Some(1));
    assert_eq!(raw_tracks[0].title, "One More Time");

    assert_eq!(raw_tracks[1].id, track_id_200);
    assert_eq!(raw_tracks[1].position, Some(3));
    assert_eq!(raw_tracks[1].title, "Aerodynamic");

    assert_eq!(raw_tracks[2].id, track_id_100);
    assert_eq!(raw_tracks[2].position, Some(5));
    assert_eq!(raw_tracks[2].title, "One More Time");

    // Query via get_local_playlist_tracks Tauri command
    let app = tauri::test::mock_app();
    let state = create_test_app_state(pool.clone());
    app.manage(state);
    let app_state = app.state::<AppState>();

    let page = get_local_playlist_tracks(app_state, playlist_id, Some(0), Some(100))
        .await
        .expect("get_local_playlist_tracks should succeed");

    assert_eq!(page.total, 3, "Total tracks in playlist must be 3");
    assert_eq!(page.tracks.len(), 3, "Page tracks count must be 3");
    assert!(!page.has_more, "has_more must be false when all tracks are fetched");

    assert_eq!(page.tracks[0].id, track_id_100);
    assert_eq!(page.tracks[0].position, Some(1));

    assert_eq!(page.tracks[1].id, track_id_200);
    assert_eq!(page.tracks[1].position, Some(3));

    assert_eq!(page.tracks[2].id, track_id_100);
    assert_eq!(page.tracks[2].position, Some(5));
}

#[tokio::test]
async fn test_duplicate_tracks_pagination() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The Beatles') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Abbey Road') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, album_id, duration_ms, track_number) VALUES (300, 'Come Together', ?, 259000, 1) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'beatles_pl', 'Repeat Playlist', 2) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert the same track at position 1 and position 5
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 5)"
    )
    .bind(playlist_id)
    .bind(track_id)
    .bind(playlist_id)
    .bind(track_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = tauri::test::mock_app();
    let state = create_test_app_state(pool.clone());
    app.manage(state);
    let app_state = app.state::<AppState>();

    // Page 1: limit 1, offset 0 -> position 1
    let page1 = get_playlist_tracks(app_state.clone(), playlist_id, Some(0), Some(1))
        .await
        .expect("Page 1 should succeed");

    assert_eq!(page1.total, 2);
    assert_eq!(page1.tracks.len(), 1);
    assert_eq!(page1.tracks[0].id, track_id);
    assert_eq!(page1.tracks[0].position, Some(1));
    assert!(page1.has_more, "has_more must be true after page 1 (1 of 2)");

    // Page 2: limit 1, offset 1 -> position 5
    let page2 = get_playlist_tracks(app_state.clone(), playlist_id, Some(1), Some(1))
        .await
        .expect("Page 2 should succeed");

    assert_eq!(page2.total, 2);
    assert_eq!(page2.tracks.len(), 1);
    assert_eq!(page2.tracks[0].id, track_id);
    assert_eq!(page2.tracks[0].position, Some(5));
    assert!(!page2.has_more, "has_more must be false after page 2 (2 of 2)");
}

#[tokio::test]
async fn test_duplicate_tracks_with_multiple_sources_not_cartesian_multiplied() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Queen') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('A Night at the Opera') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, album_id, duration_ms, track_number) VALUES (400, 'Bohemian Rhapsody', ?, 354000, 11) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // Add 3 track_sources for track 400
    sqlx::query(
        r#"
        INSERT INTO track_sources (track_id, service_id, service_track_id, format, availability_status)
        VALUES
            (?, 1, 'spotify_400', '320kbps', 'available'),
            (?, 2, 'tidal_400', 'FLAC', 'available'),
            (?, 3, 'qobuz_400', '24/96', 'available');
        "#
    )
    .bind(track_id)
    .bind(track_id)
    .bind(track_id)
    .execute(&pool)
    .await
    .unwrap();

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'queen_pl', 'Queen Duplicates', 2) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert same track at pos 1 and pos 5
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 5)"
    )
    .bind(playlist_id)
    .bind(track_id)
    .bind(playlist_id)
    .bind(track_id)
    .execute(&pool)
    .await
    .unwrap();

    let tracks = fetch_local_playlist_tracks_page(&pool, playlist_id, 0, 50)
        .await
        .expect("Fetch tracks page should succeed");

    // Must be exactly 2 tracks, despite having 3 track_sources each
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].position, Some(1));
    assert_eq!(tracks[1].position, Some(5));

    // Both should have their services aggregated properly
    for track in &tracks {
        let services = track.services.as_deref().unwrap_or("");
        assert!(services.contains("spotify"), "Must contain spotify");
        assert!(services.contains("tidal"), "Must contain tidal");
        assert!(services.contains("qobuz"), "Must contain qobuz");
    }
}
