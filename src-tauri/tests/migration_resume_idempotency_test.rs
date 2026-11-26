//! M05 (#170) — idempotencia y reanudación de migraciones.
//!
//! Costuras públicas: `retry_failed_items`, las fases por colección y la
//! recuperación de jobs `running` al arrancar. Todas usan SQLite `:memory:` y
//! gateways falsos; ninguna toca la biblioteca ni la red reales.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_tauri_lib::commands::migration::{
    resume_migration_job_with_gateway, retry_failed_items, retry_failed_items_with_gateway,
    start_migration, AlbumTransferGateway, ArtistTransferGateway, MigrationSourceAlbum,
    MigrationSourceArtist, TrackTransferGateway,
};
use syncify_tauri_lib::models::MigrationOptions;
use syncify_tauri_lib::services::operation_recovery::recover_interrupted_migrations;
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
    let retried = retry_failed_items(
        app.handle().clone(),
        app.state::<AppState>(),
        "retry-job".into(),
    )
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

    let result = retry_failed_items(
        app.handle().clone(),
        app.state::<AppState>(),
        "retry-job".into(),
    )
    .await;
    assert!(
        result.is_err(),
        "D-06: the status write failure must reach the caller"
    );
    assert!(result.unwrap_err().contains("retry status write failed"));
}

/// Cancellation can land before any item fails. The later-phase rows are still
/// `pending`, and Re-run must resume them instead of leaving the cancelled job
/// with no retryable entry point.
#[tokio::test]
async fn retry_resumes_pending_rows_left_by_cancellation() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, options, status,
            total_items, completed_items, collections)
           VALUES ('cancelled-job', 'spotify', 'qobuz', ?, 'cancelled',
                   2, 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
           (job_id, kind, source_track_id, source_track_title,
            source_track_artist, status, destination_track_id)
           VALUES ('cancelled-job', 'track', 'sp-done', 'Done', 'Artist',
                   'transferred', 'qb-done'),
                  ('cancelled-job', 'track', 'sp-pending', 'sp-pending', 'Artist',
                   'pending', NULL)"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());
    let mut gateway = FakeMigrationGateway::new();
    gateway
        .track_matches
        .insert("sp-pending".to_string(), "qb-pending".to_string());

    let retried = retry_failed_items_with_gateway(app.handle(), &pool, "cancelled-job", &gateway)
        .await
        .expect("Re-run resumes the durable pending row");
    assert_eq!(retried, 1);
    assert_eq!(
        gateway.written(),
        vec![("track".to_string(), "qb-pending".to_string())]
    );
    let statuses: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_track_id, status FROM migration_items WHERE job_id = 'cancelled-job' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        statuses,
        vec![
            ("sp-done".to_string(), "transferred".to_string()),
            ("sp-pending".to_string(), "transferred".to_string()),
        ]
    );
    let job: (String, i64) = sqlx::query_as(
        "SELECT status, completed_items FROM migration_jobs WHERE id = 'cancelled-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job, ("completed".to_string(), 2));
}

/// #363: a crash after the runner wrote its last item but before it closed the
/// job leaves `running` with every row already terminal. Retry has nothing to
/// reset; #363 forbids a `running` job without a runner, so Re-run must drive
/// that job to its terminal state instead of leaving it running forever.
#[tokio::test]
async fn retry_closes_a_running_job_whose_items_are_all_terminal() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
               (id, source_service, destination_service, options, status,
                total_items, completed_items, collections)
           VALUES ('orphan-running', 'spotify', 'qobuz', ?, 'running',
                   1, 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
               (job_id, kind, source_track_id, source_track_title,
                source_track_artist, status, destination_track_id)
           VALUES ('orphan-running', 'track', 'sp-done', 'Done', 'Artist',
                   'transferred', 'qb-done')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());

    let retried = retry_failed_items(
        app.handle().clone(),
        app.state::<AppState>(),
        "orphan-running".into(),
    )
    .await
    .expect("retry command");
    assert_eq!(retried, 0, "no non-terminal row is reset");

    let job: (String, i64) = sqlx::query_as(
        "SELECT status, completed_items FROM migration_jobs WHERE id = 'orphan-running'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        job,
        ("completed".to_string(), 1),
        "retry must close a running job whose rows are all terminal"
    );
}

