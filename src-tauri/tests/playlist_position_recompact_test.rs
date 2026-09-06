//! TASK-79: Test suite for canonical playlist position recompaction and track_count reconciliation.
//!
//! Validates:
//! 1. Discontinuous positions [0, 3, 10] are recompacted to strictly 1-indexed [1, 2, 3].
//! 2. `playlists.track_count` is reconciled to match exact `SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?`.
//! 3. Deleting an intermediate item maintains a continuous 1..N sequence without gaps.
//! 4. Zero-indexed continuous lists [0, 1, 2] become strictly 1-indexed [1, 2, 3].
//! 5. Empty playlists reconcile to track_count = 0.
//! 6. Playlist isolation: recompaction on one playlist never affects another playlist.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::recompact_playlist_positions;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Seed test services and accounts
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Spotify User', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_recompact_discontinuous_positions_and_reconcile_track_count() {
    let pool = create_test_db().await;

    // Create playlist with deliberate mismatching track_count (99 vs 3)
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_disc_1', 'Discontinuous Playlist', 99) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create tracks
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track 1', 'ISRC_1') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track 2', 'ISRC_2') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track 3', 'ISRC_3') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Insert tracks with discontinuous positions: 0, 3, 10
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
        .bind(playlist_id).bind(t1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 3)")
        .bind(playlist_id).bind(t2).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 10)")
        .bind(playlist_id).bind(t3).execute(&pool).await.unwrap();

    // Verify initial state has discordance
    let initial_count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id).fetch_one(&pool).await.unwrap();
    assert_eq!(initial_count.0, 99);

    // Recompact positions and reconcile track_count
    recompact_playlist_positions(&pool, playlist_id)
        .await
        .expect("recompact_playlist_positions should succeed");

    // Verify positions are now strictly 1, 2, 3
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], (t1, 1), "Track 1 should have position 1");
    assert_eq!(rows[1], (t2, 2), "Track 2 should have position 2");
    assert_eq!(rows[2], (t3, 3), "Track 3 should have position 3");

    // Verify track_count in playlists table matches exact COUNT(*)
    let updated_count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id).fetch_one(&pool).await.unwrap();
    let actual_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id).fetch_one(&pool).await.unwrap();

    assert_eq!(updated_count.0, 3);
    assert_eq!(updated_count.0, actual_count.0);
}

#[tokio::test]
async fn test_recompact_after_intermediate_deletion_maintains_gap_free_sequence() {
    let pool = create_test_db().await;

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_del_1', 'Deletion Playlist', 4) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('A', 'ISRC_A') RETURNING id").fetch_one(&pool).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('B', 'ISRC_B') RETURNING id").fetch_one(&pool).await.unwrap();
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('C', 'ISRC_C') RETURNING id").fetch_one(&pool).await.unwrap();
    let t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('D', 'ISRC_D') RETURNING id").fetch_one(&pool).await.unwrap();

    // Initially 1, 2, 3, 4
    for (pos, tid) in [t1, t2, t3, t4].iter().enumerate() {
        sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
            .bind(playlist_id).bind(tid).bind((pos + 1) as i64).execute(&pool).await.unwrap();
    }

    // Delete intermediate track t2 (position 2)
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?")
        .bind(playlist_id).bind(t2).execute(&pool).await.unwrap();

    // Recompact
    recompact_playlist_positions(&pool, playlist_id)
        .await
        .expect("recompact should succeed after deletion");

    // Verify remaining tracks [t1, t3, t4] are remapped to [1, 2, 3] with no gaps
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], (t1, 1));
    assert_eq!(rows[1], (t3, 2));
    assert_eq!(rows[2], (t4, 3));

    // Verify track_count
    let track_count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id).fetch_one(&pool).await.unwrap();
    assert_eq!(track_count.0, 3);
}

#[tokio::test]
async fn test_recompact_zero_indexed_becomes_one_indexed() {
    let pool = create_test_db().await;

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_zero_1', 'Zero Indexed Playlist', 0) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Z1', 'ISRC_Z1') RETURNING id").fetch_one(&pool).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Z2', 'ISRC_Z2') RETURNING id").fetch_one(&pool).await.unwrap();

    // Insert as 0-indexed [0, 1]
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
        .bind(playlist_id).bind(t1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(playlist_id).bind(t2).execute(&pool).await.unwrap();

    recompact_playlist_positions(&pool, playlist_id)
        .await
        .expect("recompact should succeed");

    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], (t1, 1));
    assert_eq!(rows[1], (t2, 2));

    let track_count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id).fetch_one(&pool).await.unwrap();
    assert_eq!(track_count.0, 2);
}

#[tokio::test]
async fn test_recompact_empty_playlist() {
    let pool = create_test_db().await;

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_empty_1', 'Empty Playlist', 15) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    recompact_playlist_positions(&pool, playlist_id)
        .await
        .expect("recompact should succeed on empty playlist");

    let track_count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id).fetch_one(&pool).await.unwrap();
    assert_eq!(track_count.0, 0, "Empty playlist must have track_count = 0");
}

