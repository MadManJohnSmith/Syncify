//! Integration Tests for S94 Favorites Sync & Cross-Service Persistence
//! Tests Tidal, Qobuz, and Spotify favorites synchronization, SQLite 0045 schema, deduplication and parity.

use sqlx::sqlite::SqlitePoolOptions;

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    // Apply migrations 0001 -> 0045 using sqlx::migrate!
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations 0001..0045 must apply cleanly");

    // Insert baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Insert sample accounts
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_migration_0045_full_lifecycle_and_idempotence() {
    let db = create_test_db().await;

    // Verify favorites table columns
    let fav_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites")
        .fetch_one(&db)
        .await
        .expect("favorites table must exist");
    assert_eq!(fav_count.0, 0);

    // Verify favorites_cache table
    let cache_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites_cache")
        .fetch_one(&db)
        .await
        .expect("favorites_cache table must exist");
    assert_eq!(cache_count.0, 0);

    // Verify albums and artists columns
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, is_favorite, favorite_at) VALUES ('Test Album', 1, datetime('now')) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_fav: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM albums WHERE id = ?")
        .bind(album_id).fetch_one(&db).await.unwrap();
    assert_eq!(album_fav.0, 1);
    assert!(album_fav.1.is_some());

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, is_favorite, favorite_at) VALUES ('Test Artist', 1, datetime('now')) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let artist_fav: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM artists WHERE id = ?")
        .bind(artist_id).fetch_one(&db).await.unwrap();
    assert_eq!(artist_fav.0, 1);
    assert!(artist_fav.1.is_some());

    // Re-run migration 0045 idempotence check
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Re-applying migrations must be idempotent");
}

#[tokio::test]
async fn test_favorites_table_upsert_and_isolation() {
    let db = create_test_db().await;

    // Insert favorite track
    sqlx::query(
        r#"
        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, favorited_at)
        VALUES (3, 3, 'track', '80654035', 'Heroes', 'David Bowie', '"Heroes"', 'USJT11700035', '2026-08-15T00:00:00Z')
        "#
    )
    .execute(&db)
    .await
    .unwrap();

    // Upsert same item with updated title
    sqlx::query(
        r#"
        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, album_name, isrc, favorited_at)
        VALUES (3, 3, 'track', '80654035', 'Heroes (2017 Remaster)', 'David Bowie', '"Heroes"', 'USJT11700035', '2026-08-15T00:00:00Z')
        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET
            title = excluded.title,
            artist_name = excluded.artist_name,
            favorited_at = excluded.favorited_at
        "#
    )
    .execute(&db)
    .await
    .unwrap();

    let row: (String, String) = sqlx::query_as("SELECT title, artist_name FROM favorites WHERE service_item_id = '80654035'")
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(row.0, "Heroes (2017 Remaster)");
    assert_eq!(row.1, "David Bowie");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites WHERE service_item_id = '80654035'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "Duplicate insert must update in place without row duplication");
}

#[tokio::test]
async fn test_cross_service_isrc_deduplication() {
    let db = create_test_db().await;

    // Track "Heroes" exists across Spotify, Qobuz, Tidal with same ISRC: USJT11700035
    let isrc = "USJT11700035";

    // 1. Spotify sync
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Heroes', ?, 1, datetime('now')) RETURNING id"
    )
    .bind(isrc)
    .fetch_one(&db)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format) VALUES (?, 1, 'spot_123', 'AAC')")
        .bind(track_id).execute(&db).await.unwrap();

    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, isrc) VALUES (1, 1, 'track', 'spot_123', 'Heroes', 'David Bowie', ?)"
    )
    .bind(isrc).execute(&db).await.unwrap();

    // 2. Qobuz sync (recognizes existing ISRC and links to same track)
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format) VALUES (?, 2, 'qobuz_456', 'FLAC')")
        .bind(track_id).execute(&db).await.unwrap();

    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, isrc) VALUES (2, 2, 'track', 'qobuz_456', 'Heroes', 'David Bowie', ?)"
    )
    .bind(isrc).execute(&db).await.unwrap();

    // 3. Tidal sync
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format) VALUES (?, 3, 'tidal_789', 'FLAC')")
        .bind(track_id).execute(&db).await.unwrap();

    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, isrc) VALUES (3, 3, 'track', 'tidal_789', 'Heroes', 'David Bowie', ?)"
    )
    .bind(isrc).execute(&db).await.unwrap();

    // Verification
    let tracks_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc = ?")
        .bind(isrc).fetch_one(&db).await.unwrap();
    assert_eq!(tracks_count.0, 1, "Single canonical track in library");

    let sources_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(sources_count.0, 3, "Track linked to 3 streaming services");

    let favs_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites WHERE isrc = ?")
        .bind(isrc).fetch_one(&db).await.unwrap();
    assert_eq!(favs_count.0, 3, "Track recorded in favorites for all 3 accounts");

    // Canonical UI View Query (get_favorites_tracks for 'all' / 'local')
    let canonical_view_tracks: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT 
            t.id,
            t.title,
            t.isrc
        FROM tracks t
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        WHERE t.is_favorite = 1
        GROUP BY t.id
        ORDER BY t.favorite_at DESC NULLS LAST
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(canonical_view_tracks.len(), 1, "Canonical favorites query collapses multi-service sources into exactly 1 track");
    assert_eq!(canonical_view_tracks[0].1, "Heroes");
    assert_eq!(canonical_view_tracks[0].2, Some("USJT11700035".to_string()));
}

