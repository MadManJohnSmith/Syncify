//! Integration Test Suite for TASK-82: Consolidación del Modelo de Favoritos y Filtros por Servicio
//!
//! Validates:
//! 1. `get_favorites_tracks` (via `perform_get_favorites_tracks`) filters correctly by streaming service
//!    (e.g., Spotify, Tidal) by joining canonical tracks with `track_sources` and `services`.
//! 2. Unfiltered and "all" favorites queries return all favorited tracks (`tracks.is_favorite = 1`).
//! 3. `get_favorites_albums` and `get_favorites_artists` filter by service and return canonical items.
//! 4. `push_favorite_to_service` (via `perform_push_favorite_sync`) updates canonical `tracks.is_favorite`,
//!    `library_entries.is_liked`, and the unified `favorites` table atomically.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    perform_get_favorites_albums, perform_get_favorites_artists, perform_get_favorites_tracks,
    perform_push_favorite_sync,
};

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_get_favorites_tracks_service_filtering() {
    let db = create_test_db().await;

    // Track 1: Spotify favorite
    let t1_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Song Spotify', 'ISRC_SP_1', 1, datetime('now')) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp_track_1')")
        .bind(t1_id).execute(&db).await.unwrap();

    // Track 2: Tidal favorite
    let t2_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Song Tidal', 'ISRC_TI_2', 1, datetime('now')) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, 'ti_track_2')")
        .bind(t2_id).execute(&db).await.unwrap();

    // Track 3: Multi-service favorite (Spotify + Tidal)
    let t3_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Song Dual', 'ISRC_DUAL_3', 1, datetime('now')) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp_track_3')")
        .bind(t3_id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, 'ti_track_3')")
        .bind(t3_id).execute(&db).await.unwrap();

    // Track 4: Spotify track, NOT favorite
    let t4_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Song NonFav', 'ISRC_NON_4', 0, NULL) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp_track_4')")
        .bind(t4_id).execute(&db).await.unwrap();

    // 1. Unfiltered query should return all 3 favorites (t1, t2, t3), excluding non-favorite (t4)
    let all_favs = perform_get_favorites_tracks(&db, None, None, None).await.expect("query should succeed");
    assert_eq!(all_favs.len(), 3, "All 3 favorited tracks must be returned when no filter is provided");
    let titles: Vec<&str> = all_favs.iter().map(|t| t.title.as_str()).collect();
    assert!(titles.contains(&"Song Spotify"));
    assert!(titles.contains(&"Song Tidal"));
    assert!(titles.contains(&"Song Dual"));
    assert!(!titles.contains(&"Song NonFav"));

    // 2. Filter "all" should return same 3 tracks
    let all_filter = perform_get_favorites_tracks(&db, Some("all".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(all_filter.len(), 3);

    // 3. Filter "spotify" should return Song Spotify (t1) and Song Dual (t3)
    let spotify_favs = perform_get_favorites_tracks(&db, Some("spotify".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(spotify_favs.len(), 2, "Spotify filter must return exactly 2 tracks");
    let sp_titles: Vec<&str> = spotify_favs.iter().map(|t| t.title.as_str()).collect();
    assert!(sp_titles.contains(&"Song Spotify"));
    assert!(sp_titles.contains(&"Song Dual"));
    assert!(!sp_titles.contains(&"Song Tidal"));
    assert!(!sp_titles.contains(&"Song NonFav"));
    for item in &spotify_favs {
        assert_eq!(item.service.to_lowercase(), "spotify");
    }

    // 4. Filter "tidal" should return Song Tidal (t2) and Song Dual (t3)
    let tidal_favs = perform_get_favorites_tracks(&db, Some("tidal".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(tidal_favs.len(), 2, "Tidal filter must return exactly 2 tracks");
    let ti_titles: Vec<&str> = tidal_favs.iter().map(|t| t.title.as_str()).collect();
    assert!(ti_titles.contains(&"Song Tidal"));
    assert!(ti_titles.contains(&"Song Dual"));
    assert!(!ti_titles.contains(&"Song Spotify"));
    for item in &tidal_favs {
        assert_eq!(item.service.to_lowercase(), "tidal");
    }

    // 5. Filter "qobuz" should return 0 tracks since none have Qobuz sources
    let qobuz_favs = perform_get_favorites_tracks(&db, Some("qobuz".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(qobuz_favs.len(), 0, "Qobuz filter must return 0 tracks");
}

#[tokio::test]
async fn test_get_favorites_albums_and_artists_service_filtering() {
    let db = create_test_db().await;

    // Seed albums
    sqlx::query("INSERT INTO albums (id, title, spotify_id, is_favorite, favorite_at) VALUES (1, 'Album Spotify', 'sp_alb_1', 1, datetime('now'))")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT INTO albums (id, title, tidal_id, is_favorite, favorite_at) VALUES (2, 'Album Tidal', 'ti_alb_2', 1, datetime('now'))")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT INTO albums (id, title, spotify_id, is_favorite, favorite_at) VALUES (3, 'Album NonFav', 'sp_alb_3', 0, NULL)")
        .execute(&db).await.unwrap();

    // Unfiltered albums
    let all_albums = perform_get_favorites_albums(&db, None, None, None).await.expect("query should succeed");
    assert_eq!(all_albums.len(), 2, "Only favorited albums returned");

    // Spotify albums
    let sp_albums = perform_get_favorites_albums(&db, Some("spotify".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(sp_albums.len(), 1);
    assert_eq!(sp_albums[0].title, "Album Spotify");
    assert_eq!(sp_albums[0].service, "spotify");

    // Tidal albums
    let ti_albums = perform_get_favorites_albums(&db, Some("tidal".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(ti_albums.len(), 1);
    assert_eq!(ti_albums[0].title, "Album Tidal");
    assert_eq!(ti_albums[0].service, "tidal");

    // Seed artists
    sqlx::query("INSERT INTO artists (id, name, spotify_id, is_favorite, favorite_at) VALUES (1, 'Artist Spotify', 'sp_art_1', 1, datetime('now'))")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT INTO artists (id, name, tidal_id, is_favorite, favorite_at) VALUES (2, 'Artist Tidal', 'ti_art_2', 1, datetime('now'))")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT INTO artists (id, name, spotify_id, is_favorite, favorite_at) VALUES (3, 'Artist NonFav', 'sp_art_3', 0, NULL)")
        .execute(&db).await.unwrap();

    // Unfiltered artists
    let all_artists = perform_get_favorites_artists(&db, None, None, None).await.expect("query should succeed");
    assert_eq!(all_artists.len(), 2);

    // Spotify artists
    let sp_artists = perform_get_favorites_artists(&db, Some("spotify".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(sp_artists.len(), 1);
    assert_eq!(sp_artists[0].name, "Artist Spotify");
    assert_eq!(sp_artists[0].service, "spotify");

    // Tidal artists
    let ti_artists = perform_get_favorites_artists(&db, Some("tidal".to_string()), None, None).await.expect("query should succeed");
    assert_eq!(ti_artists.len(), 1);
    assert_eq!(ti_artists[0].name, "Artist Tidal");
    assert_eq!(ti_artists[0].service, "tidal");
}

#[tokio::test]
async fn test_push_favorite_canonical_flags_and_library_entries() {
    let db = create_test_db().await;

    let account_id = 3i64; // Tidal account
    let service_id = 3i64;
    let service_track_id = "80654035";

    // 1. Create a track initially not favorited
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Starman', 'GBAYE7200021', 0, NULL) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)")
        .bind(track_id).bind(service_id).bind(service_track_id)
        .execute(&db).await.unwrap();

    // 2. Perform atomic push favorite sync (add favorite)
    perform_push_favorite_sync(&db, account_id, service_id, "tidal", "track", service_track_id, true)
        .await
        .expect("Push favorite sync add must succeed");

    // Check favorites table
    let fav_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM favorites WHERE account_id = ? AND item_type = 'track' AND service_item_id = ?"
    )
    .bind(account_id).bind(service_track_id).fetch_one(&db).await.unwrap();
    assert_eq!(fav_count.0, 1, "Favorites row created in unified favorites table");

    // Check canonical track flag
    let track_fav: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(track_fav.0, 1, "Canonical track is_favorite must be 1");
    assert!(track_fav.1.is_some(), "favorite_at timestamp must be set");

    // Check library_entries
    let entry: (i32,) = sqlx::query_as("SELECT is_liked FROM library_entries WHERE account_id = ? AND track_id = ?")
        .bind(account_id).bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(entry.0, 1, "library_entries.is_liked must be 1");

    // 3. Perform atomic push favorite sync (remove favorite)
    perform_push_favorite_sync(&db, account_id, service_id, "tidal", "track", service_track_id, false)
        .await
        .expect("Push favorite sync remove must succeed");

    // Check favorites table deleted
    let fav_count_after: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM favorites WHERE account_id = ? AND item_type = 'track' AND service_item_id = ?"
    )
    .bind(account_id).bind(service_track_id).fetch_one(&db).await.unwrap();
    assert_eq!(fav_count_after.0, 0, "Favorites row removed");

    // Check library_entries is_liked = 0
    let entry_after: (i32,) = sqlx::query_as("SELECT is_liked FROM library_entries WHERE account_id = ? AND track_id = ?")
        .bind(account_id).bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(entry_after.0, 0, "library_entries.is_liked must be 0");

    // Check canonical track is_favorite = 0
    let track_fav_after: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(track_fav_after.0, 0, "Canonical track is_favorite must be reset to 0");
    assert!(track_fav_after.1.is_none(), "favorite_at must be cleared to NULL");
}