/// A retry cannot reconstruct rows that were never snapshotted. Refuse it before
/// resetting item state so a partial legacy job does not become an orphaned
/// running job or silently complete a smaller migration.
#[tokio::test]
async fn retry_refuses_an_incomplete_snapshot_without_resetting_items() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, options, status,
            total_items, failed_items, collections)
           VALUES ('partial-job', 'spotify', 'qobuz', ?, 'failed',
                   2, 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
           (job_id, kind, source_track_id, source_track_title,
            source_track_artist, status, error_message)
           VALUES ('partial-job', 'track', 'sp-partial', 'Partial', 'Artist',
                   'failed', 'previous failure')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());
    let gateway = FakeMigrationGateway::new();

    let error = retry_failed_items_with_gateway(app.handle(), &pool, "partial-job", &gateway)
        .await
        .expect_err("an incomplete snapshot is not retryable");
    assert!(error.contains("incomplete durable snapshot"));
    let item: (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM migration_items WHERE job_id = 'partial-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        item,
        ("failed".to_string(), Some("previous failure".to_string()))
    );
    let job: (String,) =
        sqlx::query_as("SELECT status FROM migration_jobs WHERE id = 'partial-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job.0, "failed");
    assert!(gateway.written().is_empty());
}

struct FakeMigrationGateway {
    track_matches: HashMap<String, String>,
    album_matches: HashMap<String, String>,
    artist_matches: HashMap<String, String>,
    existing_tracks: HashSet<String>,
    existing_albums: HashSet<String>,
    existing_artists: HashSet<String>,
    fail_after_write: HashSet<String>,
    snapshot_error: Option<String>,
    written: Mutex<Vec<(String, String)>>,
}

impl FakeMigrationGateway {
    fn new() -> Self {
        Self {
            track_matches: HashMap::new(),
            album_matches: HashMap::new(),
            artist_matches: HashMap::new(),
            existing_tracks: HashSet::new(),
            existing_albums: HashSet::new(),
            existing_artists: HashSet::new(),
            fail_after_write: HashSet::new(),
            snapshot_error: None,
            written: Mutex::new(Vec::new()),
        }
    }

    fn written(&self) -> Vec<(String, String)> {
        self.written.lock().unwrap().clone()
    }

    fn record_write(&self, kind: &str, dest_id: &str) -> Result<(), String> {
        self.written
            .lock()
            .unwrap()
            .push((kind.to_string(), dest_id.to_string()));
        if self.fail_after_write.contains(dest_id) {
            Err(format!("ambiguous write failure for {dest_id}"))
        } else {
            Ok(())
        }
    }
}

impl TrackTransferGateway for FakeMigrationGateway {
    async fn existing_tracks(&self) -> Result<HashSet<String>, String> {
        if let Some(error) = &self.snapshot_error {
            Err(error.clone())
        } else {
            Ok(self.existing_tracks.clone())
        }
    }

    async fn match_track(
        &self,
        _isrc: Option<&str>,
        title: &str,
        _artist: &str,
    ) -> (f64, &'static str, Option<String>, Option<String>) {
        // Source ids are encoded in the fixture title.
        match self.track_matches.get(title) {
            Some(id) => (0.9, "metadata", Some(id.clone()), None),
            None => (0.0, "none", None, None),
        }
    }

    async fn write_track(&self, dest_track_id: &str) -> Result<(), String> {
        self.record_write("track", dest_track_id)
    }
}

impl AlbumTransferGateway for FakeMigrationGateway {
    async fn existing_albums(&self) -> Result<HashSet<String>, String> {
        if let Some(error) = &self.snapshot_error {
            Err(error.clone())
        } else {
            Ok(self.existing_albums.clone())
        }
    }

