//! Issue #65 — upgrade real del esquema de corridas (`sync_runs`,
//! `sync_run_collections`, `sync_run_errors`, migración 0106 nueva).
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `download_jobs_schema_migration_test` (D00,
//! #112): el fixture ejecuta los ficheros REALES de migración 0001–0105 sobre
//! una BD limpia, siembra datos del esquema vigente y después aplica 0106,
//! comprobando que el esquema nuevo queda operativo y que nada previo se
//! pierde. Las migraciones aplicadas nunca se editan ni se reutilizan números.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const NEW_VERSION: i64 = 106;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Siembra el esquema vigente antes del upgrade: una cuenta de Qobuz (el
/// servicio ya viene sembrado por las migraciones), la unidad mínima sobre la
/// que la ficha de cuenta corre.
async fn seed_account(pool: &Pool<Sqlite>) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("servicio qobuz sembrado por las migraciones");
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Upgrade Qobuz', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

#[tokio::test]
async fn el_upgrade_a_0106_sobre_base_0105_deja_el_esquema_operativo() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales 0001–0105.
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    let account_id = seed_account(&pool).await;

    // 2. El upgrade: el fichero real de la migración nueva, sin editar las
    //    aplicadas ni reutilizar números.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0106 está en el árbol");
    assert_eq!(
        new_migration.description, "sync runs",
        "el número 0106 es nuevo y describe las corridas"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0106 aplica sobre la base 0105");

    // 3. Nada previo se pierde: la cuenta sembrada sigue ahí.
    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .expect("cuenta tras el upgrade");
    assert_eq!(accounts, 1, "el upgrade conserva las cuentas existentes");

    // 4. El esquema nuevo queda operativo: insertar una corrida con los
    //    valores del contrato (estado y marcas de tiempo por defecto).
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("servicio qobuz");
    let run_id: i64 = sqlx::query_scalar(
        "INSERT INTO sync_runs (account_id, service_id, mode, scope, status)
         VALUES (?, ?, 'import_only', 'favorites', 'running') RETURNING id",
    )
    .bind(account_id)
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .expect("insertar corrida tras el upgrade");
    let (status, started_at, finished_at): (String, String, Option<String>) =
        sqlx::query_as("SELECT status, started_at, finished_at FROM sync_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .expect("corrida leída");
    assert_eq!(status, "running", "la corrida arranca en running");
    assert!(
        !started_at.is_empty(),
        "started_at tiene marca de tiempo por defecto"
    );
    assert_eq!(finished_at, None, "una corrida viva no tiene finished_at");

    // 5. Cobertura y huecos, con sus CHECK del contrato (dos ejes separados).
    sqlx::query(
        "INSERT INTO sync_run_collections (run_id, collection, reported, imported, coverage)
         VALUES (?, 'favorites', 2, 2, 'imported')",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("fila de cobertura");
    let invalid_coverage = sqlx::query(
        "INSERT INTO sync_run_collections (run_id, collection, reported, imported, coverage)
         VALUES (?, 'favorites', 2, 2, 'inventado')",
    )
    .bind(run_id)
    .execute(&pool)
    .await;
    assert!(
        invalid_coverage.is_err(),
        "la cobertura solo admite reported|imported|unknown (no porcentajes inventados)"
    );

    sqlx::query("INSERT INTO sync_run_errors (run_id, collection, message) VALUES (?, 'favorites', 'page gap')")
        .bind(run_id)
        .execute(&pool)
        .await
        .expect("hueco registrado");

    // 6. Estados de la ejecución: el eje de corrida admite exactamente los
    //    cuatro finales del contrato más running.
    for status in ["completed", "partial", "failed", "cancelled"] {
        let updated = sqlx::query("UPDATE sync_runs SET status = ? WHERE id = ?")
            .bind(status)
            .bind(run_id)
            .execute(&pool)
            .await
            .expect("estado de ejecución válido");
        assert_eq!(updated.rows_affected(), 1);
    }
    let invalid_status = sqlx::query("UPDATE sync_runs SET status = 'inventado' WHERE id = ?")
        .bind(run_id)
        .execute(&pool)
        .await;
    assert!(
        invalid_status.is_err(),
        "el estado de la corrida no admite valores fuera del contrato"
    );

    // 7. El índice de historial de la ficha existe.
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_sync_runs_account_started'",
    )
    .fetch_one(&pool)
    .await
    .expect("índice consultado");
    assert_eq!(index, 1, "índice sync_runs(account_id, started_at) creado");

    // 8. Y el esquema completo con la migración nueva también migra desde
    //    cero (la puerta de integridad de sqlx sigue aplicando).
    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("segunda BD en memoria");
    MIGRATOR
        .run(&fresh)
        .await
        .expect("migración completa desde cero con 0106 incluida");
}
