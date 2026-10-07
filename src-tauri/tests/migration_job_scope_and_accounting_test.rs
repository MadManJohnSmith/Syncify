//! Regression tests for audit item 4.1 (BD-6 + BD-7): migration jobs respect
//! the selected playlists and keep per-item accounting on the item's own row.
//!
//! BD-6 — start_migration stored the selected playlist ids in the job but the
//! track query only filtered by `p.source_service`, so a playlist-scoped job
//! silently migrated EVERY playlist of the source service. Now the selection
//! (external ids, the same values saved in migration_jobs.source_playlist_ids)
//! filters the query, one deduplicated item per (track, playlist).
//!
//! BD-7 — three accounting defects, all fixed and pinned here:
//!   1. the per-item result UPDATE matched `job_id AND source_track_id`
//!      (external ids are NOT unique per job: a track in two selected playlists
//!      yields two items), updating several rows in one blow. The update now
//!      addresses the item's row id.
//!   2. source_playlist_id / source_playlist_name (0017_migration_history.sql)
//!      were never written and stayed NULL forever.
//!   3. failed items never received an error_message.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;

use syncify_tauri_lib::commands::migration::{
    fetch_migration_source_tracks, insert_migration_item, record_migration_item_result,
    start_migration, MigrationSourceTrack,
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

/// Seeds services (1 = spotify, 2 = qobuz), one spotify account, three tracks
/// with service identity written through track_sources (which mirrors them
/// into library_items), and two spotify playlists:
///   - 'pl-scope-a' ("Scope A"):  Song One, Song Two
///   - 'pl-scope-b' ("Scope B"):  Song One (twice, at two positions)
/// Song Three only has a qobuz identity and lives in neither playlist.
async fn seed_scope_fixture(pool: &SqlitePool) -> (i64, i64, i64) {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();

    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Scope Album') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Scope Artist') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    let mut track_ids = Vec::new();
    for title in &["Song One", "Song Two", "Song Three"] {
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms) VALUES (?, ?, 200000) RETURNING id",
        )
        .bind(title)
        .bind(album_id)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(pool)
        .await
        .unwrap();
        track_ids.push(track_id);
    }
    let (t1, t2, t3) = (track_ids[0], track_ids[1], track_ids[2]);

    // Service identity writes (sync/import shape). The 0086 triggers mirror
    // these into library_items, exactly as production flows now do.
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-one'), (?, 1, 'sp-two'), (?, 2, 'qb-three')")
        .bind(t1)
        .bind(t2)
        .bind(t3)
        .execute(pool)
        .await
        .unwrap();

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'scope@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let playlist_a: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service) VALUES (?, 'pl-scope-a', 'Scope A', 'spotify') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let playlist_b: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service) VALUES (?, 'pl-scope-b', 'Scope B', 'spotify') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 2)",
    )
    .bind(playlist_a)
    .bind(t1)
    .bind(playlist_a)
    .bind(t2)
    .execute(pool)
    .await
    .unwrap();
    // Song One appears twice in Scope B — the job must still create ONE item
    // per (track, playlist).
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 2)",
    )
    .bind(playlist_b)
    .bind(t1)
    .bind(playlist_b)
    .bind(t1)
    .execute(pool)
    .await
    .unwrap();

    (t1, playlist_a, playlist_b)
}

type ItemRow = (
    i64,            // id
    String,         // source_track_id
    Option<String>, // source_playlist_id
    Option<String>, // source_playlist_name
    Option<String>, // status
    Option<f64>,    // match_confidence
    Option<String>, // match_method
    Option<String>, // dest_track_id
    Option<String>, // error_message
    Option<String>, // processed_at
);

