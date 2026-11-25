//! M05 (#170) — idempotencia y reanudación de migraciones.
//!
//! Costuras públicas: `retry_failed_items`, las fases por colección y la
//! recuperación de jobs `running` al arrancar. Todas usan SQLite `:memory:` y
//! gateways falsos; ninguna toca la biblioteca ni la red reales.

use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_tauri_lib::commands::migration::retry_failed_items;
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

fn options_json() -> String {
    serde_json::to_string(&MigrationOptions {
        match_threshold: 0.8,
        skip_unmatched: false,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    })
    .expect("serialize migration options")
}

/// #363 / D-06: retry used to reset rows and flip the job to `running` without
/// launching it, silently swallowing the job UPDATE error. The command must
/// drive the retry to a terminal state and preserve already transferred work.
#[tokio::test]
async fn retry_failed_items_relaunches_the_job_instead_of_leaving_it_running() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
               (id, source_service, destination_service, options, status,
                total_items, completed_items, failed_items, collections)
           VALUES ('retry-job', 'spotify', 'qobuz', ?, 'completed',
                   2, 1, 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
               (job_id, kind, source_track_id, source_track_title,
                source_track_artist, status, destination_track_id)
           VALUES ('retry-job', 'track', 'sp-done', 'Done', 'Artist',
                   'transferred', 'qb-done'),
                  ('retry-job', 'track', 'sp-retry', 'Retry', 'Artist',
                   'failed', NULL)"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());
    let retried = retry_failed_items(app.state::<AppState>(), "retry-job".into())
        .await
        .expect("retry command");
    assert_eq!(retried, 1, "only the failed row is retried");

    let (status, completed, failed): (String, i64, i64) = sqlx::query_as(
        "SELECT status, completed_items, failed_items FROM migration_jobs WHERE id = 'retry-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(
        status, "running",
        "retry must not leave an orphan running job"
    );
    assert_eq!((completed, failed), (1, 1));
}

/// #363 / D-06: the second retry write is part of the command contract, not a
/// best-effort annotation. A failed job-state UPDATE must reach the caller.
#[tokio::test]
async fn retry_failed_items_propagates_a_job_state_write_failure() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
               (id, source_service, destination_service, options, status,
                total_items, completed_items, failed_items, collections)
           VALUES ('retry-job', 'spotify', 'qobuz', ?, 'completed',
                   1, 0, 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
               (job_id, kind, source_track_id, source_track_title,
                source_track_artist, status, error_message)
           VALUES ('retry-job', 'track', 'sp-retry', 'Retry', 'Artist',
                   'failed', 'previous failure')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"CREATE TRIGGER reject_retry_running
           BEFORE UPDATE OF status ON migration_jobs
           WHEN OLD.id = 'retry-job' AND NEW.status = 'running'
           BEGIN
               SELECT RAISE(ABORT, 'retry status write failed');
           END"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());

    let result = retry_failed_items(app.state::<AppState>(), "retry-job".into()).await;
    assert!(
        result.is_err(),
        "D-06: the status write failure must reach the caller"
    );
    assert!(result.unwrap_err().contains("retry status write failed"));
}
