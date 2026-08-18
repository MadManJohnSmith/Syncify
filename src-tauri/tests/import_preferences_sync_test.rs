use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    perform_get_service_import_preferences, perform_sync_service,
    perform_update_service_import_preferences, ImportPreferences,
};
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
async fn test_import_preferences_persistence_and_defaults() {
    let pool = setup_test_db().await;

    // 1. Initial State: Returns default preferences
    let default_prefs = perform_get_service_import_preferences(&pool, "qobuz")
        .await
        .expect("Getting default preferences should succeed");

    assert_eq!(default_prefs.service_name, "qobuz");
    assert!(default_prefs.favorite_tracks);
    assert!(!default_prefs.favorite_albums);
    assert!(!default_prefs.favorite_artists);
    assert!(default_prefs.playlists);
    assert!(!default_prefs.purchases);
    assert!(!default_prefs.library_history);
    assert!(default_prefs.incremental_sync);

    // 2. Update preferences with custom granular toggles
    let custom_prefs = ImportPreferences {
        service_name: "qobuz".to_string(),
        favorite_tracks: true,
        favorite_albums: false,
        favorite_artists: true,
        playlists: false,
        purchases: true,
        library_history: true,
        include_appearances: true,
        incremental_sync: false,
    };

    let updated = perform_update_service_import_preferences(&pool, custom_prefs)
        .await
        .expect("Updating preferences should succeed");

    assert_eq!(updated.service_name, "qobuz");
    assert!(updated.favorite_tracks);
    assert!(!updated.favorite_albums);
    assert!(updated.favorite_artists);
    assert!(!updated.playlists);
    assert!(updated.purchases);
    assert!(updated.library_history);
    assert!(updated.include_appearances);
    assert!(!updated.incremental_sync);

    // 3. Verify persistence across fresh query
    let fetched = perform_get_service_import_preferences(&pool, "qobuz")
        .await
        .unwrap();
    assert_eq!(fetched.favorite_artists, true);
    assert_eq!(fetched.playlists, false);
    assert_eq!(fetched.include_appearances, true);
}

#[tokio::test]
async fn test_sync_service_without_valid_auth_fails_immediately_with_requires_auth() {
    let pool = setup_test_db().await;

    // Case 1: No account connected at all
    let err_missing = perform_sync_service(&pool, "qobuz", None, None)
        .await
        .unwrap_err();
    assert!(err_missing.starts_with("RequiresAuth"), "Error must be structured RequiresAuth: {}", err_missing);

    // Case 2: Account exists but missing token
    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let empty_creds = serde_json::json!({ "user_id": "888" }).to_string();
    let encrypted = crypto::encrypt(&empty_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'No Token Qobuz', ?, 1) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err_no_token = perform_sync_service(&pool, "qobuz", Some(account_id), None)
        .await
        .unwrap_err();
    assert!(err_no_token.starts_with("RequiresAuth"), "Must return RequiresAuth without token: {}", err_no_token);
    assert!(!err_no_token.contains("favorites error"), "Must never map missing auth to favorites error");
}

#[tokio::test]
async fn test_import_track_counts_separate_favorites_from_imported_tracks_idempotently() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, is_active)
           VALUES (?, 'Counts Test Account', 1) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 1. Simulate inserting 3 favorite tracks and 2 album-only tracks
    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('The Wall') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Favorite tracks (is_liked = 1)
    for i in 1..=3 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES (?, ?) RETURNING id")
            .bind(format!("Fav Track {}", i))
            .bind(album_id)
            .fetch_one(&pool).await.unwrap();

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked, added_at) VALUES (?, ?, 1, CURRENT_TIMESTAMP)")
            .bind(account_id)
            .bind(tid)
            .execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, availability_status) VALUES (?, ?, ?, 1, 'available')")
            .bind(tid).bind(qobuz_svc_id).bind(format!("qobuz_fav_{}", i)).execute(&pool).await.unwrap();
    }

    // Non-favorite album tracks (is_liked = 0)
    for i in 4..=5 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES (?, ?) RETURNING id")
            .bind(format!("Album Only Track {}", i))
            .bind(album_id)
            .fetch_one(&pool).await.unwrap();

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 0)")
            .bind(account_id)
            .bind(tid)
            .execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, availability_status) VALUES (?, ?, ?, 1, 'available')")
            .bind(tid).bind(qobuz_svc_id).bind(format!("qobuz_alb_{}", i)).execute(&pool).await.unwrap();
    }

    // Verify database counts
    let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let favorite_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ? AND is_liked = 1")
        .bind(account_id)
        .fetch_one(&pool).await.unwrap();
    let all_imported_entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool).await.unwrap();

    assert_eq!(total_tracks, 5, "Total tracks in library should be 5");
    assert_eq!(favorite_tracks, 3, "Favorite tracks count must be 3");
    assert_eq!(all_imported_entries, 5, "Total imported library entries must be 5");

    // Re-inserting with INSERT OR IGNORE / ON CONFLICT must be completely idempotent
    for i in 1..=3 {
        let tid: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = ?")
            .bind(format!("Fav Track {}", i))
            .fetch_one(&pool).await.unwrap();

        let _ = sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1) ON CONFLICT(account_id, track_id) DO NOTHING")
            .bind(account_id)
            .bind(tid)
            .execute(&pool).await;
    }

    let tracks_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let entries_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool).await.unwrap();

    assert_eq!(tracks_after, 5, "Idempotent insert must not duplicate tracks");
    assert_eq!(entries_after, 5, "Idempotent insert must not duplicate library entries");
}