async fn job_items(pool: &SqlitePool, job_id: &str) -> Vec<ItemRow> {
    sqlx::query_as(
        "SELECT id, source_track_id, source_playlist_id, source_playlist_name, status,
                match_confidence, match_method, dest_track_id, error_message, processed_at
         FROM migration_items WHERE job_id = ? ORDER BY id",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

// ============================================================================
// BD-6: playlist selection filters the migration, with per-playlist items
// ============================================================================

#[tokio::test]
async fn test_start_migration_migrates_only_the_selected_playlists() {
    let pool = setup_test_db().await;
    seed_scope_fixture(&pool).await;
    let app = test_app(pool.clone());

    // No qobuz account exists on purpose: matching falls back to the simulated
    // branch (no network) and every item lands on a deterministic outcome.
    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        Some(vec!["pl-scope-a".to_string()]),
        loose_options(),
        None,
        None,
    )
    .await
    .expect("start_migration must succeed");

    let items = job_items(&pool, &job_id).await;
    assert_eq!(
        items.len(),
        2,
        "only the selected playlist's tracks enter the job"
    );

    let source_track_ids: Vec<&str> = items.iter().map(|i| i.1.as_str()).collect();
    assert!(source_track_ids.contains(&"sp-one"));
    assert!(source_track_ids.contains(&"sp-two"));
    assert!(
        !source_track_ids.contains(&"qb-three"),
        "other-service tracks stay out"
    );

    // BD-6 regression core: nothing from Scope B may leak into this job.
    assert!(
        items.iter().all(|i| i.2.as_deref() == Some("pl-scope-a")),
        "every item must belong to the selected playlist"
    );

    let job: (i64, i64, i64, i64, String) = sqlx::query_as(
        "SELECT total_items, completed_items, failed_items, skipped_items, status
         FROM migration_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        job.0, 2,
        "total_items counts the selected playlist's tracks"
    );
    assert_eq!(job.1, 0);
    assert_eq!(job.2, 2, "simulated match without a transfer target fails");
    assert_eq!(job.3, 0);
    assert_eq!(job.4, "completed");

    // BD-7: the job's saved selection is exactly what the caller passed.
    let saved: Option<String> =
        sqlx::query_scalar("SELECT source_playlist_ids FROM migration_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(saved.as_deref(), Some("[\"pl-scope-a\"]"));
}

#[tokio::test]
async fn test_start_migration_with_other_selection_and_favorites_scope() {
    let pool = setup_test_db().await;
    seed_scope_fixture(&pool).await;
    let app = test_app(pool.clone());

    // Selecting Scope B migrates only Song One, once — even though the track
    // sits in that playlist twice.
    let job_b = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        Some(vec!["pl-scope-b".to_string()]),
        loose_options(),
        None,
        None,
    )
    .await
    .expect("start_migration must succeed");
    let items_b = job_items(&pool, &job_b).await;
    assert_eq!(
        items_b.len(),
        1,
        "duplicate (track, playlist) pairs collapse to one item"
    );
    assert_eq!(items_b[0].1, "sp-one");
    assert_eq!(items_b[0].2.as_deref(), Some("pl-scope-b"));
    assert_eq!(items_b[0].3.as_deref(), Some("Scope B"));

    // No selection = favorites scope: every spotify track of the library, with
    // no playlist attribution (and Song Three, qobuz-only, stays out).
    let job_all = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        loose_options(),
        None,
        None,
    )
    .await
    .expect("start_migration must succeed");
    let items_all = job_items(&pool, &job_all).await;
    assert_eq!(items_all.len(), 2);
    let ids: Vec<&str> = items_all.iter().map(|i| i.1.as_str()).collect();
    assert!(ids.contains(&"sp-one") && ids.contains(&"sp-two"));
    assert!(
        items_all.iter().all(|i| i.2.is_none() && i.3.is_none()),
        "favorites-scope items carry no playlist attribution"
    );

    // An explicitly empty selection migrates nothing.
    let job_empty = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        Some(vec![]),
        loose_options(),
        None,
        None,
    )
    .await
    .expect("start_migration must succeed");
    assert!(job_items(&pool, &job_empty).await.is_empty());
}

// ============================================================================
// BD-7: per-item accounting (playlist attribution, per-row results, errors)
// ============================================================================

