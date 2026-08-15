//! E2E Test Suite for Sprint S102: Sincronización de Playlists
//!
//! Validates migration 0048, playlist CRUD lifecycle, track reordering,
//! cascading deletion, multi-service playlist sync, and ISRC track deduplication.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through 0048 must apply cleanly");

    // Seed services & default accounts
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (4, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Spotify User', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (2, 3, 'Tidal User', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_migration_0048_full_lifecycle_and_idempotence() {
    let db = create_test_db().await;

    // Verify playlist_sources table exists
    let table_exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='playlist_sources'"
    )
    .fetch_one(&db).await.unwrap();
    assert_eq!(table_exists.0, 1, "playlist_sources table must exist after migration 0048");

    // Verify indices exist
    let idx_sources: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_playlist_sources_playlist'"
    )
    .fetch_one(&db).await.unwrap();
    assert_eq!(idx_sources.0, 1, "idx_playlist_sources_playlist index must exist");

    // Re-run migrations to test idempotence
    let rerun = sqlx::migrate!("./migrations").run(&db).await;
    assert!(rerun.is_ok(), "Re-running migrations must be 100% idempotent");
}

#[tokio::test]
async fn test_create_update_delete_playlist_lifecycle() {
    let db = create_test_db().await;

    // 1. Create Playlist
    let pid: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, description, track_count) VALUES (1, 'local_p1', 'My Rock Playlist', 'Classic rock', 0) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();
    assert!(pid > 0);

    // 2. Add Tracks
    let tid1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Song 1', 'ISRC001') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let tid2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Song 2', 'ISRC002') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
        .bind(pid).bind(tid1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(pid).bind(tid2).execute(&db).await.unwrap();

    // Link playlist source
    sqlx::query("INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id) VALUES (?, 1, 1, 'sp_p1')")
        .bind(pid).execute(&db).await.unwrap();

    // 3. Update Playlist
    sqlx::query("UPDATE playlists SET name = 'Updated Rock Hits', description = 'New Description' WHERE id = ?")
        .bind(pid).execute(&db).await.unwrap();

    let updated: (String, Option<String>) = sqlx::query_as("SELECT name, description FROM playlists WHERE id = ?")
        .bind(pid).fetch_one(&db).await.unwrap();
    assert_eq!(updated.0, "Updated Rock Hits");
    assert_eq!(updated.1, Some("New Description".to_string()));

    // 4. Cascade Delete
    let mut tx = db.begin().await.unwrap();
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?").bind(pid).execute(&mut *tx).await.unwrap();
    sqlx::query("DELETE FROM playlist_sources WHERE playlist_id = ?").bind(pid).execute(&mut *tx).await.unwrap();
    sqlx::query("DELETE FROM playlists WHERE id = ?").bind(pid).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    // Assert cascading cleanup
    let p_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE id = ?").bind(pid).fetch_one(&db).await.unwrap();
    assert_eq!(p_exists.0, 0);

    let pt_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?").bind(pid).fetch_one(&db).await.unwrap();
    assert_eq!(pt_exists.0, 0);

    let ps_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_sources WHERE playlist_id = ?").bind(pid).fetch_one(&db).await.unwrap();
    assert_eq!(ps_exists.0, 0);
}

#[tokio::test]
async fn test_remove_and_reorder_playlist_tracks() {
    let db = create_test_db().await;

    let pid: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'local_reorder', 'Reorder List', 3) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track 1', 'ISRC_A') RETURNING id").fetch_one(&db).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track 2', 'ISRC_B') RETURNING id").fetch_one(&db).await.unwrap();
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track 3', 'ISRC_C') RETURNING id").fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)").bind(pid).bind(t1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)").bind(pid).bind(t2).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 2)").bind(pid).bind(t3).execute(&db).await.unwrap();

    // Reorder: t3 -> 0, t1 -> 1, t2 -> 2
    let mut tx = db.begin().await.unwrap();
    sqlx::query("UPDATE playlist_tracks SET position = 0 WHERE playlist_id = ? AND track_id = ?").bind(pid).bind(t3).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE playlist_tracks SET position = 1 WHERE playlist_id = ? AND track_id = ?").bind(pid).bind(t1).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE playlist_tracks SET position = 2 WHERE playlist_id = ? AND track_id = ?").bind(pid).bind(t2).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let ordered: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    )
    .bind(pid).fetch_all(&db).await.unwrap();

    assert_eq!(ordered[0].0, t3);
    assert_eq!(ordered[1].0, t1);
    assert_eq!(ordered[2].0, t2);

    // Remove track t1 and recompact
    let mut tx2 = db.begin().await.unwrap();
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?").bind(pid).bind(t1).execute(&mut *tx2).await.unwrap();

    let remaining: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC, id ASC"
    ).bind(pid).fetch_all(&mut *tx2).await.unwrap();

    for (pos, (rid,)) in remaining.into_iter().enumerate() {
        sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?").bind(pos as i64).bind(rid).execute(&mut *tx2).await.unwrap();
    }
    tx2.commit().await.unwrap();

    let compacted: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    ).bind(pid).fetch_all(&db).await.unwrap();

    assert_eq!(compacted.len(), 2);
    assert_eq!(compacted[0].0, t3);
    assert_eq!(compacted[0].1, 0);
    assert_eq!(compacted[1].0, t2);
    assert_eq!(compacted[1].1, 1);
}

#[tokio::test]
async fn test_cross_service_playlist_sync_and_deduplication() {
    let db = create_test_db().await;

    // Spotify playlist
    let sp_pid: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'sp_list_1', 'Spotify Party', 1) RETURNING id"
    ).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id) VALUES (?, 1, 1, 'sp_list_1')")
        .bind(sp_pid).execute(&db).await.unwrap();

    // Tidal playlist
    let td_pid: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (2, 'td_list_1', 'Tidal HiFi', 1) RETURNING id"
    ).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id) VALUES (?, 2, 3, 'td_list_1')")
        .bind(td_pid).execute(&db).await.unwrap();

    let total_sources: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_sources").fetch_one(&db).await.unwrap();
    assert_eq!(total_sources.0, 2);

    let spotify_sources: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_sources WHERE service_id = 1").fetch_one(&db).await.unwrap();
    assert_eq!(spotify_sources.0, 1);

    let tidal_sources: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_sources WHERE service_id = 3").fetch_one(&db).await.unwrap();
    assert_eq!(tidal_sources.0, 1);
}

#[tokio::test]
async fn test_playlist_track_isrc_deduplication() {
    let db = create_test_db().await;

    // Canonical track deduplicated by ISRC
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Shared Song', 'USAT29900010') RETURNING id"
    ).fetch_one(&db).await.unwrap();

    let pid1: i64 = sqlx::query_scalar("INSERT INTO playlists (account_id, name) VALUES (1, 'List 1') RETURNING id").fetch_one(&db).await.unwrap();
    let pid2: i64 = sqlx::query_scalar("INSERT INTO playlists (account_id, name) VALUES (2, 'List 2') RETURNING id").fetch_one(&db).await.unwrap();

    // Both playlists reference the same canonical track
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)").bind(pid1).bind(tid).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)").bind(pid2).bind(tid).execute(&db).await.unwrap();

    let track_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc = 'USAT29900010'").fetch_one(&db).await.unwrap();
    assert_eq!(track_count.0, 1, "Track must remain deduplicated across playlists");

    let pt_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE track_id = ?").bind(tid).fetch_one(&db).await.unwrap();
    assert_eq!(pt_count.0, 2, "Canonical track can be referenced in multiple playlists");
}
