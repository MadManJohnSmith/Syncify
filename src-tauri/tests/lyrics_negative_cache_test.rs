//! LY03 (#184) — caché negativa persistente de resolución de letras.
//!
//! Upgrade real de la migración 0114 (`lyrics_resolution_failures`) y
//! comportamiento IPC contra una BD `:memory:` migrada de verdad: salto de
//! Fuentes con fallo vigente, caducidad y refresco, clasificación del motivo y
//! separación de la Marca de borrado.
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `migration_item_kinds_upgrade_test` (#166): el
//! fixture ejecuta los ficheros REALES de migración 0001–0113 sobre una BD
//! limpia, siembra datos del esquema vigente (pista y letra ya guardadas) y
//! después aplica 0114, comprobando que la tabla nueva aparece sin tocar lo
//! anterior, que su ventana de caducidad separa fallos vigentes de caducados y
//! que el borrado de la pista arrastra el registro. La 0098 de Marcas de
//! borrado no se edita y la caché negativa no la sustituye; el número 0114 no
//! se reutiliza (0113 lo ocupa migration_item_kinds).

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

use std::sync::Arc;
use syncify_lyrics_domain::ResolutionStatus;
use syncify_lyrics_engine::{LyricsClient, LyricsEngineConfig, LyricsSourceSetting, NoopHooks};
use syncify_tauri_lib::commands::lyrics::resolve_track_lyrics_with_client;
use syncify_tauri_lib::services::lyrics_negative_cache::{
    active_failed_sources, resolve_with_negative_cache, ttl_hours,
};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const PREVIOUS_VERSION: i64 = 113;
const NEW_VERSION: i64 = 114;

/// Aplica SOLO el fichero real de la migración nueva sobre la base previa.
fn new_migration_sql() -> std::borrow::Cow<'static, str> {
    MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0114 existe")
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

/// Memoria con nombre único por pool: `sqlite::memory:` sin nombre comparte el
/// mismo espacio de BD entre pools del proceso, y este test necesita un
/// esquema PREVIO (0001-0113) limpio.
fn unique_memory_uri() -> String {
    format!(
        "sqlite:file:ly03_upgrade_{}?mode=memory&cache=shared",
        uuid::Uuid::new_v4().simple()
    )
}

async fn setup_pre_upgrade_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&unique_memory_uri())
        .await
        .expect("Failed to connect to in-memory database");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, PREVIOUS_VERSION).await;
    pool
}

/// Siembra el esquema vigente antes del upgrade: una pista con su letra ya
/// guardada y una Marca de borrado (0098) para otra pista.
async fn seed_legacy_lyrics(pool: &Pool<Sqlite>) {
    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Legacy Song')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO lyrics (track_id, format, sync_level, source, content)
         VALUES (1, 'lrc', 'line', 'lrclib_synced', '[00:01.00]legacy line')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO tracks (id, title) VALUES (2, 'Deleted Song')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO lyrics_deletion_marks (track_id, format) VALUES (2, '*')")
        .execute(pool)
        .await
        .unwrap();
}

/// El esquema previo no conoce la tabla: el upgrade la crea (con su índice) y
/// deja intactas la letra guardada y la Marca de borrado, que son otra cosa.
#[tokio::test]
async fn test_0114_upgrade_adds_failure_registry_without_touching_history() {
    let pool = setup_pre_upgrade_db().await;
    seed_legacy_lyrics(&pool).await;

    let table_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'lyrics_resolution_failures'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        table_before, 0,
        "the previous schema has no failure registry"
    );

    sqlx::raw_sql(&new_migration_sql())
        .execute(&pool)
        .await
        .unwrap();

    let table_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'lyrics_resolution_failures'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_after, 1, "the upgrade creates the failure registry");

    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' \
         AND name = 'idx_lyrics_resolution_failures_expires'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(index, 1, "the expiry index exists");

    let lyric: (String, String) =
        sqlx::query_as("SELECT source, content FROM lyrics WHERE track_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(lyric.0, "lrclib_synced", "the stored lyrics survive");
    assert_eq!(lyric.1, "[00:01.00]legacy line");

    let mark: (String,) =
        sqlx::query_as("SELECT format FROM lyrics_deletion_marks WHERE track_id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        mark.0, "*",
        "the tombstone is still a user intent, not a failure row"
    );
    let failures: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_resolution_failures WHERE track_id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failures, 0, "the tombstone never becomes a negative entry");
}

