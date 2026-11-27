//! LY08a (#404) — almacén de versiones de letras (ADR-0018 decisiones 1-3).
//!
//! Upgrade real de la migración 0121 sobre una BD `:memory:` que ya tiene
//! filas del almacén antiguo, y comportamiento del esquema contra la ruta real
//! de migración (`MIGRATOR.run`, con su `_sqlx_migrations`):
//!
//! - la tabla nueva admite varias versiones por (pista, Fuente, idioma,
//!   formato) y la huella del contenido distingue cuál es cuál;
//! - las filas de `lyrics` quedan intactas y legibles;
//! - el índice parcial de Pin admite como mucho uno por pista;
//! - el patrón de remapeo del dedup ISRC (`UPDATE OR IGNORE` + `DELETE`) no
//!   aborta por la clave única ni por el Pin, y las versiones propias de la
//!   pista perdedora se mueven a la ganadora.
//!
//! Mina de AGENTS: una BD recién migrada no demuestra que el upgrade funcione.
//! El fixture aplica las migraciones REALES hasta 0120 sobre una base limpia,
//! siembra letras del esquema vigente y después aplica 0121 con el `MIGRATOR`
//! completo, que revalida los checksums registrados.

use std::borrow::Cow;

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::services::lyrics_versions::{
    clear_pin, content_hash, list_versions, pin_version, pinned_version, record_version,
    NewLyricsVersion, UNKNOWN_LANGUAGE,
};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const PREVIOUS_VERSION: i64 = 120;
const NEW_VERSION: i64 = 121;

/// Migrador real truncado en `max_version`: ejecuta la misma ruta de producción
/// que el arranque (incluida `_sqlx_migrations`), no `raw_sql` suelto.
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

/// Memoria con nombre único por pool: `sqlite::memory:` sin nombre comparte el
/// mismo espacio de BD entre pools del proceso, y este test necesita un
/// esquema PREVIO (0001-0120) limpio.
fn unique_memory_uri() -> String {
    format!(
        "sqlite:file:ly08a_upgrade_{}?mode=memory&cache=shared",
        uuid::Uuid::new_v4().simple()
    )
}

async fn setup_pre_upgrade_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&unique_memory_uri())
        .await
        .expect("conectar BD :memory:");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("foreign keys");
    migrator_through(PREVIOUS_VERSION)
        .run(&pool)
        .await
        .expect("migraciones reales hasta 0120");
    pool
}

async fn setup_migrated_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&unique_memory_uri())
        .await
        .expect("conectar BD :memory:");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("foreign keys");
    MIGRATOR
        .run(&pool)
        .await
        .expect("migraciones reales hasta 0121");
    pool
}

async fn seed_track(pool: &Pool<Sqlite>, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("pista sembrada")
}

async fn table_exists(pool: &Pool<Sqlite>, name: &str) -> bool {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("sqlite_master");
    count > 0
}

fn version(
    track_id: i64,
    provider_id: &str,
    language: Option<&str>,
    format: &str,
    sync_level: &str,
    content: &str,
) -> NewLyricsVersion {
    NewLyricsVersion {
        track_id,
        provider_id: provider_id.to_string(),
        language: language.map(str::to_string),
        format: format.to_string(),
        sync_level: Some(sync_level.to_string()),
        content: content.to_string(),
        embedded_in_file: false,
    }
}

/// Siembra el esquema vigente antes del upgrade: una pista con sus dos filas
/// de formato (el almacén antiguo permite una por formato) y una Marca de
/// borrado. El upgrade no puede tocar ninguna.
async fn seed_legacy_lyrics(pool: &Pool<Sqlite>) {
    let track_id = seed_track(pool, "Legacy Song").await;
    sqlx::query(
        "INSERT INTO lyrics (track_id, format, sync_level, source, content, language)
         VALUES (?, 'lrc', 'line', 'LRCLIB', '[00:01.00]legacy line', 'en')",
    )
    .bind(track_id)
    .execute(pool)
    .await
    .expect("letra lrc histórica");
    sqlx::query(
        "INSERT INTO lyrics (track_id, format, sync_level, source, content, language)
         VALUES (?, 'plain', 'none', 'LRCLIB Plain', 'legacy line', 'en')",
    )
    .bind(track_id)
    .execute(pool)
    .await
    .expect("letra plain histórica");
    sqlx::query("INSERT INTO lyrics_deletion_marks (track_id, format) VALUES (?, 'ttml')")
        .bind(track_id)
        .execute(pool)
        .await
        .expect("Marca de borrado histórica");
}

