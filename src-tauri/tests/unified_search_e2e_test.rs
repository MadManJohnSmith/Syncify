//! E2E Test Suite for Sprint S103: Búsqueda Unificada y Filtrado Avanzado
//!
//! Validates migration 0049 indices, multi-entity search (tracks, albums, artists, playlists),
//! filtering by service, favorites, download status, and pagination performance.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through 0049 must apply cleanly");

    // Seed services & default accounts
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Spotify User', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (2, 3, 'Tidal User', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_migration_0049_full_lifecycle_and_idempotence() {
    let db = create_test_db().await;

    let idx_artists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_artists_name_search'"
    )
    .fetch_one(&db).await.unwrap();
    assert_eq!(idx_artists.0, 1, "idx_artists_name_search index must exist");

    let idx_albums: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_albums_title_search'"
    )
    .fetch_one(&db).await.unwrap();
    assert_eq!(idx_albums.0, 1, "idx_albums_title_search index must exist");

    // Test idempotence
    let rerun = sqlx::migrate!("./migrations").run(&db).await;
    assert!(rerun.is_ok(), "Re-running migrations through 0049 must be 100% idempotent");
}

#[tokio::test]
async fn test_unified_search_all_entities() {
    let db = create_test_db().await;

    // Seed Bowie data
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, favorite_at) VALUES ('David Bowie', '2026-08-15T12:00:00Z') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date, favorite_at) VALUES ('Heroes', '1977-10-14', '2026-08-15T12:05:00Z') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, favorite_at) VALUES ('Heroes', ?, 'GBAYE7700037', '2026-08-15T12:10:00Z') RETURNING id"
    ).bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')").bind(track_id).bind(artist_id).execute(&db).await.unwrap();

    sqlx::query("INSERT INTO playlists (account_id, name, description) VALUES (1, 'Bowie Essentials', 'Top David Bowie songs')").execute(&db).await.unwrap();

    // Query across entities with search term "Bowie"
    let pattern = "%Bowie%";

    let artists_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists WHERE name LIKE ?").bind(pattern).fetch_one(&db).await.unwrap();
    assert_eq!(artists_count.0, 1);

    let tracks_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT t.id)
        FROM tracks t
        LEFT JOIN track_artists ta ON ta.track_id = t.id
        LEFT JOIN artists art ON art.id = ta.artist_id
        WHERE t.title LIKE ? OR art.name LIKE ?
        "#
    ).bind(pattern).bind(pattern).fetch_one(&db).await.unwrap();
    assert_eq!(tracks_count.0, 1);

    let playlists_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE name LIKE ? OR description LIKE ?")
        .bind(pattern).bind(pattern).fetch_one(&db).await.unwrap();
    assert_eq!(playlists_count.0, 1);
}

#[tokio::test]
async fn test_search_filters_favorites_and_service() {
    let db = create_test_db().await;

    // Track 1: Tidal + Favorite
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, is_favorite, favorite_at) VALUES ('Tidal Fav', 1, '2026-08-15T10:00:00Z') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, 'td_1')").bind(t1).execute(&db).await.unwrap();

    // Track 2: Spotify + Non-Favorite
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, is_favorite) VALUES ('Spotify Normal', 0) RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp_1')").bind(t2).execute(&db).await.unwrap();

    // Filter only favorites
    let fav_tracks: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tracks WHERE is_favorite = 1 OR favorite_at IS NOT NULL")
        .fetch_all(&db).await.unwrap();
    assert_eq!(fav_tracks.len(), 1);
    assert_eq!(fav_tracks[0].0, t1);

    // Filter service = tidal
    let tidal_tracks: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT t.id FROM tracks t JOIN track_sources ts ON ts.track_id = t.id WHERE ts.service_id = 3"
    ).fetch_all(&db).await.unwrap();
    assert_eq!(tidal_tracks.len(), 1);
    assert_eq!(tidal_tracks[0].0, t1);
}

#[tokio::test]
async fn test_search_download_status_filter() {
    let db = create_test_db().await;

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Downloaded Track') RETURNING id").fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, 'C:/music/track.flac', 'FLAC')")
        .bind(t1).execute(&db).await.unwrap();

    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Not Downloaded Track') RETURNING id").fetch_one(&db).await.unwrap();

    // Downloaded filter
    let dl_tracks: Vec<(i64,)> = sqlx::query_as("SELECT t.id FROM tracks t JOIN downloads d ON d.track_id = t.id").fetch_all(&db).await.unwrap();
    assert_eq!(dl_tracks.len(), 1);
    assert_eq!(dl_tracks[0].0, t1);

    // Not downloaded filter
    let not_dl: Vec<(i64,)> = sqlx::query_as("SELECT t.id FROM tracks t LEFT JOIN downloads d ON d.track_id = t.id WHERE d.id IS NULL").fetch_all(&db).await.unwrap();
    assert_eq!(not_dl.len(), 1);
    assert_eq!(not_dl[0].0, t2);
}

#[tokio::test]
async fn test_search_pagination_and_precision() {
    let db = create_test_db().await;

    for i in 0..20 {
        sqlx::query("INSERT INTO tracks (title) VALUES (?)")
            .bind(format!("Sample Song {:02}", i))
            .execute(&db).await.unwrap();
    }

    let pattern = "%Sample Song%";
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title LIKE ?").bind(pattern).fetch_one(&db).await.unwrap();
    assert_eq!(total.0, 20);

    // Page 1 (offset 0, limit 10)
    let page1: Vec<(String,)> = sqlx::query_as("SELECT title FROM tracks WHERE title LIKE ? ORDER BY title ASC LIMIT 10 OFFSET 0")
        .bind(pattern).fetch_all(&db).await.unwrap();
    assert_eq!(page1.len(), 10);
    assert_eq!(page1[0].0, "Sample Song 00");
    assert_eq!(page1[9].0, "Sample Song 09");

    // Page 2 (offset 10, limit 10)
    let page2: Vec<(String,)> = sqlx::query_as("SELECT title FROM tracks WHERE title LIKE ? ORDER BY title ASC LIMIT 10 OFFSET 10")
        .bind(pattern).fetch_all(&db).await.unwrap();
    assert_eq!(page2.len(), 10);
    assert_eq!(page2[0].0, "Sample Song 10");
    assert_eq!(page2[9].0, "Sample Song 19");
}