#[tokio::test]
async fn test_start_migration_writes_playlist_attribution_and_error_messages() {
    let pool = setup_test_db().await;
    seed_scope_fixture(&pool).await;
    let app = test_app(pool.clone());

    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        Some(vec!["pl-scope-a".to_string()]),
        loose_options(),
        None,
        None,
    )
    .await
    .expect("start_migration must succeed");

    let items = job_items(&pool, &job_id).await;
    for item in &items {
        // BD-7 (2): the 0017 playlist columns are written, not NULL.
        assert_eq!(
            item.2.as_deref(),
            Some("pl-scope-a"),
            "source_playlist_id must be written"
        );
        assert_eq!(
            item.3.as_deref(),
            Some("Scope A"),
            "source_playlist_name must be written"
        );
        // BD-7 (3): failed items carry a real error message.
        assert_eq!(item.4.as_deref(), Some("failed"));
        assert!(
            item.8
                .as_deref()
                .map(|m| !m.trim().is_empty())
                .unwrap_or(false),
            "failed items must receive an error_message, got {:?}",
            item.8
        );
        // Outcome details land on the row too.
        assert_eq!(item.6.as_deref(), Some("simulated"));
        assert!(item.5.unwrap() >= 0.5);
        assert!(
            item.7.is_none(),
            "no destination id without a destination client"
        );
        assert!(item.9.is_some(), "processed_at must be recorded");
    }
}

#[tokio::test]
async fn test_result_update_hits_only_its_own_row_when_external_ids_repeat() {
    let pool = setup_test_db().await;
    seed_scope_fixture(&pool).await;

    // BD-7 (1) core regression: the same external id twice in one job — the
    // realistic shape of a track living in two selected playlists.
    let job_id = "job-dup-ext";
    sqlx::query(
        "INSERT INTO migration_jobs (id, source_service, destination_service, options, status)
         VALUES (?, 'spotify', 'qobuz', '{}', 'running')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let track_a = MigrationSourceTrack {
        external_id: "sp-dup".to_string(),
        title: "Dup Song".to_string(),
        artist: "Dup Artist".to_string(),
        album: None,
        playlist_id: Some("pl-1".to_string()),
        playlist_name: Some("Playlist One".to_string()),
    };
    let track_b = MigrationSourceTrack {
        playlist_id: Some("pl-2".to_string()),
        playlist_name: Some("Playlist Two".to_string()),
        ..track_a.clone()
    };

    let id_a = insert_migration_item(&pool, job_id, &track_a)
        .await
        .expect("insert a")
        .expect("row id a");
    let id_b = insert_migration_item(&pool, job_id, &track_b)
        .await
        .expect("insert b")
        .expect("row id b");
    assert_ne!(id_a, id_b, "two items, two rows");

    // The playlist columns were written per row (BD-7 (2)).
    let (pl_a, name_a): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT source_playlist_id, source_playlist_name FROM migration_items WHERE id = ?",
    )
    .bind(id_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pl_a.as_deref(), Some("pl-1"));
    assert_eq!(name_a.as_deref(), Some("Playlist One"));

    // Recording item A's result must NOT touch item B — with the pre-fix
    // UPDATE (WHERE job_id AND source_track_id) both rows flipped together.
    record_migration_item_result(
        &pool,
        id_a,
        "transferred",
        1.0,
        "isrc",
        Some("dest-dup"),
        None,
    )
    .await
    .expect("record a");

    let row_b: (String, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, dest_track_id, error_message, processed_at FROM migration_items WHERE id = ?",
    )
    .bind(id_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row_b.0, "pending",
        "item B must stay untouched by item A's result"
    );
    assert_eq!(row_b.1, None);
    assert_eq!(row_b.2, None);
    assert_eq!(row_b.3, None);

    // Item B fails on its own and records its own error message.
    record_migration_item_result(
        &pool,
        id_b,
        "failed",
        0.0,
        "none",
        None,
        Some("No match found on qobuz"),
    )
    .await
    .expect("record b");

    let (status_a, dest_a, err_a): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, dest_track_id, error_message FROM migration_items WHERE id = ?",
    )
    .bind(id_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status_a, "transferred", "item A keeps its own result");
    assert_eq!(dest_a.as_deref(), Some("dest-dup"));
    assert_eq!(err_a, None, "successful items carry no error message");

    let (status_b, err_b): (String, Option<String>) =
        sqlx::query_as("SELECT status, error_message FROM migration_items WHERE id = ?")
            .bind(id_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status_b, "failed");
    assert_eq!(err_b.as_deref(), Some("No match found on qobuz"));
}

// ============================================================================
// BD-6: the track-selection helper itself
// ============================================================================

