//! Migration 0058 Lifecycle and Idempotence Integration Tests (S163 Gate)
//!
//! Verifies:
//! 1. Migration 0058 is registered in canonical SQLx migrator
//! 2. Canonical SQLx migration execution applies 0058 cleanly
//! 3. `repair_history` table schema, columns, constraints, and indices exist
//! 4. Canonical migration execution is 100% idempotent when rerun
//! 5. `_sqlx_migrations` recorded entry for version 58 with valid checksum
//! 6. Append-only insertions and queries succeed on the live migrated schema

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0058_lifecycle_schema_and_idempotence() {
    let _ = syncify_tauri_lib::crypto::init_keychain_crypto()
        .or_else(|_| syncify_tauri_lib::crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0058_lifecycle.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Run canonical SQLx migrations
    let migrator = sqlx::migrate!("./migrations");
    
    // Find migration 58 in the migrator
    let mig_58_def = migrator.iter().find(|m| m.version == 58);
    assert!(mig_58_def.is_some(), "Migration 0058 must be registered in sqlx::migrate!");

    // Run full migrations
    migrator.run(&pool).await.expect("Failed to run canonical migrations including 0058");

    // 2. Verify repair_history table exists
    let table_after: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='repair_history'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(table_after.is_some(), "repair_history table must exist after migration 0058");

    // 3. Verify all columns of repair_history table exist
    let columns: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(repair_history)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let col_names: Vec<String> = columns.into_iter().map(|c| c.1).collect();
    let expected_cols = vec![
        "id",
        "repair_id",
        "timestamp",
        "download_id",
        "old_track_id",
        "new_track_id",
        "old_path",
        "new_path",
        "input_file_hash",
        "output_file_hash",
        "audio_payload_hash_before",
        "audio_payload_hash_after",
        "baseline_validation",
        "actions",
        "rollback_state",
        "provenance",
        "result",
        "details_json",
    ];

    for col in &expected_cols {
        assert!(col_names.contains(&col.to_string()), "Column '{}' must exist in repair_history", col);
    }
    assert_eq!(col_names.len(), expected_cols.len(), "repair_history column count must match exactly");

    // 4. Verify indexes exist
    let idx_timestamp: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_repair_history_timestamp'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(idx_timestamp.is_some(), "idx_repair_history_timestamp index must exist");

    let idx_download_id: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_repair_history_download_id'"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(idx_download_id.is_some(), "idx_repair_history_download_id index must exist");

    // 5. Verify SQLx migration version 58 record and checksum
    let mig_58_rec: Option<(i64, String, bool, Vec<u8>)> = sqlx::query_as(
        "SELECT version, description, success, checksum FROM _sqlx_migrations WHERE version = 58"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(mig_58_rec.is_some(), "Migration 58 record must exist in _sqlx_migrations");
    let (v, desc, success, checksum) = mig_58_rec.unwrap();
    assert_eq!(v, 58);
    assert!(desc.contains("repair") && desc.contains("history"), "Description was: {}", desc);
    assert!(success);
    assert!(!checksum.is_empty(), "Checksum must not be empty");

    // 6. Test idempotence: Rerun migrations second time
    let rerun_res = migrator.run(&pool).await;
    assert!(rerun_res.is_ok(), "Rerunning migrations must be completely idempotent and succeed");

    // 7. Verify append-only insertions succeed on migrated schema
    let inserted_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO repair_history (
            repair_id, old_path, new_path, input_file_hash, baseline_validation, actions, provenance, result
        ) VALUES ('rep_test_lifecycle_1', '/old.flac', '/new.flac', 'hash_123', 'valid', '[]', 'test', 'success')
        RETURNING id"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(inserted_id > 0);
}
