//! Regression tests for audit item 4.2 (service → service mode in the
//! migration assistant).
//!
//! Pins three engine behaviors the mode depends on:
//!   1. preview_migration reports REAL counts — with no destination account it
//!      refuses instead of returning the removed `* 0.85` fabricated estimate,
//!      and it never writes (no job, no items).
//!   2. find_manual_match surfaces the latest manual match the user attached
//!      on a previous run of the same source → destination route, so reviewed
//!      matches are effective on the next migration.
//!   3. start_migration without a destination client never claims a transfer
//!      — not even for a manually matched track (nothing fabricates success).
//!   4. a job the user cancels is never closed as 'completed': the loop stops
//!      early and the final migration-progress event says 'cancelled'.
//!
//! get_migration_destinations is the data-driven destination list the wizard
//! renders; adding a backend destination must require no UI change.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;

use syncify_tauri_lib::commands::migration::{
    cancel_migration, find_manual_match, get_migration_destinations, preview_migration,
    start_migration, MIGRATION_DESTINATION_SERVICES,
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

fn options() -> MigrationOptions {
    MigrationOptions {
        match_threshold: 0.80,
        skip_unmatched: true,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    }
}

/// Seeds services (1 = spotify, 2 = qobuz), a spotify account, and two spotify
/// tracks with service identity (track_sources mirrors them into
/// library_items via the 0086 triggers).
async fn seed_source_fixture(pool: &SqlitePool) {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();

    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Mode Album') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Mode Artist') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    let mut track_ids = Vec::new();
    for title in &["Mode Song One", "Mode Song Two"] {
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
    let (t1, t2) = (track_ids[0], track_ids[1]);

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-mode-one'), (?, 1, 'sp-mode-two')")
        .bind(t1)
        .bind(t2)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'mode@test.dev', '{}', 1)",
    )
    .execute(pool)
    .await
    .unwrap();
}

// ============================================================================
// 1. preview_migration: real counts, honest refusal, no writes
// ============================================================================

#[tokio::test]
async fn test_preview_migration_refuses_without_destination_account_instead_of_estimating() {
    let pool = setup_test_db().await;
    seed_source_fixture(&pool).await;
    let app = test_app(pool.clone());

    // No qobuz account exists: there is no real match count, so the preview
    // must refuse — the old implementation returned `total * 0.85`.
    let result = preview_migration(
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        options(),
        None,
        None,
    )
    .await;

    let error = result.expect_err("preview without a destination account must fail");
    assert!(
        error.to_lowercase().contains("no connected qobuz account"),
        "the error must explain the missing destination account, got: {error}"
    );

    // A preview never writes: no job, no items.
    let jobs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let items: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM migration_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs.0, 0, "preview must not create migration jobs");
    assert_eq!(items.0, 0, "preview must not create migration items");
}

