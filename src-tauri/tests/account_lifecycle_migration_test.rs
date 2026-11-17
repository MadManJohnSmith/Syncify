//! Issue #85 — upgrade real de los estados de ciclo de vida (`accounts.state`,
//! migración 0107 nueva).
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `sync_run_schema_migration_test` (#65): el
//! fixture ejecuta los ficheros REALES de migración 0001–0106 sobre una BD
//! limpia, siembra cuentas del esquema vigente (activa, hermana, credencial
//! inválida) y después aplica 0107, comprobando el criterio del issue: la
//! cuenta activa queda habilitada. Las migraciones aplicadas nunca se editan
//! ni se reutilizan números.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const NEW_VERSION: i64 = 107;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Siembra el esquema vigente antes del upgrade: dos cuentas de Qobuz
/// (una activa, una hermana inactiva, esta última además con credencial
/// inválida), las tres variantes que el criterio del issue distingue.
async fn seed_accounts(pool: &Pool<Sqlite>) -> (i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("servicio qobuz sembrado por las migraciones");

    let active_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Activa', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta activa sembrada");

    let inactive_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active, credentials_invalid, invalid_reason)
         VALUES (?, 'Hermana', 0, 1, 'expired cookie') RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta hermana sembrada");

    (active_id, inactive_id)
}

#[tokio::test]
async fn el_upgrade_a_0107_deja_la_cuenta_activa_como_habilitada() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales 0001–0106, sin columna `state`.
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    let (active_id, inactive_id) = seed_accounts(&pool).await;
    let column_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name = 'state'",
    )
    .fetch_one(&pool)
    .await
    .expect("columna consultada antes del upgrade");
    assert_eq!(column_before, 0, "0106 no conoce el ciclo de vida");

    // 2. El upgrade: el fichero real de la migración nueva.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0107 está en el árbol");
    assert_eq!(
        new_migration.description, "account lifecycle states",
        "el número 0107 es nuevo y describe los estados de ciclo de vida"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0107 aplica sobre la base 0106");

    // 3. Criterio del issue: la cuenta activa conservada como habilitada.
    let active_state: String = sqlx::query_scalar("SELECT state FROM accounts WHERE id = ?")
        .bind(active_id)
        .fetch_one(&pool)
        .await
        .expect("cuenta activa tras el upgrade");
    assert_eq!(
        active_state, "enabled",
        "la cuenta que hoy está activa pasa a habilitada"
    );
    let (inactive_state, active_flag): (String, i64) =
        sqlx::query_as("SELECT state, is_active FROM accounts WHERE id = ?")
            .bind(inactive_id)
            .fetch_one(&pool)
            .await
            .expect("cuenta hermana tras el upgrade");
    assert_eq!(
        inactive_state, "enabled",
        "las demás cuentas también se habilitan (ADR-0007: habilita las demás)"
    );
    assert_eq!(
        active_flag, 0,
        "la marca de credencial inválida NO se confunde con el ciclo de vida"
    );

    // 4. El esquema nuevo queda operativo: los tres estados del contrato
    //    admitidos y todo valor fuera de ellos rechazado por el CHECK.
    for state in ["enabled", "disabled", "archived"] {
        let updated = sqlx::query("UPDATE accounts SET state = ? WHERE id = ?")
            .bind(state)
            .bind(active_id)
            .execute(&pool)
            .await
            .expect("estado de ciclo de vida válido");
        assert_eq!(updated.rows_affected(), 1);
    }
    let invalid_state = sqlx::query("UPDATE accounts SET state = 'eliminada' WHERE id = ?")
        .bind(active_id)
        .execute(&pool)
        .await;
    assert!(
        invalid_state.is_err(),
        "eliminar no es un estado: el CHECK rechaza valores fuera del contrato"
    );

    // 5. El índice de visibilidad por estado existe.
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_accounts_state'",
    )
    .fetch_one(&pool)
    .await
    .expect("índice consultado");
    assert_eq!(index, 1, "índice accounts(state) creado");

    // 6. Y el esquema completo con la migración nueva también migra desde
    //    cero (la puerta de integridad de sqlx sigue aplicando).
    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("segunda BD en memoria");
    MIGRATOR
        .run(&fresh)
        .await
        .expect("migración completa desde cero con 0107 incluida");
}
