//! M02 (#167) — la colección de artistas seguidos, end-to-end.
//!
//! Dos niveles, ambos sobre BD `:memory:` totalmente migrada y sin red (el
//! molde de `migration_albums_fake_gateway_test`):
//!  1. El motor de la fase de artistas (`run_artist_collection`) con un gateway
//!     FALSO (`ArtistTransferGateway`): match programado, escrituras observadas
//!     y fallos programados. Aquí se demuestra el criterio de aceptación "los
//!     artistas seguidos se migran end-to-end donde hay capacidad": ítems con
//!     kind 'artist', match (espejo primero, luego el gateway), follow en el
//!     destino, estados y contadores.
//!  2. El cableado del job (`start_migration` con `collections`): lectura de la
//!     caché de favoritos por cuenta, ítems de los tres tipos en un mismo job,
//!     colecciones persistidas y el gating por matriz de la colección nueva —
//!     un destino sin follow no se ofrece ni arranca.
//!
//! No hay migración nueva: 0113 deja `kind` y `collections` abiertos y el
//! espejo de artistas (0002/0038/0045/0065) ya trae ids por servicio y
//! `is_favorite` (verificado en `migration_artists_characterization_test`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_tauri_lib::commands::migration::{
    fetch_migration_source_artists, run_artist_collection, start_migration, ArtistTransferGateway,
    MigrationCounters, MigrationSourceArtist,
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

/// Gateway falso: matches programados por id externo del artista fuente,
/// escrituras observadas y fallos de escritura programados.
struct FakeArtistGateway {
    matches: HashMap<String, String>,
    existing: HashSet<String>,
    write_failures: HashSet<String>,
    written: Mutex<Vec<String>>,
}

impl FakeArtistGateway {
    fn with_match(source_id: &str, dest_id: &str) -> Self {
        Self {
            matches: HashMap::from([(source_id.to_string(), dest_id.to_string())]),
            existing: HashSet::new(),
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

impl ArtistTransferGateway for FakeArtistGateway {
    async fn existing_artists(&self) -> Result<HashSet<String>, String> {
        Ok(self.existing.clone())
    }

    async fn match_artist(
        &self,
        artist: &MigrationSourceArtist,
    ) -> Result<Option<(String, f64, &'static str)>, String> {
        Ok(self
            .matches
            .get(&artist.external_id)
            .map(|id| (id.clone(), 0.9, "metadata")))
    }

    async fn write_artist(&self, dest_artist_id: &str) -> Result<(), String> {
        if self.write_failures.contains(dest_artist_id) {
            return Err(format!("write refused for {dest_artist_id}"));
        }
        self.written
            .lock()
            .unwrap()
            .push(dest_artist_id.to_string());
        Ok(())
    }
}

fn artist(external_id: &str, name: &str) -> MigrationSourceArtist {
    MigrationSourceArtist {
        external_id: external_id.to_string(),
        name: name.to_string(),
    }
}

fn artist_options(threshold: f64) -> MigrationOptions {
    MigrationOptions {
        match_threshold: threshold,
        skip_unmatched: true,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    }
}

async fn seed_artist_job(pool: &SqlitePool) -> String {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status, total_items)
           VALUES ('artist-job', 'spotify', 'qobuz', '{}', 'running', 0)"#,
    )
    .execute(pool)
    .await
    .unwrap();
    "artist-job".to_string()
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
// El motor de la fase de artistas con gateway falso
// ============================================================================

/// Un artista con match en el gateway se sigue en el destino y queda como
/// `transferred` con su kind; un artista sin match cae en el camino honesto de
/// no-match (skipped con skip_unmatched), sin destino inventado.
#[tokio::test]
async fn test_artist_phase_matches_writes_and_records_kinds() {
    let pool = setup_test_db().await;
    let job_id = seed_artist_job(&pool).await;
    let app = test_app(pool.clone());

    let artists = vec![
        artist("sp-art-1", "Followed Artist"),
        artist("sp-art-2", "Unknown Artist"),
    ];
    let gateway = FakeArtistGateway::with_match("sp-art-1", "qb-art-1");
    let mut counters = MigrationCounters::default();

    let cancelled = run_artist_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &artists,
        &gateway,
        &artist_options(0.8),
        &mut counters,
        0,
        artists.len() as i64,
        std::time::Instant::now(),
    )
    .await
    .unwrap();
    assert!(!cancelled);

    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 2);
    let matched = &items[0];
    assert_eq!(matched.0, "artist", "the item declares its kind");
    assert_eq!(matched.1, "sp-art-1");
    assert_eq!(matched.2, "transferred");
    assert_eq!(matched.3, Some(0.9));
    assert_eq!(matched.4.as_deref(), Some("metadata"));
    assert_eq!(
        matched.5.as_deref(),
        Some("qb-art-1"),
        "the destination id is recorded"
    );
    assert_eq!(matched.6, None);
    assert_eq!(
        gateway.written(),
        vec!["qb-art-1".to_string()],
        "the matched artist was followed"
    );

