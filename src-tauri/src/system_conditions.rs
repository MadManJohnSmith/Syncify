//! Environment-driven download pauses (Item 28): metered network and low battery
//!
//! Real detection on Linux over D-Bus:
//! - NetworkManager: `org.freedesktop.NetworkManager` property `Metered`
//!   (`u`: 0 unknown, 1 yes, 2 no, 3 guess-yes, 4 guess-no).
//! - UPower: `org.freedesktop.UPower.Device` on the aggregated
//!   `/org/freedesktop/UPower/devices/DisplayDevice` (`Percentage` `d`,
//!   `Type` `u` — 2 is Battery, `State` `u` — 2 is Discharging).
//!
//! The D-Bus query runs through the standard `busctl` client (systemd) with a
//! `gdbus` (glib) fallback; both are verified on desktop Linux. Any failure —
//! client missing, service missing, timeout — is a silent fallback: the
//! network is assumed unmetered and the battery OK, never the opposite.
//!
//! When the persisted `sync_settings.pause_on_metered` /
//! `pause_on_low_battery` flags ask for it, the download worker is paused
//! while the condition holds and resumed when it clears. A pause set by the
//! user always wins: the watcher only resumes a pause it set itself.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;

/// Battery percentage at or below which downloads pause while discharging.
pub const LOW_BATTERY_THRESHOLD_PERCENT: f64 = 20.0;

/// How often the environment is re-evaluated.
pub const ENVIRONMENT_CHECK_INTERVAL_SECS: u64 = 30;

/// `true` when the user (or this watcher) paused the worker.
static AUTO_PAUSED_BY_ENVIRONMENT: AtomicBool = AtomicBool::new(false);

/// Whether the environment watcher currently holds the worker paused.
pub fn environment_pause_active() -> bool {
    AUTO_PAUSED_BY_ENVIRONMENT.load(Ordering::SeqCst)
}

/// Called whenever the user pauses downloads manually (tray/UI): from that
/// moment the user owns the pause and the watcher must not resume it.
pub fn notify_user_pause() {
    AUTO_PAUSED_BY_ENVIRONMENT.store(false, Ordering::SeqCst);
}

/// Same as [`notify_user_pause`] for the user's explicit resume.
pub fn notify_user_resume() {
    AUTO_PAUSED_BY_ENVIRONMENT.store(false, Ordering::SeqCst);
}

/// NetworkManager `Metered` values that count as metered: explicit yes (1) and
/// guess-yes (3). Unknown (0), no (2) and guess-no (4) do not pause.
pub fn nm_metered_is_metered(value: u32) -> bool {
    value == 1 || value == 3
}

/// UPower aggregated `DisplayDevice`: pause only when a real battery (Type 2)
/// is discharging (State 2) at or below the threshold. Charging or absent
/// batteries never pause.
pub fn battery_is_low(percentage: f64, device_type: u32, state: u32) -> bool {
    device_type == 2 && state == 2 && percentage <= LOW_BATTERY_THRESHOLD_PERCENT
}

/// Extracts the first number from a property dump (`busctl` prints `u 4`,
/// `d 93.5`; `gdbus` prints `(<uint32 4>,)`, `(<93.5>,)`). D-Bus type names
/// that contain digits (`uint32`, `int64`, …) are stripped first: otherwise
/// `(<uint32 4>,)` yields 32 instead of 4 and, through the gdbus fallback, a
/// metered network would never be detected.
fn parse_first_number(output: &str) -> Option<f64> {
    // gdbus decora cada valor con su tipo D-Bus; nombres como `uint32`
    // contienen dígitos que contaminan el primer número troceado.
    let cleaned = output
        .replace("uint32", " ")
        .replace("int32", " ")
        .replace("uint64", " ")
        .replace("int64", " ")
        .replace("uint16", " ")
        .replace("int16", " ");
    cleaned
        .trim()
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|chunk| !chunk.is_empty())
        .and_then(|chunk| chunk.parse::<f64>().ok())
}