    async fn match_album(
        &self,
        album: &MigrationSourceAlbum,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        Ok(self
            .album_matches
            .get(&album.external_id)
            .map(|id| (id.clone(), 0.9, "metadata")))
    }

    async fn write_album(&self, dest_album_id: &str) -> Result<(), String> {
        self.record_write("album", dest_album_id)
    }
}

impl ArtistTransferGateway for FakeMigrationGateway {
    async fn existing_artists(&self) -> Result<HashSet<String>, String> {
        if let Some(error) = &self.snapshot_error {
            Err(error.clone())
        } else {
            Ok(self.existing_artists.clone())
        }
    }

    async fn match_artist(
        &self,
        artist: &MigrationSourceArtist,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        Ok(self
            .artist_matches
            .get(&artist.external_id)
            .map(|id| (id.clone(), 0.9, "metadata")))
    }

    async fn write_artist(&self, dest_artist_id: &str) -> Result<(), String> {
        self.record_write("artist", dest_artist_id)
    }
}

async fn seed_resume_job(pool: &SqlitePool) {
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, options, status,
            total_items, completed_items, failed_items, skipped_items, collections)
           VALUES ('resume-job', 'spotify', 'qobuz', ?, 'running',
                   7, 1, 0, 0, '["tracks","albums","artists"]')"#,
    )
    .bind(options_json())
    .execute(pool)
    .await
    .unwrap();
    for (kind, source_id, status, dest_id) in [
        ("track", "sp-done", "transferred", Some("qb-done")),
        ("track", "sp-existing", "pending", None),
        ("track", "sp-new", "pending", None),
        ("album", "sp-alb-existing", "pending", None),
        ("album", "sp-alb-new", "pending", None),
        ("artist", "sp-art-existing", "pending", None),
        ("artist", "sp-art-new", "pending", None),
    ] {
        sqlx::query(
            r#"INSERT INTO migration_items
               (job_id, kind, source_track_id, source_track_title,
                source_track_artist, status, destination_track_id)
               VALUES ('resume-job', ?, ?, ?, 'Fixture Artist', ?, ?)"#,
        )
        .bind(kind)
        .bind(source_id)
        .bind(source_id)
        .bind(status)
        .bind(dest_id)
        .execute(pool)
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn resume_is_idempotent_for_tracks_albums_and_artists() {
    let pool = setup_test_db().await;
    seed_resume_job(&pool).await;
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status)
           VALUES ('completed-job', 'spotify', 'qobuz', '{}', 'completed'),
                  ('cancelled-job', 'spotify', 'qobuz', '{}', 'cancelled')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());
    let mut gateway = FakeMigrationGateway::new();
    gateway
        .track_matches
        .insert("sp-existing".to_string(), "qb-existing".to_string());
    gateway
        .track_matches
        .insert("sp-new".to_string(), "qb-new".to_string());
    gateway.existing_tracks.insert("qb-existing".to_string());
    gateway
        .album_matches
        .insert("sp-alb-existing".to_string(), "qb-alb-existing".to_string());
    gateway
        .album_matches
        .insert("sp-alb-new".to_string(), "qb-alb-new".to_string());
    gateway
        .existing_albums
        .insert("qb-alb-existing".to_string());
    gateway
        .artist_matches
        .insert("sp-art-existing".to_string(), "qb-art-existing".to_string());
    gateway
        .artist_matches
        .insert("sp-art-new".to_string(), "qb-art-new".to_string());
    gateway
        .existing_artists
        .insert("qb-art-existing".to_string());

    let interrupted = recover_interrupted_migrations(&pool)
        .await
        .expect("list interrupted migrations");
    assert_eq!(interrupted, vec!["resume-job".to_string()]);
    for job_id in interrupted {
        resume_migration_job_with_gateway(&pool, app.handle(), &job_id, &gateway)
            .await
            .expect("first resume");
    }

    assert_eq!(
        gateway.written(),
        vec![
            ("track".to_string(), "qb-new".to_string()),
            ("album".to_string(), "qb-alb-new".to_string()),
            ("artist".to_string(), "qb-art-new".to_string()),
        ],
        "only missing destination items are written"
    );
    let statuses: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_track_id, status FROM migration_items WHERE job_id = 'resume-job' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        statuses,
        vec![
            ("sp-done".to_string(), "transferred".to_string()),
            ("sp-existing".to_string(), "preexisting".to_string()),
            ("sp-new".to_string(), "transferred".to_string()),
            ("sp-alb-existing".to_string(), "preexisting".to_string()),
            ("sp-alb-new".to_string(), "transferred".to_string()),
            ("sp-art-existing".to_string(), "preexisting".to_string()),
            ("sp-art-new".to_string(), "transferred".to_string()),
        ]
    );
    let job: (String, i64, i64, i64) = sqlx::query_as(
        "SELECT status, completed_items, failed_items, skipped_items FROM migration_jobs WHERE id = 'resume-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job, ("completed".to_string(), 7, 0, 0));
    let item_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM migration_items WHERE job_id = 'resume-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(item_count, 7, "resume must not insert duplicate item rows");

    // A duplicate recovery pass sees the same durable terminal rows and must
    // not issue a second write.
    sqlx::query(
        "UPDATE migration_jobs SET status = 'running', completed_at = NULL WHERE id = 'resume-job'",
    )
    .execute(&pool)
    .await
    .unwrap();
    resume_migration_job_with_gateway(&pool, app.handle(), "resume-job", &gateway)
        .await
        .expect("second resume");
    assert_eq!(gateway.written().len(), 3, "terminal rows are not replayed");
}

