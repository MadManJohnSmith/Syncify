//! LY03 (#184) — caché negativa persistente de resolución de letras.
//!
//! Upgrade real de la migración 0114 (`lyrics_resolution_failures`) y
//! comportamiento IPC contra una BD `:memory:` migrada de verdad: salto de
//! Fuentes con fallo vigente, caducidad y refresco, clasificación del motivo y
//! separación de la Marca de borrado.
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `migration_item_kinds_upgrade_test` (#166): el
//! fixture ejecuta la ruta REAL de migración (`MIGRATOR.run`, con su
//! `_sqlx_migrations`) truncada en 0113 sobre una BD limpia, siembra datos del
//! esquema vigente (pista y letra ya guardadas) y después aplica 0114 con el
//! `MIGRATOR` completo, que revalida los checksums registrados y comprueba que
//! la tabla nueva aparece sin tocar lo anterior, que su ventana de caducidad
//! separa fallos vigentes de caducados y que el borrado de la pista arrastra el
//! registro. La 0098 de Marcas de borrado no se edita y la caché negativa no la
//! sustituye; el número 0114 no se reutiliza (0113 lo ocupa migration_item_kinds).

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

use std::borrow::Cow;
use std::sync::Arc;
use syncify_lyrics_domain::ResolutionStatus;
use syncify_lyrics_engine::{
    LyricsClient, LyricsEngineConfig, LyricsFailureKind, LyricsSourceFailure, LyricsSourceSetting,
    NoopHooks,
};
use syncify_tauri_lib::commands::lyrics::resolve_track_lyrics_with_client;
use syncify_tauri_lib::commands::{
    reorder_lyrics_providers, update_lyrics_config, update_lyrics_provider, update_track_metadata,
    UpdateTrackMetadata,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::models::LyricsConfig;
use syncify_tauri_lib::services::lyrics_negative_cache::{
    active_failed_sources, fetch_with_negative_cache, record_failures, resolve_with_negative_cache,
    ttl_hours,
};
use syncify_tauri_lib::settings_defaults::reset_all_settings;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tauri::Manager;

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const PREVIOUS_VERSION: i64 = 113;
const NEW_VERSION: i64 = 114;

/// Migrador real truncado en `max_version`: ejecuta la misma ruta de producción
/// que el arranque (incluida `_sqlx_migrations`), no `raw_sql` suelto. El
/// `MIGRATOR` completo revalida los checksums ya registrados y aplica 0114.
fn migrator_through(max_version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| migration.version <= max_version)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

async fn apply_pending_migrations(pool: &Pool<Sqlite>) {
    MIGRATOR
        .run(pool)
        .await
        .expect("la ruta real de migración aplica 0114");
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
    migrator_through(PREVIOUS_VERSION)
        .run(&pool)
        .await
        .expect("migraciones reales hasta 0113");
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

    apply_pending_migrations(&pool).await;

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

    let applied: (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), MAX(success) FROM _sqlx_migrations WHERE version = ?")
            .bind(NEW_VERSION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        applied,
        (1, 1),
        "0114 queda registrada como aplicada por la ruta real de sqlx"
    );
    let expected_previous = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= PREVIOUS_VERSION)
        .count() as i64;
    let previous: (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), MAX(version) FROM _sqlx_migrations WHERE version <= ?")
            .bind(PREVIOUS_VERSION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        previous,
        (expected_previous, PREVIOUS_VERSION),
        "la base previa tenía las migraciones reales hasta 0113, no un esquema a mano"
    );

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
    apply_pending_migrations(&pool).await;

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
    apply_pending_migrations(&pool).await;

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

fn create_test_app(pool: Pool<Sqlite>) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);
    app
}

