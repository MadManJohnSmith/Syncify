//! Seam 1 (spec #64, «Testing Decisions»): el trabajo de sincronización con
//! identidad — `SyncRun` con estados, progreso, cancelación e historial.
//!
//! El job recorre las páginas que declara el adaptador (seam 2), comprueba la
//! cancelación en el límite de página (regla derivada del ADR-0006: se detiene
//! conservando lo importado, sin reanudación a mitad), y al cerrar escribe el
//! estado de la corrida, la cobertura por colección y los huecos registrados.
//! Para Qobuz + favoritos el importador es el mismo del orquestador
//! (`sync_qobuz_favorites_phase`): el resultado de importación es el de antes
//! por construcción, no por duplicación.

use super::events::{SyncRunEmitter, SyncRunEvent};
use super::runs;
use super::source::LibrarySource;
use crate::commands::progress::SyncProgressEmitter;
use crate::commands::service::{sync_qobuz_favorites_phase, QobuzFavoritesOutcome, SyncOutcome};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Registro de corridas vivas: el id de la corrida apunta a su bandera de
/// cancelación. Vivir en memoria es suficiente: si el proceso muere, la fila
/// queda `running` huérfana y el registro de vivas la desconoce —
/// `cancel_sync_run` la cierra como `cancelled` leyendo la fila.
fn live_runs() -> &'static Mutex<HashMap<i64, Arc<AtomicBool>>> {
    static LIVE: OnceLock<Mutex<HashMap<i64, Arc<AtomicBool>>>> = OnceLock::new();
    LIVE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_live_run(run_id: i64) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    live_runs()
        .lock()
        .expect("live run registry")
        .insert(run_id, flag.clone());
    flag
}

pub fn unregister_live_run(run_id: i64) {
    live_runs()
        .lock()
        .expect("live run registry")
        .remove(&run_id);
}

/// Pide cancelación (criterio 2 del issue #65). `true` si la corrida está
/// viva y la bandera quedó marcada; el job la atiende en el límite de página.
pub fn request_cancel(run_id: i64) -> bool {
    match live_runs().lock().expect("live run registry").get(&run_id) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Identidad de la corrida que viaja en los eventos.
#[derive(Clone)]
pub struct RunMeta {
    pub run_id: i64,
    pub account_id: i64,
    pub service: String,
    pub scope: String,
}

/// Relé de progreso: reenvía los eventos del brazo por los canales existentes
/// (`sync-progress`/…) y los traduce a `sync-run-progress` con la identidad de
/// la corrida. La ficha de cuenta consume el segundo y el resto de la UI no
/// deja de recibir los primeros. Los eventos terminales del brazo no se
/// traducen aquí: el cierre lo emite el job con el estado de la fila.
struct RunProgressRelay<'a, E: SyncProgressEmitter, R: SyncRunEmitter> {
    meta: RunMeta,
    inner: Option<&'a E>,
    run_emitter: &'a R,
    started: std::time::Instant,
}

impl<E: SyncProgressEmitter, R: SyncRunEmitter> SyncProgressEmitter for RunProgressRelay<'_, E, R> {
    fn emit_sync_progress(&self, event: &crate::commands::types::SyncProgressEvent) {
        if let Some(inner) = self.inner {
            inner.emit_sync_progress(event);
        }
        if event.terminal {
            return;
        }
        self.run_emitter.emit_progress(&SyncRunEvent {
            run_id: self.meta.run_id,
            account_id: self.meta.account_id,
            service: self.meta.service.clone(),
            scope: self.meta.scope.clone(),
            phase: event.phase.clone(),
            status: event.status.clone(),
            seen: event.favorite_tracks_total,
            imported: event.imported_tracks_total,
            reported_total: None,
            error_count: 0,
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            terminal: false,
            message: event.message.clone(),
        });
    }
}

/// Resultado de la corrida tal como queda en la fila.
#[derive(Debug, Clone)]
#[allow(dead_code)] // `reported_total`/`error_count` los leen `tests/sync_run_job_fake_source_test.rs` y `tests/sync_run_qobuz_parity_test.rs`.
pub struct SyncRunResult {
    pub status: &'static str,
    pub reported_total: Option<i64>,
    pub imported_total: i64,
    pub error_count: i64,
}

