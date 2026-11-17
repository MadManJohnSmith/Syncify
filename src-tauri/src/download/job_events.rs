//! Eventos de progreso de jobs de descarga (D07, spec #111 / issue #119).
//!
//! El estado del job y el throttle del evento viven aquí, no en worker.rs: el
//! worker no necesita saber que el job existe. El watcher sondea la cola (la
//! fuente de verdad), refresca el estado del job con [`super::jobs`] y emite
//! `syncify:download_job_progress` al crear, en cada cambio con throttle y
//! siempre en las transiciones terminales y al cancelar/reanudar.
//!
//! Este módulo también guarda el registro `queue_id → job_id` que la emisión
//! de `syncify:download_progress` necesita para ganar `job_id` (ver el patch
//! documentado en el PR: la emisión vive en worker.rs, carril hermano).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use super::jobs::{self, DownloadJobProgressEvent, JobError};

/// Emisiones intermedias como mucho una por ventana y job; las terminales y
/// las que disparan los comandos de ciclo (crear, cancelar, reanudar) pasan
/// siempre. El watcher sondea cada segundo, así que la ventana corta no
/// retrasa nada en la práctica: el siguiente sondeo emite el cambio.
const EMIT_THROTTLE: Duration = Duration::from_millis(500);

/// Sondeo del watcher mientras el job tiene filas en vuelo o está encolando.
const WATCH_POLL: Duration = Duration::from_millis(1000);

/// Firma del último payload emitido por job: emitir solo si cambió.
#[derive(Debug, Clone, PartialEq)]
struct EmitState {
    at: Instant,
    key: (String, i64, i64, i64, i64, i64, i64, i64, i64),
}

fn snapshot_key(
    event: &DownloadJobProgressEvent,
) -> (String, i64, i64, i64, i64, i64, i64, i64, i64) {
    (
        event.status.clone(),
        event.total,
        event.queued,
        event.downloading,
        event.completed,
        event.failed,
        event.cancelled,
        event.skipped,
        (event.percent * 10.0) as i64,
    )
}

fn last_emitted() -> &'static Mutex<HashMap<i64, EmitState>> {
    static LAST: OnceLock<Mutex<HashMap<i64, EmitState>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_last() -> std::sync::MutexGuard<'static, HashMap<i64, EmitState>> {
    last_emitted()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `true` si el payload cambió y la ventana de throttle lo permite. Las
/// transiciones terminales pasan siempre. Guarda la firma del emitido.
fn should_emit(job_id: i64, event: &DownloadJobProgressEvent) -> bool {
    let key = snapshot_key(event);
    let mut map = lock_last();
    match map.get(&job_id) {
        Some(state) if state.key == key => return false,
        Some(state) if !event.terminal && state.at.elapsed() < EMIT_THROTTLE => return false,
        _ => {}
    }
    map.insert(
        job_id,
        EmitState {
            at: Instant::now(),
            key,
        },
    );
    true
}

/// Fuerza la emisión (crear/cancelar/reanudar): actualiza la firma sin mirar
/// la ventana, para que el siguiente sondeo no reemita lo mismo.
fn force_state(job_id: i64, event: &DownloadJobProgressEvent) {
    lock_last().insert(
        job_id,
        EmitState {
            at: Instant::now(),
            key: snapshot_key(event),
        },
    );
}

/// Registro `queue_id → job_id` para enriquecer `syncify:download_progress`
/// con `job_id`. Lo rellena el watcher por sondeo (cubre filas encoladas por
/// cualquier camino) y lo poda al terminar el job o al limpiar el historial.
///
/// Su único lector en producción es la emisión de `syncify:download_progress`
/// en worker.rs, que este carril no toca (carril hermano #85): el patch del
/// merge llama a [`queue_job_id`]. Hasta que se aplique, queda sin llamadas.
#[allow(dead_code)]
static QUEUE_JOBS: OnceLock<Mutex<HashMap<i64, i64>>> = OnceLock::new();

fn queue_jobs() -> &'static Mutex<HashMap<i64, i64>> {
    static REG: OnceLock<Mutex<HashMap<i64, i64>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_registry() -> std::sync::MutexGuard<'static, HashMap<i64, i64>> {
    queue_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// El job de una fila de la cola, si está registrado.
///
/// Consumidor previsto: la emisión de `syncify:download_progress` en
/// worker.rs (patch documentado en el PR, carril hermano #85). Sin él queda
/// sin llamadas en producción; los tests de abajo fijan su contrato.
#[allow(dead_code)]
pub fn queue_job_id(queue_id: i64) -> Option<i64> {
    lock_registry().get(&queue_id).copied()
}