/// Runs a D-Bus property query through `busctl` first and `gdbus` as fallback,
/// returning the combined stdout of the first successful client. All requested
/// properties must succeed, otherwise the answer is `None` (silent fallback).
async fn dbus_get_property(
    dest: &str,
    object_path: &str,
    interface: &str,
    props: &[&str],
) -> Option<String> {
    // busctl acepta varias propiedades en una llamada y las imprime en orden.
    if let Ok(output) = tokio::process::Command::new("busctl")
        .arg("--system")
        .arg("get-property")
        .arg(dest)
        .arg(object_path)
        .arg(interface)
        .args(props)
        .output()
        .await
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if !stdout.trim().is_empty() {
                return Some(stdout);
            }
        }
    }

    // Fallback: gdbus (parte de glib, presente donde corre la propia app);
    // acepta una propiedad por llamada, así que se concatenan en orden.
    let mut combined = String::new();
    for prop in props {
        let output = tokio::process::Command::new("gdbus")
            .arg("call")
            .arg("--system")
            .arg("--dest")
            .arg(dest)
            .arg("--object-path")
            .arg(object_path)
            .arg("--method")
            .arg("org.freedesktop.DBus.Properties.Get")
            .arg(interface)
            .arg(prop)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
    }

    if combined.trim().is_empty() {
        None
    } else {
        Some(combined)
    }
}

/// Queries NetworkManager's `Metered` property. `None` = service unavailable
/// (silent fallback: treat as unmetered).
pub async fn query_nm_metered() -> Option<u32> {
    let out = dbus_get_property(
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        &["Metered"],
    )
    .await?;
    parse_first_number(&out).map(|v| v as u32)
}

/// Queries UPower's aggregated `DisplayDevice`: `(percentage, type, state)`.
/// `None` = UPower unavailable or no battery data (silent fallback: battery OK).
pub async fn query_upower_battery() -> Option<(f64, u32, u32)> {
    let out = dbus_get_property(
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.UPower.Device",
        &["Percentage", "Type", "State"],
    )
    .await?;

    // busctl imprime una línea por propiedad, en el mismo orden pedido.
    let mut values = out.lines().filter_map(parse_first_number);
    let percentage = values.next()?;
    let device_type = values.next()? as u32;
    let state = values.next()? as u32;
    Some((percentage, device_type, state))
}

/// Real detection: why downloads should be paused right now, if at all.
pub async fn detect_pause_reason(
    pause_on_metered: bool,
    pause_on_low_battery: bool,
) -> Option<String> {
    if pause_on_metered {
        match query_nm_metered().await {
            Some(metered) if nm_metered_is_metered(metered) => {
                return Some("metered network connection".to_string());
            }
            // Sin NetworkManager (None) o red no medida: no pausar por red.
            _ => {}
        }
    }

    if pause_on_low_battery {
        match query_upower_battery().await {
            Some((percentage, device_type, state))
                if battery_is_low(percentage, device_type, state) =>
            {
                return Some(format!("low battery ({percentage:.0}% discharging)"));
            }
            // Sin UPower, sin batería o cargando: fallback silencioso, batería OK.
            _ => {}
        }
    }

    None
}

