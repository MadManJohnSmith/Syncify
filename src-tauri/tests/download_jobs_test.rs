//! D02 (#114): contrato de jobs de descarga — creación, estimación agregada y
//! encolado por lotes.
//!
//! La BD se migra con el conjunto real de migraciones (incluidas 0102/0103 y
//! 0104) en `sqlite::memory:` con una sola conexión, como la convención de
//! `db.rs`. El presupuesto del test de carga queda fijado aquí (spec #111).

use std::time::{Duration, Instant};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::download::jobs::{self, DownloadJobFilter, DownloadJobRequest};

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

/// 0002 already seeds qobuz/tidal/spotify/local; resolve ids by name.
async fn service_id(pool: &Pool<Sqlite>, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("service {name} not seeded"))
}

/// Inserts `count` tracks with deterministic ids `1..=count` and returns them.
async fn insert_tracks(pool: &Pool<Sqlite>, count: i64) -> Vec<i64> {
    let mut ids = Vec::with_capacity(count as usize);
    let mut next = 1_i64;
    while next <= count {
        let end = (next + 499).min(count);
        let mut sql = String::from("INSERT INTO tracks (id, title, duration_ms) VALUES ");
        let mut values = Vec::new();
        for id in next..=end {
            values.push(format!("({id}, 'Track {id}', 180000)"));
            ids.push(id);
        }
        sql.push_str(&values.join(", "));
        sqlx::query(&sql)
            .execute(pool)
            .await
            .expect("tracks fixture");
        next = end + 1;
    }
    ids
}

async fn mark_favorites(pool: &Pool<Sqlite>, ids: &[i64]) {
    for chunk in ids.chunks(500) {
        let list = chunk
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        sqlx::query(&format!(
            "UPDATE tracks SET is_favorite = 1 WHERE id IN ({list})"
        ))
        .execute(pool)
        .await
        .expect("favorites fixture");
    }
}

async fn insert_sources(
    pool: &Pool<Sqlite>,
    sources: &[(i64, i64, Option<&str>, Option<i64>, Option<i64>)],
) {
    for chunk in sources.chunks(400) {
        let mut sql = String::from(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES ",
        );
        let values = chunk
            .iter()
            .map(|(track_id, service_id, format, bit_depth, sample_rate)| {
                format!(
                    "({track_id}, {service_id}, 'st-{track_id}-{service_id}', {}, {}, {}, 1)",
                    format
                        .map(|f| format!("'{f}'"))
                        .unwrap_or_else(|| "NULL".into()),
                    bit_depth
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "NULL".into()),
                    sample_rate
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "NULL".into()),
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&values);
        sqlx::query(&sql)
            .execute(pool)
            .await
            .expect("track_sources fixture");
    }
}

fn base_request(kind: &str) -> DownloadJobRequest {
    DownloadJobRequest {
        kind: kind.to_string(),
        label: None,
        source_service: None,
        destination_service: None,
        preferred_account_id: None,
        quality_preference: None,
        allow_fallback: Some(true),
        priority: None,
        limit: None,
        filter: DownloadJobFilter::default(),
    }
}

fn favorites_request() -> DownloadJobRequest {
    base_request("favorites")
}

fn selection_request(ids: Vec<i64>) -> DownloadJobRequest {
    let mut req = base_request("selection");
    req.filter.track_ids = Some(ids);
    req
}

// ── 1. Job vacío ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn empty_job_estimates_zero_and_keeps_initial_state() {
    let pool = migrated_pool().await;
    let req = favorites_request();

    let estimate = jobs::estimate(&pool, &req).await.expect("estimate");
    assert_eq!(estimate.total, 0);
    assert_eq!(estimate.estimated_size_mb, 0);
    assert!(estimate.by_source.is_empty());

    let job_id = jobs::create_job(&pool, &req).await.expect("create job");
    let outcome = jobs::run_job(&pool, job_id, &req).await.expect("run job");
    assert_eq!(outcome.enqueued, 0);

    let job = jobs::get_job(&pool, job_id).await.expect("job row");
    assert_eq!(job.status, "running", "el estado inicial es running");
    assert_eq!(job.requested_count, 0);
    assert_eq!(job.estimated_count, 0);
    assert_eq!(job.skipped_count, 0);
}

// ── 2. Estimación agregada por fuente, destino y capacidad ───────────────────

