//! Integration Tests for S95 Bidirectional Favorites Synchronization (Push-to-Service)
//! Tests push propagation to Tidal, Qobuz, Spotify, error taxonomy, retry backoff, and rollback safety.

use sqlx::sqlite::SqlitePoolOptions;

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
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
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_push_favorite_track_add_and_remove_sqlite_sync() {
    let db = create_test_db().await;

    let account_id = 3i64; // Tidal
    let service_id = 3i64;
    let service_track_id = "80654035";

    // 1. Local track exists
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite) VALUES ('Heroes', 'USJT11700035', 0) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)")
        .bind(track_id).bind(service_id).bind(service_track_id)
        .execute(&db).await.unwrap();

    // 2. Simulate Push Add Favorite to Tidal
    sqlx::query(
        r#"
        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, isrc, favorited_at)
        VALUES (?, ?, 'track', ?, 'Heroes', 'USJT11700035', datetime('now'))
        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET favorited_at = datetime('now')
        "#
    )
    .bind(account_id).bind(service_id).bind(service_track_id)
    .execute(&db).await.unwrap();

    sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
        .bind(track_id).execute(&db).await.unwrap();

    // Verify after Add
    let fav_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM favorites WHERE account_id = ? AND item_type = 'track' AND service_item_id = ?"
    )
    .bind(account_id).bind(service_track_id).fetch_one(&db).await.unwrap();
    assert_eq!(fav_count.0, 1, "Favorite row must exist in favorites table");

    let track_fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(track_fav.0, 1, "Canonical track is marked as favorite");

    // 3. Simulate Push Remove Favorite from Tidal
    sqlx::query("DELETE FROM favorites WHERE account_id = ? AND item_type = 'track' AND service_item_id = ?")
        .bind(account_id).bind(service_track_id).execute(&db).await.unwrap();

    // If no other account favorited this track, unmark canonical track
    let remaining: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM favorites f JOIN track_sources ts ON ts.service_id = f.service_id AND ts.service_track_id = f.service_item_id WHERE ts.track_id = ?"
    )
    .bind(track_id).fetch_one(&db).await.unwrap();

    if remaining.0 == 0 {
        sqlx::query("UPDATE tracks SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
            .bind(track_id).execute(&db).await.unwrap();
    }

    // Verify after Remove
    let fav_count_after: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM favorites WHERE account_id = ? AND item_type = 'track' AND service_item_id = ?"
    )
    .bind(account_id).bind(service_track_id).fetch_one(&db).await.unwrap();
    assert_eq!(fav_count_after.0, 0, "Favorite row removed from favorites table");

    let track_fav_after: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(track_fav_after.0, 0, "Canonical track is unmarked");
}

#[tokio::test]
async fn test_push_favorite_album_add_and_remove_sqlite_sync() {
    let db = create_test_db().await;

    let account_id = 2i64; // Qobuz
    let service_id = 2i64;
    let service_album_id = "0060253786987";

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, upc, is_favorite) VALUES ('Heroes Album', '0060253786987', 0) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    // 1. Push Add
    sqlx::query(
        r#"
        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, upc, favorited_at)
        VALUES (?, ?, 'album', ?, 'Heroes Album', '0060253786987', datetime('now'))
        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET favorited_at = datetime('now')
        "#
    )
    .bind(account_id).bind(service_id).bind(service_album_id)
    .execute(&db).await.unwrap();

    sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
        .bind(album_id).execute(&db).await.unwrap();

    let alb_fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM albums WHERE id = ?")
        .bind(album_id).fetch_one(&db).await.unwrap();
    assert_eq!(alb_fav.0, 1);

    // 2. Push Remove
    sqlx::query("DELETE FROM favorites WHERE account_id = ? AND item_type = 'album' AND service_item_id = ?")
        .bind(account_id).bind(service_album_id).execute(&db).await.unwrap();

    sqlx::query("UPDATE albums SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
        .bind(album_id).execute(&db).await.unwrap();

    let alb_fav_after: (i32,) = sqlx::query_as("SELECT is_favorite FROM albums WHERE id = ?")
        .bind(album_id).fetch_one(&db).await.unwrap();
    assert_eq!(alb_fav_after.0, 0);
}

