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
