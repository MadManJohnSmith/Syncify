//! M01 (#166) — la colección de álbumes favoritos, end-to-end.
//!
//! Dos niveles, ambos sobre BD `:memory:` totalmente migrada y sin red:
//!  1. El motor de la fase de álbumes (`run_album_collection`) con un gateway
//!     FALSO (`AlbumTransferGateway`), el molde de `FakeLibrarySource` en
//!     sync: match programado, escrituras observadas y fallos programados.
//!     Aquí se demuestra el criterio de aceptación "un job migra álbumes
//!     favoritos end-to-end": ítems con kind, match (espejo primero, luego el
//!     gateway), escritura en el destino, estados y contadores.
//!  2. El cableado del job (`start_migration` con `collections`): lectura del
//!     espejo de favoritos por cuenta, ítems de ambos tipos en un mismo job,
//!     colecciones persistidas en el job y el gating por matriz para la
//!     colección nueva.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_tauri_lib::commands::migration::{
    run_album_collection, start_migration, AlbumTransferGateway, MigrationCounters,
    MigrationSourceAlbum,
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

/// Gateway falso: matches programados por id externo del álbum fuente,
/// escrituras observadas y fallos de escritura programados.
struct FakeAlbumGateway {
    matches: HashMap<String, String>,
    write_failures: HashSet<String>,
    written: Mutex<Vec<String>>,
}

impl FakeAlbumGateway {
    fn with_match(source_id: &str, dest_id: &str) -> Self {
        Self {
            matches: HashMap::from([(source_id.to_string(), dest_id.to_string())]),
            write_failures: HashSet::new(),
            written: Mutex::new(Vec::new()),
        }
    }

    fn failing_write(mut self, dest_id: &str) -> Self {
        self.write_failures.insert(dest_id.to_string());
        self
    }

    fn written(&self) -> Vec<String> {
        self.written.lock().unwrap().clone()
    }
}

impl AlbumTransferGateway for FakeAlbumGateway {
    async fn match_album(
        &self,
        album: &MigrationSourceAlbum,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        Ok(self
            .matches
            .get(&album.external_id)
            .map(|id| (id.clone(), 0.9, "metadata")))
    }

    async fn write_album(&self, dest_album_id: &str) -> Result<(), String> {
        if self.write_failures.contains(dest_album_id) {
            return Err(format!("write refused for {dest_album_id}"));
        }
        self.written.lock().unwrap().push(dest_album_id.to_string());
        Ok(())
    }
}

fn album(external_id: &str, title: &str) -> MigrationSourceAlbum {
    MigrationSourceAlbum {
        external_id: external_id.to_string(),
        title: title.to_string(),
        artist: "Fixture Artist".to_string(),
    }
}

async fn seed_album_job(pool: &SqlitePool) -> String {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status, total_items)
           VALUES ('album-job', 'spotify', 'qobuz', '{}', 'running', 0)"#,
    )
    .execute(pool)
    .await
    .unwrap();
    "album-job".to_string()
}

type ItemRow = (
    String,         // kind
    String,         // source_track_id
    String,         // status
    Option<f64>,    // match_confidence
    Option<String>, // match_method
    Option<String>, // destination_track_id
    Option<String>, // error_message
);

