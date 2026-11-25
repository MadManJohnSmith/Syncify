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
    resume_migration_job_with_gateway, retry_failed_items, AlbumTransferGateway,
    ArtistTransferGateway, MigrationSourceAlbum, MigrationSourceArtist, TrackTransferGateway,
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

struct FakeMigrationGateway {
    track_matches: HashMap<String, String>,
    album_matches: HashMap<String, String>,
    artist_matches: HashMap<String, String>,
    existing_tracks: HashSet<String>,
    existing_albums: HashSet<String>,
    existing_artists: HashSet<String>,
    fail_after_write: HashSet<String>,
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
        Ok(self.existing_tracks.clone())
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
        Ok(self.existing_albums.clone())
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
        Ok(self.existing_artists.clone())
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

    resume_migration_job_with_gateway(&pool, app.handle(), "resume-job", &gateway)
        .await
        .expect("first resume");

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
    // failure. A retry must observe destination membership and skip the writes.
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
