//! Tests for TASK-126: SQLx Migrations Integrity, Idempotency and Anti-Tamper Verification
//!
//! Verifies:
//! 1. Clean in-memory database application of all migrations (0001 through latest).
//! 2. Every applied migration record in `_sqlx_migrations` has `success = true` and a valid 48-byte SHA-384 checksum.
//! 3. 1:1 match between canonical compile-time migrator metadata and records in `_sqlx_migrations`.
//! 4. Idempotency: re-running `migrator.run(&pool)` succeeds without re-applying or altering hashes.
//! 5. Structural integrity: `PRAGMA integrity_check` and `PRAGMA foreign_key_check` report 0 errors.
//! 6. Anti-tampering: modifying a recorded checksum triggers a sqlx `VersionMismatch` failure upon subsequent migration runs.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;

#[tokio::test]
async fn test_sqlx_migrations_clean_application_and_checksum_integrity() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to clean in-memory SQLite database");

    let migrator = sqlx::migrate!("./migrations");
    let compile_time_migrations: Vec<_> = migrator.iter().collect();

    assert!(
        !compile_time_migrations.is_empty(),
        "Compile-time migrator must discover at least one migration"
    );

    // 1. Execute complete migration pipeline
    migrator
        .run(&pool)
        .await
        .expect("All canonical migrations must apply cleanly from scratch without checksum errors");

    // 2. Fetch all applied records from _sqlx_migrations
    let rows = sqlx::query(
        "SELECT version, description, installed_on, success, checksum, execution_time \
         FROM _sqlx_migrations ORDER BY version ASC"
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query _sqlx_migrations");

    assert_eq!(
        rows.len(),
        compile_time_migrations.len(),
        "Number of applied migrations in DB ({}) must match migrator definitions ({})",
        rows.len(),
        compile_time_migrations.len()
    );

    // 3. Verify each migration entry against compile-time definitions
    for (row, expected_mig) in rows.iter().zip(compile_time_migrations.iter()) {
        let version: i64 = row.get("version");
        let description: String = row.get("description");
        let success: bool = row.get("success");
        let checksum: Vec<u8> = row.get("checksum");
        let execution_time: i64 = row.get("execution_time");

        assert_eq!(
            version, expected_mig.version,
            "Migration version mismatch: DB has {}, expected {}",
            version, expected_mig.version
        );

        assert_eq!(
            description, expected_mig.description.as_ref(),
            "Migration description mismatch for version {}",
            version
        );

        assert!(
            success,
            "Migration {} was recorded as failed in _sqlx_migrations",
            version
        );

        // SQLx uses SHA-384 for migration checksums (48 bytes)
        assert_eq!(
            checksum.len(),
            48,
            "Migration {} checksum length must be 48 bytes (SHA-384), got {}",
            version,
            checksum.len()
        );

        assert_eq!(
            checksum.as_slice(),
            expected_mig.checksum.as_ref(),
            "Migration {} recorded checksum does not match canonical compile-time checksum",
            version
        );

        assert!(
            execution_time >= 0,
            "Migration {} has negative execution_time: {}",
            version,
            execution_time
        );
    }

    // 4. Verify version boundaries (from 0001 up to at least 0067)
    let min_v: (i64,) = sqlx::query_as("SELECT MIN(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Query MIN(version)");
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Query MAX(version)");

    assert_eq!(min_v.0, 1, "First migration version must be 1 (0001_init)");
    assert!(
        max_v.0 >= 67,
        "Max migration version must be at least 67, found {}",
        max_v.0
    );

    // 5. Verify database integrity
    let integrity: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check");
    assert_eq!(integrity.0, "ok", "Database PRAGMA integrity_check must be 'ok'");

    let fk_violations: Vec<(String, Option<i64>, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("PRAGMA foreign_key_check");
    assert!(
        fk_violations.is_empty(),
        "Clean migration must yield 0 foreign key check violations: {:?}",
        fk_violations
    );

    // 6. Verify idempotency: re-running migrator must succeed without side-effects
    migrator
        .run(&pool)
        .await
        .expect("Re-running migrator on up-to-date database must be an idempotent no-op");
}

#[tokio::test]
async fn test_sqlx_migrations_anti_tamper_guard() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to clean in-memory SQLite database");

    let migrator = sqlx::migrate!("./migrations");

    // Apply migrations
    migrator
        .run(&pool)
        .await
        .expect("Initial migration application must succeed");

    // Intentionally tamper with a recorded checksum in _sqlx_migrations (mimicking ad-hoc falsification)
    let corrupted_checksum = vec![0xDE, 0xAD, 0xBE, 0xEF];
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = 1")
        .bind(corrupted_checksum)
        .execute(&pool)
        .await
        .expect("Failed to execute test tampering");

    // Verify that sqlx detects the checksum mismatch on subsequent run
    let rerun_result = migrator.run(&pool).await;
    assert!(
        rerun_result.is_err(),
        "SQLx migrator must reject execution when _sqlx_migrations checksum has been tampered with"
    );

    let err_str = rerun_result.unwrap_err().to_string();
    assert!(
        err_str.to_lowercase().contains("modified")
            || err_str.to_lowercase().contains("checksum")
            || err_str.to_lowercase().contains("mismatch"),
        "Error message should mention modified/checksum/mismatch, got: {}",
        err_str
    );
}
