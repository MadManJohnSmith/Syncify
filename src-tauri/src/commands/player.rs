//! Fase 1 del plan de reproducción: comandos Tauri del motor nativo
//! (`syncify-player`, rodio + symphonia).
//!
//! Sustituye la ruta `<audio>` + `syncify-media://`, demostrada rota en el
//! motor media de WebKitGTK (`FormatError`; ver
//! `docs/auditoria/playback-fase0-evidencia-2026-10-17.md`).
//!
//! Reparto de responsabilidades:
//! - Aquí: UNA pista y su transporte (cargar, pausar, reanudar, buscar,
//!   volumen, ritmo). El motor se abre perezosamente en el primer
//!   `player_load`: sin salida de audio la app sigue arrancando.
//! - En la UI (`usePlayer.ts`): cola, historial, modos, crédito de escucha y
//!   persistencia.
//!
//! Eventos (el frontend descarta todo lo que no case con su `seq`):
//! - `player:tick {seq, state, position_sec, duration_sec}` — cada 250 ms
//!   mientras suena, y una vez tras cada cambio de transporte (carga, pausa,
//!   seek en pausa...).
//! - `player:ended {seq}` — una única vez por carga cuando la pista llegó a su
//!   final natural (no con `player_stop`).

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use syncify_player::{EngineError, EngineState, PlayerEngine};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::services::mpris::{MprisMetadataInput, MprisService, PlaybackStatus};

/// Intervalo del ticker de posición.
const TICK_INTERVAL: Duration = Duration::from_millis(250);

/// Payload de `player:tick`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerTickPayload {
    /// Generación de la carga actual (1, 2, ...). La UI compara con el `seq`
    /// devuelto por `player_load` y descarta eventos de una pista ya sustituida.
    pub seq: u64,
    /// `"stopped" | "playing" | "paused"`.
    pub state: &'static str,
    pub position_sec: f64,
    pub duration_sec: Option<f64>,
}

/// Payload de `player:ended`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerEndedPayload {
    pub seq: u64,
}

/// Resultado de `player_load`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerLoadResult {
    pub seq: u64,
    pub duration_sec: Option<f64>,
}

#[derive(Default)]
struct PlayerCore {
    engine: Option<PlayerEngine>,
    seq: u64,
    /// Fuerza un `player:tick` en el siguiente paso del ticker aunque el
    /// transporte esté parado (p. ej. seek en pausa: la posición cambió sin
    /// ticks de reproducción).
    dirty: bool,
    /// `player:ended` ya emitido para la carga actual.
    ended_reported: bool,
}

/// Estado gestionado del player (singleton de la app).
#[derive(Default)]
pub struct PlayerState {
    core: Mutex<PlayerCore>,
}

fn state_str(state: EngineState) -> &'static str {
    match state {
        EngineState::Stopped => "stopped",
        EngineState::Playing => "playing",
        EngineState::Paused => "paused",
    }
}

fn engine_error(err: EngineError) -> String {
    err.to_string()
}

fn lock_core<'a>(
    state: &'a State<'_, PlayerState>,
) -> Result<std::sync::MutexGuard<'a, PlayerCore>, String> {
    state
        .core
        .lock()
        .map_err(|_| "player state poisoned".to_string())
}

/// Carga un archivo local y, opcionalmente, lo reproduce. Con
/// `autoplay = false` queda cargado y pausado en la posición 0 (restauración).
#[tauri::command]
pub fn player_load(
    state: State<'_, PlayerState>,
    path: String,
    autoplay: bool,
    volume: f64,
) -> Result<PlayerLoadResult, String> {
    let mut core = lock_core(&state)?;
    if core.engine.is_none() {
        core.engine = Some(PlayerEngine::new().map_err(engine_error)?);
    }
    let Some(engine) = core.engine.as_mut() else {
        return Err("player engine unavailable".to_string());
    };
    engine.set_volume(volume.clamp(0.0, 1.0) as f32);
    let loaded = engine
        .load(Path::new(&path), autoplay)
        .map_err(engine_error)?;
    core.seq += 1;
    core.ended_reported = false;
    core.dirty = true;
    Ok(PlayerLoadResult {
        seq: core.seq,
        duration_sec: loaded.duration.map(|d| d.as_secs_f64()),
    })
}

/// Pausa (no-op si no hay nada sonando).
#[tauri::command]
pub fn player_pause(state: State<'_, PlayerState>) -> Result<(), String> {
    let mut core = lock_core(&state)?;
    if let Some(engine) = core.engine.as_mut() {
        if matches!(engine.state(), EngineState::Playing) {
            engine.pause();
            core.dirty = true;
        }
    }
    Ok(())
}

/// Reanuda la pista pausada (no-op si no hay nada cargado).
#[tauri::command]
pub fn player_resume(state: State<'_, PlayerState>) -> Result<(), String> {
    let mut core = lock_core(&state)?;
    if let Some(engine) = core.engine.as_mut() {
        if engine.loaded().is_some() {
            engine.resume().map_err(engine_error)?;
            core.dirty = true;
        }
    }
    Ok(())
}

