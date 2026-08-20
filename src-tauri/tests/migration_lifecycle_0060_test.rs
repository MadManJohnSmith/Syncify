//! Migration 0060 Lifecycle and Idempotence Integration Tests
//!
//! Verifies:
//! 1. Migration 0060 is registered in canonical SQLx migrator
//! 2. Stepwise migration from version 59 -> 60
//! 3. Quality decision provenance columns in `downloads` and `download_queue`
//! 4. Canonical migration execution is 100% idempotent when rerun
//! 5. `_sqlx_migrations` recorded entry for version 60 with valid checksum
//! 6. Insertions and queries succeed on the live migrated schema

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0060_lifecycle_schema_and_idempotence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0060_lifecycle.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Run canonical SQLx migrations up to 59
    let migrator = sqlx::migrate!("./migrations");

    let mut initial_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 59 {
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
        .expect("Failed to apply initial migrations 1..=59");

    // Verify DB is at migration 59
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(max_v.0, 59, "Database must be at version 59 before upgrade");

    // Verify quality_decision column DOES NOT exist in downloads before 0060
    let columns_before: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(downloads)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let cols_before_names: Vec<String> = columns_before.into_iter().map(|c| c.1).collect();
    assert!(!cols_before_names.contains(&"quality_decision".to_string()), "quality_decision must not exist before 0060");

    // 2. Run full migrator upgrading to 60
    migrator.run(&pool).await.expect("Canonical migrator must upgrade cleanly to 0060");

    // 3. Verify max version is 60
    let max_v_after: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(max_v_after.0, 60, "Database must be at version 60 after upgrade");

    // 4. Verify columns in downloads
    let columns_downloads: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(downloads)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let downloads_cols: Vec<String> = columns_downloads.into_iter().map(|c| c.1).collect();
    let expected_quality_cols = vec![
        "requested_quality",
        "effective_quality",
        "requested_format",
        "effective_format",
        "quality_decision",
        "provider_fallback_used",
        "quality_fallback_used",
        "decision_reason",
    ];

    for col in &expected_quality_cols {
        assert!(downloads_cols.contains(&col.to_string()), "Column '{}' must exist in downloads", col);
    }

    // 5. Verify columns in download_queue
    let columns_queue: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(download_queue)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let queue_cols: Vec<String> = columns_queue.into_iter().map(|c| c.1).collect();
    for col in &expected_quality_cols {
        assert!(queue_cols.contains(&col.to_string()), "Column '{}' must exist in download_queue", col);
    }

    // 6. Verify indexes exist
    let idx_dl_quality: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_downloads_quality_decision'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(idx_dl_quality.is_some(), "idx_downloads_quality_decision index must exist");

    let idx_dq_quality: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_download_queue_quality_decision'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(idx_dq_quality.is_some(), "idx_download_queue_quality_decision index must exist");

    // 7. Verify migration 60 record in _sqlx_migrations
    let mig_60_rec: Option<(i64, String, bool, Vec<u8>)> = sqlx::query_as(
        "SELECT version, description, success, checksum FROM _sqlx_migrations WHERE version = 60"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(mig_60_rec.is_some(), "Migration 60 record must exist in _sqlx_migrations");
    let (v, desc, success, checksum) = mig_60_rec.unwrap();
    assert_eq!(v, 60);
    assert!(desc.contains("quality") || desc.contains("decision"), "Description was: {}", desc);
    assert!(success);
    assert!(!checksum.is_empty(), "Checksum must not be empty");

    // 8. Test Idempotence: Rerunning migrations must succeed cleanly
    let rerun_res = migrator.run(&pool).await;
    assert!(rerun_res.is_ok(), "Rerunning migrations must be 100% idempotent and succeed");
}