#[tokio::test]
async fn test_preview_migration_reports_real_totals_from_the_source_library() {
    let pool = setup_test_db().await;
    seed_source_fixture(&pool).await;
    let app = test_app(pool.clone());

    // Seed a usable (structurally) qobuz account so the preview proceeds to
    // matching. The stored token is invalid, so every real search fails — the
    // honest real-world outcome is 0 matched, 2 unmatched, never an estimate.
    sqlx::query(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (2, 'qobuz@test.dev', '{\"user_auth_token\":\"invalid-token\"}', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let preview = preview_migration(
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        options(),
        None,
        None,
    )
    .await
    .expect("preview with a destination account must succeed");

    assert_eq!(
        preview.total_tracks, 2,
        "total counts the real source tracks"
    );
    assert_eq!(preview.matched_tracks, 0, "failed searches are not matches");
    assert_eq!(preview.unmatched_tracks, 2);
    assert!(
        preview.playlists.is_empty(),
        "favorites scope yields no playlist rows"
    );
}

// ============================================================================
// 2. Manual matches are effective on the next run of the same route
// ============================================================================

async fn seed_job_with_manual_match(
    pool: &SqlitePool,
    job_id: &str,
    destination_service: &str,
    source_track_id: &str,
    dest_track_id: &str,
) {
    sqlx::query(
        "INSERT INTO migration_jobs (id, source_service, destination_service, options, status)
         VALUES (?, 'spotify', ?, '{}', 'completed')",
    )
    .bind(job_id)
    .bind(destination_service)
    .execute(pool)
    .await
    .unwrap();

    let item_id: i64 = sqlx::query_scalar(
        "INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist, status)
         VALUES (?, ?, 'Mode Song One', 'Mode Artist', 'skipped') RETURNING id",
    )
    .bind(job_id)
    .bind(source_track_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // The state manual_match_item writes (4.2: manual match is reviewable).
    sqlx::query(
        "UPDATE migration_items SET destination_track_id = ?, dest_track_id = ?, match_method = 'manual', match_confidence = 1.0, status = 'matched' WHERE id = ?",
    )
    .bind(dest_track_id)
    .bind(dest_track_id)
    .bind(item_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_find_manual_match_returns_the_latest_match_for_the_route() {
    let pool = setup_test_db().await;
    seed_job_with_manual_match(&pool, "job-old", "qobuz", "sp-mode-one", "qb-old").await;

    let found = find_manual_match(&pool, "sp-mode-one", "qobuz", None)
        .await
        .expect("lookup succeeds");
    assert_eq!(found.as_deref(), Some("qb-old"));

    // A newer manual match on the same route wins.
    seed_job_with_manual_match(&pool, "job-new", "qobuz", "sp-mode-one", "qb-new").await;
    let latest = find_manual_match(&pool, "sp-mode-one", "qobuz", None)
        .await
        .expect("lookup succeeds");
    assert_eq!(
        latest.as_deref(),
        Some("qb-new"),
        "the newest manual match is applied"
    );

    // A different destination route does not inherit the match.
    let other_route = find_manual_match(&pool, "sp-mode-one", "tidal", None)
        .await
        .expect("lookup succeeds");
    assert_eq!(
        other_route, None,
        "manual matches are per destination service"
    );

    // A track never manually matched finds nothing.
    let unmatched_track = find_manual_match(&pool, "sp-mode-two", "qobuz", None)
        .await
        .expect("lookup succeeds");
    assert_eq!(unmatched_track, None);
}

#[tokio::test]
async fn test_start_migration_applies_manual_match_without_fabricating_a_transfer() {
    let pool = setup_test_db().await;
    seed_source_fixture(&pool).await;
    // Manual match from a previous run on this exact route.
    seed_job_with_manual_match(&pool, "job-previous", "qobuz", "sp-mode-one", "qb-old").await;
    let app = test_app(pool.clone());

    // No qobuz client is available in this environment: the favorite cannot be
    // added, so the item must FAIL with the real reason — never report a
    // transfer that did not happen.
    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        options(),
        None,
        None,
    )
    .await
    .expect("start_migration must succeed");

    let rows: Vec<(
        String,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT source_track_id, status, match_method, match_confidence, error_message
             FROM migration_items WHERE job_id = ? ORDER BY id",
    )
    .bind(&job_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    let manual = rows
        .iter()
        .find(|r| r.0 == "sp-mode-one")
        .expect("manual item present");
    assert_eq!(
        manual.1.as_deref(),
        Some("failed"),
        "no transfer without a destination client"
    );
    assert!(manual
        .4
        .as_deref()
        .unwrap_or("")
        .contains("could not be transferred"));

    let other = rows
        .iter()
        .find(|r| r.0 == "sp-mode-two")
        .expect("regular item present");
    assert_eq!(other.1.as_deref(), Some("failed"));
    assert_ne!(
        other.3,
        Some(1.0),
        "non-manual items keep their real match outcome"
    );
}

// ============================================================================
// 3. Data-driven destination list
// ============================================================================

#[test]
fn test_get_migration_destinations_lists_the_engine_supported_services() {
    let destinations = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(get_migration_destinations())
        .expect("command succeeds");
    assert_eq!(
        destinations,
        MIGRATION_DESTINATION_SERVICES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    );
    // Every listed destination maps to real client support in the engine.
    for destination in &destinations {
        assert!(
            [
                "qobuz",
                "tidal",
                "spotify",
                "deezer",
                "soundcloud",
                "apple_music"
            ]
            .contains(&destination.as_str()),
            "destination {destination} must have matching client support"
        );
    }
}

// ============================================================================
// 4. Cancelling a running job: the progress channel never claims completion
// ============================================================================

/// Seeds `count` spotify tracks so the migration loop lasts long enough for a
/// cancellation to land while it runs (the engine sleeps 100 ms per item).
async fn seed_many_source_tracks(pool: &SqlitePool, count: usize) {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();

    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Cancel Album') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Cancel Artist') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    for index in 0..count {
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms) VALUES (?, ?, 200000) RETURNING id",
        )
        .bind(format!("Cancel Song {}", index))
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
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, ?)",
        )
        .bind(track_id)
        .bind(format!("sp-cancel-{}", index))
        .execute(pool)
        .await
        .unwrap();
    }

    sqlx::query(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'cancel@test.dev', '{}', 1)",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_start_migration_stays_cancelled_instead_of_reporting_completion() {
    let pool = setup_test_db().await;
    seed_many_source_tracks(&pool, 12).await;
    let app = test_app(pool.clone());

    // The real cancel command, issued as soon as the job is running: the loop
    // polls the row before every item, so it stops early.
    let cancel_when_running = async {
        for _ in 0..2000 {
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT id, status FROM migration_jobs WHERE source_service = 'spotify'
                 ORDER BY rowid DESC LIMIT 1",
            )
            .fetch_optional(&pool)
            .await
            .unwrap();
            if let Some((id, status)) = row {
                if status == "running" {
                    cancel_migration(app.state::<AppState>(), id.clone())
                        .await
                        .expect("cancel_migration must succeed");
                    return Some(id);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        None
    };

    let (result, cancelled_id) = tokio::join!(
        start_migration(
            app.handle().clone(),
            app.state::<AppState>(),
            "spotify".to_string(),
            "qobuz".to_string(),
            None,
            options(),
            None,
            None
        ),
        cancel_when_running,
    );

    let job_id = result.expect("start_migration must succeed");
    let cancelled_id = cancelled_id.expect("the job must have been cancelled while running");
    assert_eq!(cancelled_id, job_id);

    let (status, total_items, completed_items): (String, i64, i64) = sqlx::query_as(
        "SELECT status, total_items, completed_items FROM migration_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        status, "cancelled",
        "a job the user cancelled must not be reported as completed"
    );
    assert_eq!(total_items, 12, "the whole source library was queued");
    assert!(
        completed_items < total_items,
        "the loop stopped early, so not every queued item was processed"
    );
}

#[tokio::test]
async fn manual_matches_are_isolated_by_destination_account() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    let account_a: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, is_active) VALUES (2, 'a@q.test', 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let account_b: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, is_active) VALUES (2, 'b@q.test', 0) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    seed_job_with_manual_match(&pool, "account-a-job", "qobuz", "sp-mode-one", "qb-a").await;
    sqlx::query("UPDATE migration_jobs SET destination_account_id = ? WHERE id = 'account-a-job'")
        .bind(account_a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        find_manual_match(&pool, "sp-mode-one", "qobuz", Some(account_a))
            .await
            .unwrap()
            .as_deref(),
        Some("qb-a")
    );
    assert_eq!(
        find_manual_match(&pool, "sp-mode-one", "qobuz", Some(account_b))
            .await
            .unwrap(),
        None
    );
    seed_job_with_manual_match(
        &pool,
        "legacy-unscoped-job",
        "qobuz",
        "sp-mode-two",
        "qb-legacy",
    )
    .await;
    assert_eq!(
        find_manual_match(&pool, "sp-mode-two", "qobuz", Some(account_a))
            .await
            .unwrap(),
        None,
        "an unscoped historical choice cannot migrate into account A"
    );
    assert_eq!(
        find_manual_match(&pool, "sp-mode-two", "qobuz", Some(account_b))
            .await
            .unwrap(),
        None,
        "nor into account B"
    );
    assert_eq!(
        find_manual_match(&pool, "sp-mode-two", "qobuz", None)
            .await
            .unwrap()
            .as_deref(),
        Some("qb-legacy")
    );
}

