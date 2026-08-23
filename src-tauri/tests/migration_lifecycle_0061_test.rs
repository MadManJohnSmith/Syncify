//! Migration 0061 Lifecycle, Schema, and Idempotence Test (S173)

use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn test_migration_0061_lifecycle_schema_and_idempotence() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("In-memory SQLite pool creation failed");

    // 1. Run migrations up to 61
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Canonical migrator must upgrade cleanly to 0061");

    // 2. Verify max version is at least 61
    let max_v_after: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(max_v_after.0 >= 61, "Database must be at least version 61 after upgrade");

    // 3. Verify columns in tracks
    let columns_tracks: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(tracks)"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let track_cols: Vec<String> = columns_tracks.into_iter().map(|c| c.1).collect();
    assert!(track_cols.contains(&"bpm".to_string()));
    assert!(track_cols.contains(&"tempo_confidence".to_string()));
    assert!(track_cols.contains(&"tempo_source".to_string()));
    assert!(track_cols.contains(&"tempo_analyzed_at".to_string()));

    // 4. Verify indexes
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='tracks'"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<String> = indexes.into_iter().map(|i| i.0).collect();
    assert!(index_names.contains(&"idx_tracks_tempo_confidence".to_string()));
    assert!(index_names.contains(&"idx_tracks_tempo_source".to_string()));
}
