//! Characterization tests for the migration engine as it stands before M01
//! (issue #166, job item kinds + favorite-album migration).
//!
//! These pin the observable behavior the album work must preserve, all on a
//! fully migrated in-memory database with no destination client available:
//!   - the threshold → status mapping on the simulated branch (the honest
//!     no-network branch acts here as the gateway under characterization),
//!   - the local destination-match short-circuit (`match_method = 'isrc'`
//!     from the service-wide mirror, transfer still attempted),
//!   - manual-match priority over the local match and its destination-account
//!     scoping,
//!   - the command-level rejections (same-service pair, capability gating),
//!   - and the M01 starting point: favorited albums are untouched by a legacy
//!     job — only track favorites enter the migration.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;

use syncify_tauri_lib::commands::migration::start_migration;
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

/// Seeds services (1 = spotify, 2 = qobuz), one spotify account, and two
/// spotify tracks (`sp-one`, `sp-two`) with service identity written through
/// track_sources, which the 0086 triggers mirror into library_items. Returns
/// the spotify account id.
async fn seed_library(pool: &SqlitePool) -> i64 {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();

    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Fixture Album') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Fixture Artist') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    for title in &["Song One", "Song Two"] {
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
    }

    let track_one: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Song One'")
        .fetch_one(pool)
        .await
        .unwrap();
    let track_two: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Song Two'")
        .fetch_one(pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-one'), (?, 1, 'sp-two')")
        .bind(track_one)
        .bind(track_two)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'char@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A spotify playlist holding both fixture tracks, external id `pl-char`.
async fn seed_playlist(pool: &SqlitePool, account_id: i64) {
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service) VALUES (?, 'pl-char', 'Characterization', 'spotify') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let track_one: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Song One'")
        .fetch_one(pool)
        .await
        .unwrap();
    let track_two: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Song Two'")
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 2)",
    )
    .bind(playlist_id)
    .bind(track_one)
    .bind(playlist_id)
    .bind(track_two)
    .execute(pool)
    .await
    .unwrap();
}

type ItemRow = (
    String,         // source_track_id
    String,         // status
    Option<f64>,    // match_confidence
    Option<String>, // match_method
    Option<String>, // destination_track_id
    Option<String>, // error_message
);

async fn job_items(pool: &SqlitePool, job_id: &str) -> Vec<ItemRow> {
    sqlx::query_as(
        "SELECT source_track_id, status, match_confidence, match_method,
                destination_track_id, error_message
         FROM migration_items WHERE job_id = ? ORDER BY id",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn job_row(pool: &SqlitePool, job_id: &str) -> (i64, i64, i64, i64, String, Option<String>) {
    sqlx::query_as(
        "SELECT total_items, completed_items, failed_items, skipped_items, status, completed_at
         FROM migration_jobs WHERE id = ?",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn start(
    app: &tauri::App<tauri::test::MockRuntime>,
    destination: &str,
    playlist_ids: Option<Vec<String>>,
    options: MigrationOptions,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
) -> Result<String, String> {
    start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        destination.to_string(),
        playlist_ids,
        options,
        source_account_id,
        destination_account_id,
    )
    .await
}

fn options(threshold: f64, skip_unmatched: bool, create_playlists: bool) -> MigrationOptions {
    MigrationOptions {
        match_threshold: threshold,
        skip_unmatched,
        create_playlists,
        merge_existing: false,
        download_matched: false,
    }
}

// ============================================================================
// Threshold → status mapping on the simulated branch
// ============================================================================

/// No destination client exists, so matching lands on the simulated branch:
/// confidence 0.85, method 'simulated', no destination id. The status then
/// follows only from the threshold and the skip_unmatched flag — with the
/// default threshold the simulated confidence clears it, so an item without a
/// transfer target FAILS (it was "matched" but could not be transferred), not
/// skipped.
#[tokio::test]
async fn test_simulated_branch_maps_threshold_and_skip_flag_to_status() {
    let pool = setup_test_db().await;
    let account = seed_library(&pool).await;
    seed_playlist(&pool, account).await;
    let app = test_app(pool.clone());

    // Default threshold 0.80 < 0.85: "matched" but no target → failed, and
    // the recorded error is the simulated branch's own message.
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        None,
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 2);
    for item in &items {
        assert_eq!(
            item.1, "failed",
            "0.85 clears 0.80 without a transfer target"
        );
        assert_eq!(item.2, Some(0.85));
        assert_eq!(item.3.as_deref(), Some("simulated"));
        assert_eq!(item.4, None, "no destination id is recorded");
        let error = item
            .5
            .as_deref()
            .expect("failed items carry an error message");
        assert!(
            error.contains("client unavailable"),
            "the simulated branch's own error reaches the item, got: {error}"
        );
    }
    let job = job_row(&pool, &job_id).await;
    assert_eq!((job.0, job.1, job.2, job.3), (2, 0, 2, 0));
    assert_eq!(
        job.4, "completed",
        "the loop finishes, so the job closes completed"
    );
    assert!(job.5.is_some(), "completed_at is stamped");
}