    let unmatched = &items[1];
    assert_eq!(unmatched.0, "artist");
    assert_eq!(
        unmatched.2, "skipped",
        "no match under the threshold with skip_unmatched"
    );
    assert_eq!(unmatched.5, None, "no destination id is fabricated");
    assert_eq!(unmatched.6, None, "skipped items carry no error message");

    let job: (i64, i64, i64) = sqlx::query_as(
        "SELECT completed_items, failed_items, skipped_items FROM migration_jobs WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (job.0, job.1, job.2),
        (1, 0, 1),
        "the counters land on the job row"
    );
}

/// El espejo local tiene prioridad sobre el gateway: un artista cuya fila
/// canónica ya conoce el id del servicio destino se escribe por ese id, sin
/// consultar el gateway.
#[tokio::test]
async fn test_artist_phase_prefers_the_local_mirror_match() {
    let pool = setup_test_db().await;
    let job_id = seed_artist_job(&pool).await;
    let app = test_app(pool.clone());

    sqlx::query(
        "INSERT INTO artists (name, is_favorite, favorite_at, spotify_id, qobuz_id) VALUES ('Mirror Artist', 1, datetime('now'), 'sp-art-m', 'qb-mirror-m')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let artists = vec![artist("sp-art-m", "Mirror Artist")];
    let gateway = FakeArtistGateway::with_match("sp-art-m", "qb-gateway-m");
    let mut counters = MigrationCounters::default();

    run_artist_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &artists,
        &gateway,
        &artist_options(0.8),
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await
    .unwrap();

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
        "the follow went to the mirror's id, not the gateway's"
    );
}

/// Un match cuyo follow falla deja el ítem `failed` con el error real, el id de
/// destino vacío y la confianza del match — la misma honestidad que pistas y
/// álbumes.
#[tokio::test]
async fn test_artist_phase_records_failed_writes() {
    let pool = setup_test_db().await;
    let job_id = seed_artist_job(&pool).await;
    let app = test_app(pool.clone());

    let artists = vec![artist("sp-art-1", "Followed Artist")];
    let gateway = FakeArtistGateway::with_match("sp-art-1", "qb-art-1").failing_write("qb-art-1");
    let mut counters = MigrationCounters::default();

    run_artist_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &artists,
        &gateway,
        &artist_options(0.8),
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await
    .unwrap();

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
        error.contains("Failed to follow artist") && error.contains("write refused"),
        "the underlying failure reaches the item, got: {error}"
    );
    assert!(gateway.written().is_empty(), "nothing was written");
    assert_eq!(
        (counters.completed, counters.failed, counters.skipped),
        (0, 1, 0)
    );
}

