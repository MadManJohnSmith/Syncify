//! Regression tests for audit items 13, 14 and 15 (BD-1, BD-3, BD-4): the
//! migration source set is the source service's own catalog, paged in full, and
//! a failed read stops the job instead of reporting an empty one as completed.
//!
//! BD-1 — `library_items` mirrors ONE row per canonical track and keeps only
//! that track's best source (0085 backfill, 0086 triggers). The migration
//! queries used `library_items.external_id` / `library_items.source_service`,
//! so a track whose mirror pointed at another provider was either dropped from
//! a job of the service it actually belongs to or carried an external id that
//! means nothing to the source API. The external id is now resolved from the
//! `track_sources` row of the source service.
//!
//! BD-3 — both queries ended in `LIMIT 1000`: a bigger library migrated only
//! its first thousand tracks, silently, because the job totals were computed
//! from the truncated page. The source set is now walked in pages.
//!
//! BD-4 — `start_migration` turned any read error into an empty list, stored
//! `total_items = 0` and closed the job as `completed`.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;

use syncify_tauri_lib::commands::migration::{
    fetch_migration_source_tracks, search_destination_track, start_migration,
};
use syncify_tauri_lib::models::MigrationOptions;
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

fn loose_options() -> MigrationOptions {
    MigrationOptions {
        match_threshold: 0.5,
        skip_unmatched: false,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    }
}

/// `count` tracks with a spotify identity and an artist link, so the
/// track_sources / track_artists triggers mirror all of them into
/// `library_items` — the table both migration queries read.
async fn seed_mirrored_tracks(pool: &SqlitePool, count: i64) {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();

    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Paging Artist') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    sqlx::query(
        r#"WITH RECURSIVE seq(value) AS (
               SELECT 1 UNION ALL SELECT value + 1 FROM seq WHERE value < ?
           )
           INSERT INTO tracks (title, duration_ms)
           SELECT 'Paged ' || value, 200000 FROM seq"#,
    )
    .bind(count)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score)
         SELECT id, 1, 'sp-' || id, 1, 100 FROM tracks",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) SELECT id, ?, 'primary' FROM tracks",
    )
    .bind(artist_id)
    .execute(pool)
    .await
    .unwrap();

    let mirrored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        mirrored, count,
        "fixture setup: every seeded track must reach the mirror"
    );
}

/// BD-1: a track present in two providers is migratable from both, and each
/// job carries the external id of the service it migrates FROM.
#[tokio::test]
async fn test_source_tracks_carry_the_source_services_own_external_id() {
    let pool = setup_test_db().await;
    seed_mirrored_tracks(&pool, 3).await;

    // A qobuz identity for one of those tracks. The mirror keeps a single
    // preferred source, so making qobuz the winner moves the mirrored
    // (source_service, external_id) away from spotify.
    let spotify_track: i64 = sqlx::query_scalar(
        "SELECT track_id FROM track_sources WHERE service_id = 1 ORDER BY track_id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score)
         VALUES (?, 2, 'qb-777', 1, 500)",
    )
    .bind(spotify_track)
    .execute(&pool)
    .await
    .unwrap();

    let mirrored: (String, String) =
        sqlx::query_as("SELECT source_service, external_id FROM library_items WHERE id = ?")
            .bind(spotify_track)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        mirrored,
        ("qobuz".to_string(), "qb-777".to_string()),
        "fixture setup: the mirror must prefer qobuz for that track"
    );

    let from_spotify = fetch_migration_source_tracks(&pool, "spotify", None)
        .await
        .expect("spotify source read must succeed");
    let from_qobuz = fetch_migration_source_tracks(&pool, "qobuz", None)
        .await
        .expect("qobuz source read must succeed");

    assert_eq!(
        from_spotify.len(),
        3,
        "BD-1: the track whose mirror points at qobuz is still a spotify track"
    );
    assert_eq!(
        from_qobuz.len(),
        1,
        "BD-1: only one track has a qobuz identity"
    );

    let spotify_ids: Vec<&str> = from_spotify
        .iter()
        .map(|t| t.external_id.as_str())
        .collect();
    assert!(
        spotify_ids.contains(&format!("sp-{}", spotify_track).as_str()),
        "BD-1: a spotify job must carry the spotify id, got {:?}",
        spotify_ids
    );
    assert!(
        !spotify_ids.contains(&"qb-777"),
        "BD-1: a spotify job must not carry the qobuz id, got {:?}",
        spotify_ids
    );
    assert_eq!(
        from_qobuz[0].external_id, "qb-777",
        "BD-1: the qobuz job carries the qobuz id"
    );
}

