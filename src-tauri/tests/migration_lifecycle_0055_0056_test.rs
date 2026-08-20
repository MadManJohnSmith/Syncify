//! Integration Test for Canonical SQLx Migration Lifecycle 0055 & 0056 (S144 Validation Gate)
//! Verifies:
//! 1. Stepwise migration from state 0054 to 0056 using SQLx canonical migrator.
//! 2. Clean application of 0055 and 0056 without manual `_sqlx_migrations` manipulation.
//! 3. Schema physical correctness on `tracks`, `downloads`, and `accounts`.
//! 4. 100% idempotency on repeated execution.

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_canonical_sqlx_migration_0055_and_0056_lifecycle_and_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_lifecycle_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test SQLite database");

    // 1. Compile the migrations
    let migrator = sqlx::migrate!("./migrations");

    // 2. Run migrations 1 through 54 incrementally
    let mut initial_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 54 {
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
        .expect("Failed to apply initial migrations 1..=54");

    // Verify DB is at migration 54
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(max_v.0, 54, "Database must be at version 54 before upgrade");

    // Verify 0055 and 0056 columns DO NOT exist yet
    let track_cols: Vec<(i64, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('tracks')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let track_col_names: Vec<String> = track_cols.into_iter().map(|(_, n)| n).collect();
    assert!(!track_col_names.contains(&"display_title".to_string()));
    assert!(!track_col_names.contains(&"source_title".to_string()));
    assert!(!track_col_names.contains(&"file_disambiguator".to_string()));

    let account_cols: Vec<(i64, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('accounts')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let account_col_names: Vec<String> = account_cols.into_iter().map(|(_, n)| n).collect();
    assert!(!account_col_names.contains(&"last_auth_checked_at".to_string()));

    // 3. Execute the stepwise SQLx migrator to upgrade from 54 -> 56
    let mut step_56_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 56 {
            step_56_migrations.push(m.clone());
        }
    }

    let step_56_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(step_56_migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };

    step_56_migrator
        .run(&pool)
        .await
        .expect("Stepwise SQLx migrator must upgrade cleanly from 54 to 56");

    // 4. Verify that SQLx registered 0055 and 0056 in `_sqlx_migrations`
    let rows_55_56: Vec<(i64, String, bool, Vec<u8>)> = sqlx::query_as(
        "SELECT version, description, success, checksum FROM _sqlx_migrations WHERE version >= 55 AND version <= 56 ORDER BY version"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows_55_56.len(), 2, "Must have exactly 2 new migration rows");

    // Check migration 55
    assert_eq!(rows_55_56[0].0, 55);
    assert_eq!(rows_55_56[0].1, "account auth telemetry and dedupe");
    assert!(rows_55_56[0].2, "Migration 55 must be marked success = true");
    let hex_55: String = rows_55_56[0].3.iter().map(|b| format!("{:02X}", b)).collect();
    assert_eq!(
        hex_55,
        "68097CB98B9596B6B957453DE254F2F2272F38164124FCE624852683217CE4A7D8DF0EA6F57CB1F54CA5CCC6EF42FEAD"
    );

    // Check migration 56
    assert_eq!(rows_55_56[1].0, 56);
    assert_eq!(rows_55_56[1].1, "track display and disambiguator");
    assert!(rows_55_56[1].2, "Migration 56 must be marked success = true");
    let hex_56: String = rows_55_56[1].3.iter().map(|b| format!("{:02X}", b)).collect();
    assert_eq!(
        hex_56,
        "0DFE64BC27306563E46BBCDE3FBD26E99851F22E7A415627B1B281A11B6B6EF26200D7A6DFDD5D1B8171E13D6D6350EC"
    );

    // 5. Verify physical schema correctness after canonical migration
    let track_cols_after: Vec<(i64, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('tracks')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let track_col_names_after: Vec<String> = track_cols_after.into_iter().map(|(_, n)| n).collect();
    assert!(track_col_names_after.contains(&"display_title".to_string()));
    assert!(track_col_names_after.contains(&"source_title".to_string()));
    assert!(track_col_names_after.contains(&"file_disambiguator".to_string()));

    let download_cols_after: Vec<(i64, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('downloads')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let download_col_names_after: Vec<String> =
        download_cols_after.into_iter().map(|(_, n)| n).collect();
    assert!(download_col_names_after.contains(&"file_disambiguator".to_string()));

    let account_cols_after: Vec<(i64, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('accounts')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let account_col_names_after: Vec<String> =
        account_cols_after.into_iter().map(|(_, n)| n).collect();
    assert!(account_col_names_after.contains(&"last_auth_checked_at".to_string()));

    // 6. Test Idempotency: Running step_56_migrator again on the upgraded DB must succeed without error
    let idempotency_res = step_56_migrator.run(&pool).await;
    assert!(
        idempotency_res.is_ok(),
        "Repeated canonical migration run must be 100% idempotent and succeed"
    );

    let total_migrations: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        total_migrations.0, 52,
        "Total migrations count must remain exactly 52 without duplicates"
    );
}