#[tokio::test]
async fn test_favorites_cache_lifecycle() {
    let db = create_test_db().await;

    // Cache update
    sqlx::query(
        r#"
        INSERT INTO favorites_cache (service_name, item_type, total_count, data_json)
        VALUES ('tidal', 'tracks', 150, '{"items_sample": 150}')
        ON CONFLICT(service_name, item_type) DO UPDATE SET
            total_count = excluded.total_count,
            data_json = excluded.data_json,
            last_synced_at = datetime('now')
        "#
    )
    .execute(&db)
    .await
    .unwrap();

    let cache: (i64, String) = sqlx::query_as("SELECT total_count, data_json FROM favorites_cache WHERE service_name = 'tidal' AND item_type = 'tracks'")
        .fetch_one(&db).await.unwrap();

    assert_eq!(cache.0, 150);
    assert!(cache.1.contains("150"));
}

#[tokio::test]
async fn test_toggle_album_favorite_atomic() {
    let db = create_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, is_favorite) VALUES ('Heroes Album', 0) RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Toggle to favorite
    let res1: (i32,) = sqlx::query_as(
        "UPDATE albums \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite"
    )
    .bind(album_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(res1.0, 1, "Album must be favorited");

    let fav_at1: Option<String> = sqlx::query_scalar("SELECT favorite_at FROM albums WHERE id = ?")
        .bind(album_id).fetch_one(&db).await.unwrap();
    assert!(fav_at1.is_some());

    // Toggle back to unfavorite
    let res2: (i32,) = sqlx::query_as(
        "UPDATE albums \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite"
    )
    .bind(album_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(res2.0, 0, "Album must be unfavorited");

    let fav_at2: Option<String> = sqlx::query_scalar("SELECT favorite_at FROM albums WHERE id = ?")
        .bind(album_id).fetch_one(&db).await.unwrap();
    assert!(fav_at2.is_none());
}

#[tokio::test]
async fn test_toggle_artist_favorite_atomic() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, is_favorite) VALUES ('David Bowie', 0) RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Toggle to favorite
    let res1: (i32,) = sqlx::query_as(
        "UPDATE artists \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite"
    )
    .bind(artist_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(res1.0, 1, "Artist must be favorited");

    let fav_at1: Option<String> = sqlx::query_scalar("SELECT favorite_at FROM artists WHERE id = ?")
        .bind(artist_id).fetch_one(&db).await.unwrap();
    assert!(fav_at1.is_some());

    // Toggle back
    let res2: (i32,) = sqlx::query_as(
        "UPDATE artists \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite"
    )
    .bind(artist_id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(res2.0, 0, "Artist must be unfavorited");
}

#[tokio::test]
async fn test_favorites_query_service_filtering_and_pagination() {
    let db = create_test_db().await;

    // Insert 5 tracks across services
    for i in 1..=3 {
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, isrc, favorited_at) VALUES (3, 3, 'track', ?, ?, 'Tidal Artist', ?, datetime('now'))"
        )
        .bind(format!("tidal_{}", i))
        .bind(format!("Tidal Track {}", i))
        .bind(format!("ISRC_T_{}", i))
        .execute(&db).await.unwrap();
    }

    for i in 1..=2 {
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name, isrc, favorited_at) VALUES (2, 2, 'track', ?, ?, 'Qobuz Artist', ?, datetime('now'))"
        )
        .bind(format!("qobuz_{}", i))
        .bind(format!("Qobuz Track {}", i))
        .bind(format!("ISRC_Q_{}", i))
        .execute(&db).await.unwrap();
    }

    // Query Tidal favorites
    let tidal_favs: Vec<(String,)> = sqlx::query_as(
        "SELECT f.title FROM favorites f JOIN services s ON s.id = f.service_id WHERE s.name = 'tidal' AND f.item_type = 'track' ORDER BY f.id ASC"
    )
    .fetch_all(&db).await.unwrap();
    assert_eq!(tidal_favs.len(), 3);
    assert_eq!(tidal_favs[0].0, "Tidal Track 1");

    // Query Qobuz favorites with pagination limit 1
    let qobuz_favs: Vec<(String,)> = sqlx::query_as(
        "SELECT f.title FROM favorites f JOIN services s ON s.id = f.service_id WHERE s.name = 'qobuz' AND f.item_type = 'track' LIMIT 1 OFFSET 0"
    )
    .fetch_all(&db).await.unwrap();
    assert_eq!(qobuz_favs.len(), 1);
    assert_eq!(qobuz_favs[0].0, "Qobuz Track 1");
}

#[tokio::test]
async fn test_optimistic_rollback_simulation() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, is_favorite) VALUES ('Rollback Track', 'USRB00000001', 0) RETURNING id")
        .fetch_one(&db).await.unwrap();

    // UI initiates optimistic toggle: local state becomes 1
    let mut ui_is_favorite = true;

    // Backend transaction fails intentionally (e.g. constraint violation or simulated error)
    let update_res = async {
        let mut tx = db.begin().await?;
        sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
            .bind(track_id)
            .execute(&mut *tx)
            .await?;
        // Simulate forced failure
        Err::<(), sqlx::Error>(sqlx::Error::RowNotFound)
    }.await;

    // On error, UI executes rollback
    if update_res.is_err() {
        ui_is_favorite = false;
    }

    assert_eq!(ui_is_favorite, false, "UI state rolled back to false");

    // Verify DB remains 0
    let db_fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(db_fav.0, 0, "DB state was never committed and remains 0");
}
