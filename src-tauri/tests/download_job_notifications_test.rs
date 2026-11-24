//! D10 (#122): notificaciones de job terminal, reintento acotado y limpieza.
//!
//! Cubre los cuatro estados terminales (`completed`, `partial`, `failed`,
//! `cancelled`), la no-emisión durante `running`, la deduplicación por
//! transición, el reintento de solo filas `failed` y la limpieza de jobs con
//! sus filas de cola sin tocar `downloads` ni `operation_journal`.

use std::sync::Mutex;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::download::job_events;
use syncify_tauri_lib::download::jobs::{self, DownloadJobFilter, DownloadJobRequest};

/// La deduplicación de notificaciones es global al proceso; los tests que la
/// tocan se serializan para no pisarse entre sí.
static NOTIFICATION_STATE: Mutex<()> = Mutex::new(());

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

async fn insert_tracks(pool: &Pool<Sqlite>, count: i64) -> Vec<i64> {
    let mut ids = Vec::with_capacity(count as usize);
    for index in 1..=count {
        let id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("D10 Track {index}"))
            .fetch_one(pool)
            .await
            .expect("track fixture");
        ids.push(id);
    }
    ids
}

fn selection_request(label: &str, track_ids: Vec<i64>) -> DownloadJobRequest {
    DownloadJobRequest {
        kind: "selection".to_string(),
        label: Some(label.to_string()),
        filter: DownloadJobFilter {
            track_ids: Some(track_ids),
            ..Default::default()
        },
        ..Default::default()
    }
}

async fn enqueue_job(pool: &Pool<Sqlite>, label: &str, track_ids: Vec<i64>) -> (i64, Vec<i64>) {
    let request = selection_request(label, track_ids);
    let job_id = jobs::create_job(pool, &request).await.expect("create job");
    jobs::run_job(pool, job_id, &request)
        .await
        .expect("enqueue job");
    let rows: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM download_queue WHERE job_id = ? ORDER BY id")
            .bind(job_id)
            .fetch_all(pool)
            .await
            .expect("queue rows");
    (job_id, rows)
}

async fn set_row_status(pool: &Pool<Sqlite>, queue_id: i64, status: &str) {
    sqlx::query("UPDATE download_queue SET status = ? WHERE id = ?")
        .bind(status)
        .bind(queue_id)
        .execute(pool)
        .await
        .expect("queue status fixture");
}

async fn terminal_job(
    pool: &Pool<Sqlite>,
    label: &str,
    tracks: Vec<i64>,
    statuses: &[&str],
) -> i64 {
    let (job_id, rows) = enqueue_job(pool, label, tracks).await;
    for (row, status) in rows.iter().zip(statuses.iter()) {
        set_row_status(pool, *row, status).await;
    }
    jobs::refresh_job_status(pool, job_id, None)
        .await
        .expect("refresh job")
        .expect("job exists");
    job_id
}

async fn snapshot(
    pool: &Pool<Sqlite>,
    job_id: i64,
) -> syncify_tauri_lib::download::jobs::DownloadJobProgressEvent {
    jobs::progress_snapshot(pool, job_id)
        .await
        .expect("snapshot")
        .expect("job exists")
}

#[tokio::test]
async fn four_terminal_states_produce_a_job_summary() {
    let _guard = NOTIFICATION_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let pool = migrated_pool().await;
    let tracks = insert_tracks(&pool, 8).await;

    let cases: [(&str, &[&str]); 4] = [
        ("D10 completed", &["complete", "complete"]),
        ("D10 partial", &["complete", "failed"]),
        ("D10 failed", &["failed", "failed"]),
        ("D10 cancelled", &["cancelled", "cancelled"]),
    ];

    for (index, (label, statuses)) in cases.into_iter().enumerate() {
        let start = index * 2;
        let job_id = terminal_job(&pool, label, tracks[start..start + 2].to_vec(), statuses).await;
        let event = snapshot(&pool, job_id).await;
        assert!(event.terminal, "{label} must be terminal");

        job_events::forget_terminal_notification(job_id);
        let (title, body) =
            job_events::take_terminal_notification(&event).expect("terminal state notifies");
        assert!(
            title.contains(label),
            "the summary keeps the label: {title}"
        );
        assert!(
            body.contains(&format!("{} completadas", event.completed)),
            "{body}"
        );
        assert!(
            body.contains(&format!("{} fallidas", event.failed)),
            "{body}"
        );
        assert!(
            body.contains(&format!("{} canceladas", event.cancelled)),
            "{body}"
        );
    }
}