#[tokio::test]
async fn estimate_counts_by_source_destination_and_capacity() {
    let pool = migrated_pool().await;
    let qobuz = service_id(&pool, "qobuz").await;
    let tidal = service_id(&pool, "tidal").await;
    let ids = insert_tracks(&pool, 3).await;
    insert_sources(
        &pool,
        &[
            (ids[0], qobuz, Some("FLAC"), Some(16), Some(44100)), // lossless
            (ids[1], tidal, Some("FLAC"), Some(24), Some(96000)), // hires
                                                                  // ids[2] sin fuentes: unknown
        ],
    )
    .await;

    let estimate = jobs::estimate(&pool, &selection_request(ids.clone()))
        .await
        .expect("estimate");

    assert_eq!(estimate.total, 3);
    assert_eq!(estimate.by_capacity.hires, 1);
    assert_eq!(estimate.by_capacity.lossless, 1);
    assert_eq!(estimate.by_capacity.unknown, 1);
    assert_eq!(estimate.by_capacity.high, 0);
    assert_eq!(estimate.by_capacity.lossy, 0);
    assert_eq!(
        estimate.estimated_size_mb,
        2 * jobs::SIZE_LOSSLESS_MB,
        "hires y lossless cuentan como FLAC"
    );

    let qobuz = estimate
        .by_source
        .iter()
        .find(|s| s.service == "qobuz")
        .expect("by_source qobuz");
    assert_eq!(qobuz.count, 1);
    let tidal = estimate
        .by_source
        .iter()
        .find(|s| s.service == "tidal")
        .expect("by_source tidal");
    assert_eq!(tidal.count, 1);

    // Destinos: cada candidato es alcanzable por su propia fuente.
    assert_eq!(
        estimate
            .by_destination
            .iter()
            .find(|s| s.service == "qobuz")
            .map(|s| s.count),
        Some(1)
    );
    assert_eq!(
        estimate
            .by_destination
            .iter()
            .find(|s| s.service == "tidal")
            .map(|s| s.count),
        Some(1)
    );
}

// ── 3. Límite ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn favorites_job_respects_limit() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 5).await;
    mark_favorites(&pool, &ids).await;

    let mut req = favorites_request();
    req.limit = Some(2);

    let job_id = jobs::create_job(&pool, &req).await.expect("create job");
    let outcome = jobs::run_job(&pool, job_id, &req).await.expect("run job");
    assert_eq!(outcome.enqueued, 2);

    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE job_id = ?")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .expect("queue count");
    assert_eq!(queued, 2);

    let job = jobs::get_job(&pool, job_id).await.expect("job row");
    assert_eq!(job.requested_count, 2);
    assert_eq!(job.estimated_count, 2);
}

// ── 4. Filtro de fuente ──────────────────────────────────────────────────────

#[tokio::test]
async fn favorites_job_filters_by_source_service() {
    let pool = migrated_pool().await;
    let qobuz = service_id(&pool, "qobuz").await;
    let tidal = service_id(&pool, "tidal").await;
    let ids = insert_tracks(&pool, 3).await;
    mark_favorites(&pool, &ids).await;
    insert_sources(
        &pool,
        &[
            (ids[0], qobuz, Some("FLAC"), Some(16), Some(44100)),
            (ids[1], qobuz, Some("FLAC"), Some(16), Some(44100)),
            (ids[2], tidal, Some("FLAC"), Some(16), Some(44100)),
        ],
    )
    .await;

    let mut req = favorites_request();
    req.source_service = Some("qobuz".into());

    let job_id = jobs::create_job(&pool, &req).await.expect("create job");
    let outcome = jobs::run_job(&pool, job_id, &req).await.expect("run job");
    assert_eq!(outcome.enqueued, 2, "solo las fuentes qobuz");

    let job = jobs::get_job(&pool, job_id).await.expect("job row");
    assert_eq!(job.requested_count, 2);
    assert_eq!(
        job.skip_breakdown_json.as_deref(),
        Some(r#"{"already_queued":0,"duplicate_input":0}"#)
    );
}

// ── 5. Dedupe ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn repeated_enqueue_does_not_duplicate_rows_or_positions() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let req = selection_request(ids.clone());

    let job_a = jobs::create_job(&pool, &req).await.expect("job a");
    let outcome_a = jobs::run_job(&pool, job_a, &req).await.expect("run a");
    assert_eq!(outcome_a.enqueued, 2);

    // Segundo job con los mismos candidatos: no duplica filas.
    let job_b = jobs::create_job(&pool, &req).await.expect("job b");
    let outcome_b = jobs::run_job(&pool, job_b, &req).await.expect("run b");
    assert_eq!(outcome_b.enqueued, 0);
    assert_eq!(outcome_b.skipped_existing, 2);

    let job_b_row = jobs::get_job(&pool, job_b).await.expect("job b row");
    assert_eq!(job_b_row.requested_count, 2);
    assert_eq!(job_b_row.skipped_count, 2);

    // Repetir el encolado del mismo job es idempotente.
    let again = jobs::enqueue_candidates(&pool, job_a, &ids, 50, None, true)
        .await
        .expect("repeat enqueue");
    assert_eq!(again.enqueued, 0);

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .expect("queue count");
    assert_eq!(rows, 2, "ni filas ni posiciones duplicadas");

    let distinct_positions: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT position) FROM download_queue")
            .fetch_one(&pool)
            .await
            .expect("positions");
    assert_eq!(distinct_positions, 2);
}

