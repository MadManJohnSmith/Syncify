//! Import preferences per service (R2 / C-08).
//!
//! **R2 (docs/HANDOFF-2026-10-11.md:29-30)** — el comportamiento cambió a
//! propósito con la migración `src-tauri/migrations/0092_import_preferences_defaults.sql`:
//! las aserciones de `test_import_preferences_persistence_and_defaults` que exigían
//! `!favorite_albums` y `!favorite_artists` describían los defaults **previos** a C-08.
//! Se actualizan al contrato de C-08 (activos: favoritos, álbumes, artistas,
//! playlists; desactivados: compras, historial, apariciones — ver
//! `docs/Errores-Pruebas-2026-10-08.md:171-172`).
//!
//! El caso explícito de fila **anterior** a 0092, y de fila nueva, lo cubre
//! `test_0092_defaults_apply_to_new_rows_and_preserve_existing_user_rows` de este
//! mismo fichero. La comprobación equivalente a nivel de semilla de 0008 está en
//! `src-tauri/tests/service_sync_settings_migration_test.rs::new_services_sync_albums_and_artists_by_default_without_resetting_users`.
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
    // C-08: álbumes y artistas pasan a activos por defecto (migración 0092).
    assert!(default_prefs.favorite_albums);
    assert!(default_prefs.favorite_artists);
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
        ..Default::default()
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

/// C-08, contrato completo de la migración `0092_import_preferences_defaults.sql`:
///
/// 1. Una fila creada DESPUÉS de 0092 recibe el alcance por defecto nuevo
///    (`sync_albums=1`, `sync_favorite_artists=1`).
/// 2. Una fila ANTERIOR a 0092 que un usuario sí configuró (tiene `last_synced`)
///    conserva sus valores (`false`): la migración reconstruye la tabla y solo
///    reescribe las filas cuyo `last_synced` es NULL.
#[tokio::test]
async fn test_0092_defaults_apply_to_new_rows_and_preserve_existing_user_rows() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    // Apply migrations up to 0091 only: state BEFORE 0092 existed.
    let migrator = sqlx::migrate!("./migrations");
    let mut pre_0092_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 91 {
            pre_0092_migrations.push(m.clone());
        }
    }

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(pre_0092_migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    partial_migrator
        .run(&pool)
        .await
        .expect("Failed to apply migrations up to 0091");

    // A row that existed BEFORE 0092 and was actually used by a user:
    // it chose not to import albums nor artists, and it did sync at least once.
    sqlx::query(
        "INSERT INTO service_sync_settings
           (service_name, sync_favorites, sync_albums, sync_favorite_artists,
            sync_playlists, sync_purchases, sync_library_history,
            sync_include_appearances, incremental_sync, last_synced)
         VALUES ('pre0092_service', 1, 0, 0, 1, 0, 0, 0, 1, '2026-10-01 12:00:00')",
    )
    .execute(&pool)
    .await
    .expect("Failed to insert pre-0092 row");

    // A pre-0092 row nobody ever configured or synced (like the 0008 seeds).
    sqlx::query(
        "INSERT INTO service_sync_settings
           (service_name, sync_favorites, sync_albums, sync_favorite_artists,
            sync_playlists, sync_purchases, sync_library_history,
            sync_include_appearances, incremental_sync, last_synced)
         VALUES ('pre0092_untouched', 1, 0, 0, 1, 0, 0, 0, 1, NULL)",
    )
    .execute(&pool)
    .await
    .expect("Failed to insert untouched pre-0092 row");

    // Sanity check: before 0092 the old defaults really were off.
    let pre_0092 = perform_get_service_import_preferences(&pool, "pre0092_service")
        .await
        .expect("Reading pre-0092 row must succeed");
    assert!(!pre_0092.favorite_albums);
    assert!(!pre_0092.favorite_artists);

    // Now apply 0092 (and anything after it).
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply migration 0092");

    // 1. Existing (pre-0092, user-configured) row keeps its values: no reset.
    let after_0092 = perform_get_service_import_preferences(&pool, "pre0092_service")
        .await
        .expect("Reading pre-0092 row after 0092 must succeed");
    assert!(
        !after_0092.favorite_albums,
        "pre-0092 row with last_synced must keep favorite_albums=false"
    );
    assert!(
        !after_0092.favorite_artists,
        "pre-0092 row with last_synced must keep favorite_artists=false"
    );

    // 2. A pre-0092 row nobody ever synced receives the new defaults (C-08).
    let untouched = perform_get_service_import_preferences(&pool, "pre0092_untouched")
        .await
        .expect("Reading untouched pre-0092 row must succeed");
    assert!(
        untouched.favorite_albums,
        "pre-0092 row with last_synced NULL must get favorite_albums=true (C-08)"
    );
    assert!(untouched.favorite_artists);

    // 3. A genuinely NEW row (created after 0092) gets the C-08 defaults.
    sqlx::query("INSERT INTO service_sync_settings (service_name) VALUES ('post0092_service')")
        .execute(&pool)
        .await
        .expect("Failed to insert post-0092 row");
    let post_0092 = perform_get_service_import_preferences(&pool, "post0092_service")
        .await
        .expect("Reading post-0092 row must succeed");
    assert!(
        post_0092.favorite_albums,
        "new row must default favorite_albums=true (C-08)"
    );
    assert!(
        post_0092.favorite_artists,
        "new row must default favorite_artists=true (C-08)"
    );
    assert!(post_0092.favorite_tracks);
    assert!(post_0092.playlists);
    assert!(!post_0092.purchases);
    assert!(!post_0092.library_history);
    assert!(!post_0092.include_appearances);
}