/// BD-3: the favorites scope pages past the old hard LIMIT 1000.
#[tokio::test]
async fn test_favorites_scope_pages_past_one_thousand_tracks() {
    let pool = setup_test_db().await;
    seed_mirrored_tracks(&pool, 1237).await;

    let tracks = fetch_migration_source_tracks(&pool, "spotify", None)
        .await
        .expect("source read must succeed");

    assert_eq!(
        tracks.len(),
        1237,
        "BD-3: every track of the source service must be migrated, not the first 1000"
    );

    let mut ids: Vec<&str> = tracks.iter().map(|t| t.external_id.as_str()).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "BD-3: pages must not repeat or skip rows");
}

/// BD-3: the playlist scope pages too, and keeps one item per (track, playlist).
#[tokio::test]
async fn test_playlist_scope_pages_past_one_thousand_rows() {
    let pool = setup_test_db().await;
    seed_mirrored_tracks(&pool, 1105).await;

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active)
         VALUES (1, 'paging@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service)
         VALUES (?, 'pl-paged', 'Paged', 'spotify') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"WITH RECURSIVE ordered AS (
               SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS pos FROM tracks
           )
           INSERT INTO playlist_tracks (playlist_id, track_id, position)
           SELECT ?, id, pos FROM ordered"#,
    )
    .bind(playlist_id)
    .execute(&pool)
    .await
    .unwrap();

    let ids = vec!["pl-paged".to_string()];
    let tracks = fetch_migration_source_tracks(&pool, "spotify", Some(&ids))
        .await
        .expect("playlist source read must succeed");

    assert_eq!(tracks.len(), 1105, "BD-3: playlist scope must page as well");
    assert!(
        tracks
            .iter()
            .all(|t| t.playlist_name.as_deref() == Some("Paged")),
        "BD-3: every paged row keeps its playlist attribution"
    );
}

/// BD-4: when the mirror read fails the job is marked failed, never completed.
#[tokio::test]
async fn test_start_migration_fails_the_job_when_the_source_read_fails() {
    let pool = setup_test_db().await;
    seed_mirrored_tracks(&pool, 2).await;
    let app = test_app(pool.clone());

    // The mirror is the only table the source query reads; dropping it
    // reproduces a broken read without depending on which error the pool hits.
    sqlx::query("DROP TABLE library_items")
        .execute(&pool)
        .await
        .unwrap();

    let state = app.state::<AppState>();
    let err = start_migration(
        app.handle().clone(),
        state,
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        loose_options(),
    )
    .await
    .expect_err("BD-4: a failed source read must not start a migration");

    assert!(
        err.contains("Failed to load migration tracks"),
        "BD-4: the read error must reach the caller, got: {}",
        err
    );

    let job: (i64, String, Option<String>) =
        sqlx::query_as("SELECT total_items, status, error_message FROM migration_jobs")
            .fetch_one(&pool)
            .await
            .expect("the job row must exist so the UI can show why it stopped");
    assert_eq!(job.1, "failed", "BD-4: the job must not close as completed");
    assert_eq!(job.0, 0, "BD-4: no item was read, so none was counted");
    assert!(
        job.2
            .unwrap_or_default()
            .contains("Failed to load migration tracks"),
        "BD-4: the reason must be persisted on the job"
    );
}

/// BD-1: the local fallback of `search_destination_track` answers with the
/// destination service's own external id, not the one the mirror happens to
/// carry. A manual match saved from this list is transferred by that id, so a
/// foreign one would send the destination API an id it does not know.
#[tokio::test]
async fn test_destination_search_returns_the_destination_services_own_id() {
    let pool = setup_test_db().await;
    seed_mirrored_tracks(&pool, 2).await;

    let track: i64 = sqlx::query_scalar(
        "SELECT track_id FROM track_sources WHERE service_id = 1 ORDER BY track_id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score)
         VALUES (?, 2, 'qb-900', 1, 500)",
    )
    .bind(track)
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());
    let matches = search_destination_track(
        app.state::<AppState>(),
        "spotify".to_string(),
        "Paged 1".to_string(),
    )
    .await
    .expect("local fallback search must succeed");

    assert_eq!(matches.len(), 1, "fixture setup: only one track matches");
    assert_eq!(
        matches[0].track_id,
        format!("sp-{}", track),
        "BD-1: the destination search must answer with the spotify id"
    );
}