async fn remember_job_queues(db: &SqlitePool, job_id: i64) -> Result<(), JobError> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM download_queue WHERE job_id = ?")
        .bind(job_id)
        .fetch_all(db)
        .await?;
    let mut registry = lock_registry();
    for id in ids {
        registry.insert(id, job_id);
    }
    Ok(())
}

/// Poda las entradas del registro de un job (terminal, borrado o limpiado).
pub fn forget_job_queues(job_id: i64) {
    lock_registry().retain(|_, job| *job != job_id);
}

/// Refresca el estado del job desde la cola y calcula el payload del evento.
/// Sin `AppHandle`: es la parte testable del ciclo.
pub async fn refresh_and_snapshot(
    db: &SqlitePool,
    job_id: i64,
) -> Result<Option<DownloadJobProgressEvent>, JobError> {
    jobs::refresh_job_status(db, job_id, None).await?;
    remember_job_queues(db, job_id).await?;
    jobs::progress_snapshot(db, job_id).await
}

/// Un sondeo del watcher: refresca, emite si toca y devuelve `Some(true)` en
/// la transición terminal, `Some(false)` mientras queda trabajo y `None` si el
/// job ya no existe (limpiado mientras miraba).
pub async fn poll_job(
    app: &AppHandle,
    db: &SqlitePool,
    job_id: i64,
) -> Result<Option<bool>, JobError> {
    let Some(event) = refresh_and_snapshot(db, job_id).await? else {
        forget_job_queues(job_id);
        return Ok(None);
    };
    if should_emit(job_id, &event) {
        let _ = app.emit("syncify:download_job_progress", &event);
    }
    if event.terminal {
        forget_job_queues(job_id);
    }
    Ok(Some(event.terminal))
}

/// Emisión inmediata por acción del usuario (crear, cancelar, reanudar):
/// pasa el throttle y fija la firma para no reemitir en el siguiente sondeo.
pub async fn emit_job_progress(
    app: &AppHandle,
    db: &SqlitePool,
    job_id: i64,
) -> Result<Option<DownloadJobProgressEvent>, JobError> {
    let Some(event) = jobs::progress_snapshot(db, job_id).await? else {
        return Ok(None);
    };
    force_state(job_id, &event);
    let _ = app.emit("syncify:download_job_progress", &event);
    Ok(Some(event))
}

/// Vigila un job hasta su transición terminal (o hasta que alguien lo borre).
/// Emite al crear (primer sondeo) y en cada cambio con throttle.
pub fn spawn_job_watch(app: AppHandle, db: SqlitePool, job_id: i64) {
    tauri::async_runtime::spawn(async move {
        loop {
            match poll_job(&app, &db, job_id).await {
                Ok(Some(true)) | Ok(None) => break,
                Ok(Some(false)) => {}
                Err(error) => {
                    tracing::warn!(
                        job_id = job_id,
                        %error,
                        "[job_events] job watcher stopped on error"
                    );
                    break;
                }
            }
            tokio::time::sleep(WATCH_POLL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(status: &str, completed: i64) -> DownloadJobProgressEvent {
        DownloadJobProgressEvent {
            job_id: 1,
            label: None,
            status: status.to_string(),
            total: 3,
            queued: 0,
            downloading: 0,
            completed,
            failed: 0,
            cancelled: 0,
            skipped: 0,
            percent: (completed as f64 / 3.0) * 100.0,
            terminal: jobs::JOB_TERMINAL_STATUSES.contains(&status),
        }
    }

    #[test]
    fn first_emit_passes_and_repeats_are_suppressed() {
        let e = event("running", 0);
        assert!(should_emit(7, &e));
        assert!(!should_emit(7, &e), "mismo payload: sin emisión");
        forget_job_queues(7);
    }

    #[test]
    fn throttle_blocks_intermediate_but_not_terminal() {
        let e = event("running", 0);
        assert!(should_emit(8, &e));
        let changed = event("running", 1);
        // Dentro de la ventana y sin ser terminal: se suprime.
        assert!(!should_emit(8, &changed));
        let term = event("completed", 3);
        // Terminal: pasa siempre.
        assert!(should_emit(8, &term));
        forget_job_queues(8);
    }

    #[test]
    fn different_jobs_do_not_share_the_throttle() {
        let e = event("running", 0);
        assert!(should_emit(9, &e));
        assert!(should_emit(10, &e));
        forget_job_queues(9);
        forget_job_queues(10);
    }

    #[test]
    fn registry_roundtrip_and_forget() {
        // `queue_job_id` no debe depender de otros tests: ids exclusivos.
        let job = 987_654;
        let mut registry = lock_registry();
        registry.insert(4_242_424, job);
        drop(registry);
        assert_eq!(queue_job_id(4_242_424), Some(job));
        assert_eq!(queue_job_id(4_242_425), None);
        forget_job_queues(job);
        assert_eq!(queue_job_id(4_242_424), None);
    }
}