#[tokio::test]
async fn test_sync_service_without_valid_auth_fails_immediately_with_requires_auth() {
    let pool = setup_test_db().await;

    // Case 1: No account connected at all
    let err_missing = perform_sync_service(&pool, "qobuz", None, None)
        .await
        .unwrap_err();
    assert!(
        err_missing.starts_with("RequiresAuth"),
        "Error must be structured RequiresAuth: {}",
        err_missing
    );

    // Case 2: Account exists but missing token
    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let empty_creds = serde_json::json!({ "user_id": "888" }).to_string();
    let encrypted = crypto::encrypt(&empty_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'No Token Qobuz', ?, 1) RETURNING id"#,
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err_no_token = perform_sync_service(&pool, "qobuz", Some(account_id), None)
        .await
        .unwrap_err();
    assert!(
        err_no_token.starts_with("RequiresAuth"),
        "Must return RequiresAuth without token: {}",
        err_no_token
    );
    assert!(
        !err_no_token.contains("favorites error"),
        "Must never map missing auth to favorites error"
    );
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
           VALUES (?, 'Counts Test Account', 1) RETURNING id"#,
    )
    .bind(qobuz_svc_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 1. Simulate inserting 3 favorite tracks and 2 album-only tracks
    let _artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('The Wall') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Favorite tracks (is_liked = 1)
    for i in 1..=3 {
        let tid: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES (?, ?) RETURNING id")
                .bind(format!("Fav Track {}", i))
                .bind(album_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked, added_at) VALUES (?, ?, 1, CURRENT_TIMESTAMP)")
            .bind(account_id)
            .bind(tid)
            .execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, availability_status) VALUES (?, ?, ?, 1, 'available')")
            .bind(tid).bind(qobuz_svc_id).bind(format!("qobuz_fav_{}", i)).execute(&pool).await.unwrap();
    }

    // Non-favorite album tracks (is_liked = 0)
    for i in 4..=5 {
        let tid: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES (?, ?) RETURNING id")
                .bind(format!("Album Only Track {}", i))
                .bind(album_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query(
            "INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 0)",
        )
        .bind(account_id)
        .bind(tid)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, availability_status) VALUES (?, ?, ?, 1, 'available')")
            .bind(tid).bind(qobuz_svc_id).bind(format!("qobuz_alb_{}", i)).execute(&pool).await.unwrap();
    }

    // Verify database counts
    let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    let favorite_tracks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM library_entries WHERE account_id = ? AND is_liked = 1",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let all_imported_entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(total_tracks, 5, "Total tracks in library should be 5");
    assert_eq!(favorite_tracks, 3, "Favorite tracks count must be 3");
    assert_eq!(
        all_imported_entries, 5,
        "Total imported library entries must be 5"
    );

    // Re-inserting with INSERT OR IGNORE / ON CONFLICT must be completely idempotent
    for i in 1..=3 {
        let tid: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = ?")
            .bind(format!("Fav Track {}", i))
            .fetch_one(&pool)
            .await
            .unwrap();

        let _ = sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1) ON CONFLICT(account_id, track_id) DO NOTHING")
            .bind(account_id)
            .bind(tid)
            .execute(&pool).await;
    }

    let tracks_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    let entries_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        tracks_after, 5,
        "Idempotent insert must not duplicate tracks"
    );
    assert_eq!(
        entries_after, 5,
        "Idempotent insert must not duplicate library entries"
    );
}