/// Above the simulated confidence the item counts as unmatched: skipped when
/// skip_unmatched is on, failed with an error message when it is off.
#[tokio::test]
async fn test_unmatched_items_skip_or_fail_following_the_skip_flag() {
    let pool = setup_test_db().await;
    let account = seed_library(&pool).await;
    seed_playlist(&pool, account).await;
    let app = test_app(pool.clone());

    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.90, true, false),
        None,
        None,
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    for item in &items {
        assert_eq!(item.1, "skipped", "0.85 < 0.90 with skip_unmatched");
        assert_eq!(item.5, None, "skipped items carry no error message");
    }
    let job = job_row(&pool, &job_id).await;
    assert_eq!(
        (job.1, job.2, job.3),
        (0, 0, 2),
        "all items counted skipped"
    );

    // Same threshold, skip flag off: the job fails the items instead.
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.90, false, false),
        None,
        None,
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    for item in &items {
        assert_eq!(item.1, "failed", "0.85 < 0.90 without skip_unmatched");
        assert!(item
            .5
            .as_deref()
            .unwrap_or("")
            .contains("client unavailable"));
    }
    let job = job_row(&pool, &job_id).await;
    assert_eq!((job.1, job.2, job.3), (0, 2, 0));
}

// ============================================================================
// Local destination match short-circuit
// ============================================================================

/// When the service-wide mirror already knows the same canonical track on the
/// destination service, the engine skips remote search entirely: the item is
/// marked with the local destination id, `match_method = 'isrc'` and full
/// confidence — and the transfer (add to favorites) is still attempted, so
/// without a client the item fails with the transfer error, not the simulated
/// one. A sibling track without the mirror identity stays on `simulated`.
#[tokio::test]
async fn test_local_destination_match_short_circuits_remote_search() {
    let pool = setup_test_db().await;
    let account = seed_library(&pool).await;
    seed_playlist(&pool, account).await;

    let track_one: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Song One'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 2, 'qb-local-1')")
        .bind(track_one)
        .execute(&pool)
        .await
        .unwrap();

    let app = test_app(pool.clone());
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        None,
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    let local = items
        .iter()
        .find(|i| i.0 == "sp-one")
        .expect("sp-one entered the job");
    assert_eq!(
        local.1, "failed",
        "the write is attempted and fails without a client"
    );
    assert_eq!(local.2, Some(1.0));
    assert_eq!(
        local.3.as_deref(),
        Some("isrc"),
        "the local match takes the isrc method"
    );
    assert_eq!(
        local.4, None,
        "the destination id is not recorded on failure"
    );
    assert!(
        local
            .5
            .as_deref()
            .unwrap_or("")
            .contains("Failed to add to favorites"),
        "the transfer error is recorded, got: {:?}",
        local.5
    );

    let simulated = items
        .iter()
        .find(|i| i.0 == "sp-two")
        .expect("sp-two entered the job");
    assert_eq!(
        simulated.3.as_deref(),
        Some("simulated"),
        "the sibling stays on remote search"
    );
}

// ============================================================================
// Manual matches: priority and destination-account scoping
// ============================================================================

/// A manual match a user attached on a previous job toward the same
/// destination account wins over the local mirror match, and its transfer is
/// attempted like any other match.
#[tokio::test]
async fn test_manual_match_beats_the_local_mirror_match() {
    let pool = setup_test_db().await;
    let account = seed_library(&pool).await;
    seed_playlist(&pool, account).await;

    let track_one: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Song One'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 2, 'qb-local-1')")
        .bind(track_one)
        .execute(&pool)
        .await
        .unwrap();

    let dest_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (2, 'dest@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status, destination_account_id)
           VALUES ('seed-manual', 'spotify', 'qobuz', '{}', 'completed', ?)"#,
    )
    .bind(dest_account)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist,
                                        destination_track_id, match_confidence, match_method, status)
           VALUES ('seed-manual', 'sp-one', 'Song One', 'Fixture Artist', 'qb-manual-1', 1.0, 'manual', 'transferred')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        Some(dest_account),
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    let item = items
        .iter()
        .find(|i| i.0 == "sp-one")
        .expect("sp-one entered the job");
    assert_eq!(
        item.3.as_deref(),
        Some("manual"),
        "manual wins over the local mirror match"
    );
    assert_eq!(item.2, Some(1.0));
    assert!(item
        .5
        .as_deref()
        .unwrap_or("")
        .contains("Failed to add to favorites"));
}