/// Detiene y descarga la pista actual. El stream de salida sigue abierto.
#[tauri::command]
pub fn player_stop(state: State<'_, PlayerState>) -> Result<(), String> {
    let mut core = lock_core(&state)?;
    if let Some(engine) = core.engine.as_mut() {
        engine.stop();
    }
    core.ended_reported = false;
    core.dirty = true;
    Ok(())
}

/// Busca una posición absoluta en segundos (acotada por el motor a la pista).
#[tauri::command]
pub fn player_seek(state: State<'_, PlayerState>, sec: f64) -> Result<(), String> {
    if !sec.is_finite() || sec < 0.0 {
        return Err("invalid seek position".to_string());
    }
    let mut core = lock_core(&state)?;
    let Some(engine) = core.engine.as_mut() else {
        return Err(engine_error(EngineError::NothingLoaded));
    };
    engine
        .seek(Duration::from_secs_f64(sec))
        .map_err(engine_error)?;
    core.dirty = true;
    Ok(())
}

/// Volumen lineal `[0, 1]`. Antes de abrir el motor es un no-op: la carga
/// aplica el volumen recibido.
#[tauri::command]
pub fn player_set_volume(state: State<'_, PlayerState>, volume: f64) -> Result<(), String> {
    let mut core = lock_core(&state)?;
    if let Some(engine) = core.engine.as_mut() {
        engine.set_volume(volume.clamp(0.0, 1.0) as f32);
    }
    Ok(())
}

/// Ritmo de reproducción (`1.0` = normal). Nota: rodio varía el tono con la
/// velocidad (sin time-stretch), a diferencia de `audio.playbackRate`.
#[tauri::command]
pub fn player_set_rate(state: State<'_, PlayerState>, rate: f64) -> Result<(), String> {
    if !rate.is_finite() || rate <= 0.0 {
        return Err("invalid playback rate".to_string());
    }
    let mut core = lock_core(&state)?;
    if let Some(engine) = core.engine.as_mut() {
        engine.set_rate(rate as f32);
    }
    Ok(())
}

/// Actualiza los metadatos y modos que ve el panel MPRIS del escritorio.
/// En plataformas sin MPRIS (o sin bus de sesión) es un no-op.
#[tauri::command]
pub fn player_mpris_update(
    state: State<'_, MprisService>,
    metadata: MprisMetadataInput,
) -> Result<(), String> {
    state.update_metadata(&metadata);
    Ok(())
}

/// Arranca el ticker de eventos del player (se llama una vez en `setup`).
pub fn spawn_player_ticker(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let (tick, ended) = collect_tick(&app);
            if let Some(payload) = tick {
                let _ = app.emit("player:tick", payload);
            }
            if let Some(payload) = ended {
                let _ = app.emit("player:ended", payload);
            }
        }
    });
}

/// Un paso del ticker: emite `player:tick` mientras suena (o si hay cambios
/// pendientes) y `player:ended` la primera vez que la cola queda vacía con la
/// pista aún cargada (final natural).
fn collect_tick(app: &AppHandle) -> (Option<PlayerTickPayload>, Option<PlayerEndedPayload>) {
    let Some(state) = app.try_state::<PlayerState>() else {
        return (None, None);
    };
    let Ok(mut core) = state.core.lock() else {
        return (None, None);
    };
    // Destructurar el guard: los préstamos disjuntos de campos no funcionan a
    // través de `DerefMut` del MutexGuard (presta el guard entero).
    let PlayerCore {
        engine,
        seq,
        dirty,
        ended_reported,
    } = &mut *core;
    let Some(engine) = engine.as_mut() else {
        return (None, None);
    };
    let engine_state = engine.state();
    // MPRIS: estado + posición (el hilo D-Bus emite PropertiesChanged si
    // cambian; la posición sólo se expone, nunca se emite en el diff).
    if let Some(mpris) = app.try_state::<MprisService>() {
        mpris.update_playback(
            match engine_state {
                EngineState::Playing => PlaybackStatus::Playing,
                EngineState::Paused => PlaybackStatus::Paused,
                EngineState::Stopped => PlaybackStatus::Stopped,
            },
            engine.position(),
        );
    }
    let playing = matches!(engine_state, EngineState::Playing);
    let tick = if playing || *dirty {
        *dirty = false;
        Some(PlayerTickPayload {
            seq: *seq,
            state: state_str(engine_state),
            position_sec: engine.position().as_secs_f64(),
            duration_sec: engine.duration().map(|d| d.as_secs_f64()),
        })
    } else {
        None
    };
    let ended = if engine.track_ended() && !*ended_reported {
        *ended_reported = true;
        Some(PlayerEndedPayload { seq: *seq })
    } else {
        None
    };
    (tick, ended)
}