#[tokio::test]
async fn running_jobs_do_not_notify_and_each_terminal_state_is_notified_once() {
    let _guard = NOTIFICATION_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let pool = migrated_pool().await;
    let tracks = insert_tracks(&pool, 2).await;
    let (job_id, rows) = enqueue_job(&pool, "D10 running", tracks).await;
    job_events::forget_terminal_notification(job_id);

    let running = snapshot(&pool, job_id).await;
    assert!(!running.terminal);
    assert!(job_events::take_terminal_notification(&running).is_none());

    for row in &rows {
        set_row_status(&pool, *row, "complete").await;
    }
    jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("refresh")
        .expect("job exists");
    let completed = snapshot(&pool, job_id).await;

    assert!(job_events::take_terminal_notification(&completed).is_some());
    assert!(
        job_events::take_terminal_notification(&completed).is_none(),
        "the same terminal status is not notified twice"
    );
}

#[tokio::test]
async fn retry_requeues_only_failed_rows_and_arms_a_new_transition() {
    let _guard = NOTIFICATION_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let pool = migrated_pool().await;
    let tracks = insert_tracks(&pool, 3).await;
    let (job_id, rows) = enqueue_job(&pool, "D10 retry", tracks).await;
    job_events::forget_terminal_notification(job_id);

    // Una fila completa, otra fallida y otra cancelada: el reintento solo debe
    // tocar la fallida y recalcular el estado desde la cola.
    set_row_status(&pool, rows[0], "complete").await;
    set_row_status(&pool, rows[1], "failed").await;
    set_row_status(&pool, rows[2], "cancelled").await;
    jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("refresh")
        .expect("job exists");
    let partial = snapshot(&pool, job_id).await;
    assert_eq!(partial.status, "partial");
    assert!(job_events::take_terminal_notification(&partial).is_some());

    let retried = jobs::retry_job_failed(&pool, job_id)
        .await
        .expect("retry")
        .expect("job exists");
    assert_eq!(retried.status, "running", "the job reopens");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM download_queue WHERE job_id = ? AND status = 'queued'",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .expect("requeued"),
        1,
        "only the failed row is requeued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM download_queue WHERE job_id = ? AND status = 'complete'",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .expect("completed"),
        1,
        "completed rows are untouched"
    );

    let running = snapshot(&pool, job_id).await;
    assert!(!running.terminal);
    assert!(
        job_events::take_terminal_notification(&running).is_none(),
        "running resets the notification window without notifying"
    );

    // El reintento falla de nuevo: es una transición nueva y vuelve a notificar.
    sqlx::query(
        "UPDATE download_queue SET status = 'failed' WHERE job_id = ? AND status = 'queued'",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .expect("fail again");
    jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("refresh")
        .expect("job exists");
    let failed_again = snapshot(&pool, job_id).await;
    assert_eq!(failed_again.status, "partial");
    assert!(
        job_events::take_terminal_notification(&failed_again).is_some(),
        "the retry arms a new terminal transition"
    );
}

#[tokio::test]
async fn clear_terminal_jobs_removes_queue_rows_but_keeps_the_ledgers() {
    let pool = migrated_pool().await;
    let tracks = insert_tracks(&pool, 5).await;

    let terminal = terminal_job(
        &pool,
        "D10 clear terminal",
        tracks[0..3].to_vec(),
        &["complete", "failed", "cancelled"],
    )
    .await;
    let (running, running_rows) =
        enqueue_job(&pool, "D10 clear running", tracks[3..5].to_vec()).await;

    // Ledger de descargas y auditoría de operaciones: no deben caer con el job.
    sqlx::query("INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'FLAC')")
        .bind(tracks[0])
        .bind("/tmp/d10-clear/kept.flac")
        .execute(&pool)
        .await
        .expect("downloads ledger fixture");
    sqlx::query(
        "INSERT INTO operation_journal (operation_id, operation_type, phase, status) \
         VALUES ('d10-clear-op', 'download', 'complete', 'completed')",
    )
    .execute(&pool)
    .await
    .expect("operation journal fixture");

    let (deleted, pruned) = jobs::clear_terminal_jobs(&pool).await.expect("clear");
    assert_eq!(deleted, 1);
    assert_eq!(pruned, vec![terminal]);

    assert!(
        jobs::get_job(&pool, terminal).await.is_err(),
        "the terminal job is gone"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM download_queue WHERE job_id = ?")
            .bind(terminal)
            .fetch_one(&pool)
            .await
            .expect("terminal rows"),
        0,
        "the terminal job's queue rows are gone"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM download_queue WHERE job_id = ?")
            .bind(running)
            .fetch_one(&pool)
            .await
            .expect("running rows"),
        running_rows.len() as i64,
        "a running job keeps its queue rows"
    );
    assert_eq!(
        jobs::get_job(&pool, running)
            .await
            .expect("running job")
            .status,
        "running"
    );

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM downloads")
            .fetch_one(&pool)
            .await
            .expect("downloads"),
        1,
        "the downloads ledger survives"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM operation_journal")
            .fetch_one(&pool)
            .await
            .expect("journal"),
        1,
        "the operation journal survives"
    );
}
