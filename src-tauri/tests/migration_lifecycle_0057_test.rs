//! Integration Test for Canonical SQLx Migration Lifecycle 0057 (S145 Validation Gate)
//! Verifies:
//! 1. Stepwise migration from state 0056 to 0057 using SQLx canonical migrator.
//! 2. Clean application of 0057 without manual `_sqlx_migrations` manipulation.
//! 3. Schema physical correctness of `service_album_availability` and its indexes.
//! 4. 100% idempotency on repeated execution.

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_canonical_sqlx_migration_0057_lifecycle_and_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_lifecycle_0057_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test SQLite database");

    // 1. Compile the migrations
    let migrator = sqlx::migrate!("./migrations");

    // 2. Run migrations 1 through 56 incrementally
    let mut initial_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 56 {
            initial_migrations.push(m.clone());
        }
    }

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(initial_migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };

    partial_migrator
        .run(&pool)
        .await
        .expect("Failed to apply initial migrations 1..=56");

    // Verify DB is at migration 56
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(max_v.0, 56, "Database must be at version 56 before upgrade");

    // Verify service_album_availability table DOES NOT exist yet
    let table_exists: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='service_album_availability'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(table_exists.is_none(), "service_album_availability must not exist before 0057");

    // 3. Execute the full canonical SQLx migrator to upgrade from 56 -> 57
    migrator
        .run(&pool)
        .await
        .expect("Canonical SQLx migrator must upgrade cleanly from 56 to 57");

    // 4. Verify that SQLx registered 0057 in `_sqlx_migrations`
    let row_57: (i64, String, bool, Vec<u8>) = sqlx::query_as(
        "SELECT version, description, success, checksum FROM _sqlx_migrations WHERE version = 57"
    )
    .fetch_one(&pool)
    .await
    .expect("Migration 57 must be present in _sqlx_migrations");

    assert_eq!(row_57.0, 57);
    assert_eq!(row_57.1, "tidal album expansion resilience");
    assert!(row_57.2, "Migration 57 must be marked success = true");

    // 5. Verify physical schema correctness of service_album_availability
    let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('service_album_availability')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let col_names: Vec<String> = cols.iter().map(|(_, n, _, _, _, _)| n.clone()).collect();
    assert!(col_names.contains(&"service_id".to_string()));
    assert!(col_names.contains(&"service_album_id".to_string()));
    assert!(col_names.contains(&"availability_status".to_string()));
    assert!(col_names.contains(&"http_status".to_string()));
    assert!(col_names.contains(&"sub_status".to_string()));
    assert!(col_names.contains(&"reason".to_string()));
    assert!(col_names.contains(&"last_checked".to_string()));

    // Verify index exists
    let index_exists: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_service_album_avail_status'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(index_exists.is_some(), "Index idx_service_album_avail_status must exist");

    // 6. Test Idempotency: Running canonical migrator again on the upgraded DB must succeed without error
    let idempotency_res = migrator.run(&pool).await;
    assert!(
        idempotency_res.is_ok(),
        "Repeated canonical migration run must be 100% idempotent and succeed"
    );

    let total_migrations: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        total_migrations.0, 53,
        "Total migrations count must remain exactly 53 without duplicates"
    );
}