async fn job_items(pool: &SqlitePool, job_id: &str) -> Vec<ItemRow> {
    sqlx::query_as(
        "SELECT kind, source_track_id, status, match_confidence, match_method,
                destination_track_id, error_message
         FROM migration_items WHERE job_id = ? ORDER BY id",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

// ============================================================================
// El motor de la fase de álbumes con gateway falso
// ============================================================================

/// Un álbum con match en el gateway se escribe en el destino y queda como
/// `transferred` con su kind; un álbum sin match cae en el camino honesto de
/// no-match (skipped con skip_unmatched), sin destino inventado.
#[tokio::test]
async fn test_album_phase_matches_writes_and_records_kinds() {
    let pool = setup_test_db().await;
    let job_id = seed_album_job(&pool).await;
    let app = test_app(pool.clone());

    let albums = vec![
        album("sp-alb-1", "Favorite Album"),
        album("sp-alb-2", "Unknown Album"),
    ];
    let gateway = FakeAlbumGateway::with_match("sp-alb-1", "qb-alb-1");
    let mut counters = MigrationCounters::default();

    let cancelled = run_album_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &albums,
        &gateway,
        &MigrationOptions {
            match_threshold: 0.8,
            skip_unmatched: true,
            create_playlists: false,
            merge_existing: false,
            download_matched: false,
        },
        &mut counters,
        0,
        albums.len() as i64,
        std::time::Instant::now(),
    )
    .await;
    assert!(!cancelled);

    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 2);
    let matched = &items[0];
    assert_eq!(matched.0, "album", "the item declares its kind");
    assert_eq!(matched.1, "sp-alb-1");
    assert_eq!(matched.2, "transferred");
    assert_eq!(matched.3, Some(0.9));
    assert_eq!(matched.4.as_deref(), Some("metadata"));
    assert_eq!(
        matched.5.as_deref(),
        Some("qb-alb-1"),
        "the destination id is recorded"
    );
    assert_eq!(matched.6, None);
    assert_eq!(
        gateway.written(),
        vec!["qb-alb-1".to_string()],
        "the matched album was written"
    );

    let unmatched = &items[1];
    assert_eq!(unmatched.0, "album");
    assert_eq!(
        unmatched.2, "skipped",
        "no match under the threshold with skip_unmatched"
    );
    assert_eq!(unmatched.5, None, "no destination id is fabricated");
    assert_eq!(unmatched.6, None, "skipped items carry no error message");

    let job: (i64, i64, i64, i64, String) = sqlx::query_as(
        "SELECT total_items, completed_items, failed_items, skipped_items, status FROM migration_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (job.1, job.2, job.3),
        (1, 0, 1),
        "the counters land on the job row"
    );
    assert_eq!(
        job.4, "running",
        "the phase does not close the job; the caller does"
    );
}

