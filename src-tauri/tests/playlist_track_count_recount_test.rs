//! Regression test for audit item 23 (BD-6): `playlists.track_count` follows the
//! tracks a playlist actually holds when tracks are removed.
//!
//! `add_to_playlist` recalculated the counter, but `remove_track` and
//! `bulk_remove_tracks` did not: deleting a track cascaded away its
//! `playlist_tracks` rows and the playlist kept advertising the old number of
//! songs (the UI shows the persisted counter, PlaylistView.vue:95).

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;

use syncify_tauri_lib::commands::library::{bulk_remove_tracks, remove_track};
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::{AppState, EnrichmentWorkerState};
use tauri::Manager;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // The application enables foreign keys on every connection (db.rs) and the
    // ON DELETE CASCADE of playlist_tracks is what removes the relations.
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

fn test_app(pool: SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    });
    app
}

async fn seed_playlist(pool: &SqlitePool, name: &str) -> i64 {
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active)
         VALUES (1, ?, '{}', 1) RETURNING id",
    )
    .bind(format!(
        "{}@test.dev",
        name.to_lowercase().replace(' ', "-")
    ))
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query_scalar(
        "INSERT INTO playlists (account_id, name, source_service, track_count)
         VALUES (?, ?, 'local', 0) RETURNING id",
    )
    .bind(account_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_track(pool: &SqlitePool, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn link(pool: &SqlitePool, playlist_id: i64, track_id: i64, position: i64) {
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
        .bind(playlist_id)
        .bind(track_id)
        .bind(position)
        .execute(pool)
        .await
        .unwrap();
}

async fn set_count(pool: &SqlitePool, playlist_id: i64, count: i64) {
    sqlx::query("UPDATE playlists SET track_count = ? WHERE id = ?")
        .bind(count)
        .bind(playlist_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn count(pool: &SqlitePool, playlist_id: i64) -> i64 {
    sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn position_of(pool: &SqlitePool, playlist_id: i64, track_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT position FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
    )
    .bind(playlist_id)
    .bind(track_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn test_remove_track_recounts_the_playlists_it_belonged_to() {
    let pool = setup_test_db().await;
    let playlist = seed_playlist(&pool, "Removal").await;
    let doomed = seed_track(&pool, "Doomed").await;
    let survivor = seed_track(&pool, "Survivor").await;
    link(&pool, playlist, doomed, 1).await;
    link(&pool, playlist, survivor, 2).await;
    set_count(&pool, playlist, 2).await;

    let app = test_app(pool.clone());
    remove_track(app.state::<AppState>(), doomed)
        .await
        .expect("remove_track must succeed");

    assert_eq!(
        count(&pool, playlist).await,
        1,
        "BD-6: the playlist counter must follow the removal"
    );
    // The cascade leaves a hole in `position`; the reconciliation the removal
    // now calls also recompacts the sequence to 1..N.
    assert_eq!(
        position_of(&pool, playlist, survivor).await,
        1,
        "BD-6: the positions left with a hole must be recompacted"
    );
}

#[tokio::test]
async fn test_bulk_remove_tracks_recounts_every_affected_playlist() {
    let pool = setup_test_db().await;
    let first = seed_playlist(&pool, "First").await;
    let second = seed_playlist(&pool, "Second").await;
    let untouched = seed_playlist(&pool, "Untouched").await;

    let a = seed_track(&pool, "A").await;
    let b = seed_track(&pool, "B").await;
    let c = seed_track(&pool, "C").await;
    link(&pool, first, a, 1).await;
    link(&pool, first, b, 2).await;
    link(&pool, second, b, 1).await;
    link(&pool, second, c, 2).await;
    link(&pool, untouched, c, 1).await;
    set_count(&pool, first, 2).await;
    set_count(&pool, second, 2).await;
    set_count(&pool, untouched, 1).await;

    let app = test_app(pool.clone());
    let removed = bulk_remove_tracks(app.state::<AppState>(), vec![a, b])
        .await
        .expect("bulk_remove_tracks must succeed");

    assert_eq!(removed, 2);
    assert_eq!(
        count(&pool, first).await,
        0,
        "BD-6: first playlist is empty"
    );
    assert_eq!(
        count(&pool, second).await,
        1,
        "BD-6: the playlist keeps the track that was not deleted"
    );
    assert_eq!(
        count(&pool, untouched).await,
        1,
        "BD-6: a playlist that held no removed track is left as it was"
    );
}

#[tokio::test]
async fn test_removing_a_track_that_is_in_no_playlist_leaves_the_counters_alone() {
    let pool = setup_test_db().await;
    let playlist = seed_playlist(&pool, "Kept").await;
    let kept = seed_track(&pool, "Kept").await;
    let loose = seed_track(&pool, "Loose").await;
    link(&pool, playlist, kept, 1).await;
    set_count(&pool, playlist, 1).await;

    let app = test_app(pool.clone());
    remove_track(app.state::<AppState>(), loose)
        .await
        .expect("remove_track must succeed");

    assert_eq!(
        count(&pool, playlist).await,
        1,
        "BD-6: a track outside every playlist must not disturb the counters"
    );
}