#[tokio::test]
async fn ambiguous_post_write_failures_are_not_written_twice_on_retry() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, options, status,
            total_items, completed_items, failed_items, skipped_items, collections)
           VALUES ('ambiguous-job', 'spotify', 'qobuz', ?, 'running',
                   3, 0, 0, 0, '["tracks","albums","artists"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    for (kind, source_id) in [
        ("track", "sp-ambiguous"),
        ("album", "sp-alb-ambiguous"),
        ("artist", "sp-art-ambiguous"),
    ] {
        sqlx::query(
            r#"INSERT INTO migration_items
               (job_id, kind, source_track_id, source_track_title,
                source_track_artist, status)
               VALUES ('ambiguous-job', ?, ?, ?, 'Fixture Artist', 'pending')"#,
        )
        .bind(kind)
        .bind(source_id)
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let app = test_app(pool.clone());
    let mut gateway = FakeMigrationGateway::new();
    gateway
        .track_matches
        .insert("sp-ambiguous".to_string(), "qb-ambiguous".to_string());
    gateway.album_matches.insert(
        "sp-alb-ambiguous".to_string(),
        "qb-alb-ambiguous".to_string(),
    );
    gateway.artist_matches.insert(
        "sp-art-ambiguous".to_string(),
        "qb-art-ambiguous".to_string(),
    );
    gateway.fail_after_write = HashSet::from([
        "qb-ambiguous".to_string(),
        "qb-alb-ambiguous".to_string(),
        "qb-art-ambiguous".to_string(),
    ]);

    resume_migration_job_with_gateway(&pool, app.handle(), "ambiguous-job", &gateway)
        .await
        .expect("first pass records the ambiguous failures");
    let failed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = 'ambiguous-job' AND status = 'failed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed, 3);
    assert_eq!(gateway.written().len(), 3);

    // The provider accepted all three writes even though the caller observed a
    // failure. A retry must reuse the persisted destination ids (not rematch)
    // and observe destination membership before skipping the writes.
    gateway.track_matches.clear();
    gateway.album_matches.clear();
    gateway.artist_matches.clear();
    gateway.existing_tracks.insert("qb-ambiguous".to_string());
    gateway
        .existing_albums
        .insert("qb-alb-ambiguous".to_string());
    gateway
        .existing_artists
        .insert("qb-art-ambiguous".to_string());
    sqlx::query(
        "UPDATE migration_items SET status = 'pending', error_message = NULL, processed_at = NULL WHERE job_id = 'ambiguous-job'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE migration_jobs SET status = 'running', completed_at = NULL WHERE id = 'ambiguous-job'")
        .execute(&pool)
        .await
        .unwrap();

    resume_migration_job_with_gateway(&pool, app.handle(), "ambiguous-job", &gateway)
        .await
        .expect("retry observes the writes that landed");
    assert_eq!(gateway.written().len(), 3, "no ambiguous write is repeated");
    let preexisting: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = 'ambiguous-job' AND status = 'preexisting'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preexisting, 3);
    let job: (String, i64, i64) = sqlx::query_as(
        "SELECT status, completed_items, failed_items FROM migration_jobs WHERE id = 'ambiguous-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job, ("completed".to_string(), 3, 0));
}