#[tokio::test]
async fn test_recompact_playlist_isolation() {
    let pool = create_test_db().await;

    let pl1: i64 = sqlx::query_scalar("INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_iso_1', 'PL 1', 2) RETURNING id").fetch_one(&pool).await.unwrap();
    let pl2: i64 = sqlx::query_scalar("INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_iso_2', 'PL 2', 2) RETURNING id").fetch_one(&pool).await.unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T1', 'ISRC_T1') RETURNING id").fetch_one(&pool).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T2', 'ISRC_T2') RETURNING id").fetch_one(&pool).await.unwrap();

    // pl1 has gap: [0, 5]
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)").bind(pl1).bind(t1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 5)").bind(pl1).bind(t2).execute(&pool).await.unwrap();

    // pl2 has positions: [7, 8]
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 7)").bind(pl2).bind(t1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 8)").bind(pl2).bind(t2).execute(&pool).await.unwrap();

    // Recompact only pl1
    recompact_playlist_positions(&pool, pl1).await.unwrap();

    // pl1 should be [1, 2]
    let pl1_rows: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC").bind(pl1).fetch_all(&pool).await.unwrap();
    assert_eq!(pl1_rows, vec![(t1, 1), (t2, 2)]);

    // pl2 must remain untouched: [7, 8]
    let pl2_rows: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC").bind(pl2).fetch_all(&pool).await.unwrap();
    assert_eq!(pl2_rows, vec![(t1, 7), (t2, 8)]);
}

#[tokio::test]
async fn test_migration_0077_recompacts_all_historical_playlists() {
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0077_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test SQLite DB");

    // 1. Prepare temp migrations dir containing 0001 through 0076 (excluding 0077)
    let mig_temp_dir = TempDir::new().unwrap();
    let src_migrations_dir = Path::new("./migrations");
    for entry in fs::read_dir(src_migrations_dir).unwrap().filter_map(|e| e.ok()) {
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.ends_with(".sql") && !file_name.starts_with("0077") {
            fs::copy(entry.path(), mig_temp_dir.path().join(&file_name)).unwrap();
        }
    }

    let pre_migrator = sqlx::migrate::Migrator::new(mig_temp_dir.path())
        .await
        .expect("Failed to build pre-migrator for 0001..=0076");
    pre_migrator
        .run(&pool)
        .await
        .expect("Failed running migrations 0001..=0076");

    // Seed services and account
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Test User', 1)")
        .execute(&pool).await.unwrap();

    // Insert 2 test playlists with intentional flaws:
    // Playlist 1: discontinuous positions [0, 5, 20], track_count = 99 (mismatched)
    let pl1: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_hist_1', 'Hist PL 1', 99) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    // Playlist 2: 0-indexed positions [0, 1], track_count = 0 (mismatched)
    let pl2: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_hist_2', 'Hist PL 2', 0) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    // Insert tracks
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T1', 'ISRC_1') RETURNING id").fetch_one(&pool).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T2', 'ISRC_2') RETURNING id").fetch_one(&pool).await.unwrap();
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T3', 'ISRC_3') RETURNING id").fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (?, ?, 0, '2024-01-01')").bind(pl1).bind(t1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (?, ?, 5, '2024-01-02')").bind(pl1).bind(t2).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (?, ?, 20, '2024-01-03')").bind(pl1).bind(t3).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (?, ?, 0, '2024-01-01')").bind(pl2).bind(t1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES (?, ?, 1, '2024-01-02')").bind(pl2).bind(t2).execute(&pool).await.unwrap();

    // Verify discordances exist BEFORE migration 0077
    let mismatched_count_pre: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM playlists p WHERE p.track_count != (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(mismatched_count_pre.0, 2);

    let discontinuous_count_pre: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_tracks GROUP BY playlist_id HAVING MAX(position) - MIN(position) + 1 != COUNT(*))"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(discontinuous_count_pre.0, 1);

    // Apply migration 0077
    let canonical_migrator = sqlx::migrate!("./migrations");
    canonical_migrator.run(&pool).await.expect("Migration 0077 must apply cleanly");

    // Verify QA criteria POST migration 0077:
    // 1. Mismatched track count must be 0
    let mismatched_count_post: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM playlists p WHERE p.track_count != (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(mismatched_count_post.0, 0, "All playlists must match exact track count");

    // 2. Discontinuous positions must be 0
    let discontinuous_count_post: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_tracks GROUP BY playlist_id HAVING MAX(position) - MIN(position) + 1 != COUNT(*))"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(discontinuous_count_post.0, 0, "No playlists should have discontinuous positions");

    // 3. Min position must be 1 for all non-empty playlists
    let zero_indexed_post: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_tracks GROUP BY playlist_id HAVING MIN(position) != 1)"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(zero_indexed_post.0, 0, "All playlist positions must be strictly 1-indexed");

    // 4. Exact positions in pl1 must be [1, 2, 3]
    let pl1_rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    ).bind(pl1).fetch_all(&pool).await.unwrap();
    assert_eq!(pl1_rows, vec![(t1, 1), (t2, 2), (t3, 3)]);

    // 5. Exact positions in pl2 must be [1, 2]
    let pl2_rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    ).bind(pl2).fetch_all(&pool).await.unwrap();
    assert_eq!(pl2_rows, vec![(t1, 1), (t2, 2)]);
}
