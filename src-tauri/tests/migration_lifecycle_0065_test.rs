//! Migration 0065 Lifecycle, Schema Alignment, and Index Integrity Test
//! TASK-32: Schema alignment (artists.qobuz_id) and critical performance indexes.

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0065_clean_run_schema_and_indexes() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0065_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Run canonical SQLx migrations (0001 -> 0065)
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Canonical migrator must upgrade cleanly from 0001 to 0065");

    // 2. Verify migration version in _sqlx_migrations
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Must fetch max migration version");
    assert!(max_v.0 >= 65, "Database must be at migration version >= 65");

    // 3. Verify column qobuz_id exists in artists
    let columns_artists: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "PRAGMA table_info(artists)"
    )
    .fetch_all(&pool)
    .await
    .expect("Must fetch artists table info");

    let artist_cols: Vec<String> = columns_artists.into_iter().map(|c| c.1).collect();
    assert!(
        artist_cols.contains(&"qobuz_id".to_string()),
        "Column 'qobuz_id' must exist in artists table"
    );

    // 4. Verify the 5 declared indexes exist in sqlite_master
    let expected_indexes = [
        "idx_artists_qobuz_id",
        "idx_download_queue_track_id",
        "idx_track_artists_artist_id",
        "idx_album_artists_artist_id",
        "idx_tracks_qobuz_id",
    ];

    for idx in expected_indexes {
        let exists: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?"
        )
        .bind(idx)
        .fetch_one(&pool)
        .await
        .expect("Failed to query index existence");

        assert_eq!(
            exists.0, 1,
            "Index {} must exist in sqlite_master",
            idx
        );
    }

    // 5. Test uniqueness on artists.qobuz_id (WHERE qobuz_id IS NOT NULL)
    sqlx::query("INSERT INTO artists (id, name, qobuz_id) VALUES (1, 'Artist One', 'qobuz_123')")
        .execute(&pool)
        .await
        .expect("Inserting artist with qobuz_id must succeed");

    // Inserting another artist with same qobuz_id must fail
    let dup_res = sqlx::query("INSERT INTO artists (id, name, qobuz_id) VALUES (2, 'Artist Two', 'qobuz_123')")
        .execute(&pool)
        .await;
    assert!(
        dup_res.is_err(),
        "Duplicate artists.qobuz_id must violate UNIQUE constraint idx_artists_qobuz_id"
    );

    // Inserting multiple artists with NULL qobuz_id must succeed (partial index)
    sqlx::query("INSERT INTO artists (id, name, qobuz_id) VALUES (3, 'Artist Three', NULL)")
        .execute(&pool)
        .await
        .expect("Inserting artist with NULL qobuz_id must succeed");

    sqlx::query("INSERT INTO artists (id, name, qobuz_id) VALUES (4, 'Artist Four', NULL)")
        .execute(&pool)
        .await
        .expect("Inserting second artist with NULL qobuz_id must succeed");

    // 6. Test EXPLAIN QUERY PLAN to verify index usage (no sequential scan)
    // 6a. Query artists by qobuz_id
    let qp_artist: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM artists WHERE qobuz_id = ?"
    )
    .bind("qobuz_123")
    .fetch_all(&pool)
    .await
    .expect("EXPLAIN QUERY PLAN for artists.qobuz_id failed");

    let detail_artist = qp_artist.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_artist.contains("USING INDEX idx_artists_qobuz_id")
            || detail_artist.contains("USING COVERING INDEX idx_artists_qobuz_id"),
        "Query on artists.qobuz_id must use index idx_artists_qobuz_id without sequential scan. Detail: {}",
        detail_artist
    );

    // 6b. Query tracks by qobuz_id
    let qp_tracks: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM tracks WHERE qobuz_id = ?"
    )
    .bind("qobuz_track_1")
    .fetch_all(&pool)
    .await
    .expect("EXPLAIN QUERY PLAN for tracks.qobuz_id failed");

    let detail_tracks = qp_tracks.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_tracks.contains("USING INDEX idx_tracks_qobuz_id")
            || detail_tracks.contains("USING COVERING INDEX idx_tracks_qobuz_id"),
        "Query on tracks.qobuz_id must use index idx_tracks_qobuz_id. Detail: {}",
        detail_tracks
    );

    // 6c. Query download_queue by track_id
    let qp_queue: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM download_queue WHERE track_id = ?"
    )
    .bind(101)
    .fetch_all(&pool)
    .await
    .expect("EXPLAIN QUERY PLAN for download_queue.track_id failed");

    let detail_queue = qp_queue.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_queue.contains("USING INDEX idx_download_queue_track_id")
            || detail_queue.contains("USING COVERING INDEX idx_download_queue_track_id"),
        "Query on download_queue.track_id must use index idx_download_queue_track_id. Detail: {}",
        detail_queue
    );

    // 6d. Query track_artists by artist_id
    let qp_ta: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM track_artists WHERE artist_id = ?"
    )
    .bind(1)
    .fetch_all(&pool)
    .await
    .expect("EXPLAIN QUERY PLAN for track_artists.artist_id failed");

    let detail_ta = qp_ta.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_ta.contains("idx_track_artists_artist"),
        "Query on track_artists.artist_id must use index. Detail: {}",
        detail_ta
    );

    // 6e. Query album_artists by artist_id
    let qp_aa: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM album_artists WHERE artist_id = ?"
    )
    .bind(1)
    .fetch_all(&pool)
    .await
    .expect("EXPLAIN QUERY PLAN for album_artists.artist_id failed");

    let detail_aa = qp_aa.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_aa.contains("idx_album_artists_artist"),
        "Query on album_artists.artist_id must use index. Detail: {}",
        detail_aa
    );
}