/// El espejo local tiene prioridad sobre el gateway: un álbum cuyo fila
/// canónica ya conoce el id del servicio destino se escribe por ese id, sin
/// consultar el gateway.
#[tokio::test]
async fn test_album_phase_prefers_the_local_mirror_match() {
    let pool = setup_test_db().await;
    let job_id = seed_album_job(&pool).await;
    let app = test_app(pool.clone());

    sqlx::query(
        "INSERT INTO albums (title, is_favorite, favorite_at, spotify_id, qobuz_id) VALUES ('Mirror Album', 1, datetime('now'), 'sp-alb-m', 'qb-mirror-m')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let albums = vec![album("sp-alb-m", "Mirror Album")];
    let gateway = FakeAlbumGateway::with_match("sp-alb-m", "qb-gateway-m");
    let mut counters = MigrationCounters::default();

    run_album_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &albums,
        &gateway,
        &MigrationOptions {
            match_threshold: 0.8,
            skip_unmatched: true,
            create_playlists: false,
            merge_existing: false,
            download_matched: false,
        },
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await;

    let items = job_items(&pool, &job_id).await;
    let item = &items[0];
    assert_eq!(item.2, "transferred");
    assert_eq!(item.3, Some(1.0));
    assert_eq!(
        item.4.as_deref(),
        Some("mirror"),
        "the mirror match takes its own method"
    );
    assert_eq!(
        item.5.as_deref(),
        Some("qb-mirror-m"),
        "the mirror's id wins"
    );
    assert_eq!(
        gateway.written(),
        vec!["qb-mirror-m".to_string()],
        "the write went to the mirror's id, not the gateway's"
    );
}

/// Un match cuyo intento de escritura falla deja el ítem `failed` con el
/// error real, el id de destino vacío y la confianza del match — la misma
/// honestidad que el flujo de pistas.
#[tokio::test]
async fn test_album_phase_records_failed_writes() {
    let pool = setup_test_db().await;
    let job_id = seed_album_job(&pool).await;
    let app = test_app(pool.clone());

    let albums = vec![album("sp-alb-1", "Favorite Album")];
    let gateway = FakeAlbumGateway::with_match("sp-alb-1", "qb-alb-1").failing_write("qb-alb-1");
    let mut counters = MigrationCounters::default();

    run_album_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &albums,
        &gateway,
        &MigrationOptions {
            match_threshold: 0.8,
            skip_unmatched: true,
            create_playlists: false,
            merge_existing: false,
            download_matched: false,
        },
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await;

    let items = job_items(&pool, &job_id).await;
    let item = &items[0];
    assert_eq!(item.2, "failed");
    assert_eq!(item.3, Some(0.9), "the match's confidence is kept");
    assert_eq!(item.4.as_deref(), Some("metadata"));
    assert_eq!(item.5, None, "the failed write records no destination id");
    let error = item
        .6
        .as_deref()
        .expect("failed items carry an error message");
    assert!(
        error.contains("Failed to add album to favorites") && error.contains("write refused"),
        "the underlying failure reaches the item, got: {error}"
    );
    assert!(gateway.written().is_empty(), "nothing was written");
    let job: (i64, i64, i64) = sqlx::query_as(
        "SELECT completed_items, failed_items, skipped_items FROM migration_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((job.0, job.1, job.2), (0, 1, 0));
}

/// Un match por debajo del umbral no se transfiere: skipped (o failed sin
/// skip_unmatched), conservando el id de destino que el match encontró —
/// mismo contrato que las pistas.
#[tokio::test]
async fn test_album_phase_applies_the_threshold_to_gateway_matches() {
    let pool = setup_test_db().await;
    let job_id = seed_album_job(&pool).await;
    let app = test_app(pool.clone());

    let albums = vec![album("sp-alb-1", "Weak Match")];
    let gateway = FakeAlbumGateway::with_match("sp-alb-1", "qb-alb-1");
    let mut counters = MigrationCounters::default();

    run_album_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &albums,
        &gateway,
        &MigrationOptions {
            match_threshold: 0.95,
            skip_unmatched: true,
            create_playlists: false,
            merge_existing: false,
            download_matched: false,
        },
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await;

    let items = job_items(&pool, &job_id).await;
    let item = &items[0];
    assert_eq!(item.2, "skipped", "0.9 < 0.95 with skip_unmatched");
    assert_eq!(
        item.5.as_deref(),
        Some("qb-alb-1"),
        "the found match is still recorded"
    );
    assert_eq!(
        (counters.completed, counters.failed, counters.skipped),
        (0, 0, 1)
    );
}

// ============================================================================
// El cableado del job: start_migration con collections
// ============================================================================

/// Seed del fixture job-level: servicios, cuenta spotify de origen con dos
/// álbumes favoritos en su caché de cuenta (0045) y uno de ellos también en
/// el espejo canónico con ambos ids.
async fn seed_albums_fixture(pool: &SqlitePool) -> i64 {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'albums@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO albums (title, is_favorite, favorite_at, spotify_id, qobuz_id) VALUES ('Mirror Album', 1, datetime('now'), 'sp-alb-1', 'qb-alb-1')",
    )
    .execute(pool)
    .await
    .unwrap();
    for (id, title) in [
        ("sp-alb-1", "Mirror Album"),
        ("sp-alb-2", "Cache-only Album"),
    ] {
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name) VALUES (?, 1, 'album', ?, ?, 'Fixture Artist')",
        )
        .bind(account_id)
        .bind(id)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    }
    account_id
}

async fn start_albums_job(
    app: &tauri::App<tauri::test::MockRuntime>,
    destination: &str,
    source_account_id: Option<i64>,
    playlist_ids: Option<Vec<String>>,
    collections: Vec<&str>,
    options: MigrationOptions,
) -> Result<String, String> {
    start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        destination.to_string(),
        playlist_ids,
        options,
        source_account_id,
        None,
        Some(collections.into_iter().map(String::from).collect()),
    )
    .await
}

fn albums_options() -> MigrationOptions {
    MigrationOptions {
        match_threshold: 0.8,
        skip_unmatched: true,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    }
}

