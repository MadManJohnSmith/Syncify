//! Jobs de descarga (D02, spec #111): contrato de creación, estimación
//! agregada y encolado por lotes sobre la cola persistente.
//!
//! La cola (`download_queue`) sigue siendo la fuente de verdad; el job es una
//! agrupación con contadores. Este módulo no ejecuta descargas ni recalcula el
//! estado terminal de un job: eso llega con el worker en tickets posteriores.
//!
//! Regla de carga (spec #111): la estimación y el encolado no hacen consultas
//! por pista. Todo se resuelve con agregados y bloques de `IN`/`VALUES`.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::{QueryBuilder, Row, Sqlite};

/// Filas por bloque de INSERT. Mantiene los parámetros muy por debajo del
/// límite de SQLite y hace el encolado de 30.000 candidatos lineal en bloques.
pub const ENQUEUE_BATCH_SIZE: usize = 500;

/// Tamaño medio asumido por pista según la mejor capacidad disponible.
/// Aproximación heredada del preflight de favoritos (35 MB por FLAC); las
/// pérdidas usan una cota menor para no inflar la estimación.
pub const SIZE_LOSSLESS_MB: i64 = 35;
pub const SIZE_HIGH_MB: i64 = 10;
pub const SIZE_LOSSY_MB: i64 = 5;

/// Kinds válidos de `download_jobs`. `url` se crea desde `import_from_url`,
/// no desde `create_download_job`.
pub const JOB_KINDS: [&str; 7] = [
    "favorites",
    "album",
    "artist",
    "playlist",
    "selection",
    "url",
    "single",
];

const ADAPTER_SERVICES: [&str; 5] = ["qobuz", "tidal", "amazon", "deezer", "soundcloud"];

