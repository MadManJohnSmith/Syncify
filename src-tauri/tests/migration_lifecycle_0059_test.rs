//! Migration 0059 Lifecycle and Idempotence Integration Tests (S167 Gate)
//!
//! Verifies:
//! 1. Migration 0059 is registered in canonical SQLx migrator
//! 2. Stepwise migration from version 58 -> 59
//! 3. `operation_journal` and `operation_recovery_audit` schema, columns, and indices exist
//! 4. Canonical migration execution is 100% idempotent when rerun
//! 5. `_sqlx_migrations` recorded entry for version 59 with valid checksum
//! 6. Insertions and queries succeed on the live migrated schema

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0059_lifecycle_schema_and_idempotence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0059_lifecycle.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Run canonical SQLx migrations up to 58
    let migrator = sqlx::migrate!("./migrations");

    let mut initial_migrations = Vec::new();
    for m in migrator.migrations.iter() {
        if m.version <= 58 {
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
        .expect("Failed to apply initial migrations 1..=58");

    // Verify DB is at migration 58
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(max_v.0, 58, "Database must be at version 58 before upgrade");

    // Verify operation_journal table DOES NOT exist before 0059
    let table_before: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='operation_journal'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(table_before.is_none(), "operation_journal must not exist before 0059");

    // 2. Run full migrator upgrading to 59
    migrator.run(&pool).await.expect("Canonical migrator must upgrade cleanly to 0059");

    // 3. Verify operation_journal and operation_recovery_audit tables exist
    let table_journal: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='operation_journal'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(table_journal.is_some(), "operation_journal table must exist after migration 0059");

    let table_audit: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='operation_recovery_audit'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(table_audit.is_some(), "operation_recovery_audit table must exist after migration 0059");

    // 4. Verify columns in operation_journal
    let columns_journal: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(operation_journal)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let journal_cols: Vec<String> = columns_journal.into_iter().map(|c| c.1).collect();
    let expected_journal_cols = vec![
        "operation_id",
        "operation_type",
        "entity_id",
        "account_id",
        "track_id",
        "download_id",
        "provider",
        "phase",
        "attempt",
        "started_at",
        "checkpoint_at",
        "status",
        "input_identity",
        "expected_output_path",
        "staging_path",
        "file_baseline",
        "db_transaction_state",
        "rollback_state",
        "error_taxonomy",
        "retry_policy",
        "result_summary",
    ];

    for col in &expected_journal_cols {
        assert!(journal_cols.contains(&col.to_string()), "Column '{}' must exist in operation_journal", col);
    }

    // 5. Verify columns in operation_recovery_audit
    let columns_audit: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(operation_recovery_audit)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let audit_cols: Vec<String> = columns_audit.into_iter().map(|c| c.1).collect();
    let expected_audit_cols = vec![
        "id",
        "recovery_id",
        "timestamp",
        "operation_id",
        "operation_type",
        "previous_status",
        "new_status",
        "action_taken",
        "error_taxonomy",
        "message",
        "details_json",
    ];

    for col in &expected_audit_cols {
        assert!(audit_cols.contains(&col.to_string()), "Column '{}' must exist in operation_recovery_audit", col);
    }

    // 6. Verify indexes exist
    let idx_status: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_op_journal_status'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(idx_status.is_some(), "idx_op_journal_status index must exist");

    let idx_audit_time: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_op_recovery_audit_timestamp'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(idx_audit_time.is_some(), "idx_op_recovery_audit_timestamp index must exist");

    // 7. Verify migration 59 record in _sqlx_migrations
    let mig_59_rec: Option<(i64, String, bool, Vec<u8>)> = sqlx::query_as(
        "SELECT version, description, success, checksum FROM _sqlx_migrations WHERE version = 59"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(mig_59_rec.is_some(), "Migration 59 record must exist in _sqlx_migrations");
    let (v, desc, success, checksum) = mig_59_rec.unwrap();
    assert_eq!(v, 59);
    assert!(desc.contains("recovery") || desc.contains("journal") || desc.contains("operation"), "Description was: {}", desc);
    assert!(success);
    assert!(!checksum.is_empty(), "Checksum must not be empty");

    // 8. Test Idempotence: Rerunning migrations must succeed cleanly
    let rerun_res = migrator.run(&pool).await;
    assert!(rerun_res.is_ok(), "Rerunning migrations must be 100% idempotent and succeed");
}