/// Un job `collections: ["albums"]` lee los álbumes favoritos de la cuenta de
/// origen, crea ítems con kind 'album' y los procesa: el álbum que el espejo
/// conoce en ambos servicios se empareja por espejo y su escritura se intenta
/// (falla sin cliente, con el error real); el álbum que solo vive en la caché
/// de cuenta no tiene match (el gateway real no busca álbumes aún) y cae en
/// skipped. El job declara sus colecciones y cierra completed.
#[tokio::test]
async fn test_albums_job_walks_the_account_favorites_and_records_kinds() {
    let pool = setup_test_db().await;
    let account = seed_albums_fixture(&pool).await;
    let app = test_app(pool.clone());

    let job_id = start_albums_job(
        &app,
        "qobuz",
        Some(account),
        None,
        vec!["albums"],
        albums_options(),
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 2, "both account favorites enter the job");
    assert!(
        items.iter().all(|i| i.0 == "album"),
        "every item declares kind 'album'"
    );

    let mirrored = items
        .iter()
        .find(|i| i.1 == "sp-alb-1")
        .expect("mirror album entered");
    assert_eq!(
        mirrored.2, "failed",
        "the mirror match's write fails without a client"
    );
    assert_eq!(mirrored.3, Some(1.0));
    assert_eq!(mirrored.4.as_deref(), Some("mirror"));
    assert!(
        mirrored
            .6
            .as_deref()
            .unwrap_or("")
            .contains("client unavailable"),
        "the real gateway's write failure is recorded, got: {:?}",
        mirrored.6
    );

    let cache_only = items
        .iter()
        .find(|i| i.1 == "sp-alb-2")
        .expect("cache album entered");
    assert_eq!(
        cache_only.2, "skipped",
        "no mirror id and no remote search: honest no-match"
    );
    assert_eq!(cache_only.4.as_deref(), Some("none"));

    let job: (i64, i64, i64, i64, String, Option<String>) = sqlx::query_as(
        "SELECT total_items, completed_items, failed_items, skipped_items, status, collections FROM migration_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job.0, 2);
    assert_eq!((job.1, job.2, job.3), (0, 1, 1));
    assert_eq!(job.4, "completed");
    assert_eq!(
        job.5.as_deref(),
        Some(r#"["albums"]"#),
        "the job records its collections"
    );
}

/// Un mismo job puede llevar pistas Y álbumes: los ítems de pistas conservan
/// su atribución de playlist y kind 'track', los de álbumes kind 'album', y
/// el total del job es la suma de ambas colecciones.
#[tokio::test]
async fn test_one_job_carries_tracks_and_albums() {
    let pool = setup_test_db().await;
    let account = seed_albums_fixture(&pool).await;

    // Una playlist spotify con una pista, como el fixture de caracterización.
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Track Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Fixture Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Song One', ?, 200000) RETURNING id",
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
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-one')")
        .bind(track_id)
        .execute(&pool)
        .await
        .unwrap();
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name, source_service) VALUES (?, 'pl-mix', 'Mixed', 'spotify') RETURNING id",
    )
    .bind(account)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(playlist_id)
        .bind(track_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = test_app(pool.clone());
    let job_id = start_albums_job(
        &app,
        "qobuz",
        Some(account),
        Some(vec!["pl-mix".to_string()]),
        vec!["tracks", "albums"],
        albums_options(),
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 3, "one track item and the two album favorites");
    let track_item = items
        .iter()
        .find(|i| i.1 == "sp-one")
        .expect("track entered");
    assert_eq!(track_item.0, "track");
    for source_id in ["sp-alb-1", "sp-alb-2"] {
        let album_item = items
            .iter()
            .find(|i| i.1 == source_id)
            .expect("album entered");
        assert_eq!(album_item.0, "album");
    }

    let job: (i64, Option<String>) =
        sqlx::query_as("SELECT total_items, collections FROM migration_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job.0, 3, "the job total is the sum of both collections");
    assert_eq!(job.1.as_deref(), Some(r#"["tracks","albums"]"#));
}

/// El gating por matriz también cubre la colección nueva: álbumes hacia un
/// destino que no los declara, una colección desconocida y una lista vacía
/// se rechazan antes de crear el job.
#[tokio::test]
async fn test_collections_gating_refuses_undeclared_or_unknown_requests() {
    let pool = setup_test_db().await;
    let account = seed_albums_fixture(&pool).await;
    let app = test_app(pool.clone());

    // Deezer no declara Albums: la colección no arranca.
    let result = start_albums_job(
        &app,
        "deezer",
        Some(account),
        None,
        vec!["albums"],
        albums_options(),
    )
    .await;
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.contains("does not support the 'albums' collection"))
            .unwrap_or(false),
        "deezer must refuse albums, got: {:?}",
        result
    );

    // Artists no lo declara nadie todavía.
    let result = start_albums_job(
        &app,
        "qobuz",
        Some(account),
        None,
        vec!["artists"],
        albums_options(),
    )
    .await;
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.contains("does not support the 'artists' collection"))
            .unwrap_or(false),
        "qobuz must refuse artists, got: {:?}",
        result
    );

    // Nombre desconocido y lista vacía: rechazados con su propio mensaje.
    let result = start_albums_job(
        &app,
        "qobuz",
        Some(account),
        None,
        vec!["bogus"],
        albums_options(),
    )
    .await;
    assert!(result
        .as_ref()
        .err()
        .map(|e| e.contains("Unknown migration collection"))
        .unwrap_or(false));
    let result =
        start_albums_job(&app, "qobuz", Some(account), None, vec![], albums_options()).await;
    assert!(result
        .as_ref()
        .err()
        .map(|e| e.contains("at least one collection"))
        .unwrap_or(false));

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0, "no gating refusal leaves a job row behind");
}

