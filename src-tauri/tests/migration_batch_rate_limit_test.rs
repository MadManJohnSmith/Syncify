//! M06 (#171) — lotes, rate limits y cancelación en la migración.
//!
//! El motor real de las tres fases ([`run_track_collection_with_policy`] y sus
//! hermanas) contra un gateway FALSO y BD `:memory:` totalmente migrada, sin
//! red: concurrencia acotada por proveedor, backoff ante 429/5xx, cancelación
//! en el límite de lote y reanudación desde las filas `pending` (M05).
//!
//! Los tests de la política pura (clasificación del error, backoff, lotes de
//! 30k, cerrojos por id) viven en `commands::migration_batch` y se ejecutan en
//! la suite `--lib`; este fichero comprueba el comportamiento observable del
//! job con un gateway falso.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_tauri_lib::commands::migration::{
    retry_failed_items_with_gateway, run_album_collection_with_policy, run_track_collection,
    run_track_collection_with_policy, AlbumTransferGateway, ArtistTransferGateway,
    MigrationCounters, MigrationSourceAlbum, MigrationSourceArtist, MigrationSourceTrack,
    TrackTransferGateway,
};
use syncify_tauri_lib::commands::migration_batch::BatchPolicy;
use syncify_tauri_lib::models::MigrationOptions;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::{AppState, EnrichmentWorkerState};
use tauri::Manager;
use tokio::sync::Notify;

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
        match_threshold: 0.8,
        skip_unmatched: false,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    }
}

async fn seed_running_job(pool: &SqlitePool, job_id: &str, total_items: i64) {
    let options = serde_json::to_string(&options()).expect("serialize options");
    sqlx::query(
        r#"INSERT INTO migration_jobs
               (id, source_service, destination_service, options, status,
                total_items, completed_items, failed_items, skipped_items, collections)
           VALUES (?, 'spotify', 'qobuz', ?, 'running', ?, 0, 0, 0, '["tracks"]')"#,
    )
    .bind(job_id)
    .bind(options)
    .bind(total_items)
    .execute(pool)
    .await
    .expect("seed migration job");
}

fn track(external_id: &str) -> MigrationSourceTrack {
    MigrationSourceTrack {
        external_id: external_id.to_string(),
        title: format!("title-{external_id}"),
        artist: "Fixture Artist".to_string(),
        album: None,
        playlist_id: None,
        playlist_name: None,
    }
}

/// Gateway falso de pistas: match programado por título, escrituras lentas y
/// fallos programados por id de destino, con el pico de concurrencia medido.
#[derive(Default)]
struct FakeTrackGateway {
    matches: Mutex<HashMap<String, String>>,
    existing: Mutex<HashSet<String>>,
    failures: Mutex<HashMap<String, VecDeque<String>>>,
    writes: Mutex<Vec<String>>,
    attempts: Mutex<HashMap<String, usize>>,
    in_flight: AtomicUsize,
    peak_in_flight: AtomicUsize,
    write_delay: Duration,
    /// Gancho síncrono tras una escritura (p. ej. disparar la cancelación del
    /// job desde fuera mientras el lote sigue en vuelo).
    on_write: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

impl FakeTrackGateway {
    fn with_matches(pairs: &[(&str, &str)]) -> Self {
        let gateway = Self::default();
        {
            let mut matches = gateway.matches.lock().unwrap();
            for (source, destination) in pairs {
                matches.insert(format!("title-{source}"), destination.to_string());
            }
        }
        gateway
    }

    fn failing(self, destination_id: &str, errors: &[&str]) -> Self {
        self.failures.lock().unwrap().insert(
            destination_id.to_string(),
            errors.iter().map(|error| error.to_string()).collect(),
        );
        self
    }

    fn slow(mut self, delay: Duration) -> Self {
        self.write_delay = delay;
        self
    }

    fn with_write_hook(mut self, hook: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        self.on_write = Some(hook);
        self
    }

    fn writes(&self) -> Vec<String> {
        self.writes.lock().unwrap().clone()
    }

    fn attempts_for(&self, destination_id: &str) -> usize {
        *self
            .attempts
            .lock()
            .unwrap()
            .get(destination_id)
            .unwrap_or(&0)
    }

    fn peak(&self) -> usize {
        self.peak_in_flight.load(Ordering::SeqCst)
    }
}

impl TrackTransferGateway for FakeTrackGateway {
    async fn existing_tracks(&self) -> Result<HashSet<String>, String> {
        Ok(self.existing.lock().unwrap().clone())
    }

