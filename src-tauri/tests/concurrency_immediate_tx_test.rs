//! Concurrency regression test for TASK-35:
//! Verifies that `BEGIN IMMEDIATE` prevents `SQLITE_BUSY_SNAPSHOT` when multiple concurrent
//! workers perform read-then-write transactions on a shared SQLite database in WAL mode,
//! and verifies `ON CONFLICT(playlist_id, position) DO UPDATE SET track_id = excluded.track_id`.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

async fn setup_test_db() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("concurrency_test.db");

    let connect_opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_opts)
        .await
        .expect("Failed to connect to SQLite pool");

    sqlx::query("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 10000;")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS playlist_tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL DEFAULT 0,
            added_at TEXT,
            UNIQUE(playlist_id, position)
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create schema");

    (pool, dir)
}

#[tokio::test]
async fn test_begin_immediate_prevents_busy_snapshot_under_concurrent_writes() {
    let (pool, _dir) = setup_test_db().await;

    // Seed initial playlist and tracks
    sqlx::query("INSERT INTO playlists (id, name) VALUES (1, 'Rock Favorites')")
        .execute(&pool)
        .await
        .unwrap();

    for i in 1..=20 {
        sqlx::query("INSERT INTO tracks (id, title) VALUES (?, ?)")
            .bind(i)
            .bind(format!("Track {}", i))
            .execute(&pool)
            .await
            .unwrap();
    }

    let pool_arc = Arc::new(pool);
    let mut handles = vec![];

    // Spawn 8 concurrent worker tasks competing for write transactions
    for worker_id in 0..8 {
        let p = Arc::clone(&pool_arc);
        handles.push(tokio::spawn(async move {
            for step in 0..5 {
                let track_id = (worker_id * 2 + (step % 2) + 1) as i64;
                let position = (step * 8 + worker_id) as i64;

                // BEGIN IMMEDIATE acquires write lock immediately, serializing against other writers
                let mut tx = p.begin_with("BEGIN IMMEDIATE")
                    .await
                    .unwrap_or_else(|e| panic!("Worker {} step {} failed BEGIN IMMEDIATE: {}", worker_id, step, e));

                // Read within transaction
                let _count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = 1")
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();

                // Small yield to increase concurrency contention
                tokio::time::sleep(Duration::from_millis(5)).await;

                // Write within transaction using ON CONFLICT (playlist_id, position)
                sqlx::query(
                    "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                     VALUES (1, ?, ?)
                     ON CONFLICT(playlist_id, position) DO UPDATE SET track_id = excluded.track_id"
                )
                .bind(track_id)
                .bind(position)
                .execute(&mut *tx)
                .await
                .unwrap_or_else(|e| panic!("Worker {} step {} failed INSERT: {}", worker_id, step, e));

                tx.commit().await.expect("Commit must succeed without SQLITE_BUSY");
            }
        }));
    }

    for h in handles {
        h.await.expect("Worker thread panicked");
    }

    // Verify consistency
    let final_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = 1")
        .fetch_one(&*pool_arc)
        .await
        .unwrap();

    assert_eq!(final_count.0, 40, "All 40 track positions should be inserted without collision");
}

#[tokio::test]
async fn test_tidal_playlist_upsert_on_conflict_position() {
    let (pool, _dir) = setup_test_db().await;

    sqlx::query("INSERT INTO playlists (id, name) VALUES (1, 'Tidal Sync')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title) VALUES (10, 'Original Track'), (20, 'Updated Track')")
        .execute(&pool)
        .await
        .unwrap();

    // 1. Initial insert at position 0
    let mut tx1 = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position)
         VALUES (?, ?, ?)
         ON CONFLICT(playlist_id, position) DO UPDATE SET
             track_id = excluded.track_id"
    )
    .bind(1)
    .bind(10)
    .bind(0)
    .execute(&mut *tx1)
    .await
    .unwrap();
    tx1.commit().await.unwrap();

    // Verify initial track at position 0
    let tid: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = 1 AND position = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tid.0, 10);

    // 2. Re-insert at same position 0 with new track_id (mirrors tidal.rs:1827-1831)
    let mut tx2 = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position)
         VALUES (?, ?, ?)
         ON CONFLICT(playlist_id, position) DO UPDATE SET
             track_id = excluded.track_id"
    )
    .bind(1)
    .bind(20)
    .bind(0)
    .execute(&mut *tx2)
    .await
    .unwrap();
    tx2.commit().await.unwrap();

    // Verify track_id was updated to 20 at position 0
    let updated_tid: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = 1 AND position = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(updated_tid.0, 20, "ON CONFLICT(playlist_id, position) must update track_id");

    // Total rows must still be 1
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn test_playlist_reorder_under_unique_position_constraint() {
    let (pool, _dir) = setup_test_db().await;

    sqlx::query("INSERT INTO playlists (id, name) VALUES (1, 'Reorder Test')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Track A'), (2, 'Track B')")
        .execute(&pool)
        .await
        .unwrap();

    // Position 0 = Track 1, Position 1 = Track 2
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, 1, 0), (1, 2, 1)")
        .execute(&pool)
        .await
        .unwrap();

    // Attempting to reorder track 2 to pos 0, and track 1 to pos 1
    // simulating what playlists.rs::reorder_playlist_tracks does:
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();

    // 1st update: track 2 to position 0
    let res1 = sqlx::query("UPDATE playlist_tracks SET position = 0 WHERE playlist_id = 1 AND track_id = 2")
        .execute(&mut *tx)
        .await;

    println!("Update track 2 to pos 0 result: {:?}", res1);
    assert!(res1.is_err(), "Directly updating to pos 0 while track 1 is at pos 0 MUST trigger UNIQUE collision");
    tx.rollback().await.unwrap();

    // Staged reordering avoiding UNIQUE collisions:
    let mut tx_staged = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("UPDATE playlist_tracks SET position = -position - 1 WHERE playlist_id = 1")
        .execute(&mut *tx_staged)
        .await
        .unwrap();

    sqlx::query("UPDATE playlist_tracks SET position = 0 WHERE playlist_id = 1 AND track_id = 2")
        .execute(&mut *tx_staged)
        .await
        .unwrap();

    sqlx::query("UPDATE playlist_tracks SET position = 1 WHERE playlist_id = 1 AND track_id = 1")
        .execute(&mut *tx_staged)
        .await
        .unwrap();

    tx_staged.commit().await.unwrap();

    // Verify successful reordering
    let p0: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = 1 AND position = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    let p1: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = 1 AND position = 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(p0.0, 2);
    assert_eq!(p1.0, 1);
}