async fn insert_failure(pool: &Pool<Sqlite>, track_id: i64, provider_id: &str) {
    sqlx::query(
        "INSERT INTO lyrics_resolution_failures \
             (track_id, provider_id, reason, detail, expires_at) \
         VALUES (?, ?, 'not_found', 'stale', datetime('now', '+1 day'))",
    )
    .bind(track_id)
    .bind(provider_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn failure_count(pool: &Pool<Sqlite>) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_resolution_failures")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Cliente hermético: Apple Music sin token en los hooks falla antes de tocar
/// la red, así que el motivo queda clasificado sin contactar ningún servicio.
fn offline_client() -> LyricsClient {
    offline_client_with_retry_frequency("hourly")
}

fn offline_client_with_retry_frequency(retry_frequency: &str) -> LyricsClient {
    let mut config = LyricsEngineConfig::default();
    config.retry_frequency = retry_frequency.to_string();
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

/// Cambiar idioma/cadencia, una Fuente o su prioridad invalida la evidencia
/// clasificada bajo la política anterior. El reset completo hace lo mismo
/// dentro de su transacción.
#[tokio::test]
async fn settings_changes_clear_the_negative_cache() {
    let pool = setup_migrated_db().await;
    let app = create_test_app(pool.clone());
    let state = app.state::<AppState>();

    insert_failure(&pool, 1, "apple_music_ttml").await;
    update_lyrics_config(
        state.clone(),
        LyricsConfig {
            id: 1,
            min_sync_level: "none".to_string(),
            preferred_language: "es".to_string(),
            storage_format: "lrc".to_string(),
            auto_fetch_on_import: false,
            retry_failed: true,
            retry_frequency: "daily".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        failure_count(&pool).await,
        0,
        "update_lyrics_config invalida la caché negativa"
    );

    insert_failure(&pool, 1, "apple_music_ttml").await;
    update_lyrics_provider(state.clone(), "apple_music_ttml".to_string(), true, 1)
        .await
        .unwrap();
    assert_eq!(
        failure_count(&pool).await,
        0,
        "update_lyrics_provider invalida la caché negativa"
    );

    insert_failure(&pool, 1, "apple_music_ttml").await;
    reorder_lyrics_providers(
        state.clone(),
        vec!["lrclib_synced".to_string(), "apple_music_ttml".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(
        failure_count(&pool).await,
        0,
        "reorder_lyrics_providers invalida la caché negativa"
    );

    insert_failure(&pool, 1, "apple_music_ttml").await;
    reset_all_settings(&pool).await.unwrap();
    assert_eq!(
        failure_count(&pool).await,
        0,
        "reset_all_settings invalida la caché negativa en su transacción"
    );
}

/// Editar título/artista/álbum cambia la identidad contra la que se resolvió:
/// la evidencia negativa de esa pista se olvida, sin tocar la de las demás.
#[tokio::test]
async fn metadata_change_clears_only_that_tracks_negative_cache() {
    let pool = setup_migrated_db().await;
    let app = create_test_app(pool.clone());
    let state = app.state::<AppState>();
    insert_failure(&pool, 1, "apple_music_ttml").await;
    insert_failure(&pool, 2, "lrclib_synced").await;

    update_track_metadata(
        state,
        1,
        UpdateTrackMetadata {
            title: Some("Corregido".to_string()),
            album_name: None,
            artist_name: None,
            track_number: None,
            disc_number: None,
            isrc: None,
            explicit: None,
            genre: None,
            year: None,
            bpm: None,
            musical_key: None,
            mb_track_id: None,
            _mb_release_id: None,
            _upc: None,
            _copyright: None,
            _composer: None,
            label: None,
            cover_art_url: None,
        },
    )
    .await
    .unwrap();

    let remaining: Vec<(i64,)> =
        sqlx::query_as("SELECT track_id FROM lyrics_resolution_failures ORDER BY track_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        remaining,
        vec![(2,)],
        "clear_for_track limpia la pista editada y respeta la ajena"
    );
}

#[test]
fn ttl_follows_the_retry_cadence_and_falls_back_to_weekly() {
    assert_eq!(ttl_hours("hourly"), 1);
    assert_eq!(ttl_hours("daily"), 24);
    assert_eq!(ttl_hours("weekly"), 168);
    assert_eq!(
        ttl_hours("always"),
        0,
        "`always` es la cadencia de la UI: no debe suprimir la Fuente"
    );
    assert_eq!(ttl_hours("never"), 168);
    assert_eq!(ttl_hours("  DAILY "), 24);
    assert_eq!(ttl_hours("unknown"), 168);
}

/// `always` no suprime: la fila puede registrarse, pero su `expires_at` no
/// supera a `now` y la segunda resolución vuelve a sondear la Fuente.
#[tokio::test]
async fn always_retry_frequency_does_not_skip_the_source() {
    let pool = setup_migrated_db().await;
    let client = offline_client_with_retry_frequency("always");

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
    assert_eq!(first.failures.len(), 1);

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
    assert!(
        second.skipped.is_empty(),
        "`always` no debe saltar Fuentes: {:?}",
        second.skipped
    );
    assert_eq!(
        second.failures.len(),
        1,
        "la Fuente se vuelve a sondear y registra el fallo otra vez"
    );
    let active = active_failed_sources(&pool, 1).await.unwrap();
    assert!(
        active.is_empty(),
        "ninguna fila de `always` queda vigente: {active:?}"
    );
}

/// `record_failures` usa la ventana configurada, no la de fábrica: con
/// `hourly` la fila debe caducar en una hora, no en 168.
#[tokio::test]
async fn recorded_window_follows_the_hourly_retry_cadence() {
    let pool = setup_migrated_db().await;
    let client = offline_client();

    resolve_with_negative_cache(
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

    let within_the_hour: i64 = sqlx::query_scalar(
        "SELECT expires_at BETWEEN datetime('now', '+59 minutes') \
                               AND datetime('now', '+61 minutes') \
         FROM lyrics_resolution_failures \
         WHERE track_id = 1 AND provider_id = 'apple_music_ttml'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        within_the_hour, 1,
        "hourly escribe una ventana de una hora, no la semana de fábrica"
    );
}

/// La ruta batch (`fetch_with_negative_cache`) participa del mismo contrato:
/// registra el fallo de la primera pasada y salta la Fuente en la segunda.
#[tokio::test]
async fn batch_fetch_records_and_skips_fresh_failures() {
    let pool = setup_migrated_db().await;
    let client = offline_client();

    let first = fetch_with_negative_cache(&pool, &client, Some(1), "Artist", "Song", 180.0, false)
        .await
        .unwrap();
    assert!(first.response.is_none());
    assert_eq!(
        first.failures.len(),
        1,
        "la primera pasada registra el fallo"
    );

    let second = fetch_with_negative_cache(&pool, &client, Some(1), "Artist", "Song", 180.0, false)
        .await
        .unwrap();
    assert_eq!(
        second.skipped,
        vec!["apple_music_ttml".to_string()],
        "la ruta batch también salta la Fuente con fallo vigente"
    );
}

/// El detalle se redacta en la frontera de persistencia: aunque un llamador
/// construya el fallo con la URL cruda, la BD no guarda el `usertoken`.
#[tokio::test]
async fn persisted_detail_redacts_credentials() {
    let pool = setup_migrated_db().await;
    let failures = vec![LyricsSourceFailure {
        provider_id: "musixmatch_richsync".to_string(),
        kind: LyricsFailureKind::SourceUnavailable,
        detail: "error sending request for url \
                 (https://api.musixmatch.com/ws/1.1/matcher.lyrics.get?usertoken=SECRET123&format=json)"
            .to_string(),
    }];

    record_failures(&pool, 1, &failures, "daily").await.unwrap();

    let detail: String = sqlx::query_scalar(
        "SELECT detail FROM lyrics_resolution_failures \
         WHERE track_id = 1 AND provider_id = 'musixmatch_richsync'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !detail.contains("SECRET123"),
        "el token no llega a la BD: {detail}"
    );
    assert!(
        detail.contains("usertoken=[REDACTED]"),
        "el parámetro queda redactado: {detail}"
    );
}

/// Un fallo de la caché (BD cerrada) no puede tumbar una resolución que no
/// depende de ella: se registra el aviso y la cascada sigue.
#[tokio::test]
async fn cache_failure_does_not_abort_resolution() {
    let pool = setup_migrated_db().await;
    let client = offline_client();
    pool.close().await;

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
    .expect("la resolución sigue aunque la caché no esté disponible");

    assert_eq!(report.resolution.status, ResolutionStatus::NotFound);
    assert_eq!(report.failures.len(), 1);
}