/// La ventana de caducidad es la que separa un fallo vigente de uno caducado:
/// el lector solo devuelve las fuentes cuya `expires_at` sigue en el futuro.
#[tokio::test]
async fn test_0114_failure_expiry_window_separates_active_from_expired() {
    let pool = setup_pre_upgrade_db().await;
    seed_legacy_lyrics(&pool).await;
    sqlx::raw_sql(&new_migration_sql())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"INSERT INTO lyrics_resolution_failures
               (track_id, provider_id, reason, detail, expires_at)
           VALUES (1, 'lrclib_synced', 'not_found', 'No lyrics found',
                   datetime('now', '+7 days')),
                  (1, 'spotify_color', 'requires_auth', 'sp_dc is not configured',
                   datetime('now', '-1 minute')),
                  (1, 'kugou', 'rate_limited', 'HTTP 429', datetime('now', '+1 hour'))"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let active: Vec<String> = sqlx::query_scalar(
        "SELECT provider_id FROM lyrics_resolution_failures \
         WHERE track_id = 1 AND expires_at > datetime('now') ORDER BY provider_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        active,
        vec!["kugou".to_string(), "lrclib_synced".to_string()],
        "only non-expired failures are active"
    );

    let expired: Vec<String> = sqlx::query_scalar(
        "SELECT provider_id FROM lyrics_resolution_failures \
         WHERE track_id = 1 AND expires_at <= datetime('now')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        expired,
        vec!["spotify_color".to_string()],
        "an expired failure stops suppressing the source"
    );
}

/// La pista es la dueña del registro: borrarla lo arrastra, como el resto de
/// datos derivados (FK con ON DELETE CASCADE).
#[tokio::test]
async fn test_0114_failure_rows_cascade_with_the_track() {
    let pool = setup_pre_upgrade_db().await;
    seed_legacy_lyrics(&pool).await;
    sqlx::raw_sql(&new_migration_sql())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO lyrics_resolution_failures (track_id, provider_id, reason, expires_at) \
         VALUES (1, 'lrclib_synced', 'not_found', datetime('now', '+1 day'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM tracks WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_resolution_failures WHERE track_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0, "deleting the track removes its failures");
}

/// BD migrada de verdad (0001–0114) para los tests de comportamiento: la tabla
/// de la caché negativa es la misma que verá la app, no una copia a mano.
async fn setup_migrated_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&unique_memory_uri())
        .await
        .expect("Failed to connect to in-memory database");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    MIGRATOR
        .run(&pool)
        .await
        .expect("migraciones reales sobre una BD limpia");
    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Song'), (2, 'Other')")
        .execute(&pool)
        .await
        .unwrap();
    pool
}

/// Cliente hermético: Apple Music sin token en los hooks falla antes de tocar
/// la red, así que el motivo queda clasificado sin contactar ningún servicio.
fn offline_client() -> LyricsClient {
    let mut config = LyricsEngineConfig::default();
    config.retry_frequency = "hourly".to_string();
    config.sources = vec![LyricsSourceSetting {
        provider_id: "apple_music_ttml".to_string(),
        enabled: true,
        priority: 1,
    }];
    LyricsClient::with_parts_and_config(reqwest::Client::new(), Arc::new(NoopHooks), config)
}