// ── 6. Error de BD ───────────────────────────────────────────────────────────

#[tokio::test]
async fn database_error_is_returned_not_swallowed() {
    let pool = migrated_pool().await;
    pool.close().await;
    let req = favorites_request();
    let result = jobs::estimate(&pool, &req).await;
    assert!(result.is_err(), "un pool cerrado debe devolver error");
}

// ── 7. Test de carga: 30.000 candidatos ──────────────────────────────────────

#[tokio::test]
async fn load_30000_candidates_estimate_and_enqueue_within_budget() {
    // Presupuesto fijado por este test (spec #111): 30.000 candidatos se
    // estiman y encolan en menos de 20 s en el runner de CI más lento.
    const BUDGET: Duration = Duration::from_secs(20);

    let pool = migrated_pool().await;
    let qobuz = service_id(&pool, "qobuz").await;
    let ids = insert_tracks(&pool, 30_000).await;
    let sources: Vec<(i64, i64, Option<&str>, Option<i64>, Option<i64>)> = ids
        .iter()
        .map(|id| (*id, qobuz, Some("FLAC"), Some(16_i64), Some(44100_i64)))
        .collect();
    insert_sources(&pool, &sources).await;

    let req = selection_request(ids.clone());
    let started = Instant::now();
    let estimate = jobs::estimate(&pool, &req).await.expect("estimate");
    let job_id = jobs::create_job(&pool, &req).await.expect("create job");
    let outcome = jobs::run_job(&pool, job_id, &req).await.expect("run job");
    let elapsed = started.elapsed();

    assert_eq!(estimate.total, 30_000);
    assert_eq!(outcome.enqueued, 30_000);
    assert!(
        elapsed < BUDGET,
        "30k candidatos tardaron {elapsed:?}, presupuesto {BUDGET:?}"
    );

    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE job_id = ?")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .expect("queue count");
    assert_eq!(queued, 30_000);

    let job = jobs::get_job(&pool, job_id).await.expect("job row");
    assert_eq!(job.requested_count, 30_000);
    assert_eq!(job.estimated_count, 30_000);
}

// ── 8. D07 (#119): payload del evento y ciclo del job ────────────────────────
//
// El payload de `syncify:download_job_progress` y el estado del job se
// recalculan desde la cola; aquí se fija el contrato que consumirá D08.

async fn set_queue_status(pool: &Pool<Sqlite>, track_id: i64, status: &str) {
    sqlx::query("UPDATE download_queue SET status = ? WHERE track_id = ? AND status NOT IN ('complete', 'cancelled')")
        .bind(status)
        .bind(track_id)
        .execute(pool)
        .await
        .expect("queue status fixture");
}

async fn enqueue_job(pool: &Pool<Sqlite>, req: &DownloadJobRequest) -> (i64, Vec<i64>) {
    let job_id = jobs::create_job(pool, req).await.expect("create job");
    let outcome = jobs::run_job(pool, job_id, req).await.expect("run job");
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT track_id FROM download_queue WHERE job_id = ? ORDER BY track_id",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .expect("queue rows");
    assert_eq!(outcome.enqueued, ids.len() as i64);
    (job_id, ids)
}