/// Un match por debajo del umbral no se transfiere: skipped (o failed sin
/// skip_unmatched), conservando el id de destino que el match encontró —
/// mismo contrato que las demás colecciones.
#[tokio::test]
async fn test_artist_phase_applies_the_threshold_to_gateway_matches() {
    let pool = setup_test_db().await;
    let job_id = seed_artist_job(&pool).await;
    let app = test_app(pool.clone());

    let artists = vec![artist("sp-art-1", "Weak Match")];
    let gateway = FakeArtistGateway::with_match("sp-art-1", "qb-art-1");
    let mut counters = MigrationCounters::default();

    run_artist_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &artists,
        &gateway,
        &artist_options(0.95),
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await
    .unwrap();

    let items = job_items(&pool, &job_id).await;
    let item = &items[0];
    assert_eq!(item.2, "skipped", "0.9 < 0.95 with skip_unmatched");
    assert_eq!(
        item.5.as_deref(),
        Some("qb-art-1"),
        "the found match is still recorded"
    );
    assert_eq!(
        (counters.completed, counters.failed, counters.skipped),
        (0, 0, 1)
    );
}

/// Una cancelación aterrizada antes/fuera de la fase detiene el bucle en la
/// primera comprobación: la fase devuelve `cancelled` y no escribe resultados
/// inventados sobre los ítems pendientes.
#[tokio::test]
async fn test_artist_phase_stops_when_the_job_is_cancelled() {
    let pool = setup_test_db().await;
    let job_id = seed_artist_job(&pool).await;
    let app = test_app(pool.clone());
    sqlx::query("UPDATE migration_jobs SET status = 'cancelled' WHERE id = ?")
        .bind(&job_id)
        .execute(&pool)
        .await
        .unwrap();

    let artists = vec![artist("sp-art-1", "Followed Artist")];
    let gateway = FakeArtistGateway::with_match("sp-art-1", "qb-art-1");
    let mut counters = MigrationCounters::default();

    let cancelled = run_artist_collection(
        &pool,
        app.handle(),
        &job_id,
        "spotify",
        "qobuz",
        None,
        &artists,
        &gateway,
        &artist_options(0.8),
        &mut counters,
        0,
        1,
        std::time::Instant::now(),
    )
    .await
    .unwrap();
    assert!(cancelled, "the phase reports the cancellation");
    assert!(
        gateway.written().is_empty(),
        "no follow happens after the cancellation"
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = ? AND status = 'pending'",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, 1, "the pending row is left for the cancellation");
}

// ============================================================================
// La lectura de origen: qué artistas seguidos entran al job
// ============================================================================

/// Con cuenta de origen explícita, la lectura son los favoritos de artista de
/// ESA cuenta y ESE servicio (0045): ni las pistas de la misma cuenta ni los
/// artistas de otra cuenta entran, y un título vacío no produce un ítem sin
/// nombre.
#[tokio::test]
async fn test_source_artist_read_scopes_the_account_cache() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    let account_a: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'a@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let account_b: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'b@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for (account, item_type, service_id, external_id, title) in [
        (account_a, "artist", 1, "sp-a", "Artist A"),
        (account_a, "track", 1, "sp-track", "Track A"),
        (account_a, "artist", 1, "sp-empty", ""),
        (account_b, "artist", 1, "sp-b", "Artist B"),
    ] {
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(account)
        .bind(service_id)
        .bind(item_type)
        .bind(external_id)
        .bind(title)
        .execute(&pool)
        .await
        .unwrap();
    }

    let artists = fetch_migration_source_artists(&pool, "spotify", Some(account_a))
        .await
        .unwrap();
    let pairs: Vec<(String, String)> = artists
        .into_iter()
        .map(|a| (a.external_id, a.name))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("sp-a".to_string(), "Artist A".to_string()),
            ("sp-empty".to_string(), "Unknown Artist".to_string()),
        ],
        "only this account's artist favorites enter, with an honest name"
    );
}

