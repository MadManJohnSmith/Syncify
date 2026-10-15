//! WF-RES-01: transiciones de estado de la cola que la mejora de fiabilidad
//! debe respetar.
//!
//! Cubre dos defensas sobre la base real de migraciones:
//!
//! 1. `mark_complete` solo promueve a `complete` una fila que el worker sigue
//!    poseyendo como `downloading`. Cancelar mientras la transferencia está en
//!    vuelo debe dejar la fila en `cancelled` aunque el resultado ya esté listo.
//! 2. El rechazo de credenciales es un veredicto sobre la CUENTA, no sobre el
//!    intento: cuando una cuenta hermana completa la pista, el rechazo de la
//!    cuenta principal se persiste para que el orden de fallback no la vuelva a
//!    poner delante en la siguiente pista del mismo servicio.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::DownloadResult;
use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};
use tempfile::TempDir;

async fn migrated_pool(dir: &TempDir, name: &str) -> sqlx::SqlitePool {
    let db_path = dir.path().join(name);
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("connect test db");
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("migrations");
    pool
}

async fn status_of(pool: &sqlx::SqlitePool, queue_id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(pool)
        .await
        .expect("queue row exists")
}

/// Cancelar con el resultado ya descargado no puede escribir `complete`.
#[tokio::test]
async fn in_flight_cancellation_is_not_overwritten_by_completion() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "cancel_race.db").await;

    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'qobuz') ON CONFLICT(id) DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Audited Artist')")
        .execute(&pool)
        .await
        .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Cancelled Race', 200000, 'USAAA2000001') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let queue_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, progress_percent) VALUES (?, 'downloading', 50.0) RETURNING id",
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    // El usuario cancela mientras la transferencia está en vuelo.
    sqlx::query("UPDATE download_queue SET status = 'cancelled' WHERE id = ?")
        .bind(queue_id)
        .execute(&pool)
        .await
        .unwrap();

    let res = DownloadResult {
        file_path: dir
            .path()
            .join("cancelled_race.flac")
            .to_string_lossy()
            .to_string(),
        title: "Cancelled Race".to_string(),
        artist: "Audited Artist".to_string(),
        service: "qobuz".to_string(),
        ..Default::default()
    };
    worker.mark_complete(queue_id, &res).await;

    assert_eq!(
        status_of(&pool, queue_id).await,
        "cancelled",
        "la cancelación del usuario debe ganar sobre la promoción"
    );
}

/// Una fila que el worker sigue poseyendo sí se promueve, y no se duplica el
/// registro en `downloads` cuando la promoción gana exactamente una vez.
#[tokio::test]
async fn owned_row_is_promoted_once() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "promote_once.db").await;

    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'qobuz') ON CONFLICT(id) DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Owned Row', 200000, 'USAAA2000002') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let queue_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, progress_percent) VALUES (?, 'downloading', 90.0) RETURNING id",
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));
    let res = DownloadResult {
        file_path: dir
            .path()
            .join("owned_row.flac")
            .to_string_lossy()
            .to_string(),
        title: "Owned Row".to_string(),
        service: "qobuz".to_string(),
        ..Default::default()
    };
    worker.mark_complete(queue_id, &res).await;

    assert_eq!(status_of(&pool, queue_id).await, "complete");
    let ledger_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        ledger_rows, 1,
        "una pista completa genera un registro único"
    );
}

/// El rechazo de la cuenta primaria persiste aunque una hermana complete la pista.
#[tokio::test]
async fn sibling_success_persists_rejected_primary_account() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "rejected_account.db").await;

    // Las migraciones siembran `services` sin IDs fijos: se resuelven por nombre.
    let qobuz_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tidal_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, is_active) VALUES (10, ?, 1), (11, ?, 1)")
        .bind(qobuz_id)
        .bind(tidal_id)
        .execute(&pool)
        .await
        .unwrap();

    let orchestrator = DownloadOrchestrator::new().with_db(pool.clone());
    orchestrator
        .persist_rejected_primary_account(Some(10), "qobuz")
        .await;

    let rejected: i64 =
        sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rejected, 1, "la cuenta rechazada queda invalidada");

    let untouched: i64 =
        sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE id = 11")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        untouched, 0,
        "una cuenta de otro servicio no se invalida por este camino"
    );

    // El veredicto no se revierte ni se reescribe al marcarla con otro servicio.
    orchestrator
        .persist_rejected_primary_account(Some(10), "tidal")
        .await;
    let after_mismatch: i64 =
        sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after_mismatch, 1);

    // Sin cuenta previa no hay identidad que excluir.
    orchestrator
        .persist_rejected_primary_account(None, "qobuz")
        .await;
    let totals: (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(credentials_invalid), 0) FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(totals, (2, 1));
}
