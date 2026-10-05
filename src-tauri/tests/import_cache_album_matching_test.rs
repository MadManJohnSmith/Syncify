//! Ítem 22: ImportCache album matching regression tests.
//!
//! Validates that `get_or_create_album_with_compilation`:
//! 1. Links albums by (normalized title + artist) edition identity: two artists
//!    with the same album title NEVER share an album row.
//! 2. Reuses the existing album of the same artist on repeated imports.
//! 3. Matches albums linked to the artist even when the link is not the
//!    primary one (collaboration rows with `is_primary = 0`).
//! 4. Breaks same-title collisions for one artist by completeness (imported
//!    track count, then total duration) and registers the tie, keeping every
//!    candidate row intact.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::import_cache::ImportCache;

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

#[tokio::test]
async fn test_same_title_different_artists_stay_separate() {
    let pool = setup_db().await;

    let mut cache = ImportCache::new();
    let artist_a = cache.get_or_create_artist(&pool, "Queen").await.unwrap();
    let artist_b = cache.get_or_create_artist(&pool, "The Cure").await.unwrap();
    assert_ne!(artist_a, artist_b);

    let album_a = cache
        .get_or_create_album_with_compilation(
            &pool,
            &format!("{}:greatest hits", artist_a),
            "Greatest Hits",
            artist_a,
            None,
            None,
            false,
        )
        .await
        .expect("Album A must be created");

    // Fresh cache: same title, different artist. The old title-only
    // "SELECT ... ORDER BY id DESC" fallback could return album A here.
    let mut cache = ImportCache::new();
    let album_b = cache
        .get_or_create_album_with_compilation(
            &pool,
            &format!("{}:greatest hits", artist_b),
            "Greatest Hits",
            artist_b,
            None,
            None,
            false,
        )
        .await
        .expect("Album B must be created");

    assert_ne!(
        album_a, album_b,
        "Same-titled albums of different artists must never be merged"
    );

    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM albums WHERE LOWER(title) = 'greatest hits'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 2, "Both editions must exist as separate rows");
}

#[tokio::test]
async fn test_same_artist_reuses_album_across_caches() {
    let pool = setup_db().await;

    let mut cache = ImportCache::new();
    let artist = cache
        .get_or_create_artist(&pool, "Artist One")
        .await
        .unwrap();
    let first = cache
        .get_or_create_album_with_compilation(
            &pool,
            &format!("{}:nightsongs", artist),
            "Night Songs",
            artist,
            None,
            None,
            false,
        )
        .await
        .unwrap();

    // Fresh cache simulates a later import session for the same edition.
    let mut cache = ImportCache::new();
    let second = cache
        .get_or_create_album_with_compilation(
            &pool,
            &format!("{}:nightsongs", artist),
            "Night Songs",
            artist,
            None,
            None,
            false,
        )
        .await
        .unwrap();

    assert_eq!(
        first, second,
        "Repeated import must reuse the artist's album"
    );

    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM albums WHERE LOWER(title) = 'night songs'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 1, "No duplicate album row for the same edition");
}

#[tokio::test]
async fn test_matches_album_with_non_primary_artist_link() {
    let pool = setup_db().await;

    let mut cache = ImportCache::new();
    let artist = cache
        .get_or_create_artist(&pool, "Collab Artist")
        .await
        .unwrap();

    // Collaboration album: the artist is linked, but not as the primary artist.
    let seeded: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, is_compilation) VALUES ('Duo Sessions', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 0)")
        .bind(seeded)
        .bind(artist)
        .execute(&pool)
        .await
        .unwrap();

    let matched = cache
        .get_or_create_album_with_compilation(
            &pool,
            &format!("{}:duo sessions", artist),
            "Duo Sessions",
            artist,
            None,
            None,
            false,
        )
        .await
        .expect("Matching must succeed");

    assert_eq!(
        matched, seeded,
        "A non-primary album_artists link still identifies the edition"
    );

    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM albums WHERE LOWER(title) = 'duo sessions'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows, 1,
        "Matching by artist link must not duplicate the album"
    );
}

#[tokio::test]
async fn test_title_collision_resolved_by_completeness_and_registered() {
    let pool = setup_db().await;

    let mut cache = ImportCache::new();
    let artist = cache
        .get_or_create_artist(&pool, "Tie Breaker")
        .await
        .unwrap();

    // Two same-title rows for the same artist: a fragmented stub with 1 track
    // and a fuller edition with 3 tracks. The stub is created FIRST so the old
    // "newest row wins" behaviour would have picked wrongly.
    let stub: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, is_compilation) VALUES ('Anthology', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(stub)
        .bind(artist)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('A1', ?)")
        .bind(stub)
        .execute(&pool)
        .await
        .unwrap();

    let full: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, is_compilation) VALUES ('Anthology', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(full)
        .bind(artist)
        .execute(&pool)
        .await
        .unwrap();
    for title in ["F1", "F2", "F3"] {
        sqlx::query("INSERT INTO tracks (title, album_id, duration_ms) VALUES (?, ?, 200000)")
            .bind(title)
            .bind(full)
            .execute(&pool)
            .await
            .unwrap();
    }

    let chosen = cache
        .get_or_create_album_with_compilation(
            &pool,
            &format!("{}:anthology", artist),
            "Anthology",
            artist,
            None,
            None,
            false,
        )
        .await
        .expect("Collision resolution must succeed");

    assert_eq!(
        chosen, full,
        "The collision must reuse the edition with the most imported tracks"
    );

    // The tie is registered, not silently merged: both rows survive untouched.
    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM albums WHERE LOWER(title) = 'anthology'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows, 2,
        "Collision resolution must keep every candidate row"
    );

    let (stub_tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
        .bind(stub)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stub_tracks, 1, "The losing candidate must not be modified");
}
