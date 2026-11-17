//! Persistencia del agregado `SyncRun` (ADR-0006: «Trabajo de sincronización»).
//!
//! `sync_runs` (una fila por corrida), `sync_run_collections` (cobertura por
//! colección) y `sync_run_errors` (los huecos registrados). Escrituras todas
//! con propagación de error: el ratchet de escrituras silenciosas de
//! `docs/Deuda_Tecnica_y_UX.md` (D-06) no puede subir.

use serde::Serialize;

use super::source::SyncScope;

/// Cuántas corridas se conservan por cuenta (decisión 22 del ADR-0006).
pub const KEEP_RUNS_PER_ACCOUNT: i64 = 20;

/// Registro de corrida tal como lo consume la ficha de cuenta.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SyncRunRecord {
    pub id: i64,
    pub account_id: i64,
    pub service_id: i64,
    pub mode: String,
    pub scope: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub reported_total: Option<i64>,
    pub imported_total: i64,
    pub error_count: i64,
}

/// Crea la corrida en estado `running` con sus marcas de tiempo y devuelve el
/// id (criterio 1 del issue #65: `start_sync_run` devuelve un id y persiste la
/// corrida con estado y marcas de tiempo).
pub async fn insert_running_run(
    db: &sqlx::SqlitePool,
    account_id: i64,
    service_id: i64,
    scope: SyncScope,
    mode: &str,
) -> Result<i64, String> {
    sqlx::query_scalar(
        "INSERT INTO sync_runs (account_id, service_id, mode, scope, status, started_at)
         VALUES (?, ?, ?, ?, 'running', CURRENT_TIMESTAMP) RETURNING id",
    )
    .bind(account_id)
    .bind(service_id)
    .bind(mode)
    .bind(scope.as_str())
    .fetch_one(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))
}

/// Cierra la corrida con el estado final de la ejecución y su marca de tiempo.
pub async fn finish_run(
    db: &sqlx::SqlitePool,
    run_id: i64,
    status: &str,
    reported_total: Option<i64>,
    imported_total: i64,
    error_count: i64,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE sync_runs
         SET status = ?, finished_at = CURRENT_TIMESTAMP,
             reported_total = ?, imported_total = ?, error_count = ?
         WHERE id = ?",
    )
    .bind(status)
    .bind(reported_total)
    .bind(imported_total)
    .bind(error_count)
    .bind(run_id)
    .execute(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))?;
    Ok(())
}

/// Cobertura por colección: el eje de cobertura nunca se mezcla con el estado
/// de la corrida (decisión 5). Un total ausente es `unknown`, no un porcentaje.
pub async fn upsert_run_collection(
    db: &sqlx::SqlitePool,
    run_id: i64,
    collection: &str,
    reported: Option<i64>,
    imported: i64,
) -> Result<(), String> {
    let coverage = match reported {
        None => "unknown",
        Some(reported) if imported >= reported => "imported",
        Some(_) => "reported",
    };
    sqlx::query(
        "INSERT INTO sync_run_collections (run_id, collection, reported, imported, coverage)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(run_id, collection) DO UPDATE SET
           reported = excluded.reported,
           imported = excluded.imported,
           coverage = excluded.coverage",
    )
    .bind(run_id)
    .bind(collection)
    .bind(reported)
    .bind(imported)
    .bind(coverage)
    .execute(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))?;
    Ok(())
}

/// Registra un hueco (una página que persiste el fallo): la corrida continúa,
/// el error queda (decisión 17).
pub async fn record_run_error(
    db: &sqlx::SqlitePool,
    run_id: i64,
    collection: &str,
    message: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO sync_run_errors (run_id, collection, message)
         VALUES (?, ?, ?)",
    )
    .bind(run_id)
    .bind(collection)
    .bind(message)
    .execute(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))?;
    Ok(())
}

pub async fn get_run(db: &sqlx::SqlitePool, run_id: i64) -> Result<Option<SyncRunRecord>, String> {
    sqlx::query_as(
        "SELECT id, account_id, service_id, mode, scope, status, started_at,
                finished_at, reported_total, imported_total, error_count
         FROM sync_runs WHERE id = ?",
    )
    .bind(run_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))
}

/// Historial de la cuenta, el más reciente primero (decisión 22).
pub async fn list_runs(
    db: &sqlx::SqlitePool,
    account_id: i64,
    limit: i64,
) -> Result<Vec<SyncRunRecord>, String> {
    sqlx::query_as(
        "SELECT id, account_id, service_id, mode, scope, status, started_at,
                finished_at, reported_total, imported_total, error_count
         FROM sync_runs WHERE account_id = ?
         ORDER BY started_at DESC, id DESC LIMIT ?",
    )
    .bind(account_id)
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))
}

/// Poda a las N más recientes por cuenta (decisión 22): nunca borra catálogo
/// ni fuentes, solo el historial sobrante.
pub async fn prune_runs(db: &sqlx::SqlitePool, account_id: i64, keep: i64) -> Result<u64, String> {
    let result = sqlx::query(
        "DELETE FROM sync_runs
         WHERE account_id = ? AND id NOT IN (
             SELECT id FROM sync_runs WHERE account_id = ?
             ORDER BY started_at DESC, id DESC LIMIT ?
         )",
    )
    .bind(account_id)
    .bind(account_id)
    .bind(keep)
    .execute(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))?;
    Ok(result.rows_affected())
}

/// ¿Hay una corrida en vivo para esta cuenta? También cubre corridas huérfanas
/// tras un cierre brusco: sin registro vivo, el estado `running` de la fila es
/// la señal de ocupación (el job marca su fila al terminar).
pub async fn has_running_run(db: &sqlx::SqlitePool, account_id: i64) -> Result<bool, String> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_runs WHERE account_id = ? AND status = 'running'",
    )
    .bind(account_id)
    .fetch_one(db)
    .await
    .map_err(|e| format!("sync run persistence error: {}", e))?;
    Ok(count > 0)
}
