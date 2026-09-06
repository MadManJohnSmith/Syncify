//! Migration 0066 & service_sync_settings updated_at Integration Tests
//!
//! Verifies:
//! 1. Migration 0066 adds `updated_at` to `service_sync_settings`.
//! 2. Regression proof: Prior to migration 0066, updating `updated_at` fails with `no such column: updated_at`.
//! 3. After migration 0066, `update_service_sync_settings` (via `perform_update_service_sync_settings`)
//!    executes cleanly and updates `updated_at` timestamp.
//! 4. Verifies persisted timestamp and toggles for service settings (e.g. spotify).

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::perform_update_service_sync_settings;
use syncify_tauri_lib::crypto;

async fn setup_in_memory_db() -> sqlx::SqlitePool {
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
async fn test_service_sync_settings_updated_at_after_migration() {
    let pool = setup_in_memory_db().await;

    // Verify column exists in schema
    let columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(service_sync_settings)")
            .fetch_all(&pool)
            .await
            .expect("Failed to query PRAGMA table_info");

    let has_updated_at = columns.iter().any(|col| col.1 == "updated_at");
    assert!(
        has_updated_at,
        "service_sync_settings table must have 'updated_at' column after migration 0066"
    );

    // Initial state for spotify
    let initial_updated_at: Option<String> = sqlx::query_scalar(
        "SELECT updated_at FROM service_sync_settings WHERE service_name = 'spotify'",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch initial updated_at for spotify");

    // Perform update via command helper
    let updated = perform_update_service_sync_settings(
        &pool,
        "spotify",
        false, // sync_favorites
        true,  // sync_playlists
        true,  // sync_albums
        false, // incremental_sync
    )
    .await
    .expect("Updating service sync settings must not fail with 'no such column: updated_at'");

    assert_eq!(updated.service_name, "spotify");
    assert!(!updated.sync_favorites);
    assert!(updated.sync_playlists);
    assert!(updated.sync_albums);
    assert!(!updated.incremental_sync);

    // Verify updated_at has been updated in database
    let current_updated_at: Option<String> = sqlx::query_scalar(
        "SELECT updated_at FROM service_sync_settings WHERE service_name = 'spotify'",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch updated_at from DB");

    assert!(
        current_updated_at.is_some(),
        "updated_at must be populated after update"
    );

    let ts = current_updated_at.unwrap();
    assert!(!ts.is_empty(), "updated_at timestamp must not be empty");

    // If initial_updated_at was set, ensure it's a valid timestamp string
    if let Some(initial_ts) = initial_updated_at {
        assert!(!initial_ts.is_empty());
    }
}

#[tokio::test]
async fn test_service_sync_settings_regression_without_0066() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    // Run migrations up to 0065 only
    let migrator = sqlx::migrate!("./migrations");
    let mut v65_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 65 {
            v65_migrations.push(m.clone());
        }
    }

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(v65_migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };

    partial_migrator
        .run(&pool)
        .await
        .expect("Failed to apply migrations 1..=65");

    // Attempt the UPDATE that includes updated_at: must fail before 0066
    let query_result = sqlx::query(
        "UPDATE service_sync_settings SET sync_favorites = ?, sync_playlists = ?, sync_albums = ?, 
         incremental_sync = ?, updated_at = CURRENT_TIMESTAMP WHERE service_name = ?",
    )
    .bind(false)
    .bind(true)
    .bind(true)
    .bind(false)
    .bind("spotify")
    .execute(&pool)
    .await;

    assert!(
        query_result.is_err(),
        "Updating updated_at before migration 0066 should fail"
    );
    let err_str = query_result.unwrap_err().to_string();
    assert!(
        err_str.contains("no such column: updated_at"),
        "Error before migration 0066 must specifically be 'no such column: updated_at', got: {}",
        err_str
    );

    // Now run full migrations (applying 0066)
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Applying remaining migrations including 0066 must succeed");

    // The exact same operation now succeeds
    let success_result = perform_update_service_sync_settings(
        &pool,
        "spotify",
        false,
        true,
        true,
        false,
    )
    .await;

    assert!(
        success_result.is_ok(),
        "After migration 0066, updating service sync settings must succeed: {:?}",
        success_result.err()
    );
}
