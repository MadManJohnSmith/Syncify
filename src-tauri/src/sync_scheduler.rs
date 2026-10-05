//! Scheduled library synchronization (Item 29)
//!
//! Arms a background scheduler that fires the same sync flow as the manual
//! one (`tray::sync_all_services`, which drives the canonical
//! `perform_sync_service_with_emitter` engine) using the existing
//! `sync_settings` row: `auto_sync_enabled` toggle plus
//! `sync_interval_value`/`sync_interval_unit` (`minutes` | `hours` | `days`).
//!
//! Guarantees:
//! - The toggle is re-read on every tick, so enabling/disabling takes effect
//!   without restarting the app.
//! - A sync is never fired while another one is running: both the tray entry
//!   and this scheduler share the `SYNC_IN_PROGRESS` guard.
//! - While the environment pause (Item 28: metered network / low battery) is
//!   active, scheduled syncs are skipped too.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;

/// Guard shared with the tray's manual "Sync All Services" entry: a CAS flag
/// so a manual sync and a scheduled sync can never run at the same time.
static SYNC_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Whether a sync (manual or scheduled) is currently running.
pub fn is_sync_in_progress() -> bool {
    SYNC_IN_PROGRESS.load(Ordering::SeqCst)
}

/// Try to acquire the sync slot. Returns `false` if a sync is already running.
pub fn try_begin_sync() -> bool {
    SYNC_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

/// Release the sync slot.
pub fn end_sync() {
    SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
}

/// How often the scheduler re-evaluates the persisted schedule.
const SCHEDULER_TICK_SECS: u64 = 30;

/// Map the persisted `sync_interval_value`/`sync_interval_unit` pair to a
/// duration. Unknown units fall back to hours (the persisted default), and the
/// value is clamped to at least 1.
pub fn interval_duration(value: u32, unit: &str) -> Duration {
    let value = value.max(1) as u64;
    match unit.trim().to_ascii_lowercase().as_str() {
        "minute" | "minutes" => Duration::from_secs(value * 60),
        "day" | "days" => Duration::from_secs(value * 60 * 60 * 24),
        // "hours" es el valor por defecto de sync_settings; cualquier unidad
        // desconocida cae aquí para que el scheduler nunca quede sin cadencia.
        _ => Duration::from_secs(value * 60 * 60),
    }
}

/// The schedule read back from `sync_settings` on every tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SyncSchedule {
    auto_sync_enabled: bool,
    interval: Duration,
}

async fn load_schedule(db: &crate::DbPool) -> Option<SyncSchedule> {
    let row: Option<(bool, i64, String)> = sqlx::query_as(
        "SELECT auto_sync_enabled, sync_interval_value, sync_interval_unit FROM sync_settings WHERE id = 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    row.map(|(auto_sync_enabled, value, unit)| SyncSchedule {
        auto_sync_enabled,
        interval: interval_duration(value.max(1) as u32, &unit),
    })
}

/// Scheduler loop: on every tick it re-reads the schedule and fires
/// `tray::sync_all_services` when the configured interval has elapsed since
/// the last scheduled sync.
async fn scheduler_loop(app: tauri::AppHandle) {
    let mut tick = tokio::time::interval(Duration::from_secs(SCHEDULER_TICK_SECS));
    // Un tick perdido (equipo suspendido) no debe encolar una ráfaga de syncs.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // El primer disparo ocurre tras un intervalo completo desde el arranque;
    // el sync al iniciar la app es territorio del ajuste `sync_on_startup`.
    let mut last_run: Option<std::time::Instant> = None;

    loop {
        tick.tick().await;

        let db = app.state::<crate::AppState>().db.clone();
        let Some(schedule) = load_schedule(&db).await else {
            // sync_settings no existe todavía (primer arranque): reintentar luego.
            continue;
        };

        if !schedule.auto_sync_enabled {
            // Al reactivar el toggle, el siguiente disparo espera un intervalo
            // completo en lugar de recuperar el tiempo apagado.
            last_run = None;
            continue;
        }

        // Respeto del ítem 28: sin sync programado mientras las descargas están
        // auto-pausadas por red medida o batería baja.
        if is_sync_in_progress() || crate::system_conditions::environment_pause_active() {
            continue;
        }

        let elapsed_since_last = last_run.map(|t| t.elapsed()).unwrap_or(Duration::MAX);
        if elapsed_since_last < schedule.interval {
            continue;
        }

        tracing::info!(
            interval_secs = schedule.interval.as_secs(),
            "Scheduled sync interval elapsed; firing sync across active services"
        );
        last_run = Some(std::time::Instant::now());

        if let Err(e) = crate::tray::sync_all_services(&app).await {
            tracing::error!("Scheduled sync failed: {}", e);
        }
    }
}

/// Arms the scheduled sync background task. Called once from `.setup()`.
pub fn start_sync_scheduler(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(scheduler_loop(app));
    tracing::info!(
        tick_secs = SCHEDULER_TICK_SECS,
        "Scheduled sync scheduler armed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_duration_maps_persisted_units() {
        assert_eq!(interval_duration(15, "minutes"), Duration::from_secs(900));
        assert_eq!(interval_duration(2, "hours"), Duration::from_secs(7200));
        assert_eq!(interval_duration(1, "days"), Duration::from_secs(86400));
        // Singular y mayúsculas también se aceptan.
        assert_eq!(interval_duration(30, "Minutes"), Duration::from_secs(1800));
        // Unidad desconocida → horas; valor ≤ 0 → 1.
        assert_eq!(interval_duration(3, "weeks"), Duration::from_secs(3 * 3600));
        assert_eq!(interval_duration(0, "hours"), Duration::from_secs(3600));
    }

    #[test]
    fn sync_guard_is_exclusive() {
        // Estado limpio al entrar (el test corre en un solo proceso por hilo,
        // pero por si acaso liberamos primero).
        end_sync();
        assert!(!is_sync_in_progress());
        assert!(try_begin_sync(), "first acquire must succeed");
        assert!(is_sync_in_progress());
        assert!(!try_begin_sync(), "second acquire must be rejected");
        end_sync();
        assert!(!is_sync_in_progress());
        assert!(try_begin_sync(), "acquire works again after release");
        end_sync();
    }
}
