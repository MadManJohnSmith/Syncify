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
use tauri::{AppHandle, Emitter, Runtime};

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

/// Título y cuerpo de la notificación de un job terminal (#122).
///
/// Devuelve `None` mientras el job no es terminal: nunca se notifica un job
/// `running`. El llamador solo lo usa en la transición terminal, así que cada
/// job notifica su estado final una sola vez.
pub fn terminal_notification(event: &DownloadJobProgressEvent) -> Option<(String, String)> {
    if !event.terminal {
        return None;
    }
    let label = event
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Job #{}", event.job_id));
    let title = match event.status.as_str() {
        "completed" => format!("Descarga completada: {}", label),
        "partial" => format!("Descarga parcial: {}", label),
        "failed" => format!("Descarga fallida: {}", label),
        "cancelled" => format!("Descarga cancelada: {}", label),
        _ => return None,
    };
    let body = format!(
        "{} completadas · {} fallidas · {} omitidas · {} canceladas",
        event.completed, event.failed, event.skipped, event.cancelled
    );
    Some((title, body))
}

/// Último estado terminal notificado por job. `running` borra la entrada: un
/// reintento que reabre el job arma una transición nueva, mientras que dos
/// sondeos del mismo estado terminal no repiten el aviso.
static NOTIFIED_TERMINAL: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();

fn notified_terminal() -> &'static Mutex<HashMap<i64, String>> {
    NOTIFIED_TERMINAL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_notified_terminal() -> std::sync::MutexGuard<'static, HashMap<i64, String>> {
    notified_terminal()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Notificación terminal que toca emitir, si aún no se notificó ese estado.
///
/// Devuelve `None` para `running` (y reinicia la ventana del job) y para un
/// estado terminal ya notificado. Es la parte testable sin Tauri.
pub fn take_terminal_notification(event: &DownloadJobProgressEvent) -> Option<(String, String)> {
    let Some(notification) = terminal_notification(event) else {
        forget_terminal_notification(event.job_id);
        return None;
    };

    let mut notified = lock_notified_terminal();
    if notified.get(&event.job_id).map(String::as_str) == Some(event.status.as_str()) {
        return None;
    }
    notified.insert(event.job_id, event.status.clone());
    Some(notification)
}

/// Olvida el último estado terminal notificado de un job (al limpiarlo o en
/// tests). No toca el registro queue→job.
pub fn forget_terminal_notification(job_id: i64) {
    lock_notified_terminal().remove(&job_id);
}

/// Entrega la notificación terminal fuera del ciclo de sondeo: el estado ya
/// está persistido y `show_notification` respeta las preferencias del usuario.
pub fn spawn_terminal_notification<R: Runtime>(
    app: &AppHandle<R>,
    event: &DownloadJobProgressEvent,
) {
    let Some((title, body)) = take_terminal_notification(event) else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = crate::tray::show_notification(app, title, body).await {
            tracing::warn!(%error, "[job_events] job notification failed");
        }
    });
}

/// Notifica desde los caminos que no reciben `AppHandle` (fallo de preflight o
/// de encolado). Sin app no hace nada.
pub async fn notify_terminal_if_available(db: &SqlitePool, job_id: i64) {
    let Some(app) = crate::commands::get_global_app_handle() else {
        return;
    };
    match jobs::progress_snapshot(db, job_id).await {
        Ok(Some(event)) => spawn_terminal_notification(app, &event),
        Ok(None) => {}
        Err(error) => tracing::warn!(
            job_id = job_id,
            %error,
            "[job_events] terminal notification snapshot failed"
        ),
    }
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
    // #122: una notificación por transición terminal; `running` reinicia la
    // ventana de deduplicación. El spawn no bloquea al watcher ni al worker.
    spawn_terminal_notification(app, &event);
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
    // Las acciones de ciclo (cancelar, reintentar, crear) también cierran el
    // job fuera del sondeo: la transición terminal se notifica desde aquí.
    spawn_terminal_notification(app, &event);
    Ok(Some(event))
}

/// Arranca el watcher cuando la app ya está disponible. Los jobs creados por
/// los caminos de encolado de pistas (single, selección, URL) no reciben el
/// `AppHandle` como argumento; sin esto su transición terminal no se derivaría
/// ni notificaría.
pub fn watch_job_if_available(db: &SqlitePool, job_id: i64) {
    match crate::commands::get_global_app_handle() {
        Some(app) => spawn_job_watch(app.clone(), db.clone(), job_id),
        None => tracing::debug!(
            job_id = job_id,
            "[job_events] job watcher skipped: no app handle"
        ),
    }
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
    fn terminal_states_notify_once_with_the_summary() {
        for status in ["completed", "partial", "failed", "cancelled"] {
            let mut e = event(status, 2);
            e.failed = 1;
            e.skipped = 4;
            e.cancelled = 3;
            e.label = Some("Album X".to_string());
            let (title, body) = terminal_notification(&e).expect("terminal debe notificar");
            assert!(title.contains("Album X"), "{}", title);
            assert!(
                body.contains("2 completadas")
                    && body.contains("1 fallidas")
                    && body.contains("4 omitidas")
                    && body.contains("3 canceladas"),
                "{}",
                body
            );
        }
    }

    #[test]
    fn running_jobs_never_notify() {
        assert!(terminal_notification(&event("running", 0)).is_none());
        assert!(terminal_notification(&event("queued", 0)).is_none());
    }

    #[test]
    fn terminal_without_label_falls_back_to_job_id() {
        let e = event("completed", 3);
        let (title, _) = terminal_notification(&e).unwrap();
        assert!(title.contains("Job #1"), "{}", title);
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
