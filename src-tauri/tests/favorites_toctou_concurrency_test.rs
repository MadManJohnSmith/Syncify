//! Regress & Concurrency Test for Favorites TOCTOU (TASK-54)
//!
//! Verifies that concurrent calls to `upsert_canonical_favorite_artist` and
//! `upsert_canonical_favorite_album` for the same artist name do not produce
//! `UNIQUE constraint failed: artists.name` errors.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::{
    upsert_canonical_favorite_album, upsert_canonical_favorite_artist,
};

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(20)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            favorite_at TEXT
        );
        CREATE UNIQUE INDEX idx_artists_name_unique ON artists(name);

        CREATE TABLE albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            upc TEXT,
            cover_art_url TEXT,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            favorite_at TEXT
        );

        CREATE TABLE album_artists (
            album_id INTEGER NOT NULL REFERENCES albums(id),
            artist_id INTEGER NOT NULL REFERENCES artists(id),
            is_primary INTEGER DEFAULT 1,
            PRIMARY KEY (album_id, artist_id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to create test schema");

    pool
}

#[tokio::test]
async fn test_concurrent_favorite_artist_upsert_no_unique_violation() {
    let db = setup_test_db().await;
    let artist_name = "Concurrent Test Artist";
    let concurrency = 30;

    let mut handles = Vec::new();
    for i in 0..concurrency {
        let pool = db.clone();
        let service_artist_id = format!("srv_art_{}", i);
        let handle = tokio::spawn(async move {
            upsert_canonical_favorite_artist(
                &pool,
                1,
                &service_artist_id,
                artist_name,
            )
            .await
        });
        handles.push(handle);
    }

    let mut returned_ids = Vec::new();
    for h in handles {
        let res = h.await.expect("Task panicked");
        assert!(
            res.is_ok(),
            "Concurrent upsert_canonical_favorite_artist failed: {:?}",
            res.err()
        );
        returned_ids.push(res.unwrap());
    }

    // All tasks must resolve to the EXACT same artist ID
    let first_id = returned_ids[0];
    for id in &returned_ids {
        assert_eq!(*id, first_id, "All concurrent tasks must return the same artist ID");
    }

    // Verify exactly one row exists in artists table
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE name = ?")
        .bind(artist_name)
        .fetch_one(&db)
        .await
        .expect("Query failed");
    assert_eq!(count, 1, "Exactly one artist row should exist");

    // Verify favorite attributes
    let (is_fav, fav_at): (i64, Option<String>) = sqlx::query_as(
        "SELECT is_favorite, favorite_at FROM artists WHERE id = ?"
    )
    .bind(first_id)
    .fetch_one(&db)
    .await
    .expect("Query failed");

    assert_eq!(is_fav, 1);
    assert!(fav_at.is_some(), "favorite_at must be populated");
}

#[tokio::test]
async fn test_concurrent_favorite_album_upsert_no_unique_artist_violation() {
    let db = setup_test_db().await;
    let artist_name = "Concurrent Album Artist";
    let concurrency = 20;

    let mut handles = Vec::new();
    for i in 0..concurrency {
        let pool = db.clone();
        let album_title = format!("Album Volume {}", i % 3);
        let upc = format!("UPC_TEST_{}", i % 3);
        let service_album_id = format!("srv_alb_{}", i);
        let handle = tokio::spawn(async move {
            upsert_canonical_favorite_album(
                &pool,
                1,
                &service_album_id,
                &album_title,
                artist_name,
                Some(&upc),
                Some("https://example.com/cover.jpg"),
            )
            .await
        });
        handles.push(handle);
    }

    for h in handles {
        let res = h.await.expect("Task panicked");
        assert!(
            res.is_ok(),
            "Concurrent upsert_canonical_favorite_album failed: {:?}",
            res.err()
        );
    }

    // Verify exactly one artist was created despite concurrent album insertions
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE name = ?")
        .bind(artist_name)
        .fetch_one(&db)
        .await
        .expect("Query failed");
    assert_eq!(
        count, 1,
        "Exactly one artist row must exist after concurrent album upserts"
    );
}