#[tokio::test]
async fn progress_snapshot_matches_the_event_contract() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 3).await;
    let mut req = selection_request(ids.clone());
    req.label = Some("Contrato D07".into());

    let (job_id, rows) = enqueue_job(&pool, &req).await;
    assert_eq!(rows.len(), 3);

    // Recién creado: en cola, sin terminales, sin porcentaje.
    let snap = jobs::progress_snapshot(&pool, job_id)
        .await
        .expect("snapshot")
        .expect("job exists");
    assert_eq!(snap.job_id, job_id);
    assert_eq!(snap.label.as_deref(), Some("Contrato D07"));
    assert_eq!(snap.status, "running");
    assert_eq!(snap.total, 3);
    assert_eq!(snap.queued, 3);
    assert_eq!(snap.downloading, 0);
    assert_eq!(snap.completed, 0);
    assert_eq!(snap.failed, 0);
    assert_eq!(snap.cancelled, 0);
    assert_eq!(snap.skipped, 0);
    assert!((snap.percent - 0.0).abs() < f64::EPSILON);
    assert!(!snap.terminal);

    // Descarga en vuelo: cuenta como downloading, no como terminada.
    set_queue_status(&pool, rows[0], "downloading").await;
    let snap = jobs::progress_snapshot(&pool, job_id)
        .await
        .expect("snapshot")
        .expect("job exists");
    assert_eq!(snap.downloading, 1);
    assert_eq!(snap.queued, 2);
    assert!(!snap.terminal);

    // Un completo y un fallido: el porcentaje avanza con los terminales.
    set_queue_status(&pool, rows[1], "complete").await;
    set_queue_status(&pool, rows[2], "failed").await;
    let snap = jobs::progress_snapshot(&pool, job_id)
        .await
        .expect("snapshot")
        .expect("job exists");
    assert_eq!(snap.completed, 1);
    assert_eq!(snap.failed, 1);
    assert!((snap.percent - (2.0 / 3.0 * 100.0)).abs() < 1e-9);

    // Sin nada activo el refresco deriva terminal, y el payload lo declara.
    set_queue_status(&pool, rows[0], "complete").await;
    let job = jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("refresh")
        .expect("job exists");
    assert_eq!(job.status, "partial");
    let snap = jobs::progress_snapshot(&pool, job_id)
        .await
        .expect("snapshot")
        .expect("job exists");
    assert_eq!(snap.status, "partial");
    assert!(snap.terminal);
    assert!((snap.percent - 100.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn refresh_derives_completed_failed_partial_and_cancelled() {
    // Todo completo → completed.
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let (job_id, rows) = enqueue_job(&pool, &selection_request(ids.clone())).await;
    for id in &rows {
        set_queue_status(&pool, *id, "complete").await;
    }
    let job = jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "completed");
    assert_eq!(job.completed_count, 2);
    assert_eq!(job.failed_count, 0);
    assert!(job.completed_at.is_some());

    // Todo fallido → failed.
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let (job_id, rows) = enqueue_job(&pool, &selection_request(ids.clone())).await;
    for id in &rows {
        set_queue_status(&pool, *id, "failed").await;
    }
    let job = jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "failed");
    assert_eq!(job.failed_count, 2);

    // Mezcla → partial.
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let (job_id, rows) = enqueue_job(&pool, &selection_request(ids.clone())).await;
    set_queue_status(&pool, rows[0], "complete").await;
    set_queue_status(&pool, rows[1], "failed").await;
    let job = jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "partial");

    // Solo canceladas → cancelled.
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let (job_id, rows) = enqueue_job(&pool, &selection_request(ids.clone())).await;
    for id in &rows {
        set_queue_status(&pool, *id, "cancelled").await;
    }
    let job = jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "cancelled");
}

#[tokio::test]
async fn cancel_job_cancels_only_pending_rows_and_keeps_the_active_one() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 3).await;
    let (job_id, rows) = enqueue_job(&pool, &selection_request(ids.clone())).await;
    set_queue_status(&pool, rows[0], "downloading").await;

    let outcome = jobs::cancel_job(&pool, job_id)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(outcome.cancelled_rows, 2, "solo las pendientes");

    // La descarga activa no se toca: sigue downloading y el job sigue vivo.
    let still_downloading: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM download_queue WHERE job_id = ? AND status = 'downloading'",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .expect("active row");
    assert_eq!(still_downloading, 1);
    assert_eq!(outcome.job.status, "running");

    // Terminada la activa: completada + canceladas → partial.
    set_queue_status(&pool, rows[0], "complete").await;
    let job = jobs::refresh_job_status(&pool, job_id, None)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "partial");
    assert_eq!(job.completed_count, 1);
}

#[tokio::test]
async fn cancel_before_enqueue_blocks_the_background_enqueue() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let req = selection_request(ids.clone());

    let job_id = jobs::create_job(&pool, &req).await.expect("create job");
    let outcome = jobs::cancel_job(&pool, job_id)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(outcome.cancelled_rows, 0);
    assert_eq!(
        outcome.job.status, "cancelled",
        "sin filas, el job acaba ya"
    );

    // El encolado en curso no inserta nada (guardia en enqueue_candidates).
    let enqueued = jobs::enqueue_candidates(&pool, job_id, &ids, 50, None, true)
        .await
        .expect("enqueue after cancel");
    assert_eq!(enqueued.enqueued, 0);

    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE job_id = ?")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .expect("queue count");
    assert_eq!(queued, 0);

    // Y el refresco posterior al encolado no resurrecta el job.
    let job = jobs::refresh_job_status(&pool, job_id, Some(true))
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "cancelled");
}