/// Loads `pause_on_metered` / `pause_on_low_battery` from `sync_settings`.
async fn load_pause_flags(db: &crate::DbPool) -> (bool, bool) {
    let row: Option<(bool, bool)> = sqlx::query_as(
        "SELECT pause_on_metered, pause_on_low_battery FROM sync_settings WHERE id = 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    // Defaults de sync_settings (settings.rs): ambas pausas activadas.
    row.unwrap_or((true, true))
}

/// One watcher iteration: apply the environment pause policy to the worker.
async fn apply_environment_pause(app: &tauri::AppHandle) {
    // El estado se suelta al cerrar el bloque: no se retiene durante la
    // detección de red/batería.
    let (pause_on_metered, pause_on_low_battery, worker_state) = {
        let state = app.state::<crate::AppState>();
        let (pause_on_metered, pause_on_low_battery) = load_pause_flags(&state.db).await;
        (
            pause_on_metered,
            pause_on_low_battery,
            state.worker_state.clone(),
        )
    };

    match detect_pause_reason(pause_on_metered, pause_on_low_battery).await {
        Some(reason) => {
            if !worker_state.is_paused() {
                worker_state.pause();
                AUTO_PAUSED_BY_ENVIRONMENT.store(true, Ordering::SeqCst);
                tracing::warn!("Downloads auto-paused by environment: {}", reason);
            }
        }
        None => {
            // La condición se resolvió: reanudar SOLO la pausa que puso el
            // watcher (una pausa del usuario nunca se levanta sola).
            if AUTO_PAUSED_BY_ENVIRONMENT.swap(false, Ordering::SeqCst) && worker_state.is_paused()
            {
                worker_state.resume();
                tracing::info!("Downloads auto-resumed: environment conditions cleared");
            }
        }
    }
}

/// Watcher loop applying the pause policy every `ENVIRONMENT_CHECK_INTERVAL_SECS`.
async fn environment_watch_loop(app: tauri::AppHandle) {
    let mut tick = tokio::time::interval(Duration::from_secs(ENVIRONMENT_CHECK_INTERVAL_SECS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        apply_environment_pause(&app).await;
    }
}

/// Arms the environment watcher. Called once from `.setup()`.
pub fn start_environment_watch(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(environment_watch_loop(app));
    tracing::info!(
        interval_secs = ENVIRONMENT_CHECK_INTERVAL_SECS,
        threshold_percent = LOW_BATTERY_THRESHOLD_PERCENT,
        "Environment watch armed (NetworkManager metered + UPower low battery)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busctl_and_gdbus_outputs_parse() {
        assert_eq!(parse_first_number("u 4"), Some(4.0));
        assert_eq!(parse_first_number("u 1\n"), Some(1.0));
        assert_eq!(parse_first_number("d 93.5"), Some(93.5));
        assert_eq!(parse_first_number("(<uint32 4>,)"), Some(4.0));
        assert_eq!(parse_first_number("(<93.5>,)"), Some(93.5));
        assert_eq!(parse_first_number(""), None);
        assert_eq!(parse_first_number("(,)"), None);
    }

    #[test]
    fn nm_metered_values_map_to_pause_only_on_yes() {
        assert!(nm_metered_is_metered(1), "explicit yes pauses");
        assert!(nm_metered_is_metered(3), "guess-yes pauses");
        assert!(!nm_metered_is_metered(0), "unknown does not pause");
        assert!(!nm_metered_is_metered(2), "no does not pause");
        assert!(!nm_metered_is_metered(4), "guess-no does not pause");
    }

    #[test]
    fn battery_pause_requires_battery_discharging_below_threshold() {
        assert!(battery_is_low(15.0, 2, 2), "discharging at 15% pauses");
        assert!(battery_is_low(20.0, 2, 2), "threshold is inclusive");
        assert!(
            !battery_is_low(21.0, 2, 2),
            "above threshold does not pause"
        );
        assert!(!battery_is_low(10.0, 2, 1), "charging does not pause");
        assert!(
            !battery_is_low(10.0, 0, 2),
            "no real battery does not pause"
        );
    }

    #[test]
    fn environment_pause_flag_tracks_notify_calls() {
        notify_user_pause();
        assert!(
            !environment_pause_active(),
            "user pause releases the watcher flag"
        );

        AUTO_PAUSED_BY_ENVIRONMENT.store(true, Ordering::SeqCst);
        assert!(environment_pause_active());
        notify_user_resume();
        assert!(!environment_pause_active());
    }
}