#[tokio::test]
async fn test_0121_upgrade_keeps_legacy_lyrics_readable() {
    let pool = setup_pre_upgrade_db().await;
    seed_legacy_lyrics(&pool).await;

    assert!(
        !table_exists(&pool, "lyrics_versions").await,
        "el esquema previo no tiene almacén de versiones"
    );

    MIGRATOR
        .run(&pool)
        .await
        .expect("la ruta real de migración aplica 0121");

    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = ? AND success = 1",
    )
    .bind(NEW_VERSION)
    .fetch_one(&pool)
    .await
    .expect("ledger de migraciones");
    assert_eq!(applied, 1, "el ledger registra 0121 como aplicada");

    assert!(
        table_exists(&pool, "lyrics_versions").await,
        "el upgrade crea el almacén de versiones"
    );

    let legacy: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT format, sync_level, content, COALESCE(source, '')
         FROM lyrics ORDER BY format",
    )
    .fetch_all(&pool)
    .await
    .expect("filas antiguas legibles");
    assert_eq!(
        legacy,
        vec![
            (
                "lrc".to_string(),
                "line".to_string(),
                "[00:01.00]legacy line".to_string(),
                "LRCLIB".to_string(),
            ),
            (
                "plain".to_string(),
                "none".to_string(),
                "legacy line".to_string(),
                "LRCLIB Plain".to_string(),
            ),
        ],
        "las filas antiguas siguen intactas y en el mismo orden"
    );

    let marks: Vec<(String,)> = sqlx::query_as("SELECT format FROM lyrics_deletion_marks")
        .fetch_all(&pool)
        .await
        .expect("Marcas de borrado legibles");
    assert_eq!(marks, vec![("ttml".to_string(),)]);

    let versions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_versions")
        .fetch_one(&pool)
        .await
        .expect("almacén nuevo vacío");
    assert_eq!(versions, 0, "la migración no inventa versiones");

    let columns: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('lyrics_versions')")
            .fetch_all(&pool)
            .await
            .expect("columnas del almacén");
    let columns: Vec<String> = columns.into_iter().map(|(name,)| name).collect();
    for expected in [
        "id",
        "track_id",
        "provider_id",
        "language",
        "format",
        "sync_level",
        "content",
        "content_hash",
        "embedded_in_file",
        "obtained_at",
        "pinned_at",
    ] {
        assert!(
            columns.iter().any(|column| column == expected),
            "falta la columna {expected}: {columns:?}"
        );
    }

    let pin_index: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_lyrics_versions_pin_per_track'",
    )
    .fetch_one(&pool)
    .await
    .expect("índice parcial de Pin");
    assert!(
        pin_index.contains("WHERE pinned_at IS NOT NULL"),
        "el Pin por pista se limita con un índice parcial: {pin_index}"
    );
}