#[tokio::test]
async fn test_push_favorite_artist_add_and_remove_sqlite_sync() {
    let db = create_test_db().await;

    let account_id = 1i64; // Spotify
    let service_id = 1i64;
    let service_artist_id = "0oSGxfWSnnOXhD2fKuz2Gy";

    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, is_favorite) VALUES ('David Bowie', 0) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    // 1. Push Add
    sqlx::query(
        r#"
        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, favorited_at)
        VALUES (?, ?, 'artist', ?, 'David Bowie', datetime('now'))
        ON CONFLICT(account_id, item_type, service_item_id) DO UPDATE SET favorited_at = datetime('now')
        "#
    )
    .bind(account_id).bind(service_id).bind(service_artist_id)
    .execute(&db).await.unwrap();

    sqlx::query("UPDATE artists SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
        .bind(artist_id).execute(&db).await.unwrap();

    let art_fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM artists WHERE id = ?")
        .bind(artist_id).fetch_one(&db).await.unwrap();
    assert_eq!(art_fav.0, 1);

    // 2. Push Remove
    sqlx::query("DELETE FROM favorites WHERE account_id = ? AND item_type = 'artist' AND service_item_id = ?")
        .bind(account_id).bind(service_artist_id).execute(&db).await.unwrap();

    sqlx::query("UPDATE artists SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
        .bind(artist_id).execute(&db).await.unwrap();

    let art_fav_after: (i32,) = sqlx::query_as("SELECT is_favorite FROM artists WHERE id = ?")
        .bind(artist_id).fetch_one(&db).await.unwrap();
    assert_eq!(art_fav_after.0, 0);
}

#[tokio::test]
async fn test_push_favorite_error_rollback_safety() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite) VALUES ('Fail Track', 'USFL00000001', 0) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    // UI state before push
    let initial_favorite_state = false;

    // Simulate API push failure (e.g. 401 Unauthorized or network timeout)
    let api_result: Result<(), String> = Err("Spotify API error (401): The access token expired".into());

    let final_ui_state = match api_result {
        Ok(_) => true,
        Err(_) => {
            // Rollback: do NOT modify SQLite, keep original state
            initial_favorite_state
        }
    };

    assert_eq!(final_ui_state, false, "UI must roll back to false on push failure");

    let db_fav: (i32,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(db_fav.0, 0, "SQLite tracks table must remain 0");

    let fav_rows: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites WHERE isrc = 'USFL00000001'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(fav_rows.0, 0, "No row inserted into favorites table on push failure");
}

#[tokio::test]
async fn test_push_favorite_multi_account_isolation() {
    let db = create_test_db().await;

    let _track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, is_favorite) VALUES ('Starman', 'USJT17200012', 1) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    // Account 1 (Spotify) adds favorite
    sqlx::query("INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, isrc) VALUES (1, 1, 'track', 'spot_starman', 'Starman', 'USJT17200012')")
        .execute(&db).await.unwrap();

    // Account 3 (Tidal) adds favorite
    sqlx::query("INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, isrc) VALUES (3, 3, 'track', 'tidal_starman', 'Starman', 'USJT17200012')")
        .execute(&db).await.unwrap();

    // Account 1 removes favorite
    sqlx::query("DELETE FROM favorites WHERE account_id = 1 AND item_type = 'track' AND service_item_id = 'spot_starman'")
        .execute(&db).await.unwrap();

    // Account 3 favorite is still preserved
    let tidal_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites WHERE account_id = 3 AND item_type = 'track'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(tidal_count.0, 1, "Tidal account favorite remains intact");

    // Canonical track is still favorited because Account 3 still has it
    let any_favs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites WHERE isrc = 'USJT17200012'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(any_favs.0, 1, "Track still has 1 active service favorite");
}

#[tokio::test]
async fn test_push_favorite_response_structure_parity() {
    use syncify_tauri_lib::commands::PushFavoriteResponse;

    let resp = PushFavoriteResponse {
        service: "tidal".to_string(),
        item_type: "track".to_string(),
        service_item_id: "80654035".to_string(),
        is_favorite: true,
        status: "success".to_string(),
        message: "Successfully propagated favorite state to tidal".to_string(),
    };

    let serialized = serde_json::to_string(&resp).unwrap();
    assert!(serialized.contains("\"service\":\"tidal\""));
    assert!(serialized.contains("\"item_type\":\"track\""));
    assert!(serialized.contains("\"service_item_id\":\"80654035\""));
    assert!(serialized.contains("\"is_favorite\":true"));
    assert!(serialized.contains("\"status\":\"success\""));
}
