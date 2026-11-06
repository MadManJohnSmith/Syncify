//! Caracterización del estado de un job de descarga (ADR-0022).
//!
//! Ejercita contra SQLite real con las migraciones del árbol las transiciones
//! que el worker aplica: claim con espera, fallo condicionado por estado,
//! reintento manual, recuperación única de arranque y cancelación repetida.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::download::{recover_interrupted_jobs, CancellationRegistry};
use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations apply");
    pool
}

async fn seed_track(pool: &SqlitePool, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("seed track")
}

async fn insert_job(
    pool: &SqlitePool,
    track_id: i64,
    status: &str,
    error: Option<&str>,
    retry_count: i64,
    next_attempt_at: Option<&str>,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, error_message, last_error, retry_count, next_attempt_at) VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(track_id)
    .bind(status)
    .bind(error)
    .bind(error)
    .bind(retry_count)
    .bind(next_attempt_at)
    .fetch_one(pool)
    .await
    .expect("insert queue row")
}

async fn status_of(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("row exists")
}

fn worker(pool: &SqlitePool) -> DownloadWorker {
    DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1))
}

#[tokio::test]
async fn claim_respects_next_attempt_at() {
    let pool = migrated_pool().await;
    let tid = seed_track(&pool, "claim").await;
    let due = insert_job(
        &pool,
        tid,
        "failed",
        Some("timeout"),
        1,
        Some("2000-01-01 00:00:00"),
    )
    .await;
    let future = insert_job(
        &pool,
        tid,
        "failed",
        Some("timeout"),
        1,
        Some("2999-01-01 00:00:00"),
    )
    .await;

    let w = worker(&pool);
    let claimed = w
        .claim_next_item()
        .await
        .expect("a due failed retry is claimable");
    assert_eq!(claimed.0, due);
    assert_eq!(status_of(&pool, due).await, "downloading");
    assert!(
        w.claim_next_item().await.is_none(),
        "a future next_attempt_at must not be claimed"
    );
    assert_eq!(status_of(&pool, future).await, "failed");
}

#[tokio::test]
async fn mark_failed_does_not_overwrite_cancelled_row() {
    let pool = migrated_pool().await;
    let tid = seed_track(&pool, "cancelled").await;
    let id = insert_job(&pool, tid, "cancelled", Some("cancelled by user"), 0, None).await;

    worker(&pool).mark_failed(id, "connection reset").await;

    let row: (String, Option<String>, Option<String>, i64, Option<String>) = sqlx::query_as(
        "SELECT status, error_message, failure_class, retry_count, next_attempt_at FROM download_queue WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "cancelled");
    assert_eq!(row.1.as_deref(), Some("cancelled by user"));
    assert_eq!(row.2, None);
    assert_eq!(row.3, 0);
    assert_eq!(row.4, None);
}

#[tokio::test]
async fn mark_failed_persists_class_and_retry_window() {
    let pool = migrated_pool().await;
    let tid = seed_track(&pool, "transient").await;
    let id = insert_job(&pool, tid, "downloading", None, 0, None).await;

    worker(&pool)
        .mark_failed(id, "connection reset by peer")
        .await;

    let row: (String, Option<String>, Option<String>, i64, Option<String>) = sqlx::query_as(
        "SELECT status, failure_class, next_attempt_at, retry_count, error_message FROM download_queue WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "failed");
    assert_eq!(row.1.as_deref(), Some("temporary_network_failure"));
    assert!(
        row.2.is_some(),
        "a retryable failure must schedule next_attempt_at"
    );
    assert_eq!(row.3, 1);
    assert_eq!(row.4.as_deref(), Some("connection reset by peer"));
}

#[tokio::test]
async fn manual_retry_clears_errors_only_from_failed() {
    let pool = migrated_pool().await;
    let tid = seed_track(&pool, "retry").await;
    let failed = insert_job(
        &pool,
        tid,
        "failed",
        Some("RequiresAuth: reconnect"),
        4,
        Some("2999-01-01 00:00:00"),
    )
    .await;
    sqlx::query("UPDATE download_queue SET decision_reason = 'RequiresAuth', progress_percent = 42 WHERE id = ?")
        .bind(failed)
        .execute(&pool)
        .await
        .unwrap();
    let cancelled = insert_job(&pool, tid, "cancelled", Some("user"), 2, None).await;

    syncify_tauri_lib::commands::queue::perform_retry_queue_item(&pool, failed)
        .await
        .expect("manual retry of a failed row");
    syncify_tauri_lib::commands::queue::perform_retry_queue_item(&pool, cancelled)
        .await
        .expect("manual retry is a no-op outside failed");

    let row: (String, Option<String>, Option<String>, i64, f64, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT status, error_message, last_error, retry_count, progress_percent, next_attempt_at, decision_reason FROM download_queue WHERE id = ?",
        )
        .bind(failed)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.0, "queued");
    assert_eq!(row.1, None);
    assert_eq!(row.2, None);
    assert_eq!(row.3, 0);
    assert_eq!(row.4, 0.0);
    assert_eq!(row.5, None);
    assert_eq!(row.6, None);
    assert_eq!(
        status_of(&pool, cancelled).await,
        "cancelled",
        "manual retry only transitions failed rows"
    );
}

#[tokio::test]
async fn recover_interrupted_jobs_only_touches_downloading() {
    let pool = migrated_pool().await;
    let tid = seed_track(&pool, "recover").await;
    let downloading = insert_job(&pool, tid, "downloading", None, 0, None).await;
    let failed = insert_job(&pool, tid, "failed", Some("boom"), 1, None).await;
    let cancelled = insert_job(&pool, tid, "cancelled", None, 0, None).await;

    let recovered = recover_interrupted_jobs(&pool).await;

    assert_eq!(recovered, 1);
    assert_eq!(status_of(&pool, downloading).await, "queued");
    assert_eq!(status_of(&pool, failed).await, "failed");
    assert_eq!(status_of(&pool, cancelled).await, "cancelled");
}

#[tokio::test]
async fn repeated_cancellation_is_idempotent() {
    let registry = CancellationRegistry::new();
    let token = registry.register(7);
    assert!(registry.cancel(7));
    assert!(token.is_cancelled());
    assert!(registry.cancel(7), "a second cancel is harmless");
    assert!(!registry.cancel(8), "an unknown job has nothing to abort");
    registry.release(7);
    assert!(!registry.cancel(7));

    let pool = migrated_pool().await;
    let tid = seed_track(&pool, "cancel").await;
    let id = insert_job(&pool, tid, "downloading", None, 0, None).await;
    let cancel_sql =
        "UPDATE download_queue SET status = 'cancelled' WHERE id = ? AND status IN ('queued', 'downloading')";
    let first = sqlx::query(cancel_sql)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let second = sqlx::query(cancel_sql)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(first.rows_affected(), 1);
    assert_eq!(second.rows_affected(), 0);
    assert_eq!(status_of(&pool, id).await, "cancelled");
}