#[tokio::test]
async fn test_store_keeps_several_versions_per_source_language_and_format() {
    let pool = setup_migrated_db().await;
    let track_id = seed_track(&pool, "Versioned Song").await;

    let first = record_version(
        &pool,
        &version(
            track_id,
            "lrclib_synced",
            Some("en"),
            "lrc",
            "line",
            "[00:01.00]first line",
        ),
    )
    .await
    .expect("primera versión");
    let second = record_version(
        &pool,
        &version(
            track_id,
            "lrclib_synced",
            Some("en"),
            "lrc",
            "line",
            "[00:01.00]second line",
        ),
    )
    .await
    .expect("segunda versión en el mismo hueco");

    assert_ne!(first.id, second.id);
    assert_eq!(first.language, "en");
    assert_eq!(second.format, "lrc");
    assert_eq!(first.content_hash, content_hash("[00:01.00]first line"));
    assert_eq!(first.content_hash.len(), 64);

    let history = list_versions(&pool, track_id).await.expect("historial");
    assert_eq!(history.len(), 2, "conviven dos versiones del mismo hueco");

    // Repetir la misma observación no crea duplicados ni pierde la identidad;
    // solo refresca los atributos derivados.
    let mut refreshed = version(
        track_id,
        "lrclib_synced",
        Some("en"),
        "lrc",
        "word",
        "[00:01.00]first line",
    );
    refreshed.embedded_in_file = true;
    let refreshed = record_version(&pool, &refreshed)
        .await
        .expect("observación repetida");
    assert_eq!(
        refreshed.id, first.id,
        "la observación idéntica es la misma fila"
    );
    assert_eq!(refreshed.sync_level, "word");
    assert_eq!(refreshed.embedded_in_file, 1);
    assert_eq!(
        refreshed.obtained_at, first.obtained_at,
        "obtained_at conserva la primera observación"
    );
    assert_eq!(
        list_versions(&pool, track_id)
            .await
            .expect("historial")
            .len(),
        2
    );

    // La identidad se normaliza: mayúsculas y espacios no crean otra versión;
    // el idioma desconocido es `und`, no NULL.
    let normalized = record_version(
        &pool,
        &version(
            track_id,
            " LRCLIB_Synced ",
            Some("EN"),
            "LRC",
            "line",
            "[00:02.00]third line",
        ),
    )
    .await
    .expect("identidad normalizada");
    assert_eq!(normalized.provider_id, "lrclib_synced");
    assert_eq!(normalized.language, "en");
    assert_eq!(normalized.format, "lrc");

    let undetermined = record_version(
        &pool,
        &version(
            track_id,
            "lrclib_synced",
            None,
            "plain",
            "none",
            "plain text without language evidence",
        ),
    )
    .await
    .expect("idioma desconocido");
    assert_eq!(undetermined.language, UNKNOWN_LANGUAGE);

    let invalid = record_version(
        &pool,
        &version(track_id, "lrclib_synced", Some("en"), "lrc", "ultra", "x"),
    )
    .await;
    assert!(invalid.is_err(), "el vocabulario de sync_level se valida");

    let empty_provider = record_version(
        &pool,
        &version(track_id, "   ", Some("en"), "lrc", "line", "x"),
    )
    .await;
    assert!(empty_provider.is_err(), "la Fuente no puede quedar vacía");
}

#[tokio::test]
async fn test_pin_is_unique_per_track_and_follows_the_version() {
    let pool = setup_migrated_db().await;
    let track_id = seed_track(&pool, "Pinned Song").await;
    let other_track = seed_track(&pool, "Other Song").await;

    let first = record_version(
        &pool,
        &version(track_id, "lrclib_synced", Some("en"), "lrc", "line", "one"),
    )
    .await
    .expect("primera versión");
    let second = record_version(
        &pool,
        &version(track_id, "lrclib_synced", Some("en"), "lrc", "line", "two"),
    )
    .await
    .expect("segunda versión");

    let pinned = pin_version(&pool, track_id, first.id)
        .await
        .expect("Pin de la primera versión");
    assert_eq!(pinned.id, first.id);
    assert!(pinned.pinned_at.is_some());

    let repinned = pin_version(&pool, track_id, second.id)
        .await
        .expect("reemplazo del Pin");
    assert_eq!(repinned.id, second.id);
    assert_eq!(
        pinned_version(&pool, track_id)
            .await
            .expect("lectura del Pin")
            .expect("hay Pin")
            .id,
        second.id
    );
    let pinned_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM lyrics_versions
         WHERE track_id = ? AND pinned_at IS NOT NULL",
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .expect("un solo Pin");
    assert_eq!(pinned_count, 1);

    let error = pin_version(&pool, other_track, second.id)
        .await
        .expect_err("una versión de otra pista no se puede fijar");
    assert!(error.contains("does not belong"), "{error}");
    assert_eq!(
        pinned_version(&pool, track_id)
            .await
            .expect("lectura del Pin")
            .expect("el Pin anterior sigue")
            .id,
        second.id,
        "un Pin rechazado no deja la pista sin Pin"
    );

    assert!(clear_pin(&pool, track_id).await.expect("retirar Pin"));
    assert!(!clear_pin(&pool, track_id)
        .await
        .expect("sin Pin que retirar"));
    assert!(pinned_version(&pool, track_id)
        .await
        .expect("lectura sin Pin")
        .is_none());

    // El borrado de la pista arrastra sus versiones (FK ON DELETE CASCADE).
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(track_id)
        .execute(&pool)
        .await
        .expect("borrar pista");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_versions WHERE track_id = ?")
            .bind(track_id)
            .fetch_one(&pool)
            .await
            .expect("versiones borradas");
    assert_eq!(remaining, 0);
}