#[tokio::test]
async fn retry_job_failed_requeues_rows_and_reopens_the_job() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 2).await;
    let (job_id, rows) = enqueue_job(&pool, &selection_request(ids.clone())).await;
    for id in &rows {
        set_queue_status(&pool, *id, "failed").await;
    }
    sqlx::query("UPDATE download_queue SET retry_count = 3, next_attempt_at = datetime('now', '+30 seconds') WHERE job_id = ?")
        .bind(job_id)
        .execute(&pool)
        .await
        .expect("retry fixture");
    jobs::mark_job_failed(&pool, job_id, "boom")
        .await
        .expect("mark failed");

    let job = jobs::retry_job_failed(&pool, job_id)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "running", "el job se reabre");
    assert!(job.error.is_none());

    let cleaned: (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(retry_count), 0), COUNT(next_attempt_at) \
         FROM download_queue WHERE job_id = ? AND status = 'queued'",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .expect("requeued rows");
    let (queued, retries, scheduled) = cleaned;
    assert_eq!(queued, 2);
    assert_eq!(retries, 0, "el reintento manual empieza de cero");
    assert_eq!(scheduled, 0, "sin espera programada");

    // Sin filas fallidas no toca el job: uno completado sigue completado.
    let pool2 = migrated_pool().await;
    let ids2 = insert_tracks(&pool2, 1).await;
    let (job2, rows2) = enqueue_job(&pool2, &selection_request(ids2.clone())).await;
    set_queue_status(&pool2, rows2[0], "complete").await;
    jobs::refresh_job_status(&pool2, job2, None)
        .await
        .expect("expect D07");
    let job = jobs::retry_job_failed(&pool2, job2)
        .await
        .expect("expect D07")
        .expect("expect D07");
    assert_eq!(job.status, "completed", "sin fallos no se reabre");
}

#[tokio::test]
async fn job_account_override_does_not_rewrite_service_preference() {
    let pool = migrated_pool().await;
    let qobuz = service_id(&pool, "qobuz").await;

    sqlx::query(
        "INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active)
         VALUES (501, ?, 'Default Qobuz', 'default@qobuz.test', 1),
                (502, ?, 'Override Qobuz', 'override@qobuz.test', 1)",
    )
    .bind(qobuz)
    .bind(qobuz)
    .execute(&pool)
    .await
    .expect("accounts fixture");
    sqlx::query(
        "INSERT INTO service_preferred_accounts (service_name, account_id) VALUES ('qobuz', 501)",
    )
    .execute(&pool)
    .await
    .expect("service preference fixture");

    let ids = insert_tracks(&pool, 1).await;
    insert_sources(
        &pool,
        &[(ids[0], qobuz, Some("FLAC"), Some(16), Some(44100))],
    )
    .await;

    let mut req = selection_request(ids);
    req.destination_service = Some("qobuz".to_string());
    req.preferred_account_id = Some(502);
    let job = jobs::create_job(&pool, &req).await.expect("create job");
    jobs::run_job(&pool, job, &req).await.expect("run job");

    let stored = jobs::get_job(&pool, job).await.expect("job row");
    assert_eq!(stored.preferred_account_id, Some(502));
    let service_default: i64 = sqlx::query_scalar(
        "SELECT account_id FROM service_preferred_accounts WHERE service_name = 'qobuz'",
    )
    .fetch_one(&pool)
    .await
    .expect("service preference survives");
    assert_eq!(
        service_default, 501,
        "a job override must not rewrite the global service preference"
    );
}

#[tokio::test]
async fn clear_terminal_jobs_keeps_running_ones() {
    let pool = migrated_pool().await;
    let ids = insert_tracks(&pool, 4).await;

    // Selecciones disjuntas: deduplicar la cola no debe interferir.
    let (terminal_a, _) = enqueue_job(&pool, &selection_request(vec![ids[0], ids[1]])).await;
    jobs::mark_job_failed(&pool, terminal_a, "boom")
        .await
        .expect("expect D07");
    let (running, _) = enqueue_job(&pool, &selection_request(vec![ids[2], ids[3]])).await;

    let (deleted, pruned) = jobs::clear_terminal_jobs(&pool).await.expect("clear");
    assert_eq!(deleted, 1);
    assert_eq!(pruned, vec![terminal_a]);

    let job = jobs::get_job(&pool, running).await.expect("running job");
    assert_eq!(job.status, "running");
    assert!(
        jobs::get_job(&pool, terminal_a).await.is_err(),
        "el terminal cayó"
    );
}