    async fn match_track(
        &self,
        _isrc: Option<&str>,
        title: &str,
        _artist: &str,
    ) -> (f64, &'static str, Option<String>, Option<String>) {
        match self.matches.lock().unwrap().get(title) {
            Some(destination_id) => (1.0, "metadata", Some(destination_id.clone()), None),
            None => (0.0, "none", None, None),
        }
    }

    async fn write_track(&self, dest_track_id: &str) -> Result<(), String> {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_in_flight.fetch_max(now, Ordering::SeqCst);
        {
            let mut attempts = self.attempts.lock().unwrap();
            *attempts.entry(dest_track_id.to_string()).or_insert(0) += 1;
        }
        if let Some(hook) = &self.on_write {
            hook(dest_track_id);
        }
        tokio::time::sleep(self.write_delay).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);

        let programmed = {
            let mut failures = self.failures.lock().unwrap();
            failures
                .get_mut(dest_track_id)
                .and_then(|queue| queue.pop_front())
        };
        if let Some(error) = programmed {
            return Err(error);
        }
        self.existing
            .lock()
            .unwrap()
            .insert(dest_track_id.to_string());
        self.writes.lock().unwrap().push(dest_track_id.to_string());
        Ok(())
    }
}

impl AlbumTransferGateway for FakeTrackGateway {
    async fn existing_albums(&self) -> Result<HashSet<String>, String> {
        Ok(HashSet::new())
    }

    async fn match_album(
        &self,
        _album: &MigrationSourceAlbum,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        Ok(None)
    }

    async fn write_album(&self, _dest_album_id: &str) -> Result<(), String> {
        Ok(())
    }
}

impl ArtistTransferGateway for FakeTrackGateway {
    async fn existing_artists(&self) -> Result<HashSet<String>, String> {
        Ok(HashSet::new())
    }

    async fn match_artist(
        &self,
        _artist: &MigrationSourceArtist,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        Ok(None)
    }

