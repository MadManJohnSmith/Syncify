//! M01 (#166) — upgrade real de la migración 0113 (`migration_items.kind` y
//! `migration_jobs.collections`).
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `library_fts_migration_upgrade_test` (#99): el
//! fixture ejecuta los ficheros REALES de migración 0001–0112 sobre una BD
//! limpia, siembra filas del esquema vigente (ítems de migración históricos,
//! que solo podían ser de pistas) y después aplica 0113, comprobando que las
//! filas históricas quedan como `kind = 'track'`, que los jobs legados quedan
//! con `collections` NULL y que los ítems nuevos pueden declarar su tipo. La
//! 0017 aplicada no se edita; el número 0113 no se reutiliza (0112 lo ocupa
//! library_memberships).

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const PREVIOUS_VERSION: i64 = 112;
const NEW_VERSION: i64 = 113;

/// Aplica SOLO el fichero real de la migración nueva sobre la base previa.
/// Re-ejecutar 0001-0112 sobre el esquema ya actualizado fallaría en sus
/// propias sentencias históricas (p. ej. el índice de 0001 sobre
/// downloads(status), columna que la 0002 reconstruida ya no tiene): el
/// upgrade es únicamente 0113.
fn new_migration_sql() -> std::borrow::Cow<'static, str> {
    MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0113 existe")
        .sql
        .clone()
}

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Memoria con nombre único por pool: `sqlite::memory:` sin nombre comparte
/// el mismo espacio de BD entre pools del proceso, y este test necesita un
/// esquema PREVIO (0001-0112) limpio — un resto del esquema final de otro
/// test rompería la 0001 (su índice sobre downloads(status) choca con la
/// downloads reconstruida por 0002, que ya no tiene esa columna).
fn unique_memory_uri() -> String {
    format!(
        "sqlite:file:m01_upgrade_{}?mode=memory&cache=shared",
        uuid::Uuid::new_v4().simple()
    )
}

async fn setup_pre_upgrade_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&unique_memory_uri())
        .await
        .expect("Failed to connect to in-memory database");
    apply_migrations_through(&pool, PREVIOUS_VERSION).await;
    pool
}

/// Siembra el esquema vigente antes del upgrade: un job legado de pistas y
/// dos ítems históricos, tal y como 0017 + 0067 + 0090 los escribían.
async fn seed_legacy_job(pool: &Pool<Sqlite>) -> String {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, source_playlist_ids, options, status, total_items)
           VALUES ('legacy-job', 'spotify', 'qobuz', '["pl-1"]', '{"match_threshold":0.8}', 'completed', 2)"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist,
                                        source_track_album, source_playlist_id, source_playlist_name,
                                        status, match_confidence, match_method)
           VALUES ('legacy-job', 'sp-one', 'Song One', 'Artist', 'Album One', 'pl-1', 'Playlist One',
                   'transferred', 1.0, 'isrc'),
                  ('legacy-job', 'sp-two', 'Song Two', 'Artist', NULL, 'pl-1', 'Playlist One',
                   'failed', 0.85, 'simulated')"#,
    )
    .execute(pool)
    .await
    .unwrap();
    "legacy-job".to_string()
}

/// El esquema previo no conoce ni `kind` ni `collections`: el upgrade añade
/// ambas columnas y el índice por kind, sin tocar las filas históricas.
#[tokio::test]
async fn test_0113_upgrade_preserves_legacy_items_as_tracks() {
    let pool = setup_pre_upgrade_db().await;
    seed_legacy_job(&pool).await;

    // El esquema previo no tiene las columnas nuevas.
    let kind_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('migration_items') WHERE name = 'kind'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind_before, 0, "the previous schema has no kind column");
    let collections_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('migration_jobs') WHERE name = 'collections'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        collections_before, 0,
        "the previous schema has no collections column"
    );

    sqlx::raw_sql(&new_migration_sql())
        .execute(&pool)
        .await
        .unwrap();

    // Las filas históricas sobreviven, marcadas como pistas.
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT source_track_id, status, kind FROM migration_items WHERE job_id = 'legacy-job' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "no historical item is lost by the upgrade");
    for (source_track_id, status, kind) in &rows {
        assert_eq!(
            kind, "track",
            "a pre-0113 item can only be a track ({source_track_id})"
        );
        assert!(
            status == "transferred" || status == "failed",
            "history keeps its own statuses ({source_track_id} = {status})"
        );
    }

    // El job legado no declara colecciones: NULL sigue significando el flujo
    // histórico de favoritos de pistas.
    let legacy_collections: Option<String> =
        sqlx::query_scalar("SELECT collections FROM migration_jobs WHERE id = 'legacy-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        legacy_collections, None,
        "a legacy job keeps NULL collections"
    );
}

/// Tras el upgrade, un ítem puede declarar `album` y un job puede declarar
/// sus colecciones — la capacidad nueva que 0113 habilita.
#[tokio::test]
async fn test_0113_upgrade_accepts_new_kinds_and_collections() {
    let pool = setup_pre_upgrade_db().await;
    seed_legacy_job(&pool).await;
    sqlx::raw_sql(&new_migration_sql())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status, total_items, collections)
           VALUES ('album-job', 'spotify', 'qobuz', '{}', 'pending', 1, '["albums"]')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, kind, source_track_id, source_track_title, source_track_artist, status)
           VALUES ('album-job', 'album', 'sp-alb-1', 'Favorite Album', 'Artist', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let (kind, title): (String, String) = sqlx::query_as(
        "SELECT kind, source_track_title FROM migration_items WHERE job_id = 'album-job'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "album");
    assert_eq!(title, "Favorite Album");

    let collections: String =
        sqlx::query_scalar("SELECT collections FROM migration_jobs WHERE id = 'album-job'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(collections, r#"["albums"]"#);

    // El índice por kind existe (se sirve el detalle por tipo sin full scan).
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_migration_items_job_kind'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(index, 1);
}

/// El default NOT NULL es lo que salva el upgrade: un ítem insertado SIN
/// kind sobre el esquema ya actualizado sigue siendo una pista — la cintura
/// de seguridad para cualquier escritor que aún no sepa de kinds.
#[tokio::test]
async fn test_0113_kind_defaults_to_track_for_unaware_writers() {
    let pool = setup_pre_upgrade_db().await;
    sqlx::raw_sql(&new_migration_sql())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status)
           VALUES ('some-job', 'spotify', 'qobuz', '{}', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist, status)
           VALUES ('some-job', 'sp-x', 'Song X', 'Artist', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let kind: String =
        sqlx::query_scalar("SELECT kind FROM migration_items WHERE source_track_id = 'sp-x'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kind, "track");
}
