//! D03 (#115): claim programado, reintentos con backoff, cancelación segura y
//! estado terminal de los jobs de descarga.
//!
//! La BD se migra con el conjunto real de migraciones en `sqlite::memory:` con
//! una sola conexión, como el resto de tests de descargas. Los tests ejercitan
//! el worker y las transiciones sobre filas reales, no sobre mocks.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::commands::queue::perform_retry_queue_item;
use syncify_tauri_lib::download::jobs::refresh_job_state;
use syncify_tauri_lib::download::DownloadResult;
use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

async fn migrated_pool() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory pool");
    MIGRATOR.run(&pool).await.expect("fresh migrations");
    pool
}

async fn worker(pool: &Pool<Sqlite>) -> DownloadWorker {
    DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1))
}

/// Inserta un job y sus filas. `rows` son `(status, retry_count)` y cada fila
/// usa un track id consecutivo a partir de `track_base`.
async fn seed_job(pool: &Pool<Sqlite>, job_id: i64, track_base: i64, rows: &[(&str, i64)]) {
    sqlx::query("INSERT INTO download_jobs (id, kind, requested_count) VALUES (?, 'selection', ?)")
        .bind(job_id)
        .bind(rows.len() as i64)
        .execute(pool)
        .await
        .expect("job fixture");

    for (offset, (status, retry)) in rows.iter().enumerate() {
        let queue_id = track_base + offset as i64;
        sqlx::query("INSERT INTO tracks (id, title, duration_ms) VALUES (?, ?, 180000)")
            .bind(queue_id)
            .bind(format!("Track {queue_id}"))
            .execute(pool)
            .await
            .expect("track fixture");
        sqlx::query(
            "INSERT INTO download_queue (id, track_id, status, retry_count, service_name, job_id) \
             VALUES (?, ?, ?, ?, 'qobuz', ?)",
        )
        .bind(queue_id)
        .bind(queue_id)
        .bind(*status)
        .bind(*retry)
        .bind(job_id)
        .execute(pool)
        .await
        .expect("queue fixture");
    }
}

async fn queue_row(
    pool: &Pool<Sqlite>,
    queue_id: i64,
) -> (String, i64, Option<String>, Option<String>) {
    sqlx::query_as(
        "SELECT status, retry_count, failure_class, next_attempt_at FROM download_queue WHERE id = ?",
    )
    .bind(queue_id)
    .fetch_one(pool)
    .await
    .expect("queue row")
}

fn download_result() -> DownloadResult {
    DownloadResult {
        file_path: "/tmp/syncify-d03-cancelled.flac".to_string(),
        bit_depth: 16,
        sample_rate: 44_100,
        title: "Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        release_date: None,
        track_number: 1,
        disc_number: 1,
        isrc: None,
        service: "qobuz".to_string(),
        origin_service: Some("qobuz".to_string()),
        origin_service_track_id: Some("1".to_string()),
        effective_service: Some("qobuz".to_string()),
        effective_service_track_id: Some("1".to_string()),
        fallback_reason: None,
        match_method: None,
        match_confidence: None,
        phase_timings: None,
        quality_decision: None,
        channels: Some(2),
        bitrate: Some(1411),
    }
}

#[tokio::test]
async fn transient_failure_requeues_with_backoff_and_claim_waits_for_due() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;
    let w = worker(&pool).await;

    assert!(w.claim_next_item().await.is_some());
    w.mark_failed(1, "error sending request: connection reset")
        .await;

    let (status, retry_count, failure_class, next_attempt_at) = queue_row(&pool, 1).await;
    assert_eq!(status, "queued");
    assert_eq!(retry_count, 1);
    assert_eq!(failure_class.as_deref(), Some("temporary_network_failure"));
    assert!(next_attempt_at.is_some());

    // La ventana de backoff aún no venció: el claim no debe tomarla.
    assert!(w.claim_next_item().await.is_none());

    sqlx::query(
        "UPDATE download_queue SET next_attempt_at = datetime('now', '-1 second') WHERE id = 1",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(w.claim_next_item().await.is_some());
}

#[tokio::test]
async fn transient_failure_respects_max_attempts() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;
    let w = worker(&pool).await;

    for expected_retry in [1_i64, 2] {
        assert!(w.claim_next_item().await.is_some());
        w.mark_failed(1, "error sending request: connection reset")
            .await;
        let (status, retry_count, _, _) = queue_row(&pool, 1).await;
        assert_eq!(status, "queued");
        assert_eq!(retry_count, expected_retry);
        sqlx::query(
            "UPDATE download_queue SET next_attempt_at = datetime('now', '-1 second') WHERE id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    // Tercer fallo: agotó los 3 intentos de TemporaryNetworkFailure.
    assert!(w.claim_next_item().await.is_some());
    w.mark_failed(1, "error sending request: connection reset")
        .await;
    let (status, retry_count, failure_class, next_attempt_at) = queue_row(&pool, 1).await;
    assert_eq!(status, "failed");
    assert_eq!(retry_count, 99);
    assert_eq!(failure_class.as_deref(), Some("temporary_network_failure"));
    assert!(next_attempt_at.is_none());
}

#[tokio::test]
async fn auth_failure_is_permanent_and_not_retried() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;
    let w = worker(&pool).await;

    assert!(w.claim_next_item().await.is_some());
    w.mark_failed(1, "HTTP 401 Unauthorized: token revoked")
        .await;

    let (status, retry_count, failure_class, next_attempt_at) = queue_row(&pool, 1).await;
    assert_eq!(status, "failed");
    assert_eq!(retry_count, 99);
    assert_eq!(failure_class.as_deref(), Some("auth_invalid"));
    assert!(next_attempt_at.is_none());
}