    async fn write_artist(&self, _dest_artist_id: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Gateway falso de álbumes para el reintento del match.
#[derive(Default)]
struct FakeAlbumGateway {
    attempts: AtomicUsize,
    failures: Mutex<VecDeque<String>>,
}

impl AlbumTransferGateway for FakeAlbumGateway {
    async fn existing_albums(&self) -> Result<HashSet<String>, String> {
        Ok(HashSet::new())
    }

    async fn match_album(
        &self,
        album: &MigrationSourceAlbum,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.failures.lock().unwrap().pop_front() {
            return Err(error);
        }
        Ok(Some((format!("qb-{}", album.external_id), 1.0, "metadata")))
    }

    async fn write_album(&self, _dest_album_id: &str) -> Result<(), String> {
        Ok(())
    }
}

async fn item_statuses(pool: &SqlitePool, job_id: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT source_track_id, status FROM migration_items WHERE job_id = ? ORDER BY id",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .expect("read item rows")
}

/// Concurrencia: nunca más escrituras en vuelo que el techo del proveedor, y
/// el techo se aprovecha (no es un bucle secuencial disfrazado).
#[tokio::test]
async fn track_batch_never_exceeds_the_provider_concurrency_ceiling() {
    let pool = setup_test_db().await;
    let job_id = "concurrency-job";
    seed_running_job(&pool, job_id, 12).await;
    let app = test_app(pool.clone());

    let tracks: Vec<_> = (0..12).map(|i| track(&format!("sp-{i}"))).collect();
    let pairs: Vec<(String, String)> = (0..12)
        .map(|i| (format!("sp-{i}"), format!("qb-{i}")))
        .collect();
    let pairs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(source, destination)| (source.as_str(), destination.as_str()))
        .collect();
    let gateway = FakeTrackGateway::with_matches(&pairs).slow(Duration::from_millis(20));
    let policy = BatchPolicy::for_tests(12, 3);
    let mut counters = MigrationCounters::default();

    let cancelled = run_track_collection_with_policy(
        &pool,
        app.handle(),
        job_id,
        "qobuz",
        None,
        &tracks,
        &gateway,
        &options(),
        &mut counters,
        0,
        12,
        std::time::Instant::now(),
        &policy,
    )
    .await
    .expect("track batch");

    assert!(!cancelled);
    assert_eq!(
        gateway.peak(),
        3,
        "the provider ceiling bounds the writes in flight"
    );
    assert_eq!(counters.completed, 12);
    assert_eq!(counters.failed, 0);
    assert_eq!(gateway.writes().len(), 12);
    assert_eq!(
        item_statuses(&pool, job_id)
            .await
            .iter()
            .filter(|(_, status)| status == "transferred")
            .count(),
        12
    );
}

/// 429 y 5xx se reintentan con backoff; un 404 no se reintenta. El segundo
/// intento del 503 agota los tres intentos y el ítem queda fallido con motivo.
#[tokio::test]
async fn track_batch_backs_off_on_rate_limits_and_server_errors() {
    let pool = setup_test_db().await;
    let job_id = "retry-job";
    seed_running_job(&pool, job_id, 3).await;
    let app = test_app(pool.clone());

    let tracks = vec![track("sp-1"), track("sp-2"), track("sp-3")];
    let gateway =
        FakeTrackGateway::with_matches(&[("sp-1", "qb-1"), ("sp-2", "qb-2"), ("sp-3", "qb-3")])
            .failing("qb-1", &["Qobuz API error (429): slow down"])
            .failing(
                "qb-2",
                &[
                    "Tidal API error 503: unavailable",
                    "Tidal API error 503: unavailable",
                ],
            )
            .failing("qb-3", &["Qobuz API error (404): not found"]);
    let policy = BatchPolicy::for_tests(3, 2);
    let mut counters = MigrationCounters::default();

    let cancelled = run_track_collection_with_policy(
        &pool,
        app.handle(),
        job_id,
        "qobuz",
        None,
        &tracks,
        &gateway,
        &options(),
        &mut counters,
        0,
        3,
        std::time::Instant::now(),
        &policy,
    )
    .await
    .expect("track batch");

    assert!(!cancelled);
    assert_eq!(gateway.attempts_for("qb-1"), 2, "429 retried once");
    assert_eq!(gateway.attempts_for("qb-2"), 3, "503 retried twice");
    assert_eq!(gateway.attempts_for("qb-3"), 1, "404 is permanent");
    assert_eq!(counters.completed, 2);
    assert_eq!(counters.failed, 1);

    let statuses: HashMap<_, _> = item_statuses(&pool, job_id).await.into_iter().collect();
    assert_eq!(
        statuses.get("sp-1").map(String::as_str),
        Some("transferred")
    );
    assert_eq!(
        statuses.get("sp-2").map(String::as_str),
        Some("transferred")
    );
    assert_eq!(statuses.get("sp-3").map(String::as_str), Some("failed"));

    let (failed,): (i64,) = sqlx::query_as("SELECT failed_items FROM migration_jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(failed, 1);
    // The batch persisted its counters; the item row keeps the reason.
    let (reason,): (Option<String>,) = sqlx::query_as(
        "SELECT error_message FROM migration_items WHERE job_id = ? AND source_track_id = 'sp-3'",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(reason.unwrap().contains("404"));
}

/// Cancelar a mitad de un lote: el lote en curso termina entero, los ítems
/// posteriores quedan `pending` (nunca a medias) y Re-run los retoma sin
/// duplicar las escrituras ya hechas.
#[tokio::test]
async fn cancellation_stops_at_the_batch_boundary_and_resumes_without_duplicates() {
    let pool = setup_test_db().await;
    let job_id = "cancel-job";
    seed_running_job(&pool, job_id, 10).await;
    let app = test_app(pool.clone());

    let tracks: Vec<_> = (0..10).map(|i| track(&format!("sp-{i}"))).collect();
    let pairs: Vec<(String, String)> = (0..10)
        .map(|i| (format!("sp-{i}"), format!("qb-{i}")))
        .collect();
    let pairs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(source, destination)| (source.as_str(), destination.as_str()))
        .collect();

    // La cancelación se dispara desde dentro del lote 2 (destino qb-4) con el
    // mismo UPDATE que ejecuta el comando; el lote sigue hasta su frontera.
    let notify = Arc::new(Notify::new());
    let armed = Arc::new(AtomicBool::new(true));
    let hook_notify = notify.clone();
    let hook_armed = armed.clone();
    let gateway = FakeTrackGateway::with_matches(&pairs)
        .slow(Duration::from_millis(5))
        .with_write_hook(Arc::new(move |destination_id: &str| {
            if destination_id == "qb-4" && hook_armed.swap(false, Ordering::SeqCst) {
                hook_notify.notify_one();
            }
        }));

    let canceller = {
        let pool = pool.clone();
        let job_id = job_id.to_string();
        let notify = notify.clone();
        tokio::spawn(async move {
            notify.notified().await;
            sqlx::query(
                "UPDATE migration_jobs SET status = 'cancelled', completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'running'",
            )
            .bind(job_id)
            .execute(&pool)
            .await
            .expect("cancel job");
        })
    };

    let policy = BatchPolicy::for_tests(3, 2);
    let mut counters = MigrationCounters::default();
    let cancelled = run_track_collection_with_policy(
        &pool,
        app.handle(),
        job_id,
        "qobuz",
        None,
        &tracks,
        &gateway,
        &options(),
        &mut counters,
        0,
        10,
        std::time::Instant::now(),
        &policy,
    )
    .await
    .expect("track batch");
    canceller.await.expect("canceller task");

    assert!(cancelled, "the runner reports the cancellation");
    let statuses = item_statuses(&pool, job_id).await;
    assert_eq!(statuses.len(), 10);
    let terminal = statuses
        .iter()
        .filter(|(_, status)| status == "transferred")
        .count();
    let pending = statuses
        .iter()
        .filter(|(_, status)| status == "pending")
        .count();
    assert_eq!(
        (terminal, pending),
        (6, 4),
        "two full batches ran; nothing is left half-done: {statuses:?}"
    );
    assert_eq!(gateway.writes().len(), 6);

    // Re-run: only the pending rows are executed (M05 idempotency applies the
    // same rule to every runner), and each destination is written once.
    let retried = retry_failed_items_with_gateway(&app.handle().clone(), &pool, job_id, &gateway)
        .await
        .expect("resume cancelled job");
    assert_eq!(retried, 4);

    let (status, completed, failed, skipped): (String, i64, i64, i64) = sqlx::query_as(
        "SELECT status, completed_items, failed_items, skipped_items FROM migration_jobs WHERE id = ?",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed");
    assert_eq!((completed, failed, skipped), (10, 0, 0));

    let mut writes = gateway.writes();
    writes.sort();
    writes.dedup();
    assert_eq!(writes.len(), 10, "resume must not rewrite a destination");
    assert!(item_statuses(&pool, job_id)
        .await
        .iter()
        .all(|(_, status)| status == "transferred"));
}

/// El match de álbumes también reintenta 429/5xx antes de darse por vencido.
#[tokio::test]
async fn album_match_retries_transient_errors_before_migrating() {
    let pool = setup_test_db().await;
    let job_id = "album-retry-job";
    seed_running_job(&pool, job_id, 1).await;
    let app = test_app(pool.clone());

    let albums = vec![MigrationSourceAlbum {
        external_id: "sp-alb-1".to_string(),
        title: "Favorite Album".to_string(),
        artist: "Fixture Artist".to_string(),
    }];
    let gateway = FakeAlbumGateway {
        attempts: AtomicUsize::new(0),
        failures: Mutex::new(VecDeque::from([
            "Tidal API error 503: unavailable".to_string()
        ])),
    };
    let policy = BatchPolicy::for_tests(10, 2);
    let mut counters = MigrationCounters::default();

    let cancelled = run_album_collection_with_policy(
        &pool,
        app.handle(),
        job_id,
        "spotify",
        "qobuz",
        None,
        &albums,
        &gateway,
        &options(),
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
        &policy,
    )
    .await
    .expect("album batch");

    assert!(!cancelled);
    assert_eq!(gateway.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(counters.completed, 1);
    let statuses: HashMap<_, _> = item_statuses(&pool, job_id).await.into_iter().collect();
    assert_eq!(
        statuses.get("sp-alb-1").map(String::as_str),
        Some("transferred")
    );
}

/// La fase con la política de producción sigue siendo reanudable desde el
/// snapshot durable: correr dos veces no duplica nada.
#[tokio::test]
async fn running_the_track_phase_twice_does_not_duplicate_writes() {
    let pool = setup_test_db().await;
    let job_id = "idempotent-job";
    seed_running_job(&pool, job_id, 4).await;
    let app = test_app(pool.clone());

    let tracks: Vec<_> = (0..4).map(|i| track(&format!("sp-{i}"))).collect();
    let gateway = FakeTrackGateway::with_matches(&[
        ("sp-0", "qb-0"),
        ("sp-1", "qb-1"),
        ("sp-2", "qb-2"),
        ("sp-3", "qb-3"),
    ]);

    let mut counters = MigrationCounters::default();
    run_track_collection(
        &pool,
        app.handle(),
        job_id,
        "qobuz",
        None,
        &tracks,
        &gateway,
        &options(),
        &mut counters,
        0,
        4,
        std::time::Instant::now(),
    )
    .await
    .expect("first pass");
    assert_eq!(gateway.writes().len(), 4);

    let mut counters = MigrationCounters::default();
    run_track_collection(
        &pool,
        app.handle(),
        job_id,
        "qobuz",
        None,
        &tracks,
        &gateway,
        &options(),
        &mut counters,
        0,
        4,
        std::time::Instant::now(),
    )
    .await
    .expect("second pass");
    assert_eq!(
        gateway.writes().len(),
        4,
        "terminal rows are not re-executed"
    );
}
