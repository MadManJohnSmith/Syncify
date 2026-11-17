//! Eventos de corrida (`sync-run-*`) hacia la UI (ADR-0006, «IPC y UI»).
//!
//! Cuatro canales nuevos y simétricos con el catálogo de
//! `ui/src/api/tauri.ts`: `sync-run-started`, `sync-run-progress`,
//! `sync-run-finished` y `sync-run-error`. El gate
//! `ui/src/__tests__/api/tauri_events_symmetry.spec.ts` exige que cada literal
//! emitido desde Rust tenga su constante en el catálogo.

use serde::Serialize;
use tauri::{Emitter, Runtime};

/// Evento de corrida: identidad de la corrida, fase en curso, vistos e
/// importados, tiempo transcurrido y estado de ejecución. El estado de la
/// corrida es el eje de ejecución (decisiones 5 y 9 del ADR-0006), nunca
/// cobertura.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SyncRunEvent {
    pub run_id: i64,
    pub account_id: i64,
    pub service: String,
    pub scope: String,
    /// Fase en curso (las mismas fases que el brazo emite hoy:
    /// `authenticating`, `fetching_favorite_tracks`,
    /// `importing_favorite_tracks`, `persisting`).
    pub phase: String,
    /// `running` mientras corre; el terminal llega con el estado final
    /// (`completed` | `partial` | `failed` | `cancelled`).
    pub status: String,
    /// Ítems vistos en la corrida.
    pub seen: u64,
    /// Ítems importados en la corrida.
    pub imported: u64,
    /// Total que el proveedor reporta, si lo publica.
    pub reported_total: Option<i64>,
    pub error_count: u64,
    pub elapsed_ms: u64,
    pub terminal: bool,
    pub message: String,
}

impl SyncRunEvent {
    pub fn started(
        run_id: i64,
        account_id: i64,
        service: &str,
        scope: &str,
        elapsed_ms: u64,
    ) -> Self {
        Self {
            run_id,
            account_id,
            service: service.to_string(),
            scope: scope.to_string(),
            phase: "started".to_string(),
            status: "running".to_string(),
            seen: 0,
            imported: 0,
            reported_total: None,
            error_count: 0,
            elapsed_ms,
            terminal: false,
            message: "Sync run started".to_string(),
        }
    }

    pub fn finished(
        run_id: i64,
        account_id: i64,
        service: &str,
        scope: &str,
        status: &str,
        seen: u64,
        imported: u64,
        reported_total: Option<i64>,
        error_count: u64,
        elapsed_ms: u64,
        message: String,
    ) -> Self {
        Self {
            run_id,
            account_id,
            service: service.to_string(),
            scope: scope.to_string(),
            phase: "finished".to_string(),
            status: status.to_string(),
            seen,
            imported,
            reported_total,
            error_count,
            elapsed_ms,
            terminal: true,
            message,
        }
    }
}

/// Trait de emisión de corrida, paralelo a `SyncProgressEmitter`: la
/// producción lo implementa `AppHandle` (emisión real al frontend) y los
/// tests un colector en memoria.
pub trait SyncRunEmitter: Send + Sync {
    fn emit_started(&self, event: &SyncRunEvent);
    fn emit_progress(&self, event: &SyncRunEvent);
    fn emit_finished(&self, event: &SyncRunEvent);
    fn emit_error(&self, event: &SyncRunEvent);
}

impl<R: Runtime> SyncRunEmitter for tauri::AppHandle<R> {
    fn emit_started(&self, event: &SyncRunEvent) {
        let _ = self.emit("sync-run-started", event);
    }

    fn emit_progress(&self, event: &SyncRunEvent) {
        let _ = self.emit("sync-run-progress", event);
    }

    fn emit_finished(&self, event: &SyncRunEvent) {
        let _ = self.emit("sync-run-finished", event);
    }

    fn emit_error(&self, event: &SyncRunEvent) {
        let _ = self.emit("sync-run-error", event);
    }
}

/// No-op: para pruebas y rutas sin frontend.
impl SyncRunEmitter for () {
    fn emit_started(&self, _event: &SyncRunEvent) {}
    fn emit_progress(&self, _event: &SyncRunEvent) {}
    fn emit_finished(&self, _event: &SyncRunEvent) {}
    fn emit_error(&self, _event: &SyncRunEvent) {}
}