#[tokio::test]
async fn rejected_quality_is_permanent_and_not_retried() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;
    let w = worker(&pool).await;

    assert!(w.claim_next_item().await.is_some());
    w.mark_failed(1, "RejectedQuality: obtained 320 kbps instead of FLAC")
        .await;

    let (status, retry_count, failure_class, next_attempt_at) = queue_row(&pool, 1).await;
    assert_eq!(status, "failed");
    assert_eq!(retry_count, 99);
    assert_eq!(failure_class.as_deref(), Some("rejected_quality"));
    assert!(next_attempt_at.is_none());
}

#[tokio::test]
async fn cancel_between_claim_and_mark_downloading_does_not_revive() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;
    let w = worker(&pool).await;

    assert!(w.claim_next_item().await.is_some());
    sqlx::query("UPDATE download_queue SET status = 'cancelled' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    assert!(!w.mark_downloading(1).await);
    let (status, _, _, _) = queue_row(&pool, 1).await;
    assert_eq!(status, "cancelled");
}

#[tokio::test]
async fn cancelled_row_does_not_write_completion_or_ledger() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;
    let w = worker(&pool).await;

    assert!(w.claim_next_item().await.is_some());
    sqlx::query("UPDATE download_queue SET status = 'cancelled' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    assert!(!w.mark_complete(1, &download_result()).await);

    let (status, _, _, _) = queue_row(&pool, 1).await;
    assert_eq!(status, "cancelled");
    let ledger: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger, 0);
}

async fn job_status(pool: &Pool<Sqlite>, job_id: i64) -> (String, i64, i64) {
    sqlx::query_as("SELECT status, completed_count, failed_count FROM download_jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .expect("job row")
}

#[tokio::test]
async fn job_transitions_to_partial_completed_failed_and_cancelled() {
    let pool = migrated_pool().await;

    seed_job(
        &pool,
        10,
        1,
        &[("complete", 0), ("failed", 99), ("cancelled", 0)],
    )
    .await;
    refresh_job_state(&pool, 10).await.unwrap();
    assert_eq!(job_status(&pool, 10).await, ("partial".to_string(), 1, 1));

    seed_job(&pool, 11, 11, &[("complete", 0), ("complete", 0)]).await;
    refresh_job_state(&pool, 11).await.unwrap();
    assert_eq!(job_status(&pool, 11).await, ("completed".to_string(), 2, 0));

    seed_job(&pool, 12, 21, &[("failed", 99), ("failed", 99)]).await;
    refresh_job_state(&pool, 12).await.unwrap();
    assert_eq!(job_status(&pool, 12).await, ("failed".to_string(), 0, 2));

    seed_job(&pool, 13, 31, &[("cancelled", 0), ("cancelled", 0)]).await;
    refresh_job_state(&pool, 13).await.unwrap();
    assert_eq!(job_status(&pool, 13).await, ("cancelled".to_string(), 0, 0));
}

#[tokio::test]
async fn pending_row_keeps_job_running() {
    let pool = migrated_pool().await;
    seed_job(&pool, 20, 1, &[("queued", 0), ("complete", 0)]).await;
    refresh_job_state(&pool, 20).await.unwrap();
    assert_eq!(job_status(&pool, 20).await, ("running".to_string(), 1, 0));
}

#[tokio::test]
async fn manual_retry_returns_terminal_job_to_running() {
    let pool = migrated_pool().await;
    seed_job(&pool, 5, 1, &[("failed", 1)]).await;
    sqlx::query(
        "UPDATE download_queue SET error_message = 'error sending request: connection reset' WHERE id = 1",
    )
    .execute(&pool)
    .await
    .unwrap();
    refresh_job_state(&pool, 5).await.unwrap();
    assert_eq!(job_status(&pool, 5).await, ("failed".to_string(), 0, 1));

    perform_retry_queue_item(&pool, 1).await.unwrap();

    let (status, _, _, _) = queue_row(&pool, 1).await;
    assert_eq!(status, "queued");
    assert_eq!(job_status(&pool, 5).await, ("running".to_string(), 0, 0));
}

#[tokio::test]
async fn queued_row_is_claimed_after_reset_with_a_fresh_worker() {
    let pool = migrated_pool().await;
    seed_job(&pool, 7, 1, &[("queued", 0)]).await;

    let first = worker(&pool).await;
    assert!(first.claim_next_item().await.is_some());

    // Mismo reset de arranque que `DownloadWorker::run`: la fila interrumpida
    // vuelve a `queued` y el worker nuevo la reclama.
    sqlx::query(
        "UPDATE download_queue SET status = 'queued', started_at = NULL WHERE status = 'downloading'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let second = worker(&pool).await;
    assert!(second.claim_next_item().await.is_some());
}

#[tokio::test]
async fn failed_rows_with_skipped_count_are_partial() {
    let pool = migrated_pool().await;
    seed_job(&pool, 14, 41, &[("failed", 99)]).await;
    sqlx::query("UPDATE download_jobs SET skipped_count = 2 WHERE id = 14")
        .execute(&pool)
        .await
        .unwrap();

    refresh_job_state(&pool, 14).await.unwrap();

    assert_eq!(job_status(&pool, 14).await, ("partial".to_string(), 0, 1));
}
