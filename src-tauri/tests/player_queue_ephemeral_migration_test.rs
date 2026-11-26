//! Issue #134 — upgrade real de la migración 0119.
//!
//! Una BD recién migrada no demuestra que el rebuild conserve la cola: este test
//! ejecuta las migraciones REALES 0001–0118, siembra pendientes/historial/current
//! bajo el esquema anterior y después aplica 0119. Comprueba además el XOR de
//! identidad, la FK de proveedor, los índices y la conversión de una efímera.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");
const NEW_VERSION: i64 = 119;

async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

async fn setup_pre_upgrade_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("FK activas");
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    pool
}

async fn apply_new_migration(pool: &Pool<Sqlite>) {
    let migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0119 está en el árbol");
    assert_eq!(
        migration.description, "player queue ephemeral entries",
        "0119 describe la identidad efímera de la cola"
    );
    sqlx::raw_sql(&migration.sql)
        .execute(pool)
        .await
        .expect("0119 aplica sobre la base 0118");
}

async fn seed_legacy_queue(pool: &Pool<Sqlite>) -> (i64, i64) {
    let track_a: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Pendiente legacy') RETURNING id")
            .fetch_one(pool)
            .await
            .expect("track pendiente");
    let track_b: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Historial legacy') RETURNING id")
            .fetch_one(pool)
            .await
            .expect("track historial");
    sqlx::query(
        "INSERT INTO player_queue_entries \
         (entry_id, track_id, kind, position, enqueued_at) VALUES \
         ('legacy-pending', ?, 'pending', 7, '2026-01-01T00:00:00Z'), \
         ('legacy-history', ?, 'history', 4, '2026-01-02T00:00:00Z'), \
         ('legacy-current', ?, 'current', 0, '2026-01-03T00:00:00Z')",
    )
    .bind(track_a)
    .bind(track_b)
    .bind(track_a)
    .execute(pool)
    .await
    .expect("cola legacy sembrada");
    (track_a, track_b)
}

#[tokio::test]
async fn upgrade_0119_conserva_la_cola_legacy_y_sus_indices() {
    let pool = setup_pre_upgrade_db().await;
    let (track_a, track_b) = seed_legacy_queue(&pool).await;

    apply_new_migration(&pool).await;

    let rows: Vec<(
        String,
        i64,
        String,
        i64,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT entry_id, track_id, kind, position, enqueued_at, service_id, \
                    service_track_id, listen_session_id, promotion_error \
             FROM player_queue_entries ORDER BY position, rowid",
    )
    .fetch_all(&pool)
    .await
    .expect("cola tras el upgrade");
    assert_eq!(
        rows,
        vec![
            (
                "legacy-current".into(),
                track_a,
                "current".into(),
                0,
                "2026-01-03T00:00:00Z".into(),
                None,
                None,
                None,
                None,
            ),
            (
                "legacy-history".into(),
                track_b,
                "history".into(),
                4,
                "2026-01-02T00:00:00Z".into(),
                None,
                None,
                None,
                None,
            ),
            (
                "legacy-pending".into(),
                track_a,
                "pending".into(),
                7,
                "2026-01-01T00:00:00Z".into(),
                None,
                None,
                None,
                None,
            ),
        ],
        "el rebuild conserva identidad, orden y timestamps"
    );

    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_index_list('player_queue_entries') \
         WHERE name NOT LIKE 'sqlite_autoindex%'",
    )
    .fetch_all(&pool)
    .await
    .expect("índices de la cola");
    assert!(indexes.contains(&"idx_player_queue_entries_kind_position".into()));
    assert!(indexes.contains(&"idx_player_queue_entries_single_current".into()));

    let duplicate_current = sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES ('otra-current', ?, 'current', 1)",
    )
    .bind(track_b)
    .execute(&pool)
    .await;
    assert!(
        duplicate_current.is_err(),
        "el índice parcial de current sigue siendo único"
    );

    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(track_a)
        .execute(&pool)
        .await
        .expect("borrar track legacy");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM player_queue_entries WHERE track_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("entradas de biblioteca restantes");
    assert_eq!(remaining, 1, "ON DELETE CASCADE sigue activo");
}

#[tokio::test]
async fn upgrade_0119_acepta_efimeras_y_rechaza_identidades_incompletas() {
    let pool = setup_pre_upgrade_db().await;
    apply_new_migration(&pool).await;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("Qobuz sembrado por migraciones");
    let xor_track: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Track para XOR') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("track válido para probar el XOR");

    sqlx::query(
        "INSERT INTO player_queue_entries \
         (entry_id, service_id, service_track_id, kind, position) \
         VALUES ('efimera', ?, 'qobuz-track-1', 'pending', 0)",
    )
    .bind(service_id)
    .execute(&pool)
    .await
    .expect("entrada efímera válida");
    sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES ('biblioteca-valida', ?, 'pending', 0)",
    )
    .bind(xor_track)
    .execute(&pool)
    .await
    .expect("una entrada de biblioteca sigue siendo válida sin proveedor");

    let invalid_rows = [
        ("sin-identidad", None, None, None),
        ("solo-servicio", None, Some(service_id), None),
        (
            "servicio-y-track",
            Some(xor_track),
            Some(service_id),
            Some("x"),
        ),
        ("id-vacio", None, Some(service_id), Some("   ")),
    ];
    for (entry_id, track_id, service, provider_track_id) in invalid_rows {
        let result = sqlx::query(
            "INSERT INTO player_queue_entries \
             (entry_id, track_id, service_id, service_track_id, kind, position) \
             VALUES (?, ?, ?, ?, 'pending', 1)",
        )
        .bind(entry_id)
        .bind(track_id)
        .bind(service)
        .bind(provider_track_id)
        .execute(&pool)
        .await;
        assert!(result.is_err(), "{entry_id}: el XOR debe rechazarla");
    }

    let unknown_service = sqlx::query(
        "INSERT INTO player_queue_entries \
         (entry_id, service_id, service_track_id, kind, position) \
         VALUES ('servicio-inexistente', 999999, 'x', 'pending', 1)",
    )
    .execute(&pool)
    .await;
    assert!(unknown_service.is_err(), "la FK de servicio sigue activa");

    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Promocionada') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("track para promoción");
    let session_id = "0b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5b";
    let updated = sqlx::query(
        "UPDATE player_queue_entries \
         SET track_id = ?, service_id = NULL, service_track_id = NULL, \
             listen_session_id = ?, promotion_error = NULL \
         WHERE entry_id = 'efimera' AND track_id IS NULL",
    )
    .bind(track_id)
    .bind(session_id)
    .execute(&pool)
    .await
    .expect("conversión a biblioteca")
    .rows_affected();
    assert_eq!(updated, 1, "la efímera se convierte una vez");
    let identity: (Option<i64>, Option<i64>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT track_id, service_id, service_track_id, listen_session_id \
         FROM player_queue_entries WHERE entry_id = 'efimera'",
    )
    .fetch_one(&pool)
    .await
    .expect("identidad promocionada");
    assert_eq!(
        identity,
        (Some(track_id), None, None, Some(session_id.into()))
    );
}

#[tokio::test]
async fn migracion_0119_compone_desde_cero() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    MIGRATOR.run(&pool).await.expect("migraciones completas");
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('player_queue_entries')")
            .fetch_all(&pool)
            .await
            .expect("columnas de la cola");
    for expected in [
        "track_id",
        "service_id",
        "service_track_id",
        "listen_session_id",
        "promotion_error",
    ] {
        assert!(columns.iter().any(|name| name == expected), "{expected}");
    }
}