/// El remapeo de dedup ISRC mueve las versiones propias de la pista perdedora,
/// descarta la copia idéntica sin abortar y arrastra el Pin de la perdedora
/// cuando la ganadora no tenía uno. Es el patrón que #411 ejecutará.
#[tokio::test]
async fn test_isrc_remap_uses_update_or_ignore_and_moves_the_pin() {
    let pool = setup_migrated_db().await;
    let winner = seed_track(&pool, "Winner").await;
    let loser = seed_track(&pool, "Loser").await;

    let shared_content = "[00:01.00]shared line";
    let winner_shared = record_version(
        &pool,
        &version(
            winner,
            "lrclib_synced",
            Some("en"),
            "lrc",
            "line",
            shared_content,
        ),
    )
    .await
    .expect("versión compartida de la ganadora");
    let loser_shared = record_version(
        &pool,
        &version(
            loser,
            "lrclib_synced",
            Some("en"),
            "lrc",
            "line",
            shared_content,
        ),
    )
    .await
    .expect("copia idéntica en la perdedora");
    let loser_unique = record_version(
        &pool,
        &version(loser, "kugou", Some("en"), "lrc", "word", "unique to loser"),
    )
    .await
    .expect("versión exclusiva de la perdedora");

    pin_version(&pool, loser, loser_unique.id)
        .await
        .expect("Pin en la perdedora");

    sqlx::query("UPDATE OR IGNORE lyrics_versions SET track_id = ? WHERE track_id = ?")
        .bind(winner)
        .bind(loser)
        .execute(&pool)
        .await
        .expect("el remapeo no aborta por la clave única");
    sqlx::query("DELETE FROM lyrics_versions WHERE track_id = ?")
        .bind(loser)
        .execute(&pool)
        .await
        .expect("la copia idéntica de la perdedora se retira");

    let winner_versions = list_versions(&pool, winner).await.expect("versiones");
    assert_eq!(
        winner_versions.len(),
        2,
        "la ganadora conserva la suya y hereda la exclusiva"
    );
    assert!(
        winner_versions.iter().any(|v| v.id == winner_shared.id),
        "la copia idéntica de la ganadora prevalece"
    );
    assert!(
        !winner_versions.iter().any(|v| v.id == loser_shared.id),
        "la copia idéntica de la perdedora se descartó"
    );
    assert!(
        winner_versions.iter().any(|v| v.id == loser_unique.id),
        "la versión exclusiva de la perdedora se movió"
    );
    assert_eq!(
        pinned_version(&pool, winner)
            .await
            .expect("Pin de la ganadora")
            .expect("el Pin se movió con la versión")
            .id,
        loser_unique.id
    );
    let loser_left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lyrics_versions WHERE track_id = ?")
            .bind(loser)
            .fetch_one(&pool)
            .await
            .expect("perdedora vacía");
    assert_eq!(loser_left, 0);
}

/// Si las dos pistas tienen Pin, el de la ganadora prevalece: la fila idéntica
/// de la perdedora choca con el índice parcial y `OR IGNORE` la deja para el
/// DELETE. El remapeo no aborta ni deja dos Pins en la ganadora.
#[tokio::test]
async fn test_isrc_remap_keeps_winner_pin_when_both_tracks_have_one() {
    let pool = setup_migrated_db().await;
    let winner = seed_track(&pool, "Pinned Winner").await;
    let loser = seed_track(&pool, "Pinned Loser").await;

    let content = "[00:01.00]same line";
    let winner_version = record_version(
        &pool,
        &version(winner, "lrclib_synced", Some("en"), "lrc", "line", content),
    )
    .await
    .expect("versión de la ganadora");
    let loser_version = record_version(
        &pool,
        &version(loser, "lrclib_synced", Some("en"), "lrc", "line", content),
    )
    .await
    .expect("versión de la perdedora");
    pin_version(&pool, winner, winner_version.id)
        .await
        .expect("Pin de la ganadora");
    pin_version(&pool, loser, loser_version.id)
        .await
        .expect("Pin de la perdedora");

    sqlx::query("UPDATE OR IGNORE lyrics_versions SET track_id = ? WHERE track_id = ?")
        .bind(winner)
        .bind(loser)
        .execute(&pool)
        .await
        .expect("el conflicto de Pin no aborta el remapeo");
    sqlx::query("DELETE FROM lyrics_versions WHERE track_id = ?")
        .bind(loser)
        .execute(&pool)
        .await
        .expect("la fila perdedora se retira");

    let winner_versions = list_versions(&pool, winner).await.expect("versiones");
    assert_eq!(winner_versions.len(), 1);
    assert_eq!(
        pinned_version(&pool, winner)
            .await
            .expect("Pin de la ganadora")
            .expect("sigue habiendo Pin")
            .id,
        winner_version.id
    );
}