#[tokio::test]
async fn test_fetch_migration_source_tracks_scopes_and_deduplicates() {
    let pool = setup_test_db().await;
    let (_t1, _pl_a, _pl_b) = seed_scope_fixture(&pool).await;

    let scoped =
        fetch_migration_source_tracks(&pool, "spotify", Some(&["pl-scope-a".to_string()]), None)
            .await
            .expect("fetch scoped");
    assert_eq!(scoped.len(), 2);
    assert!(scoped
        .iter()
        .all(|t| t.playlist_id.as_deref() == Some("pl-scope-a")
            && t.playlist_name.as_deref() == Some("Scope A")));
    assert!(scoped.iter().all(|t| t.external_id.starts_with("sp-")));

    // Song One sits twice in Scope B — one item only.
    let dup =
        fetch_migration_source_tracks(&pool, "spotify", Some(&["pl-scope-b".to_string()]), None)
            .await
            .expect("fetch dup playlist");
    assert_eq!(dup.len(), 1);
    assert_eq!(dup[0].external_id, "sp-one");

    // Both playlists at once: Song One yields one item per playlist.
    let both = fetch_migration_source_tracks(
        &pool,
        "spotify",
        Some(&["pl-scope-a".to_string(), "pl-scope-b".to_string()]),
        None,
    )
    .await
    .expect("fetch both playlists");
    assert_eq!(both.len(), 3, "one item per (track, playlist) pair");
    assert_eq!(
        both.iter().filter(|t| t.external_id == "sp-one").count(),
        2,
        "Song One appears once per playlist with its own attribution"
    );
    assert!(both
        .iter()
        .any(|t| t.playlist_id.as_deref() == Some("pl-scope-a")));
    assert!(both
        .iter()
        .any(|t| t.playlist_id.as_deref() == Some("pl-scope-b")));

    // Unknown selection: nothing.
    let unknown =
        fetch_migration_source_tracks(&pool, "spotify", Some(&["pl-nope".to_string()]), None)
            .await
            .expect("fetch unknown");
    assert!(unknown.is_empty());

    // Favorites scope: every spotify item, no playlist attribution, and the
    // qobuz-only track stays out.
    let favorites = fetch_migration_source_tracks(&pool, "spotify", None, None)
        .await
        .expect("fetch favorites");
    assert_eq!(favorites.len(), 2);
    assert!(favorites
        .iter()
        .all(|t| t.playlist_id.is_none() && t.playlist_name.is_none()));
    assert!(favorites.iter().all(|t| t.external_id != "qb-three"));

    // Empty selection: nothing.
    let empty = fetch_migration_source_tracks(&pool, "spotify", Some(&[]), None)
        .await
        .expect("fetch empty");
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_job_items_row_helper_reflects_written_accounting() {
    let pool = setup_test_db().await;
    let job_id = "job-helper";
    sqlx::query(
        "INSERT INTO migration_jobs (id, source_service, destination_service, options, status)
         VALUES (?, 'spotify', 'qobuz', '{}', 'running')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let track = MigrationSourceTrack {
        external_id: "sp-helper".to_string(),
        title: "Helper Song".to_string(),
        artist: "Helper Artist".to_string(),
        album: Some("Helper Album".to_string()),
        playlist_id: None,
        playlist_name: None,
    };
    let item_id = insert_migration_item(&pool, job_id, &track)
        .await
        .expect("insert")
        .expect("row id");

    let rows = job_items(&pool, job_id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, item_id);
    assert_eq!(rows[0].1, "sp-helper");
    assert_eq!(rows[0].4.as_deref(), Some("pending"), "items start pending");

    // Sanity: the row reads back through the same projection the CLI/UI uses.
    let raw: (String, Option<String>) = sqlx::query_as(
        "SELECT source_track_title, source_track_album FROM migration_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(raw.0, "Helper Song");
    assert_eq!(raw.1.as_deref(), Some("Helper Album"));
}

#[tokio::test]
async fn chosen_source_account_filters_favorites_and_homonymous_playlists() {
    let pool = setup_test_db().await;
    let (first_track, first_playlist, _) = seed_scope_fixture(&pool).await;
    let account_a: i64 =
        sqlx::query_scalar("SELECT id FROM accounts WHERE email = 'scope@test.dev'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let account_b: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'other@test.dev', '{}', 0) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let second_track: i64 = sqlx::query_scalar(
        "SELECT track_id FROM track_sources WHERE service_id = 1 AND service_track_id = 'sp-two'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (?, ?), (?, ?)")
        .bind(account_a)
        .bind(first_track)
        .bind(account_b)
        .bind(second_track)
        .execute(&pool)
        .await
        .unwrap();
    let other_playlist: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service) VALUES (?, 'pl-scope-a', 'Other Scope', 'spotify') RETURNING id",
    ).bind(account_b).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(other_playlist)
        .bind(second_track)
        .execute(&pool)
        .await
        .unwrap();

    // Removing a favorite retains its library_entries row but excludes it
    // from explicit-account migrations.
    sqlx::query("UPDATE library_entries SET is_liked = 0 WHERE account_id = ? AND track_id = ?")
        .bind(account_b)
        .bind(second_track)
        .execute(&pool)
        .await
        .unwrap();
    let removed = fetch_migration_source_tracks(&pool, "spotify", None, Some(account_b))
        .await
        .unwrap();
    assert!(
        removed.is_empty(),
        "unliked tracks do not migrate as favorites"
    );
    sqlx::query("UPDATE library_entries SET is_liked = 1 WHERE account_id = ? AND track_id = ?")
        .bind(account_b)
        .bind(second_track)
        .execute(&pool)
        .await
        .unwrap();

    let a_favorites = fetch_migration_source_tracks(&pool, "spotify", None, Some(account_a))
        .await
        .unwrap();
    let b_favorites = fetch_migration_source_tracks(&pool, "spotify", None, Some(account_b))
        .await
        .unwrap();
    assert_eq!(
        a_favorites
            .iter()
            .map(|t| t.external_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sp-one"]
    );
    assert_eq!(
        b_favorites
            .iter()
            .map(|t| t.external_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sp-two"]
    );
    let unscoped = fetch_migration_source_tracks(&pool, "spotify", None, None)
        .await
        .unwrap();
    assert_eq!(unscoped.len(), 2, "None reads the full service mirror");
    // None must not silently become the active account, even with two rows.
    // The playlist path has the same legacy service-wide contract.
    let all_playlists =
        fetch_migration_source_tracks(&pool, "spotify", Some(&["pl-scope-a".to_string()]), None)
            .await
            .unwrap();
    assert_eq!(all_playlists.len(), 3);
    assert!(all_playlists
        .iter()
        .any(|track| track.playlist_name.as_deref() == Some("Other Scope")));

    let ids = ["pl-scope-a".to_string()];
    let a_playlist = fetch_migration_source_tracks(&pool, "spotify", Some(&ids), Some(account_a))
        .await
        .unwrap();
    let b_playlist = fetch_migration_source_tracks(&pool, "spotify", Some(&ids), Some(account_b))
        .await
        .unwrap();
    assert_eq!(a_playlist.len(), 2);
    assert_eq!(b_playlist.len(), 1);
    assert_eq!(b_playlist[0].external_id, "sp-two");
    assert_eq!(b_playlist[0].playlist_name.as_deref(), Some("Other Scope"));
    assert_ne!(first_playlist, other_playlist);
}

#[tokio::test]
async fn legacy_job_keeps_null_source_and_reconstructs_all_service_playlists() {
    let pool = setup_test_db().await;
    let (first_track, _, _) = seed_scope_fixture(&pool).await;
    let account_b: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'second-legacy@test.dev', '{}', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let playlist_b: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service) VALUES (?, 'pl-legacy-b', 'Legacy B', 'spotify') RETURNING id",
    )
    .bind(account_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(playlist_b)
        .bind(first_track)
        .execute(&pool)
        .await
        .unwrap();

    let app = test_app(pool.clone());
    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".into(),
        "qobuz".into(),
        Some(vec!["pl-legacy-b".into()]),
        loose_options(),
        None,
        None,
    )
    .await
    .unwrap();
    let source_account: Option<i64> =
        sqlx::query_scalar("SELECT source_account_id FROM migration_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        source_account, None,
        "a service-wide read must stay NULL in history"
    );
    let items = job_items(&pool, &job_id).await;
    assert_eq!(
        items.len(),
        1,
        "playlist of the inactive account must enter the global job"
    );
    assert_eq!(items[0].2.as_deref(), Some("pl-legacy-b"));
}
