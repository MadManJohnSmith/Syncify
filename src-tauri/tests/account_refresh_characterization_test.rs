//! A03-ter (#342) — caracterización del punto reactivo de descarga.
//!
//! Fija el comportamiento de `persist_rejected_primary_account` ANTES de
//! sustituir su `UPDATE` ad-hoc por la puerta de `account_refresh`
//! (`handle_credential_rejection`, ADR-0007 decisión 14). Los cuatro tests
//! cubren el contrato observable actual: el veredicto que escribe, la
//! idempotencia de la marca, el aislamiento por servicio y los no-ops.
//!
//! Sin red, BD `:memory:` completamente migrada y cuentas desechables.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Pool;
use sqlx::Sqlite;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;

async fn setup_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones completas");
    pool
}

async fn service_id(pool: &Pool<Sqlite>, service: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por las migraciones")
}

/// Cuenta mínima, sin credenciales: el `UPDATE` ad-hoc no las consulta.
async fn insert_account(pool: &Pool<Sqlite>, service: &str) -> i64 {
    let sid = service_id(pool, service).await;
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email, is_active) \
         VALUES (?, 'Caracterización', 'char@test.invalid', 1) RETURNING id",
    )
    .bind(sid)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

async fn account_row(pool: &Pool<Sqlite>, account_id: i64) -> (i64, Option<String>, Option<String>, Option<String>) {
    sqlx::query_as(
        "SELECT COALESCE(credentials_invalid, 0), invalid_reason, last_auth_error, \
         last_auth_error_at FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("cuenta presente")
}

/// El veredicto escrito hoy: marca la cuenta con los textos documentados del
/// `UPDATE` ad-hoc y sella `last_auth_error_at` (el latch A-4 lee esa fecha).
#[tokio::test]
async fn marks_the_unmarked_account_with_the_documented_verdict() {
    let pool = setup_db().await;
    let account_id = insert_account(&pool, "qobuz").await;
    let orchestrator = DownloadOrchestrator::new().with_db(pool.clone());

    orchestrator
        .persist_rejected_primary_account(Some(account_id), "qobuz")
        .await;

    let (invalid, reason, last_err, last_at) = account_row(&pool, account_id).await;
    assert_eq!(invalid, 1, "la cuenta rechazada queda invalidada");
    assert_eq!(reason.as_deref(), Some("token_expired"));
    assert_eq!(
        last_err.as_deref(),
        Some("Provider rejected credentials")
    );
    let stamp = last_at.expect("el latch A-4 necesita la fecha de la marca");
    chrono::DateTime::parse_from_rfc3339(&stamp).expect("last_auth_error_at en RFC 3339");
}

/// Un veredicto previo no se reescribe: segunda llamada no toca la fila
/// (la cláusula `COALESCE(credentials_invalid, 0) = 0` la excluye).
#[tokio::test]
async fn does_not_rewrite_an_already_marked_account() {
    let pool = setup_db().await;
    let account_id = insert_account(&pool, "tidal").await;
    sqlx::query(
        "UPDATE accounts SET credentials_invalid = 1, invalid_reason = 'otro-motivo', \
         last_auth_error = 'error-previo', last_auth_error_at = '2026-01-01T00:00:00+00:00' \
         WHERE id = ?",
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .unwrap();
    let orchestrator = DownloadOrchestrator::new().with_db(pool.clone());

    orchestrator
        .persist_rejected_primary_account(Some(account_id), "tidal")
        .await;

    let (invalid, reason, last_err, last_at) = account_row(&pool, account_id).await;
    assert_eq!(invalid, 1);
    assert_eq!(reason.as_deref(), Some("otro-motivo"), "no se reescribe");
    assert_eq!(last_err.as_deref(), Some("error-previo"), "no se reescribe");
    assert_eq!(
        last_at.as_deref(),
        Some("2026-01-01T00:00:00+00:00"),
        "el sello original no cambia"
    );
}

/// El veredicto es por CUENTA y SERVICIO: llamar con otro servicio no toca la
/// fila (el `service_id IN (SELECT ...)` no matchea).
#[tokio::test]
async fn never_touches_accounts_of_another_service() {
    let pool = setup_db().await;
    let account_id = insert_account(&pool, "qobuz").await;
    let orchestrator = DownloadOrchestrator::new().with_db(pool.clone());

    orchestrator
        .persist_rejected_primary_account(Some(account_id), "tidal")
        .await;

    let (invalid, _, _, last_at) = account_row(&pool, account_id).await;
    assert_eq!(invalid, 0, "otro servicio no invalida esta cuenta");
    assert_eq!(last_at, None, "sin marca no hay sello");
}

/// Sin cuenta identificada o sin pool no hay nada que marcar: no-ops seguros.
#[tokio::test]
async fn without_account_or_db_is_a_noop() {
    let pool = setup_db().await;
    let account_id = insert_account(&pool, "qobuz").await;
    let inexistente = account_id + 1000;

    let orchestrator = DownloadOrchestrator::new().with_db(pool.clone());
    orchestrator.persist_rejected_primary_account(None, "qobuz").await;
    orchestrator
        .persist_rejected_primary_account(Some(inexistente), "qobuz")
        .await;

    // Orquestador SIN pool: el camino de producción sin BD tampoco debe panics.
    let sin_db = DownloadOrchestrator::new();
    sin_db
        .persist_rejected_primary_account(Some(account_id), "qobuz")
        .await;

    let totals: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(credentials_invalid), 0) FROM accounts",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(totals, (1, 0), "ninguna llamada marcó nada");
}