#[derive(Debug)]
pub enum JobError {
    Db(sqlx::Error),
    Invalid(String),
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobError::Db(e) => write!(f, "database error: {e}"),
            JobError::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for JobError {}

impl From<sqlx::Error> for JobError {
    fn from(value: sqlx::Error) -> Self {
        JobError::Db(value)
    }
}

/// Filtro de candidatos. Cada kind usa el campo que le corresponde.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJobFilter {
    pub track_ids: Option<Vec<i64>>,
    pub album_id: Option<i64>,
    pub artist_id: Option<i64>,
    pub playlist_id: Option<i64>,
    pub track_id: Option<i64>,
    /// Solo para `kind = favorites`: `tracks` | `albums` | `artists` | `all`.
    pub item_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJobRequest {
    pub kind: String,
    pub label: Option<String>,
    pub source_service: Option<String>,
    pub destination_service: Option<String>,
    pub preferred_account_id: Option<i64>,
    pub quality_preference: Option<String>,
    pub allow_fallback: Option<bool>,
    pub priority: Option<i64>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub filter: DownloadJobFilter,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCount {
    pub service: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapacityCounts {
    pub hires: i64,
    pub lossless: i64,
    pub high: i64,
    pub lossy: i64,
    /// Candidatos sin ninguna fuente conocida en `track_sources`.
    pub unknown: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJobEstimate {
    pub total: i64,
    /// Servicios de los que proceden las fuentes conocidas de los candidatos.
    pub by_source: Vec<ServiceCount>,
    /// Destinos con adaptador que podrían servir cada candidato (ISRC conocido
    /// o fuente propia). Es capacidad potencial: la resolución viva llega en D04.
    pub by_destination: Vec<ServiceCount>,
    /// Mejor tier de calidad disponible por candidato.
    pub by_capacity: CapacityCounts,
    pub estimated_size_mb: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueOutcome {
    pub enqueued: i64,
    /// Ya estaban `queued`/`downloading` en la cola.
    pub skipped_existing: i64,
    /// Repetidos dentro del propio lote de entrada.
    pub skipped_duplicate_input: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJobRow {
    pub id: i64,
    pub kind: String,
    pub label: Option<String>,
    pub status: String,
    pub source_service: Option<String>,
    pub destination_service: Option<String>,
    pub preferred_account_id: Option<i64>,
    pub quality_preference: Option<String>,
    pub allow_fallback: i64,
    pub filter_json: Option<String>,
    pub requested_count: i64,
    pub estimated_count: i64,
    pub estimated_size_mb: Option<f64>,
    pub completed_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
    pub skip_breakdown_json: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Vocabulario canónico de calidad (espejo de `normalize_quality_preference`
/// en `commands/queue.rs`). Los alias de la UI se pliegan a hires/lossless/
/// high/any; un valor desconocido degrada a NULL con warning, igual que el
/// resto de la cola, en vez de convertir un job válido en un error de usuario.
fn normalize_job_quality(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }
    match value.to_ascii_lowercase().as_str() {
        "hires" | "hi_res" | "hi-res" | "hi_res_lossless" | "hires_lossless" | "flac_hires"
        | "flac_24" => Some("hires".to_string()),
        "lossless" | "cd" | "flac" | "flac_16" => Some("lossless".to_string()),
        "high" | "320" | "320kbps" | "aac" | "mp3" | "lossy" | "standard" | "low" | "medium"
        | "ogg" => Some("high".to_string()),
        "any" | "best" | "auto" => Some("any".to_string()),
        unknown => {
            tracing::warn!(
                raw_quality = %unknown,
                "[download_jobs] Unknown quality preference degraded to NULL (worker default applies)"
            );
            None
        }
    }
}

fn validate_kind(kind: &str) -> Result<(), JobError> {
    if !JOB_KINDS.contains(&kind) {
        return Err(JobError::Invalid(format!(
            "unknown download job kind '{kind}'"
        )));
    }
    Ok(())
}

fn valid_sql_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_request(req: &DownloadJobRequest) -> Result<Option<String>, JobError> {
    validate_kind(&req.kind)?;
    let quality = normalize_job_quality(req.quality_preference.as_deref());
    if let Some(service) = req.source_service.as_deref() {
        let service =
            if service.eq_ignore_ascii_case("all") || service.eq_ignore_ascii_case("local") {
                None
            } else {
                Some(service)
            };
        if let Some(service) = service {
            if !valid_sql_identifier(service) {
                return Err(JobError::Invalid(format!(
                    "invalid source service '{service}'"
                )));
            }
        }
    }
    if let Some(destination) = req.destination_service.as_deref() {
        if !valid_sql_identifier(destination) {
            return Err(JobError::Invalid(format!(
                "invalid destination service '{destination}'"
            )));
        }
    }
    if let Some(item_type) = req.filter.item_type.as_deref() {
        let normalized = item_type.to_lowercase();
        let allowed = matches!(
            normalized.as_str(),
            "tracks" | "track" | "albums" | "album" | "artists" | "artist" | "all"
        );
        if !allowed {
            return Err(JobError::Invalid(format!(
                "invalid item type '{item_type}'"
            )));
        }
    }
    match req.kind.as_str() {
        "album" if req.filter.album_id.is_none() => Err(JobError::Invalid(
            "album_id is required for kind=album".into(),
        )),
        "artist" if req.filter.artist_id.is_none() => Err(JobError::Invalid(
            "artist_id is required for kind=artist".into(),
        )),
        "playlist" if req.filter.playlist_id.is_none() => Err(JobError::Invalid(
            "playlist_id is required for kind=playlist".into(),
        )),
        "selection" => match &req.filter.track_ids {
            Some(ids) if !ids.is_empty() => Ok(quality),
            _ => Err(JobError::Invalid(
                "track_ids is required for kind=selection".into(),
            )),
        },
        "single" if req.filter.track_id.is_none() => Err(JobError::Invalid(
            "track_id is required for kind=single".into(),
        )),
        // `url` se crea desde `import_from_url`; no resuelve candidatos aquí.
        "url" => Ok(quality),
        _ => Ok(quality),
    }
}

/// Normaliza el filtro de fuente a un nombre de servicio o nada.
fn source_service_filter(req: &DownloadJobRequest) -> Option<&str> {
    req.source_service.as_deref().filter(|s| {
        !s.is_empty() && !s.eq_ignore_ascii_case("all") && !s.eq_ignore_ascii_case("local")
    })
}

fn source_clause(service: Option<&str>) -> String {
    match service {
        None => String::new(),
        Some(service) => format!(
            " AND EXISTS (SELECT 1 FROM track_sources ts_f JOIN services s_f ON s_f.id = ts_f.service_id \
             WHERE ts_f.track_id = t.id AND ts_f.available = 1 AND s_f.name = '{service}')"
        ),
    }
}

/// SQL de la CTE `candidates(track_id)` para el kind del request. Los ids y
/// nombres se interpolaron solo tras validarlos (i64 / identificador seguro).
fn candidates_sql(req: &DownloadJobRequest) -> Result<String, JobError> {
    validate_kind(&req.kind)?;
    let source = source_service_filter(req);
    let source_clause = source_clause(source);

    let sql = match req.kind.as_str() {
        "favorites" => {
            let item_type = req
                .filter
                .item_type
                .as_deref()
                .unwrap_or("all")
                .to_lowercase();
            let want_albums = matches!(item_type.as_str(), "all" | "album" | "albums");
            let want_artists = matches!(item_type.as_str(), "all" | "artist" | "artists");

            let mut parts = vec![format!(
                "SELECT DISTINCT t.id AS track_id FROM tracks t \
                 LEFT JOIN favorites f ON f.item_type = 'track' \
                   AND (f.service_item_id = CAST(t.id AS TEXT) OR (t.isrc IS NOT NULL AND f.service_item_id = t.isrc)) \
                 LEFT JOIN library_entries le ON le.track_id = t.id \
                 WHERE (t.favorite_at IS NOT NULL OR t.is_favorite = 1 OR f.id IS NOT NULL OR le.is_liked = 1){source_clause}"
            )];
            if want_albums {
                parts.push(format!(
                    "SELECT DISTINCT t.id AS track_id FROM tracks t \
                     JOIN albums a ON a.id = t.album_id \
                     LEFT JOIN favorites f ON f.item_type = 'album' AND f.service_item_id = CAST(a.id AS TEXT) \
                     WHERE (a.favorite_at IS NOT NULL OR a.is_favorite = 1 OR f.id IS NOT NULL){source_clause}"
                ));
            }
            if want_artists {
                parts.push(format!(
                    "SELECT DISTINCT t.id AS track_id FROM tracks t \
                     JOIN track_artists ta ON ta.track_id = t.id \
                     LEFT JOIN favorites f ON f.item_type = 'artist' AND f.service_item_id = CAST(ta.artist_id AS TEXT) \
                     WHERE f.id IS NOT NULL{source_clause}"
                ));
            }
            format!("SELECT track_id FROM ({})", parts.join(" UNION "))
        }
        "album" => {
            let album_id = req.filter.album_id.unwrap_or_default();
            format!(
                "SELECT t.id AS track_id FROM tracks t WHERE t.album_id = {album_id}{source_clause}"
            )
        }
        "artist" => {
            let artist_id = req.filter.artist_id.unwrap_or_default();
            format!(
                "SELECT t.id AS track_id FROM tracks t \
                 JOIN track_artists ta ON ta.track_id = t.id \
                 WHERE ta.artist_id = {artist_id}{source_clause}"
            )
        }
        "playlist" => {
            let playlist_id = req.filter.playlist_id.unwrap_or_default();
            format!(
                "SELECT pt.track_id FROM playlist_tracks pt \
                 JOIN tracks t ON t.id = pt.track_id \
                 WHERE pt.playlist_id = {playlist_id}{source_clause}"
            )
        }
        "selection" => {
            let ids: Vec<String> = req
                .filter
                .track_ids
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|id| id.to_string())
                .collect();
            format!(
                "SELECT t.id AS track_id FROM tracks t WHERE t.id IN ({}){source_clause}",
                ids.join(",")
            )
        }
        "single" => {
            let track_id = req.filter.track_id.unwrap_or_default();
            format!("SELECT t.id AS track_id FROM tracks t WHERE t.id = {track_id}{source_clause}")
        }
        other => {
            return Err(JobError::Invalid(format!(
                "kind '{other}' does not resolve candidates here"
            )))
        }
    };
    Ok(sql)
}

fn limit_clause(limit: Option<i64>) -> String {
    match limit {
        Some(lim) if lim > 0 => format!(" LIMIT {lim}"),
        _ => String::new(),
    }
}

/// Estimación agregada: un puñado de consultas con `GROUP BY`, sin recorrer
/// pistas en Rust.
pub async fn estimate(
    db: &SqlitePool,
    req: &DownloadJobRequest,
) -> Result<DownloadJobEstimate, JobError> {
    let _quality = validate_request(req)?;
    let candidates = candidates_sql(req)?;
    let limit = limit_clause(req.limit);

    let capacity_sql = format!(
        "WITH candidates AS ({candidates}), \
         best AS ( \
           SELECT c.track_id, \
             MAX(CASE \
               WHEN ts.id IS NULL THEN NULL \
               WHEN ts.bit_depth >= 24 OR ts.sample_rate >= 88200 THEN 3 \
               WHEN UPPER(COALESCE(ts.format, '')) IN ('FLAC', 'ALAC', 'WAV') OR COALESCE(ts.bit_depth, 0) >= 16 THEN 2 \
               WHEN COALESCE(ts.bitrate, 0) >= 256 THEN 1 \
               ELSE 0 END) AS tier \
           FROM (SELECT * FROM candidates{limit}) c \
           LEFT JOIN track_sources ts ON ts.track_id = c.track_id AND ts.available = 1 \
           GROUP BY c.track_id \
         ) \
         SELECT COUNT(*) AS total, \
                COALESCE(SUM(CASE WHEN tier = 3 THEN 1 ELSE 0 END), 0) AS hires, \
                COALESCE(SUM(CASE WHEN tier = 2 THEN 1 ELSE 0 END), 0) AS lossless, \
                COALESCE(SUM(CASE WHEN tier = 1 THEN 1 ELSE 0 END), 0) AS high, \
                COALESCE(SUM(CASE WHEN tier = 0 THEN 1 ELSE 0 END), 0) AS lossy, \
                COALESCE(SUM(CASE WHEN tier IS NULL THEN 1 ELSE 0 END), 0) AS unknown \
         FROM best"
    );

    let row = sqlx::query(&capacity_sql).fetch_one(db).await?;
    let by_capacity = CapacityCounts {
        hires: row.try_get("hires")?,
        lossless: row.try_get("lossless")?,
        high: row.try_get("high")?,
        lossy: row.try_get("lossy")?,
        unknown: row.try_get("unknown")?,
    };

    let source_sql = format!(
        "WITH candidates AS ({candidates}) \
         SELECT s.name AS service, COUNT(DISTINCT c.track_id) AS count \
         FROM (SELECT * FROM candidates{limit}) c \
         JOIN track_sources ts ON ts.track_id = c.track_id AND ts.available = 1 \
         JOIN services s ON s.id = ts.service_id \
         GROUP BY s.name ORDER BY count DESC, s.name ASC"
    );
    let source_rows = sqlx::query(&source_sql).fetch_all(db).await?;
    let mut by_source = Vec::with_capacity(source_rows.len());
    for r in source_rows {
        by_source.push(ServiceCount {
            service: r.try_get("service")?,
            count: r.try_get("count")?,
        });
    }

    let target_values = ADAPTER_SERVICES
        .iter()
        .map(|service| format!("('{service}')"))
        .collect::<Vec<_>>()
        .join(", ");
    let destination_sql = format!(
        "WITH candidates AS ({candidates}), \
         targets(service) AS (VALUES {target_values}) \
         SELECT tg.service AS service, \
                COALESCE(SUM(CASE WHEN \
                  EXISTS (SELECT 1 FROM tracks t WHERE t.id = c.track_id AND t.isrc IS NOT NULL AND t.isrc <> '') \
                  OR EXISTS (SELECT 1 FROM track_sources ts2 JOIN services s2 ON s2.id = ts2.service_id \
                             WHERE ts2.track_id = c.track_id AND ts2.available = 1 AND s2.name = tg.service) \
                THEN 1 ELSE 0 END), 0) AS count \
         FROM (SELECT * FROM candidates{limit}) c CROSS JOIN targets tg \
         GROUP BY tg.service ORDER BY count DESC, tg.service ASC"
    );
    let destination_rows = sqlx::query(&destination_sql).fetch_all(db).await?;
    let mut by_destination = Vec::with_capacity(destination_rows.len());
    for r in destination_rows {
        let service: String = r.try_get("service")?;
        let count: i64 = r.try_get("count")?;
        if let Some(requested) = req.destination_service.as_deref() {
            if !requested.eq_ignore_ascii_case(&service) {
                continue;
            }
        }
        by_destination.push(ServiceCount { service, count });
    }
    // Si el destino pedido no está entre los adaptadores, se declara en cero.
    if let Some(requested) = req.destination_service.as_deref() {
        if !by_destination
            .iter()
            .any(|d| d.service.eq_ignore_ascii_case(requested))
        {
            by_destination.push(ServiceCount {
                service: requested.to_lowercase(),
                count: 0,
            });
        }
    }

    let total = by_capacity.hires
        + by_capacity.lossless
        + by_capacity.high
        + by_capacity.lossy
        + by_capacity.unknown;
    let estimated_size_mb = (by_capacity.hires + by_capacity.lossless) * SIZE_LOSSLESS_MB
        + by_capacity.high * SIZE_HIGH_MB
        + by_capacity.lossy * SIZE_LOSSY_MB;

    Ok(DownloadJobEstimate {
        total,
        by_source,
        by_destination,
        by_capacity,
        estimated_size_mb,
    })
}

/// Resuelve los ids de candidatos sin consultas por pista. Para `selection`
/// valida la existencia por bloques de `IN` y conserva el orden de entrada.
pub async fn resolve_candidate_ids(
    db: &SqlitePool,
    req: &DownloadJobRequest,
) -> Result<Vec<i64>, JobError> {
    validate_request(req)?;
    let limit = req.limit.filter(|l| *l > 0);

    if req.kind == "selection" {
        let input: Vec<i64> = req.filter.track_ids.clone().unwrap_or_default();
        let mut seen = HashSet::new();
        let mut ordered: Vec<i64> = Vec::with_capacity(input.len());
        for id in input {
            if seen.insert(id) {
                ordered.push(id);
            }
        }
        let mut existing = HashSet::new();
        for chunk in ordered.chunks(ENQUEUE_BATCH_SIZE) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!("SELECT id FROM tracks WHERE id IN ({placeholders})");
            let mut query = sqlx::query_scalar::<_, i64>(&sql);
            for id in chunk {
                query = query.bind(*id);
            }
            for id in query.fetch_all(db).await? {
                existing.insert(id);
            }
        }
        let mut resolved: Vec<i64> = ordered
            .into_iter()
            .filter(|id| existing.contains(id))
            .collect();
        if let Some(lim) = limit {
            resolved.truncate(lim as usize);
        }
        return Ok(resolved);
    }

    let candidates = candidates_sql(req)?;
    let sql = format!(
        "SELECT track_id FROM (SELECT * FROM ({candidates}) ORDER BY track_id ASC{})",
        limit_clause(limit)
    );
    let rows: Vec<(i64,)> = sqlx::query_as(&sql).fetch_all(db).await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Encola candidatos en bloques: un único `MAX(position)`, dedupe set-based
/// contra lo ya encolado y un INSERT multi-VALUES por bloque, todo en una
/// transacción.
pub async fn enqueue_candidates(
    db: &SqlitePool,
    job_id: i64,
    candidate_ids: &[i64],
    priority: i64,
    quality_preference: Option<&str>,
    allow_fallback: bool,
) -> Result<EnqueueOutcome, JobError> {
    let mut seen = HashSet::new();
    let ids: Vec<i64> = candidate_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    let skipped_duplicate_input = (candidate_ids.len() - ids.len()) as i64;

    let mut tx = db.begin().await?;

    // D07: si el job fue cancelado mientras el encolado corría, no inserta
    // nada. La comprobación comparte transacción con el INSERT, así que una
    // cancelación posterior o anterior queda serializada con este encolado.
    let job_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM download_jobs WHERE id = ?")
            .bind(job_id)
            .fetch_optional(&mut *tx)
            .await?;
    if job_status.as_deref() == Some("cancelled") {
        return Ok(EnqueueOutcome::default());
    }

    let mut existing = HashSet::new();
    for chunk in ids.chunks(ENQUEUE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "SELECT track_id FROM download_queue \
             WHERE status IN ('queued', 'downloading') AND track_id IN ({placeholders})"
        );
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for id in chunk {
            query = query.bind(*id);
        }
        for id in query.fetch_all(&mut *tx).await? {
            existing.insert(id);
        }
    }

    let base: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) FROM download_queue WHERE status = 'queued'",
    )
    .fetch_one(&mut *tx)
    .await?;

    let mut enqueued: i64 = 0;
    let mut position = base;
    let fallback_flag: i64 = if allow_fallback { 1 } else { 0 };
    let quality: Option<String> = quality_preference.map(|q| q.to_string());

    for chunk in ids.chunks(ENQUEUE_BATCH_SIZE) {
        let rows: Vec<i64> = chunk
            .iter()
            .copied()
            .filter(|id| !existing.contains(id))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
            "INSERT INTO download_queue \
             (track_id, priority, quality_preference, status, progress_percent, retry_count, \
              position, job_id, allow_fallback, created_at) ",
        );
        qb.push_values(rows.iter(), |mut b, track_id| {
            position += 1;
            b.push_bind(*track_id)
                .push_bind(priority)
                .push_bind(quality.clone())
                .push("'queued'")
                .push_bind(0.0_f64)
                .push_bind(0_i64)
                .push_bind(position)
                .push_bind(job_id)
                .push_bind(fallback_flag)
                .push("CURRENT_TIMESTAMP");
        });
        let result = qb.build().execute(&mut *tx).await?;
        enqueued += result.rows_affected() as i64;
    }

    tx.commit().await?;

    Ok(EnqueueOutcome {
        enqueued,
        skipped_existing: existing.len() as i64,
        skipped_duplicate_input,
    })
}

/// Crea la fila del job y devuelve su id. El trabajo real corre después.
pub async fn create_job<'e, E>(executor: E, req: &DownloadJobRequest) -> Result<i64, JobError>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let quality = validate_request(req)?;
    let filter_json = serde_json::to_string(&req.filter)
        .map_err(|e| JobError::Invalid(format!("invalid job filter: {e}")))?;
    let allow_fallback: i64 = if req.allow_fallback.unwrap_or(true) {
        1
    } else {
        0
    };
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_jobs (
            kind, label, status, source_service, destination_service,
            preferred_account_id, quality_preference, allow_fallback,
            filter_json, requested_count, estimated_count, created_at, updated_at
        ) VALUES (?, ?, 'running', ?, ?, ?, ?, ?, ?, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        RETURNING id
        "#,
    )
    .bind(&req.kind)
    .bind(req.label.as_deref())
    .bind(source_service_filter(req))
    .bind(req.destination_service.as_deref())
    .bind(req.preferred_account_id)
    .bind(quality.filter(|q| q != "any"))
    .bind(allow_fallback)
    .bind(filter_json)
    .fetch_one(executor)
    .await?;
    Ok(id)
}

/// Estima, persiste la estimación y encola los candidatos del job.
/// Devuelve el resultado del encolado para los contadores del job.
pub async fn run_job(
    db: &SqlitePool,
    job_id: i64,
    req: &DownloadJobRequest,
) -> Result<EnqueueOutcome, JobError> {
    let estimate = estimate(db, req).await?;
    sqlx::query(
        "UPDATE download_jobs SET estimated_count = ?, estimated_size_mb = ?, \
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(estimate.total)
    .bind(estimate.estimated_size_mb as f64)
    .bind(job_id)
    .execute(db)
    .await?;

    let ids = resolve_candidate_ids(db, req).await?;
    let priority = req.priority.unwrap_or(50).clamp(0, 100);
    let outcome = enqueue_candidates(
        db,
        job_id,
        &ids,
        priority,
        req.quality_preference.as_deref(),
        req.allow_fallback.unwrap_or(true),
    )
    .await?;

    let breakdown = serde_json::json!({
        "already_queued": outcome.skipped_existing,
        "duplicate_input": outcome.skipped_duplicate_input,
    });
    sqlx::query(
        "UPDATE download_jobs SET requested_count = ?, skipped_count = ?, \
         skip_breakdown_json = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(ids.len() as i64)
    .bind(outcome.skipped_existing + outcome.skipped_duplicate_input)
    .bind(breakdown.to_string())
    .bind(job_id)
    .execute(db)
    .await?;

    Ok(outcome)
}

/// Suma una omisión ya decidida durante la ejecución (p. ej. `no_provider`) al
/// contador del job y a su desglose. El encolado ya no ve esa pista: la fila de
/// cola se cierra como omitida, nunca como fallo.
pub async fn record_skipped(
    db: &SqlitePool,
    job_id: i64,
    reason: &str,
    count: i64,
) -> Result<(), JobError> {
    if count <= 0 {
        return Ok(());
    }
    let current: Option<(Option<String>,)> =
        sqlx::query_as("SELECT skip_breakdown_json FROM download_jobs WHERE id = ?")
            .bind(job_id)
            .fetch_optional(db)
            .await?;
    let Some((current,)) = current else {
        return Ok(());
    };
    let mut breakdown: serde_json::Map<String, serde_json::Value> = current
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let previous = breakdown
        .get(reason)
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    breakdown.insert(
        reason.to_string(),
        serde_json::Value::from(previous + count),
    );
    sqlx::query(
        "UPDATE download_jobs SET skipped_count = skipped_count + ?, \
         skip_breakdown_json = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(count)
    .bind(serde_json::Value::Object(breakdown).to_string())
    .bind(job_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn mark_job_failed(db: &SqlitePool, job_id: i64, error: &str) -> Result<(), JobError> {
    sqlx::query(
        "UPDATE download_jobs SET status = 'failed', error = ?, \
         completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(error)
    .bind(job_id)
    .execute(db)
    .await?;
    // D10: el fallo de encolado/preflight también es una transición terminal.
    super::job_events::notify_terminal_if_available(db, job_id).await;
    Ok(())
}

// ===== D07 (spec #111 / issue #119): estado del job, snapshot y ciclo =====
//
// La cola (`download_queue`) sigue siendo la fuente de verdad: el estado y los
// contadores del job se recalculan desde ella, nunca al revés. El worker no
// llama aquí (módulo de jobs, no worker.rs); el watcher de `job_events` sondea
// y los comandos de ciclo refrescan al actuar.

/// Estados terminales del vocabulario de `download_jobs` (migración 0102).
pub const JOB_TERMINAL_STATUSES: [&str; 4] = ["completed", "partial", "cancelled", "failed"];

/// Payload del evento `syncify:download_job_progress`. Los nombres de campo
/// son el contrato que consume D08: snake_case, tal cual emite serde.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DownloadJobProgressEvent {
    pub job_id: i64,
    pub label: Option<String>,
    pub status: String,
    pub total: i64,
    pub queued: i64,
    pub downloading: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub skipped: i64,
    pub percent: f64,
    pub terminal: bool,
}

/// Contadores vivos de la cola para un job, por estado de fila.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobQueueCounts {
    pub queued: i64,
    pub downloading: i64,
    /// Filas `complete` en `download_queue` (vocabulario de la cola).
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
}

impl JobQueueCounts {
    pub fn active(&self) -> i64 {
        self.queued + self.downloading
    }

    pub fn total_rows(&self) -> i64 {
        self.queued + self.downloading + self.completed + self.failed + self.cancelled
    }
}

async fn queue_counts(db: &SqlitePool, job_id: i64) -> Result<JobQueueCounts, JobError> {
    let rows = sqlx::query(
        "SELECT status, COUNT(*) AS c FROM download_queue WHERE job_id = ? GROUP BY status",
    )
    .bind(job_id)
    .fetch_all(db)
    .await?;
    let mut counts = JobQueueCounts::default();
    for row in rows {
        let status: String = row.try_get("status")?;
        let c: i64 = row.try_get("c")?;
        match status.as_str() {
            "queued" => counts.queued = c,
            "downloading" => counts.downloading = c,
            "complete" => counts.completed = c,
            "failed" => counts.failed = c,
            "cancelled" => counts.cancelled = c,
            other => tracing::warn!(
                job_id = job_id,
                status = %other,
                "[download_jobs] unknown queue status in job counters"
            ),
        }
    }
    Ok(counts)
}

/// Estado terminal derivado de los contadores finales de la cola. Un job sin
/// ninguna fila de resultado (todo `skipped`, nada que hacer) cuenta como
/// `completed`: no hay trabajo pendiente que el job pudiera hacer.
fn derive_terminal_status(completed: i64, failed: i64, cancelled: i64) -> &'static str {
    if completed > 0 && (failed > 0 || cancelled > 0) {
        "partial"
    } else if completed > 0 {
        "completed"
    } else if failed > 0 {
        "failed"
    } else if cancelled > 0 {
        "cancelled"
    } else {
        "completed"
    }
}

/// Recalcula estado y contadores del job desde la cola y los persiste.
///
/// `enqueue_done`: `Some(true)` cuando el llamador sabe que el encolado terminó
/// (`run_job`); `None` lo infiere de `requested_count > 0`. Un job ya terminal
/// no se recalcula aquí: abrirlo de nuevo es cosa de `retry_job_failed`.
/// Lee un job sin fallar si no existe: los caminos de refresco y de ciclo
/// distinguen «job borrado» de «error de BD».
async fn find_job(db: &SqlitePool, job_id: i64) -> Result<Option<DownloadJobRow>, JobError> {
    let sql = format!("SELECT {JOB_COLUMNS} FROM download_jobs WHERE id = ?");
    Ok(sqlx::query_as::<_, DownloadJobRow>(&sql)
        .bind(job_id)
        .fetch_optional(db)
        .await?)
}

pub async fn refresh_job_status(
    db: &SqlitePool,
    job_id: i64,
    enqueue_done: Option<bool>,
) -> Result<Option<DownloadJobRow>, JobError> {
    let Some(job) = find_job(db, job_id).await? else {
        return Ok(None);
    };
    let counts = queue_counts(db, job_id).await?;
    let enqueue_done = enqueue_done.unwrap_or(job.requested_count > 0);

    let status = if JOB_TERMINAL_STATUSES.contains(&job.status.as_str()) {
        job.status.clone()
    } else if counts.active() > 0 || !enqueue_done {
        "running".to_string()
    } else {
        derive_terminal_status(counts.completed, counts.failed, counts.cancelled).to_string()
    };

    sqlx::query(
        "UPDATE download_jobs SET status = ?, completed_count = ?, failed_count = ?, \
         updated_at = CURRENT_TIMESTAMP, \
         completed_at = CASE \
           WHEN ? IN ('completed', 'partial', 'cancelled', 'failed') AND completed_at IS NULL \
             THEN CURRENT_TIMESTAMP \
           WHEN ? = 'running' THEN NULL \
           ELSE completed_at END \
         WHERE id = ?",
    )
    .bind(&status)
    .bind(counts.completed)
    .bind(counts.failed)
    .bind(&status)
    .bind(&status)
    .bind(job_id)
    .execute(db)
    .await?;

    find_job(db, job_id).await
}

/// Snapshot del payload del evento de job. Lee la cola directamente para que
/// los contadores sean vivos aunque la fila del job vaya con retraso.
pub async fn progress_snapshot(
    db: &SqlitePool,
    job_id: i64,
) -> Result<Option<DownloadJobProgressEvent>, JobError> {
    let Some(job) = find_job(db, job_id).await? else {
        return Ok(None);
    };
    let counts = queue_counts(db, job_id).await?;

    let total = if job.requested_count > 0 {
        job.requested_count
    } else {
        counts.total_rows()
    };
    let done = counts.completed + counts.failed + counts.cancelled + job.skipped_count;
    let percent = if total > 0 {
        ((done as f64 / total as f64) * 100.0).min(100.0)
    } else {
        0.0
    };

    Ok(Some(DownloadJobProgressEvent {
        job_id,
        label: job.label.clone(),
        status: job.status.clone(),
        total,
        queued: counts.queued,
        downloading: counts.downloading,
        completed: counts.completed,
        failed: counts.failed,
        cancelled: counts.cancelled,
        skipped: job.skipped_count,
        percent,
        terminal: JOB_TERMINAL_STATUSES.contains(&job.status.as_str()),
    }))
}

/// Resultado de cancelar un job: filas pendientes canceladas y job refrescado.
#[derive(Debug, Clone)]
pub struct CancelJobOutcome {
    pub cancelled_rows: i64,
    pub job: DownloadJobRow,
}

/// Cancela un job: solo filas pendientes (`queued`), como fija D03; la
/// descarga activa termina. El estado del job se recalcula desde la cola.
///
/// Si el encolado en segundo plano aún corre, `enqueue_candidates` comprueba
/// el estado del job dentro de su transacción y no inserta nada.
pub async fn cancel_job(
    db: &SqlitePool,
    job_id: i64,
) -> Result<Option<CancelJobOutcome>, JobError> {
    let cancelled_rows = sqlx::query(
        "UPDATE download_queue SET status = 'cancelled' \
         WHERE job_id = ? AND status = 'queued'",
    )
    .bind(job_id)
    .execute(db)
    .await?
    .rows_affected() as i64;

    let Some(job) = find_job(db, job_id).await? else {
        return Ok(None);
    };
    let counts = queue_counts(db, job_id).await?;

    let job = if counts.downloading > 0 {
        // Descarga en vuelo: sigue hasta terminar (sin cancelación
        // cooperativa en beta) y el refresco derivará entonces.
        refresh_job_status(db, job_id, None).await?.unwrap_or(job)
    } else if counts.total_rows() == 0 && job.status == "running" && job.requested_count == 0 {
        // Cancelación antes del encolado: sin filas todavía y el encolado en
        // curso no insertará nada (guardia en `enqueue_candidates`). El job
        // termina `cancelled` ahora, no «completado vacío».
        sqlx::query(
            "UPDATE download_jobs SET status = 'cancelled', error = NULL, \
             completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(job_id)
        .execute(db)
        .await?;
        find_job(db, job_id).await?.unwrap_or(job)
    } else {
        refresh_job_status(db, job_id, Some(true))
            .await?
            .unwrap_or(job)
    };

    Ok(Some(CancelJobOutcome {
        cancelled_rows,
        job,
    }))
}

/// Reencola las filas `failed` de un job y reabre el job si estaba terminal.
/// Es el mismo ManualRetry por fila de `commands/queue.rs`, acotado al job y
/// sin tope de intentos. Devuelve el job actualizado; sin filas que reencolar
/// no toca el job.
pub async fn retry_job_failed(
    db: &SqlitePool,
    job_id: i64,
) -> Result<Option<DownloadJobRow>, JobError> {
    let requeued = sqlx::query(
        "UPDATE download_queue SET status = 'queued', error_message = NULL, last_error = NULL, \
         decision_reason = NULL, progress_percent = 0, started_at = NULL, retry_count = 0, \
         next_attempt_at = NULL WHERE job_id = ? AND status = 'failed'",
    )
    .bind(job_id)
    .execute(db)
    .await?
    .rows_affected();

    if requeued == 0 {
        return find_job(db, job_id).await;
    }

    sqlx::query(
        "UPDATE download_jobs SET status = 'running', error = NULL, completed_at = NULL, \
         updated_at = CURRENT_TIMESTAMP WHERE id = ? \
         AND status IN ('completed', 'partial', 'cancelled', 'failed')",
    )
    .bind(job_id)
    .execute(db)
    .await?;

    refresh_job_status(db, job_id, None).await
}

/// Borra los jobs terminales del historial junto con sus filas de la cola y
/// devuelve cuántos cayeron y los ids borrados (para que la capa de comandos
/// pode el registro queue→job y la deduplicación de notificaciones).
///
/// El ledger `downloads` y el `operation_journal` no se tocan: son el
/// historial de ficheros y la auditoría de operaciones del usuario. Las filas
/// de la cola de jobs `running` tampoco se tocan.
pub async fn clear_terminal_jobs(db: &SqlitePool) -> Result<(u64, Vec<i64>), JobError> {
    let mut tx = db.begin().await?;
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM download_jobs \
         WHERE status IN ('completed', 'partial', 'cancelled', 'failed')",
    )
    .fetch_all(&mut *tx)
    .await?;
    if ids.is_empty() {
        return Ok((0, Vec::new()));
    }
    sqlx::query(
        "DELETE FROM download_queue WHERE job_id IN ( \
           SELECT id FROM download_jobs \
           WHERE status IN ('completed', 'partial', 'cancelled', 'failed'))",
    )
    .execute(&mut *tx)
    .await?;
    let deleted = sqlx::query(
        "DELETE FROM download_jobs \
         WHERE status IN ('completed', 'partial', 'cancelled', 'failed')",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok((deleted, ids))
}

const JOB_COLUMNS: &str = "id, kind, label, status, source_service, destination_service, \
     preferred_account_id, quality_preference, allow_fallback, filter_json, \
     requested_count, estimated_count, estimated_size_mb, completed_count, \
     failed_count, skipped_count, skip_breakdown_json, error, created_at, \
     updated_at, started_at, completed_at";

pub async fn get_jobs(
    db: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<DownloadJobRow>, JobError> {
    let cap = limit.filter(|l| *l > 0).unwrap_or(50).min(500);
    let sql = format!(
        "SELECT {JOB_COLUMNS} FROM download_jobs ORDER BY created_at DESC, id DESC LIMIT {cap}"
    );
    let rows = sqlx::query_as::<_, DownloadJobRow>(&sql)
        .fetch_all(db)
        .await?;
    Ok(rows)
}

pub async fn get_job(db: &SqlitePool, job_id: i64) -> Result<DownloadJobRow, JobError> {
    let sql = format!("SELECT {JOB_COLUMNS} FROM download_jobs WHERE id = ?");
    sqlx::query_as::<_, DownloadJobRow>(&sql)
        .bind(job_id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| JobError::Invalid(format!("download job {job_id} not found")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(kind: &str) -> DownloadJobRequest {
        DownloadJobRequest {
            kind: kind.to_string(),
            label: None,
            source_service: None,
            destination_service: None,
            preferred_account_id: None,
            quality_preference: None,
            allow_fallback: None,
            priority: None,
            limit: None,
            filter: DownloadJobFilter::default(),
        }
    }

    #[test]
    fn rejects_unknown_kinds_and_missing_filters() {
        assert!(validate_request(&req("bogus")).is_err());
        assert!(validate_request(&req("album")).is_err());
        assert!(validate_request(&req("selection")).is_err());
        assert!(validate_request(&req("favorites")).is_ok());
    }

    #[test]
    fn url_kind_is_creatable_but_does_not_resolve_candidates() {
        let request = req("url");
        assert!(validate_request(&request).is_ok());
        assert!(candidates_sql(&request).is_err());
    }

    #[test]
    fn candidate_sql_uses_validated_iddentifiers() {
        let mut r = req("selection");
        r.filter.track_ids = Some(vec![1, 2, 3]);
        let sql = candidates_sql(&r).unwrap();
        assert!(sql.contains("IN (1,2,3)"));

        let mut r = req("favorites");
        r.source_service = Some("qobuz' OR 1=1 --".to_string());
        assert!(validate_request(&r).is_err());
    }
}