/// Sin cuenta de origen la lectura es el espejo del servicio: filas de
/// `artists` con la columna del servicio y `is_favorite = 1`. Un servicio sin
/// columna de espejo no inventa artistas: lee cero.
#[tokio::test]
async fn test_source_artist_read_falls_back_to_the_service_mirror() {
    let pool = setup_test_db().await;
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    for (name, is_favorite, spotify_id, qobuz_id) in [
        ("Favorite Mirror", 1, Some("sp-fav"), Some("qb-fav")),
        ("Not A Favorite", 0, Some("sp-no"), Some("qb-no")),
        ("Qobuz Only", 1, None, Some("qb-only")),
    ] {
        sqlx::query(
            "INSERT INTO artists (name, is_favorite, spotify_id, qobuz_id) VALUES (?, ?, ?, ?)",
        )
        .bind(name)
        .bind(is_favorite)
        .bind(spotify_id)
        .bind(qobuz_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let artists = fetch_migration_source_artists(&pool, "spotify", None)
        .await
        .unwrap();
    let pairs: Vec<(String, String)> = artists
        .into_iter()
        .map(|a| (a.external_id, a.name))
        .collect();
    assert_eq!(
        pairs,
        vec![("sp-fav".to_string(), "Favorite Mirror".to_string())],
        "the service-wide read is the favorite mirror rows with the service id"
    );

    for service in ["deezer", "soundcloud", "apple_music"] {
        let artists = fetch_migration_source_artists(&pool, service, None)
            .await
            .unwrap();
        assert!(
            artists.is_empty(),
            "{service} has no artists mirror column, so the read is empty"
        );
    }
}

// ============================================================================
// El cableado del job: start_migration con collections
// ============================================================================

/// Seed del fixture job-level: servicios, una cuenta spotify de origen con dos
/// artistas seguidos en su caché de cuenta (0045) y uno de ellos también en el
/// espejo canónico con ambos ids. Devuelve la cuenta de origen.
async fn seed_artists_fixture(pool: &SqlitePool) -> i64 {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'artists@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO artists (name, is_favorite, favorite_at, spotify_id, qobuz_id) VALUES ('Mirror Artist', 1, datetime('now'), 'sp-art-1', 'qb-art-1')",
    )
    .execute(pool)
    .await
    .unwrap();
    for (id, name) in [("sp-art-1", "Mirror Artist"), ("sp-art-2", "Cache Artist")] {
        sqlx::query(
            "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name) VALUES (?, 1, 'artist', ?, ?, ?)",
        )
        .bind(account_id)
        .bind(id)
        .bind(name)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }
    account_id
}

async fn start_artists_job(
    app: &tauri::App<tauri::test::MockRuntime>,
    destination: &str,
    source_account_id: Option<i64>,
    collections: Vec<&str>,
) -> Result<String, String> {
    start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        destination.to_string(),
        None,
        artist_options(0.8),
        source_account_id,
        None,
        Some(collections.into_iter().map(String::from).collect()),
    )
    .await
}

/// A favorite album in the account cache (0045), which is what the job-level
/// album read walks when a source account is explicit.
async fn seed_account_album_favorite(pool: &SqlitePool, account_id: i64) {
    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, artist_name) VALUES (?, 1, 'album', 'sp-alb-1', 'Favorite Album', 'Fixture Artist')",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .unwrap();
}

/// Un job `collections: ["artists"]` lee los artistas seguidos de la cuenta de
/// origen, crea ítems con kind 'artist' y los procesa: el artista que el espejo
/// conoce en ambos servicios se empareja por espejo y su follow se intenta
/// (falla sin cliente, con el error real); el que solo vive en la caché de
/// cuenta no tiene match (el gateway real no busca artistas aún) y cae en
/// skipped. El job declara sus colecciones y cierra completed.
#[tokio::test]
async fn test_artists_job_walks_the_account_follows_and_records_kinds() {
    let pool = setup_test_db().await;
    let account = seed_artists_fixture(&pool).await;
    let app = test_app(pool.clone());

    let job_id = start_artists_job(&app, "qobuz", Some(account), vec!["artists"])
        .await
        .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    assert_eq!(items.len(), 2, "both account follows enter the job");
    assert!(
        items.iter().all(|i| i.0 == "artist"),
        "every item declares kind 'artist'"
    );

    let mirrored = items
        .iter()
        .find(|i| i.1 == "sp-art-1")
        .expect("mirror artist entered");
    assert_eq!(
        mirrored.2, "failed",
        "the mirror match's follow fails without a client"
    );
    assert_eq!(mirrored.3, Some(1.0));
    assert_eq!(mirrored.4.as_deref(), Some("mirror"));
    assert!(
        mirrored
            .6
            .as_deref()
            .unwrap_or("")
            .contains("client unavailable"),
        "the real gateway's follow failure is recorded, got: {:?}",
        mirrored.6
    );

    let cache_only = items
        .iter()
        .find(|i| i.1 == "sp-art-2")
        .expect("cache artist entered");
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
        Some(r#"["artists"]"#),
        "the job records its collections"
    );
}