/// Ejecuta la corrida de favoritos (scope `Favorites`) sobre el adaptador dado
/// y cierra su fila con el estado correcto:
///
/// - cancelada → `cancelled` (lo importado se conserva; no se reanuda);
/// - credencial inválida → `failed` (decisión 20);
/// - huecos registrados → `partial` (decisión 17);
/// - sin huecos → `completed`.
pub async fn run_favorites_job<S, E, R>(
    db: &sqlx::SqlitePool,
    meta: &RunMeta,
    service_id: i64,
    source: &S,
    prefs: &crate::commands::ImportPreferences,
    emitter: Option<&E>,
    run_emitter: &R,
    cancel: Arc<AtomicBool>,
) -> Result<SyncRunResult, String>
where
    S: LibrarySource,
    E: SyncProgressEmitter,
    R: SyncRunEmitter,
{
    let started = std::time::Instant::now();
    tracing::debug!(
        "sync run {}: starting favorites walk for {} (account {})",
        meta.run_id,
        source.service_name(),
        meta.account_id
    );

    run_emitter.emit_started(&SyncRunEvent::started(
        meta.run_id,
        meta.account_id,
        &meta.service,
        &meta.scope,
        0,
    ));

    let enrichment_engine = crate::services::enrichment::EnrichmentEngine::with_db(db.clone());
    let mut arm = SyncOutcome::default();

    let relay = RunProgressRelay {
        meta: meta.clone(),
        inner: emitter,
        run_emitter,
        started,
    };

    let phase = sync_qobuz_favorites_phase(
        db,
        &meta.service,
        meta.account_id,
        service_id,
        source,
        prefs,
        Some(&relay),
        &enrichment_engine,
        &mut arm,
        Some(&cancel),
    )
    .await;

    let phase = match phase {
        Ok(outcome) => outcome,
        Err(error) => {
            // Fallo de persistencia o del entorno: la corrida falla y el error
            // queda registrado; nada se convierte en un éxito mudo.
            let _ = runs::record_run_error(db, meta.run_id, "favorites", &error).await;
            let _ = runs::finish_run(db, meta.run_id, "failed", None, 0, 1).await;
            let _ = runs::prune_runs(db, meta.account_id, runs::KEEP_RUNS_PER_ACCOUNT).await;
            run_emitter.emit_error(&SyncRunEvent::finished(
                meta.run_id,
                meta.account_id,
                &meta.service,
                &meta.scope,
                "failed",
                0,
                0,
                None,
                1,
                started.elapsed().as_millis() as u64,
                error.clone(),
            ));
            unregister_live_run(meta.run_id);
            return Err(error);
        }
    };

    let (status, message) = match phase {
        QobuzFavoritesOutcome::AuthFailed(msg) => {
            // El rechazo de sesión es un hueco total: entra en el mismo canal
            // de huecos para que error_count y sync_run_errors cuadren.
            arm.errors.push(msg.clone());
            ("failed", msg)
        }
        QobuzFavoritesOutcome::Cancelled => (
            "cancelled",
            format!(
                "Sync run cancelled: {} seen, {} imported (work preserved)",
                arm.favorite_tracks_total, arm.imported_tracks_total
            ),
        ),
        QobuzFavoritesOutcome::Completed => {
            if arm.errors.is_empty() {
                (
                    "completed",
                    format!(
                        "Sync run completed: {} seen, {} imported",
                        arm.favorite_tracks_total, arm.imported_tracks_total
                    ),
                )
            } else {
                (
                    "partial",
                    format!(
                        "Sync run partial: {} seen, {} imported, {} gap(s)",
                        arm.favorite_tracks_total,
                        arm.imported_tracks_total,
                        arm.errors.len()
                    ),
                )
            }
        }
    };

    for error in &arm.errors {
        let _ = runs::record_run_error(db, meta.run_id, "favorites", error).await;
    }

    // Cierre de los contadores de la cuenta: el mismo snapshot final que el
    // orquestador escribe tras sus brazos (sin errores → complete; con
    // importados y huecos → partial; sin nada → error). Ni un error ni una
    // cancelación certifican una colección completa.
    let snapshot_state = if arm.errors.is_empty() && status != "cancelled" {
        "complete"
    } else if arm.favorite_tracks_total > 0 {
        "partial"
    } else {
        "error"
    };
    if arm.reported_favorites.is_some() {
        crate::commands::service::record_account_import_count(
            db,
            meta.account_id,
            "favorites",
            arm.reported_favorites,
            arm.favorite_tracks_total as i64,
            snapshot_state,
        )
        .await?;
    }

    // Estado de la corrida (eje de ejecución)...
    let result = runs::finish_run(
        db,
        meta.run_id,
        status,
        arm.reported_favorites,
        arm.imported_tracks_total as i64,
        arm.errors.len() as i64,
    )
    .await;
    // ...y cobertura por colección (eje de cobertura), nunca mezclados.
    let coverage = runs::upsert_run_collection(
        db,
        meta.run_id,
        "favorites",
        arm.reported_favorites,
        arm.favorite_tracks_total as i64,
    )
    .await;
    // Historial podado a las últimas 20 por cuenta (decisión 22).
    let prune = runs::prune_runs(db, meta.account_id, runs::KEEP_RUNS_PER_ACCOUNT).await;

    if let Err(e) = result.and(coverage).and(prune) {
        unregister_live_run(meta.run_id);
        return Err(e);
    }

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let final_event = SyncRunEvent::finished(
        meta.run_id,
        meta.account_id,
        &meta.service,
        &meta.scope,
        status,
        arm.favorite_tracks_total,
        arm.imported_tracks_total,
        arm.reported_favorites,
        arm.errors.len() as u64,
        elapsed_ms,
        message,
    );
    if status == "failed" {
        run_emitter.emit_error(&final_event);
    } else {
        run_emitter.emit_finished(&final_event);
    }

    unregister_live_run(meta.run_id);
    Ok(SyncRunResult {
        status,
        reported_total: arm.reported_favorites,
        imported_total: arm.imported_tracks_total as i64,
        error_count: arm.errors.len() as i64,
    })
}
