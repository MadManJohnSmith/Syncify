//! BD-5 (migration 0088): the Spotify importer must capture the DECLARED
//! release total, not preserve a locally derived count.
//!
//! Validates `SpotifyClient::get_or_create_album`:
//! 1. Upsert by spotify_id: when the API declares a release total, it overwrites
//!    the stored (possibly derived) `total_tracks` and is recorded in
//!    `declared_total_tracks`; the imported count stays in `local_track_count`.
//! 2. Compilation re-import: the same declaration capture happens on the
//!    compilation dedup UPDATE path.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::import_cache::get_or_create_canonical_various_artists;
use syncify_tauri_lib::services::spotify::{SpotifyAlbum, SpotifyClient};

async fn setup_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn album_counts(pool: &sqlx::SqlitePool, album_id: i64) -> (i32, i32, i32) {
    sqlx::query_as(
        "SELECT total_tracks, declared_total_tracks, local_track_count FROM albums WHERE id = ?",
    )
    .bind(album_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn test_upsert_captures_declared_total_over_derived_count() {
    let pool = setup_db().await;
    let client = SpotifyClient::new("test-token".to_string(), None, 0);

    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Local Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // A locally imported edition: total_tracks was derived from the library
    // (5 imported tracks), nothing was ever declared by a service.
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, spotify_id, is_compilation) VALUES ('Local Edition', 'sp-alb-declared', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();
    for title in ["L1", "L2", "L3", "L4", "L5"] {
        sqlx::query("INSERT INTO tracks (title, album_id) VALUES (?, ?)")
            .bind(title)
            .bind(album_id)
            .execute(&pool)
            .await
            .unwrap();
    }
    assert_eq!(
        album_counts(&pool, album_id).await,
        (5, 0, 5),
        "Seed: derived total of 5, nothing declared"
    );

    // The service now declares 12 for the same spotify_id: the old
    // COALESCE(albums.total_tracks, excluded.total_tracks) kept the derived 5
    // and the declaration was never captured.
    let album = SpotifyAlbum {
        id: "sp-alb-declared".to_string(),
        name: "Local Edition".to_string(),
        total_tracks: Some(12),
        ..Default::default()
    };
    let returned = client
        .get_or_create_album(&pool, &album, artist_id)
        .await
        .expect("get_or_create_album must succeed");
    assert_eq!(
        returned, album_id,
        "Upsert must reuse the row by spotify_id"
    );

    assert_eq!(
        album_counts(&pool, album_id).await,
        (12, 12, 5),
        "BD-5: declared 12 wins in total_tracks AND declared_total_tracks; local count stays 5"
    );
}

#[tokio::test]
async fn test_compilation_reimport_captures_declared_total() {
    let pool = setup_db().await;
    let client = SpotifyClient::new("test-token".to_string(), None, 0);

    let va_id = get_or_create_canonical_various_artists(&pool)
        .await
        .unwrap();

    // Existing compilation with 2 imported tracks and no declared total yet.
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, is_compilation) VALUES ('Indie Compilation', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(va_id)
        .execute(&pool)
        .await
        .unwrap();
    for title in ["C1", "C2"] {
        sqlx::query("INSERT INTO tracks (title, album_id) VALUES (?, ?)")
            .bind(title)
            .bind(album_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    let album = SpotifyAlbum {
        id: "sp-comp-1".to_string(),
        name: "Indie Compilation".to_string(),
        album_type: Some("compilation".to_string()),
        total_tracks: Some(8),
        ..Default::default()
    };
    let returned = client
        .get_or_create_album(&pool, &album, va_id)
        .await
        .expect("get_or_create_album must succeed");
    assert_eq!(returned, album_id, "Compilation dedup must reuse the row");

    assert_eq!(
        album_counts(&pool, album_id).await,
        (8, 8, 2),
        "BD-5: the compilation path writes the declared 8 into total_tracks and declared_total_tracks"
    );
}
