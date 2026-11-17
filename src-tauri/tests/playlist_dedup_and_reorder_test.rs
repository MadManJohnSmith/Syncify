//! TASK-107 / PL00: Test suite for position recompaction, track_count sync, and name disambiguation.
//!
//! Validates:
//! 1. Repeated tracks inside a playlist are KEPT (PL00 #153 allows duplicates) and
//!    positions are recompacted to strictly 1-indexed, sequential, gap-free (1..N).
//! 2. `playlists.track_count` is synchronized to match exact `COUNT(*)` in `playlist_tracks`
//!    (duplicates counted).
//! 3. Duplicate playlist names within the same account are disambiguated with `(2)`, `(3)`, etc.
//! 4. Disambiguation handles preexisting `(2)` suffixes without colliding.
//! 5. Playlists in different accounts with identical names are preserved independently.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{
    recompact_playlist_positions, sanitize_playlists_in_pool, sanitize_single_playlist,
};

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
    sqlx::query(
        "INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy'), (2, 'tidal', 1, 'hires')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Spotify User', 1), (2, 2, 'Tidal User', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn test_repeated_tracks_are_kept_and_recompacted() {
    let pool = create_test_db().await;

    // Create tracks
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Track A', 'ISRC_A') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Track B', 'ISRC_B') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let t3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Track C', 'ISRC_C') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create playlist with deliberately desynchronized track_count
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'sp_pl_1', 'Dedup Test', 999) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert tracks with deliberate repetitions and gaps:
    // t1 at pos 5
    // t2 at pos 10
    // t1 at pos 15 (repeat, legal since PL00 #153)
    // t3 at pos 20
    // t2 at pos 25 (repeat)
    // t1 at pos 30 (repeat)
    sqlx::query(
        r#"
        INSERT INTO playlist_tracks (playlist_id, track_id, position)
        VALUES
            (?, ?, 5),
            (?, ?, 10),
            (?, ?, 15),
            (?, ?, 20),
            (?, ?, 25),
            (?, ?, 30);
        "#,
    )
    .bind(playlist_id)
    .bind(t1)
    .bind(playlist_id)
    .bind(t2)
    .bind(playlist_id)
    .bind(t1)
    .bind(playlist_id)
    .bind(t3)
    .bind(playlist_id)
    .bind(t2)
    .bind(playlist_id)
    .bind(t1)
    .execute(&pool)
    .await
    .unwrap();

    let initial_tracks_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(initial_tracks_count.0, 6);

    // Sanitize single playlist
    let rows_kept = sanitize_single_playlist(&pool, playlist_id)
        .await
        .expect("sanitize_single_playlist must succeed");

    assert_eq!(rows_kept, 6, "All 6 occurrences are kept (no dedup purge)");

    // Verify remaining tracks and positions
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 6, "Every occurrence stays in the playlist");
    assert_eq!(
        rows,
        vec![(t1, 1), (t2, 2), (t1, 3), (t3, 4), (t2, 5), (t1, 6)],
        "Recompacted 1..N in original order, repetitions preserved"
    );

    // Verify track_count synchronization
    let final_track_count: (i64,) =
        sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_track_count.0, 6, "track_count counts duplicates");
}