#[tokio::test]
async fn migration_rejects_identical_pair_and_accepts_distinct_inactive_account() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    let a: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (2, 'a@q.test', '{}', 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let b: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (2, 'b@q.test', '{}', 0) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let app = test_app(pool.clone());
    // A global mirror is not a safe source for transfers within one service,
    // even when the destination is an explicit different account.
    for (source, destination) in [
        (None, None),
        (None, Some(b)),
        (Some(a), None),
        (Some(a), Some(a)),
    ] {
        let preview = preview_migration(
            app.state::<AppState>(),
            "qobuz".into(),
            "qobuz".into(),
            Some(vec![]),
            options(),
            source,
            destination,
        )
        .await;
        assert!(
            preview
                .unwrap_err()
                .contains("explicitly selected, different accounts"),
            "preview must reject non-explicit or identical same-service pairs"
        );
    }
    let case_variant = preview_migration(
        app.state::<AppState>(),
        "QOBUZ".into(),
        "qobuz".into(),
        Some(vec![]),
        options(),
        None,
        Some(b),
    )
    .await;
    assert!(case_variant
        .unwrap_err()
        .contains("explicitly selected, different accounts"));
    let valid_preview = preview_migration(
        app.state::<AppState>(),
        "qobuz".into(),
        "qobuz".into(),
        Some(vec![]),
        options(),
        Some(a),
        Some(b),
    )
    .await;
    // Both account ids pass validation. These fixture credentials are empty,
    // so the expected error is client availability, not same-service scope.
    assert!(valid_preview.unwrap_err().contains(&format!("account {b}")));
    let identical = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "qobuz".into(),
        "qobuz".into(),
        Some(vec![]),
        options(),
        None,
        Some(a),
    )
    .await;
    assert!(identical.unwrap_err().contains("different accounts"));
    for (source, destination) in [(Some(a), None), (Some(a), Some(a))] {
        let result = start_migration(
            app.handle().clone(),
            app.state::<AppState>(),
            "qobuz".into(),
            "qobuz".into(),
            Some(vec![]),
            options(),
            source,
            destination,
        )
        .await;
        assert!(result
            .unwrap_err()
            .contains("explicitly selected, different accounts"));
    }
    let jobs_before_valid: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs_before_valid, 0, "invalid pairs must not create jobs");
    let foreign = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "qobuz".into(),
        "spotify".into(),
        Some(vec![]),
        options(),
        Some(a),
        Some(b),
    )
    .await;
    assert!(foreign.unwrap_err().contains("does not belong"));
    let job = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "qobuz".into(),
        "qobuz".into(),
        Some(vec![]),
        options(),
        Some(a),
        Some(b),
    )
    .await
    .unwrap();
    let stored: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT source_account_id, destination_account_id FROM migration_jobs WHERE id = ?",
    )
    .bind(&job)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        (Some(a), Some(b)),
        "inactive destination is explicitly allowed"
    );
}

#[tokio::test]
async fn historical_job_accounts_are_nullable_and_detach_on_account_removal() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    let account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email) VALUES (2, 'history@q.test') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO migration_jobs (id, source_service, destination_service, options) VALUES ('old-job', 'spotify', 'qobuz', '{}')")
        .execute(&pool).await.unwrap();
    let app = test_app(pool.clone());
    let history = syncify_tauri_lib::commands::migration::get_migration_history(
        app.state::<AppState>(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(history[0].source_account_id, None);
    assert_eq!(history[0].destination_account_id, None);
    sqlx::query("UPDATE migration_jobs SET destination_account_id = ? WHERE id = 'old-job'")
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    let job = syncify_tauri_lib::commands::migration::get_migration_details(
        app.state::<AppState>(),
        "old-job".into(),
    )
    .await
    .unwrap();
    assert_eq!(
        job.destination_account_id, None,
        "ON DELETE SET NULL preserves history"
    );
}