/// Manual matches are scoped to the destination account of the job that made
/// them: an explicit account must not inherit a choice made for another
/// account, and a NULL-destination job must not inherit one made for an
/// account (nor the other way around).
#[tokio::test]
async fn test_manual_matches_do_not_cross_destination_accounts() {
    let pool = setup_test_db().await;
    let account = seed_library(&pool).await;
    seed_playlist(&pool, account).await;

    let dest_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (2, 'dest@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status, destination_account_id)
           VALUES ('seed-manual', 'spotify', 'qobuz', '{}', 'completed', ?)"#,
    )
    .bind(dest_account)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist,
                                        destination_track_id, match_confidence, match_method, status)
           VALUES ('seed-manual', 'sp-one', 'Song One', 'Fixture Artist', 'qb-manual-1', 1.0, 'manual', 'transferred')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());

    // A different explicit account inherits nothing: the item falls through to
    // the simulated branch.
    let other_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (2, 'other@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        Some(other_account),
    )
    .await
    .expect("job must start");
    let items = job_items(&pool, &job_id).await;
    let item = items.iter().find(|i| i.0 == "sp-one").unwrap();
    assert_eq!(
        item.3.as_deref(),
        Some("simulated"),
        "another account does not inherit the manual match"
    );

    // A NULL-destination job inherits nothing either: the scoping query pins
    // destination_account_id IS NULL, and the seeded match carries an account.
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        None,
    )
    .await
    .expect("job must start");
    let items = job_items(&pool, &job_id).await;
    let item = items.iter().find(|i| i.0 == "sp-one").unwrap();
    assert_eq!(
        item.3.as_deref(),
        Some("simulated"),
        "a NULL-destination job does not inherit an account match"
    );
}

// ============================================================================
// Command-level rejections
// ============================================================================

/// Same-service transfers need two explicit, different accounts; the command
/// refuses everything else and leaves no job row behind.
#[tokio::test]
async fn test_same_service_pair_without_two_accounts_is_refused() {
    let pool = setup_test_db().await;
    seed_library(&pool).await;
    let app = test_app(pool.clone());

    let result = start(
        &app,
        "spotify",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        None,
    )
    .await;
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.contains(
                "Same-service migration requires two explicitly selected, different accounts"
            ))
            .unwrap_or(false),
        "the same-service guard rejects the NULL pair, got: {:?}",
        result
    );

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0, "the refusal happens before any job row exists");
}

/// The capability matrix is authoritative at the command level: a playlist
/// transfer toward a destination that does not declare Playlists is refused
/// before the job exists, even when the UI is bypassed.
#[tokio::test]
async fn test_playlist_transfer_toward_unsupported_destination_is_refused() {
    let pool = setup_test_db().await;
    seed_library(&pool).await;
    let app = test_app(pool.clone());

    let result = start(
        &app,
        "soundcloud",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, true),
        None,
        None,
    )
    .await;
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.contains("does not support playlist transfer"))
            .unwrap_or(false),
        "soundcloud must refuse a playlist transfer, got: {:?}",
        result
    );

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        jobs, 0,
        "the gating refusal happens before any job row exists"
    );
}

// ============================================================================
// The M01 starting point: favorited albums are untouched by a legacy job
// ============================================================================

/// Favorited albums — both the canonical mirror (`albums.is_favorite` with a
/// service id) and the per-account cache (`favorites`, item_type = 'album') —
/// produce no migration items today: the engine only walks track favorites.
/// M01 builds the album collection on top of this pin.
#[tokio::test]
async fn test_legacy_job_does_not_migrate_favorited_albums() {
    let pool = setup_test_db().await;
    let account = seed_library(&pool).await;
    seed_playlist(&pool, account).await;

    sqlx::query(
        "INSERT INTO albums (title, is_favorite, favorite_at, spotify_id, qobuz_id) VALUES ('Favorite Album', 1, datetime('now'), 'sp-alb-1', 'qb-alb-1')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name) VALUES (?, 1, 'album', 'sp-alb-1', 'Favorite Album', 'Fixture Artist')",
    )
    .bind(account)
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());

    // Playlist-scoped: only the playlist's tracks enter the job.
    let job_id = start(
        &app,
        "qobuz",
        Some(vec!["pl-char".to_string()]),
        options(0.80, true, false),
        None,
        None,
    )
    .await
    .expect("job must start");
    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 2, "only the two playlist tracks enter the job");
    assert!(
        items.iter().all(|i| i.0 != "sp-alb-1" && i.0 != "qb-alb-1"),
        "no album id appears as a migration item"
    );

    // Library-wide (the favorites view): still tracks only — the album mirror
    // is invisible to the engine.
    let job_id = start(&app, "qobuz", None, options(0.80, true, false), None, None)
        .await
        .expect("job must start");
    let items = job_items(&pool, &job_id).await;
    assert_eq!(
        items.len(),
        2,
        "the library-wide job also walks tracks only"
    );
    assert!(
        items.iter().all(|i| i.0 != "sp-alb-1" && i.0 != "qb-alb-1"),
        "no album id appears as a migration item"
    );
}