#[tokio::test]
async fn test_sanitize_all_playlists_global() {
    let pool = create_test_db().await;

    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song 1', 'ISRC_1') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song 2', 'ISRC_2') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let t3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song 3', 'ISRC_3') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Account 1: 3 playlists with identical name "Workout Beats"
    let p1: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_a1_1', 'Workout Beats', 100) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let p2: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_a1_2', 'Workout Beats', 200) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let p3: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'pl_a1_3', 'Workout Beats', 300) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // Account 2: 1 playlist with same name "Workout Beats" (different account)
    let p4: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (2, 'pl_a2_1', 'Workout Beats', 400) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // Insert tracks with duplicates in p1: [t1 at 2, t2 at 8, t1 at 12]
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 2), (?, ?, 8), (?, ?, 12)")
        .bind(p1).bind(t1)
        .bind(p1).bind(t2)
        .bind(p1).bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    // Insert tracks with gaps in p2: [t2 at 5, t3 at 19]
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 5), (?, ?, 19)")
        .bind(p2).bind(t2)
        .bind(p2).bind(t3)
        .execute(&pool)
        .await
        .unwrap();

    // Insert track in p3: [t3 at 0]
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
        .bind(p3)
        .bind(t3)
        .execute(&pool)
        .await
        .unwrap();

    // Run global sanitization
    let stats = sanitize_playlists_in_pool(&pool)
        .await
        .expect("sanitize_playlists_in_pool must succeed");

    assert_eq!(
        stats.duplicate_tracks_purged, 0,
        "PL00: repetitions are legal, nothing is purged"
    );
    assert_eq!(stats.playlist_names_disambiguated, 2);

    // Verify names in Account 1
    let name_p1: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let name_p2: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p2)
        .fetch_one(&pool)
        .await
        .unwrap();
    let name_p3: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p3)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(name_p1.0, "Workout Beats");
    assert_eq!(name_p2.0, "Workout Beats (2)");
    assert_eq!(name_p3.0, "Workout Beats (3)");

    // Verify name in Account 2 (untouched because it is in a different account)
    let name_p4: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p4)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name_p4.0, "Workout Beats");

    // Verify p1 positions: repetitions preserved, recompacted 1..3
    let p1_tracks: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC")
        .bind(p1).fetch_all(&pool).await.unwrap();
    assert_eq!(p1_tracks, vec![(t1, 1), (t2, 2), (t1, 3)]);

    // Verify p2 positions: 1, 2
    let p2_tracks: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC")
        .bind(p2).fetch_all(&pool).await.unwrap();
    assert_eq!(p2_tracks, vec![(t2, 1), (t3, 2)]);

    // Verify p3 positions: 1
    let p3_tracks: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC")
        .bind(p3).fetch_all(&pool).await.unwrap();
    assert_eq!(p3_tracks, vec![(t3, 1)]);

    // Verify track_count synchronization
    let cnt_p1: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(p1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cnt_p2: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(p2)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cnt_p3: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(p3)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt_p1.0, 3);
    assert_eq!(cnt_p2.0, 2);
    assert_eq!(cnt_p3.0, 1);
}

#[tokio::test]
async fn test_disambiguation_with_existing_suffixed_names() {
    let pool = create_test_db().await;

    // Account 1 has "Chillout" and already has "Chillout (2)"
    let p1: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (1, 'sp_c1', 'Chillout') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let p2: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (1, 'sp_c2', 'Chillout (2)') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // Now a 3rd playlist is imported with duplicate name "Chillout"
    let p3: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (1, 'sp_c3', 'Chillout') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // Sanitize
    let stats = sanitize_playlists_in_pool(&pool)
        .await
        .expect("sanitize_playlists_in_pool must succeed");

    assert_eq!(stats.playlist_names_disambiguated, 1);

    let name_p1: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let name_p2: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p2)
        .fetch_one(&pool)
        .await
        .unwrap();
    let name_p3: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(p3)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(name_p1.0, "Chillout");
    assert_eq!(name_p2.0, "Chillout (2)");
    assert_eq!(
        name_p3.0, "Chillout (3)",
        "Must skip existing (2) and assign (3)"
    );
}

#[tokio::test]
async fn test_recompact_playlist_positions_keeps_repeated_tracks() {
    let pool = create_test_db().await;

    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song A', 'ISRC_11') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song B', 'ISRC_12') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let pl: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'sp_recompact', 'Recompact Pl', 50) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 10), (?, ?, 20), (?, ?, 30)")
        .bind(pl).bind(t1)
        .bind(pl).bind(t2)
        .bind(pl).bind(t1) // Repeat of t1, legal since PL00 #153
        .execute(&pool)
        .await
        .unwrap();

    recompact_playlist_positions(&pool, pl)
        .await
        .expect("recompact_playlist_positions must succeed");

    let rows: Vec<(i64, i64)> = sqlx::query_as("SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC")
        .bind(pl).fetch_all(&pool).await.unwrap();

    assert_eq!(rows.len(), 3, "recompaction does not purge repetitions");
    assert_eq!(rows, vec![(t1, 1), (t2, 2), (t1, 3)]);

    let count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(pl)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3, "track_count counts every occurrence");
}

#[tokio::test]
async fn test_empty_playlist_sanitization() {
    let pool = create_test_db().await;

    let pl: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count) VALUES (1, 'sp_empty', 'Empty Pl', 77) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let rows_kept = sanitize_single_playlist(&pool, pl)
        .await
        .expect("sanitize_single_playlist must succeed on empty playlist");

    assert_eq!(rows_kept, 0);

    let count: (i64,) = sqlx::query_as("SELECT track_count FROM playlists WHERE id = ?")
        .bind(pl)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count.0, 0,
        "Empty playlist track_count must be reconciled to 0"
    );
}
