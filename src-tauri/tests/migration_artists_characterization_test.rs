//! M02 (#167) — caracterización del flujo de migración antes de artistas
//! seguidos, y verificación del esquema que la colección va a usar.
//!
//! Tres pines sobre BD `:memory:` totalmente migrada y sin red, con el molde de
//! `migration_engine_characterization_test` (M01) y
//! `migration_albums_fake_gateway_test`:
//!  1. un job legacy (sin `collections`) no migra artistas seguidos — los
//!     favoritos de artista del espejo canónico y de la caché de cuenta quedan
//!     fuera, como los álbumes en M01;
//!  2. la matriz declara `artists` exactamente donde el cliente del destino
//!     puede seguir (M02: qobuz, tidal, spotify) y `start_migration` rechaza la
//!     colección en el resto antes de crear el job (este pin nació en el hito 1
//!     con «ningún destino declara artistas» y el hito 2 lo actualizó a
//!     propósito junto con la matriz);
//!  3. el esquema vigente (0113) ya acepta ítems `kind = 'artist'` y jobs con
//!     `collections = '["artists"]'`: M02 no necesita migración nueva, que es
//!     la verificación pedida antes de tocar el motor. La 0017, la 0045 y la
//!     0113 no se editan y el número siguiente sigue libre para #184.

use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_tauri_lib::commands::migration::{
    get_migration_capabilities, start_migration, MigrationCollection,
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

fn migration_options() -> MigrationOptions {
    MigrationOptions {
        match_threshold: 0.8,
        skip_unmatched: true,
        create_playlists: false,
        merge_existing: false,
        download_matched: false,
    }
}

/// Dos artistas seguidos de spotify: uno en el espejo canónico
/// (`artists.is_favorite` con `spotify_id`/`qobuz_id`) y otro solo en la caché
/// de la cuenta (`favorites`, item_type = 'artist'), que es como las dos vías
/// de lectura del flujo los encuentran. Devuelve la cuenta de origen.
async fn seed_followed_artists(pool: &SqlitePool) -> i64 {
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

/// El punto de partida de M02: un job sin `collections` sigue siendo el flujo
/// legacy de favoritos de pista y no toca los artistas seguidos, aunque el
/// espejo y la caché de cuenta los tengan.
#[tokio::test]
async fn test_legacy_job_does_not_migrate_followed_artists() {
    let pool = setup_test_db().await;
    let account = seed_followed_artists(&pool).await;
    let app = test_app(pool.clone());

    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        migration_options(),
        Some(account),
        None,
        None,
    )
    .await
    .expect("legacy job must start");

    let artist_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM migration_items WHERE job_id = ? AND (kind = 'artist' OR source_track_id IN ('sp-art-1', 'sp-art-2'))",
    )
    .bind(&job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        artist_items, 0,
        "a legacy job does not read followed artists"
    );

    let job: (i64, Option<String>) =
        sqlx::query_as("SELECT total_items, collections FROM migration_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job.0, 0, "no track favorites were seeded");
    assert_eq!(job.1, None, "a legacy job keeps NULL collections");
}

/// M02 flips the M01 pin: the matrix declares artists exactly where the
/// destination client can follow (Qobuz, Tidal, Spotify), and gates the
/// collection everywhere else before the job row exists.
#[tokio::test]
async fn test_artists_collection_is_offered_only_where_follow_exists() {
    let pool = setup_test_db().await;
    seed_followed_artists(&pool).await;
    let app = test_app(pool.clone());

    let capabilities = get_migration_capabilities().await.unwrap();
    assert!(!capabilities.is_empty(), "the matrix has destinations");
    for destination in &capabilities {
        let artists = destination
            .collections
            .iter()
            .find(|cell| cell.collection == MigrationCollection::Artists)
            .unwrap_or_else(|| panic!("{} has no artists cell", destination.service));
        let expected = matches!(destination.service.as_str(), "qobuz" | "tidal" | "spotify");
        assert_eq!(
            artists.supported, expected,
            "{} artists support is the client's follow path",
            destination.service
        );
    }

    // A destination without follow refuses the collection before the job row.
    let result = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "deezer".to_string(),
        None,
        migration_options(),
        None,
        None,
        Some(vec!["artists".to_string()]),
    )
    .await;
    let error = result.expect_err("deezer cannot follow artists");
    assert!(
        error.contains("does not support the 'artists' collection"),
        "the refusal names the collection, got: {error}"
    );

    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0, "the refusal happens before the job row");

    // Qobuz declares it: the job starts and carries the collection on the wire.
    let job_id = start_migration(
        app.handle().clone(),
        app.state::<AppState>(),
        "spotify".to_string(),
        "qobuz".to_string(),
        None,
        migration_options(),
        None,
        None,
        Some(vec!["artists".to_string()]),
    )
    .await
    .expect("qobuz declares artists");
    let collections: Option<String> =
        sqlx::query_scalar("SELECT collections FROM migration_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(collections.as_deref(), Some(r#"["artists"]"#));
}

/// La verificación de esquema de M02: la 0113 ya dejó `kind` como TEXT libre y
/// `collections` como JSON de nombres de cable, y el espejo de artistas (0002,
/// 0038, 0045, 0065) ya guarda `is_favorite` y los ids por servicio. Un ítem
/// `kind = 'artist'` y un job `collections = '["artists"]'` se insertan hoy sin
/// migración nueva — y la última migración aplicada sigue siendo la 0113, que
/// no se toca.
#[tokio::test]
async fn test_schema_already_accepts_artist_items_and_collections() {
    let pool = setup_test_db().await;

    let latest_version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        latest_version, 113,
        "the schema already carries the M01 kinds migration"
    );

    // El espejo de artistas expone lo que la colección necesita leer y escribir.
    for column in [
        "is_favorite",
        "favorite_at",
        "spotify_id",
        "tidal_id",
        "qobuz_id",
    ] {
        let present: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('artists') WHERE name = ?")
                .bind(column)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(present, 1, "artists.{column} exists in the current schema");
    }

    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status, total_items, collections)
           VALUES ('artist-job', 'spotify', 'qobuz', '{}', 'pending', 1, '["artists"]')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    // Los ítems siguen siendo track-shaped (0017): `source_track_title` y
    // `source_track_artist` son NOT NULL. Un ítem de artista lleva su nombre en
    // ambas, que es su única etiqueta humana.
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, kind, source_track_id, source_track_title,
                                        source_track_artist, status)
           VALUES ('artist-job', 'artist', 'sp-art-1', 'Mirror Artist', 'Mirror Artist', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let (kind, collections): (String, String) = sqlx::query_as(
        "SELECT mi.kind, mj.collections FROM migration_items mi
         JOIN migration_jobs mj ON mj.id = mi.job_id
         WHERE mj.id = 'artist-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "artist");
    assert_eq!(collections, r#"["artists"]"#);
}