#[tokio::test]
async fn matched_destination_survives_a_crash_before_the_result_update() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, options, status,
            total_items, collections)
           VALUES ('crash-job', 'spotify', 'qobuz', ?, 'running', 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
           (job_id, kind, source_track_id, source_track_title, source_track_artist, status)
           VALUES ('crash-job', 'track', 'sp-crash', 'sp-crash', 'Artist', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"CREATE TRIGGER reject_terminal_item_result
           BEFORE UPDATE OF status ON migration_items
           WHEN OLD.status = 'matched' AND NEW.status IN ('transferred', 'preexisting')
           BEGIN
               SELECT RAISE(ABORT, 'result update interrupted');
           END"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());
    let mut gateway = FakeMigrationGateway::new();
    gateway
        .track_matches
        .insert("sp-crash".to_string(), "qb-crash".to_string());

    let error = resume_migration_job_with_gateway(&pool, app.handle(), "crash-job", &gateway)
        .await
        .expect_err("the terminal result write is interrupted");
    assert!(error.contains("result update interrupted"));
    assert_eq!(gateway.written().len(), 1, "the remote write landed once");
    let matched: (String, Option<String>) = sqlx::query_as(
        "SELECT status, destination_track_id FROM migration_items WHERE job_id = 'crash-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        matched,
        ("matched".to_string(), Some("qb-crash".to_string()))
    );

    // Simulate the next process: the provider now reports the item present and
    // the runner no longer has the original match map.
    gateway.track_matches.clear();
    gateway.existing_tracks.insert("qb-crash".to_string());
    sqlx::query("DROP TRIGGER reject_terminal_item_result")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE migration_jobs SET status = 'running', completed_at = NULL WHERE id = 'crash-job'",
    )
    .execute(&pool)
    .await
    .unwrap();
    resume_migration_job_with_gateway(&pool, app.handle(), "crash-job", &gateway)
        .await
        .expect("resume reconciles the exact persisted destination");
    assert_eq!(
        gateway.written().len(),
        1,
        "the landed write is not repeated"
    );
    let item: (String,) =
        sqlx::query_as("SELECT status FROM migration_items WHERE job_id = 'crash-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(item.0, "preexisting");
}

#[tokio::test]
async fn retry_relaunches_a_matched_item_after_destination_snapshot_failure() {
    let pool = setup_test_db().await;
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, options, status,
            total_items, collections)
           VALUES ('snapshot-job', 'spotify', 'qobuz', ?, 'running', 1, '["tracks"]')"#,
    )
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items
           (job_id, kind, source_track_id, source_track_title, source_track_artist, status)
           VALUES ('snapshot-job', 'track', 'sp-snapshot', 'sp-snapshot', 'Artist', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());
    let mut gateway = FakeMigrationGateway::new();
    gateway
        .track_matches
        .insert("sp-snapshot".to_string(), "qb-snapshot".to_string());
    gateway.snapshot_error = Some("destination snapshot unavailable".to_string());

    let error = resume_migration_job_with_gateway(&pool, app.handle(), "snapshot-job", &gateway)
        .await
        .expect_err("snapshot failure stops the runner before writing");
    assert!(error.contains("destination snapshot unavailable"));
    let matched: (String, Option<String>) = sqlx::query_as(
        "SELECT status, destination_track_id FROM migration_items WHERE job_id = 'snapshot-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        matched,
        ("matched".to_string(), Some("qb-snapshot".to_string()))
    );
    assert!(gateway.written().is_empty());
    let job: (String,) =
        sqlx::query_as("SELECT status FROM migration_jobs WHERE id = 'snapshot-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job.0, "failed");

    gateway.snapshot_error = None;
    let retried = retry_failed_items_with_gateway(app.handle(), &pool, "snapshot-job", &gateway)
        .await
        .expect("retry relaunches the matched row");
    assert_eq!(retried, 1);
    assert_eq!(
        gateway.written(),
        vec![("track".to_string(), "qb-snapshot".to_string())]
    );
    let item: (String,) =
        sqlx::query_as("SELECT status FROM migration_items WHERE job_id = 'snapshot-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(item.0, "transferred");
}

#[tokio::test]
async fn startup_recovery_refuses_an_incomplete_legacy_snapshot() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    let account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'resume@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for (kind, source_id, title) in [
        ("album", "sp-album-later", "Later Album"),
        ("artist", "sp-artist-later", "Later Artist"),
    ] {
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name) VALUES (?, 1, ?, ?, ?, 'Fixture Artist')",
        )
        .bind(account)
        .bind(kind)
        .bind(source_id)
        .bind(title)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        r#"INSERT INTO migration_jobs
           (id, source_service, destination_service, source_account_id, options,
            status, total_items, collections)
           VALUES ('later-job', 'spotify', 'qobuz', ?, ?, 'running', 2,
                   '["albums","artists"]')"#,
    )
    .bind(account)
    .bind(options_json())
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());
    let gateway = FakeMigrationGateway::new();

    let interrupted = recover_interrupted_migrations(&pool).await.unwrap();
    assert_eq!(interrupted, vec!["later-job".to_string()]);
    let error = resume_migration_job_with_gateway(&pool, app.handle(), "later-job", &gateway)
        .await
        .expect_err("a partial historical snapshot must not be expanded");
    assert!(error.contains("incomplete durable snapshot"));
    assert!(gateway.written().is_empty());
    let item_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM migration_items WHERE job_id = 'later-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        item_count, 0,
        "the mutable source mirror is not materialized"
    );
    let job: (String, Option<String>) =
        sqlx::query_as("SELECT status, error_message FROM migration_jobs WHERE id = 'later-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job.0, "failed");
    assert!(job.1.unwrap().contains("incomplete durable snapshot"));
}

/// The snapshot and the `running` transition are one transaction. If inserting
/// the snapshot fails, neither the rows nor the running metadata may survive.
#[tokio::test]
async fn failed_initial_snapshot_never_publishes_a_running_job() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Snapshot Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Snapshot Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Snapshot Track', ?, 200000) RETURNING id",
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-snapshot')",
    )
    .bind(track_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"CREATE TRIGGER reject_initial_snapshot
           BEFORE INSERT ON migration_items
           BEGIN
               SELECT RAISE(ABORT, 'snapshot insert interrupted');
           END"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = test_app(pool.clone());

    let error = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        MigrationOptions {
            match_threshold: 0.8,
            skip_unmatched: false,
            create_playlists: false,
            merge_existing: false,
            download_matched: false,
        },
        None,
        None,
        None,
    )
    .await
    .expect_err("the injected snapshot failure reaches the caller");
    assert!(error.contains("snapshot insert interrupted"));
    let job: (String, i64, Option<String>) = sqlx::query_as(
        "SELECT status, total_items, started_at FROM migration_jobs ORDER BY rowid DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job, ("failed".to_string(), 0, None));
    let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(item_count, 0, "the partial snapshot rolled back");
}