/// Primera resolución: no hay letra, el fallo se guarda clasificado. Segunda:
/// la Fuente con fallo vigente se salta y no se vuelve a sondear.
#[tokio::test]
async fn resolution_records_classified_failure_and_second_attempt_skips_it() {
    let pool = setup_migrated_db().await;
    let client = offline_client();

    let first = resolve_with_negative_cache(
        &pool,
        &client,
        Some(1),
        "Artist",
        "Song",
        None,
        180.0,
        false,
    )
    .await
    .unwrap();
    assert_eq!(first.resolution.status, ResolutionStatus::NotFound);
    assert_eq!(first.skipped.len(), 0, "la primera pasada no salta nada");
    assert_eq!(first.failures.len(), 1);

    let (reason, detail): (String, String) = sqlx::query_as(
        "SELECT reason, detail FROM lyrics_resolution_failures \
         WHERE track_id = 1 AND provider_id = 'apple_music_ttml'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        reason, "requires_auth",
        "el motivo llega clasificado a la BD"
    );
    assert!(
        detail.contains("token"),
        "el detalle conserva el diagnóstico: {detail}"
    );

    let second = resolve_with_negative_cache(
        &pool,
        &client,
        Some(1),
        "Artist",
        "Song",
        None,
        180.0,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        second.skipped,
        vec!["apple_music_ttml".to_string()],
        "el fallo vigente se salta"
    );
    assert!(
        second.failures.is_empty(),
        "saltar una Fuente no produce un fallo nuevo"
    );
    let active = active_failed_sources(&pool, 1).await.unwrap();
    assert!(active.contains("apple_music_ttml"));
}

/// Un fallo caducado deja de suprimir la Fuente: la siguiente resolución la
/// vuelve a sondear y refresca la caducidad con el fallo recién observado.
#[tokio::test]
async fn expired_failure_is_probed_again_and_refreshed() {
    let pool = setup_migrated_db().await;
    let client = offline_client();

    sqlx::query(
        "INSERT INTO lyrics_resolution_failures \
             (track_id, provider_id, reason, detail, expires_at) \
         VALUES (1, 'apple_music_ttml', 'not_found', 'stale', datetime('now', '-1 minute'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let report = resolve_with_negative_cache(
        &pool,
        &client,
        Some(1),
        "Artist",
        "Song",
        None,
        180.0,
        false,
    )
    .await
    .unwrap();
    assert!(
        report.skipped.is_empty(),
        "un fallo caducado no se salta: la Fuente se vuelve a sondear"
    );
    assert_eq!(report.failures.len(), 1);

    let (reason, fresh): (String, i64) = sqlx::query_as(
        "SELECT reason, expires_at > datetime('now') FROM lyrics_resolution_failures \
         WHERE track_id = 1 AND provider_id = 'apple_music_ttml'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reason, "requires_auth", "el refresco actualiza el motivo");
    assert_eq!(fresh, 1, "el refresco vuelve a dejar la fila vigente");
}

/// La Marca de borrado es voluntad del usuario, no un fallo de resolución: la
/// ruta de comando que la consulta termina sin escribir en la caché negativa.
#[tokio::test]
async fn tombstone_is_never_recorded_as_a_failure() {
    let pool = setup_migrated_db().await;
    sqlx::query("INSERT INTO lyrics_deletion_marks (track_id, format) VALUES (1, '*')")
        .execute(&pool)
        .await
        .unwrap();
    let client = offline_client();

    let payload = resolve_track_lyrics_with_client(
        &client,
        &pool,
        "Artist".to_string(),
        "Song".to_string(),
        None,
        Some(180.0),
        None,
        Some(1),
    )
    .await
    .unwrap();
    assert_eq!(payload.status, ResolutionStatus::NotFound);

    let failures: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_resolution_failures WHERE track_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        failures, 0,
        "a tombstoned track keeps intent and evidence separate"
    );
}

#[test]
fn ttl_follows_the_retry_cadence_and_falls_back_to_weekly() {
    assert_eq!(ttl_hours("hourly"), 1);
    assert_eq!(ttl_hours("daily"), 24);
    assert_eq!(ttl_hours("weekly"), 168);
    assert_eq!(ttl_hours("never"), 168);
    assert_eq!(ttl_hours("  DAILY "), 24);
    assert_eq!(ttl_hours("unknown"), 168);
}