/// Una petición `collections: ["playlists"]` a solas se rechaza antes de
/// existir el job: el espejo de listas se alimenta de los ítems de pista
/// transferidos por el propio job, así que sin la lectura de favoritos de
/// pistas no hay nada que leer ni que espejar y el job cerraría como
/// "completed" vacío (antes de M01 ese mismo clic migraba los favoritos, así
/// que la ruta visible no puede quedarse en un éxito hueco). El invariante
/// vive en el servidor para cubrir también una UI obsoleta que no lo calcule.
#[tokio::test]
async fn test_playlists_collection_alone_is_refused_before_the_job_exists() {
    let pool = setup_test_db().await;
    let account = seed_albums_fixture(&pool).await;
    let app = test_app(pool.clone());

    // Qobuz declara Playlists, así que la petición pasa el gating de matriz y
    // llega al invariante de combinación: el rechazo es por la combinación,
    // no por capacidad.
    let result = start_albums_job(
        &app,
        "qobuz",
        Some(account),
        None,
        vec!["playlists"],
        albums_options(),
    )
    .await;
    let error = result.expect_err("playlists alone must not start a job");
    assert!(
        error.contains("cannot run on its own") && error.contains("'tracks'"),
        "the refusal names the missing collection, got: {error}"
    );

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        jobs, 0,
        "the refusal happens before the job row, so nothing can close empty"
    );
}

/// Una cancelación que aterriza durante la fase de pistas no abre la fase de
/// álbumes: el job cancelado no gana filas de álbum `pending` que sobrevivan a
/// la cancelación (la fase solo arranca si `!cancelled`).
#[tokio::test]
async fn test_cancelled_track_phase_does_not_open_the_album_phase() {
    let pool = setup_test_db().await;
    seed_albums_fixture(&pool).await;

    // Una pista de la biblioteca del origen: sin ella la fase de pistas no
    // llega a comprobar el estado del job y la cancelación no tiene ventana.
    let track_album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Track Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Song One', ?, 200000) RETURNING id",
    )
    .bind(track_album_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Fixture Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-one')")
        .bind(track_id)
        .execute(&pool)
        .await
        .unwrap();

    // La cancelación llega con el primer ítem de pista, como si el usuario
    // hubiera pulsado Cancelar durante la fase de pistas. Un trigger la hace
    // determinista: no compite con el bucle por la BD.
    sqlx::query(
        r#"CREATE TRIGGER cancel_job_on_first_track_item
           AFTER INSERT ON migration_items
           WHEN NEW.kind = 'track'
           BEGIN
               UPDATE migration_jobs SET status = 'cancelled' WHERE id = NEW.job_id;
           END"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());
    // Sin cuenta de origen explícita, la lectura es la del espejo de servicio
    // entero: la pista sembrada entra sin necesidad de library_entries.
    let job_id = start_albums_job(
        &app,
        "qobuz",
        None,
        None,
        vec!["tracks", "albums"],
        albums_options(),
    )
    .await
    .expect("job must start");

    let (status,): (String,) = sqlx::query_as("SELECT status FROM migration_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "cancelled",
        "the cancelled track phase closes the job as cancelled"
    );

    let album_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = ? AND kind = 'album'",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        album_items, 0,
        "no pending album row outlives the cancellation"
    );

    let track_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = ? AND kind = 'track'",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        track_items, 1,
        "the track items were the ones already written"
    );
}