/// Un mismo job puede llevar pistas, álbumes y artistas: los tres tipos
/// declaran su kind, el total es la suma y las colecciones quedan en el job.
#[tokio::test]
async fn test_one_job_carries_tracks_albums_and_artists() {
    let pool = setup_test_db().await;
    let account = seed_artists_fixture(&pool).await;

    // Un álbum favorito en la caché de la cuenta, para el ítem de tipo álbum.
    seed_account_album_favorite(&pool, account).await;

    // Una playlist spotify con una pista, como el fixture de caracterización
    // de M01.
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Track Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let track_artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Track Artist') RETURNING id")
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
        .bind(track_artist_id)
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
    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        Some(vec!["pl-mix".to_string()]),
        artist_options(0.8),
        Some(account),
        None,
        Some(vec![
            "tracks".to_string(),
            "albums".to_string(),
            "artists".to_string(),
        ]),
    )
    .await
    .expect("job must start");

    let items = job_items(&pool, &job_id).await;
    let count_kind = |kind: &str| items.iter().filter(|i| i.0 == kind).count();
    assert_eq!(count_kind("track"), 1, "the playlist track entered");
    assert_eq!(count_kind("album"), 1, "the favorite album entered");
    assert_eq!(count_kind("artist"), 2, "both account follows entered");

    let job: (i64, Option<String>) =
        sqlx::query_as("SELECT total_items, collections FROM migration_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        job.0, 4,
        "the job total is the sum of the three collections"
    );
    assert_eq!(job.1.as_deref(), Some(r#"["tracks","albums","artists"]"#));
}

/// El gating por matriz cubre la colección nueva: artistas hacia un destino sin
/// follow se rechaza antes de crear el job, igual que una colección
/// desconocida — así "un destino sin follow no se ofrece" tiene su invariante
/// en el servidor, no solo en la tarjeta del wizard.
#[tokio::test]
async fn test_artists_gating_refuses_destinations_without_follow() {
    let pool = setup_test_db().await;
    let account = seed_artists_fixture(&pool).await;
    let app = test_app(pool.clone());

    for destination in ["deezer", "soundcloud", "apple_music"] {
        let result = start_artists_job(&app, destination, Some(account), vec!["artists"]).await;
        let error = result.expect_err("a destination without follow must refuse artists");
        assert!(
            error.contains("does not support the 'artists' collection"),
            "{destination} must refuse artists, got: {error}"
        );
    }

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0, "no gating refusal leaves a job row behind");
}

/// Una cancelación que aterriza durante la fase de álbumes no abre la fase de
/// artistas: el job cancelado no gana filas de artista `pending` que sobrevivan
/// a la cancelación, y cierra como cancelled.
#[tokio::test]
async fn test_cancelled_album_phase_does_not_open_the_artist_phase() {
    let pool = setup_test_db().await;
    let account = seed_artists_fixture(&pool).await;
    seed_account_album_favorite(&pool, account).await;

    // La cancelación llega al insertarse el primer ítem de álbum, como si el
    // usuario hubiera pulsado Cancelar durante esa fase. Un trigger la hace
    // determinista: no compite con el bucle por la BD.
    sqlx::query(
        r#"CREATE TRIGGER cancel_job_on_first_album_item
           AFTER INSERT ON migration_items
           WHEN NEW.kind = 'album'
           BEGIN
               UPDATE migration_jobs SET status = 'cancelled' WHERE id = NEW.job_id;
           END"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = test_app(pool.clone());
    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        artist_options(0.8),
        Some(account),
        None,
        Some(vec!["albums".to_string(), "artists".to_string()]),
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
        "the cancelled album phase closes the job as cancelled"
    );

    let artist_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = ? AND kind = 'artist'",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        artist_items, 0,
        "no pending artist row outlives the cancellation"
    );
}
