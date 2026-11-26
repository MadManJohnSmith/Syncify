//! Control del transporte desde el backend (#133, P02 — parte de #130).
//!
//! MPRIS y la bandeja no pueden depender de la webview: sus acciones se
//! ejecutan aquí sobre el motor (`PlayerState`) y el agregado de cola
//! persistido (`commands::playback_queue`). La UI sigue siendo una vista y se
//! sincroniza con los eventos del backend (`player:advanced`,
//! `player:modes-changed`).
//!
//! Reparto:
//! - `run_action`: play/pause/playPause/stop/next/previous/seek absoluto o
//!   relativo, con la misma semántica que la UI aplicaba antes de P02.
//! - `set_shuffle` / `set_repeat`: modos persistidos; el primero reordena los
//!   pendientes en la MISMA transacción y devuelve el orden nuevo para que la
//!   proyección de la UI no diverja.
//!
//! Los errores no escalan: el panel MPRIS no tiene canal de error; se
//! registran en el log y el estado anterior queda intacto.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::playback_queue::{
    perform_get_playback_queue, perform_set_modes, perform_set_position, perform_set_shuffle,
    PlaybackQueueEntryDto,
};
use crate::services::mpris::MprisService;
use crate::services::playback_advance::{
    run_manual_next, run_manual_previous, PlayerAdvancedPayload,
};
use crate::services::playback_session::PlayerState;
use crate::services::stream_cache::StreamCacheState;

/// Evento con el que el backend publica modos que cambiaron sin la UI (MPRIS).
/// `pending_entry_ids` viaja cuando el shuffle reordenó la cola pendiente.
pub const PLAYER_MODES_CHANGED_EVENT: &str = "player:modes-changed";

/// `RepeatMode` del backend; en MPRIS se traduce a `LoopStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    /// `off | all | one` tal y como lo persiste `player_state`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }

    pub fn from_mpris_loop_status(value: &str) -> Option<Self> {
        match value {
            "None" => Some(Self::Off),
            "Track" => Some(Self::One),
            "Playlist" => Some(Self::All),
            _ => None,
        }
    }
}

/// Acción de transporte que llega de MPRIS (o de cualquier consumidor del
/// backend). Los offsets/posiciones viajan en microsegundos, como el contrato
/// MPRIS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendAction {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    SeekByMicros(i64),
    SetPositionMicros { track_id: i64, position_us: i64 },
}

/// Payload de `player:modes-changed`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModesChangedPayload {
    pub shuffle: bool,
    pub repeat: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_entry_ids: Option<Vec<String>>,
}

/// Ejecuta una acción de transporte y devuelve el avance si cambió de pista.
/// Es `async` para que las pruebas la esperen; MPRIS usa `spawn_action`.
pub async fn run_action<R: Runtime>(
    app: &AppHandle<R>,
    action: BackendAction,
) -> Result<Option<PlayerAdvancedPayload>, String> {
    match action {
        BackendAction::Play => play(app).await,
        BackendAction::Pause => pause(app).await,
        BackendAction::PlayPause => {
            if current_status(app) == crate::services::mpris::PlaybackStatus::Playing {
                pause(app).await
            } else {
                play(app).await
            }
        }
        BackendAction::Stop => stop(app).await,
        BackendAction::Next => run_manual_next(app).await,
        BackendAction::Previous => run_manual_previous(app).await,
        BackendAction::SeekByMicros(offset) => {
            seek_relative(app, offset).await?;
            Ok(None)
        }
        BackendAction::SetPositionMicros {
            track_id,
            position_us,
        } => {
            seek_absolute(app, track_id, position_us).await?;
            Ok(None)
        }
    }
}

/// Lanza la acción en el runtime de Tauri y emite `player:advanced` si el
/// backend abrió otra pista. Un fallo queda en el log (MPRIS no tiene canal de
/// error); la UI no es necesaria para que la acción ocurra.
pub fn spawn_action<R: Runtime + 'static>(app: AppHandle<R>, action: BackendAction) {
    tauri::async_runtime::spawn(async move {
        match run_action(&app, action).await {
            Ok(Some(payload)) => {
                let _ = app.emit("player:advanced", &payload);
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("player: acción {:?} desde MPRIS falló: {e}", action),
        }
    });
}

fn current_status<R: Runtime>(app: &AppHandle<R>) -> crate::services::mpris::PlaybackStatus {
    app.try_state::<PlayerState>()
        .map(|state| state.playback_status())
        .unwrap_or_default()
}

/// Persiste la posición del motor para que la restauración no dependa de la
/// webview. Un fallo de BD no revierte el transporte: queda en el log.
async fn persist_position<R: Runtime>(app: &AppHandle<R>, position_sec: f64) {
    let Some(db) = app
        .try_state::<crate::AppState>()
        .map(|state| state.db.clone())
    else {
        return;
    };
    if let Err(e) = perform_set_position(&db, position_sec).await {
        tracing::warn!("player: no se pudo persistir la posición {position_sec}: {e}");
    }
}

fn engine_position_sec<R: Runtime>(app: &AppHandle<R>) -> Option<f64> {
    app.try_state::<PlayerState>()
        .and_then(|state| state.position())
        .map(|position| position.as_secs_f64())
}

async fn play<R: Runtime>(app: &AppHandle<R>) -> Result<Option<PlayerAdvancedPayload>, String> {
    let Some(state) = app.try_state::<PlayerState>() else {
        return Ok(None);
    };
    match state.playback_status() {
        crate::services::mpris::PlaybackStatus::Playing => Ok(None),
        crate::services::mpris::PlaybackStatus::Paused => {
            state.resume()?;
            if let Some(position) = state.position() {
                persist_position(app, position.as_secs_f64()).await;
            }
            Ok(None)
        }
        crate::services::mpris::PlaybackStatus::Stopped => {
            let db = app.state::<crate::AppState>().db.clone();
            let snapshot = perform_get_playback_queue(&db).await?;
            match snapshot.current.as_ref() {
                Some(entry) => {
                    reopen_entry(app, entry.clone()).await?;
                    Ok(None)
                }
                // Sin pista actual: play arranca la cabeza de los pendientes,
                // igual que el play sin argumentos de la UI.
                None => run_manual_next(app).await,
            }
        }
    }
}

async fn pause<R: Runtime>(app: &AppHandle<R>) -> Result<Option<PlayerAdvancedPayload>, String> {
    let Some(state) = app.try_state::<PlayerState>() else {
        return Ok(None);
    };
    state.pause()?;
    if let Some(position) = state.position() {
        persist_position(app, position.as_secs_f64()).await;
    }
    Ok(None)
}

async fn stop<R: Runtime>(app: &AppHandle<R>) -> Result<Option<PlayerAdvancedPayload>, String> {
    let Some(state) = app.try_state::<PlayerState>() else {
        return Ok(None);
    };
    let Some(cache) = app.try_state::<StreamCacheState>() else {
        return Ok(None);
    };
    state.stop(&cache)?;
    persist_position(app, 0.0).await;
    Ok(None)
}

/// Recarga la entrada actual desde cero (play tras final/stop). El volumen
/// sale del estado persistido, como en el avance de P01.
async fn reopen_entry<R: Runtime>(
    app: &AppHandle<R>,
    entry: PlaybackQueueEntryDto,
) -> Result<(), String> {
    let db = app.state::<crate::AppState>().db.clone();
    let snapshot = perform_get_playback_queue(&db).await?;
    let volume = if snapshot.muted { 0.0 } else { snapshot.volume };
    let cache = app.state::<StreamCacheState>();
    let state = app.state::<PlayerState>();
    if entry.track_id > 0 {
        state
            .open(app, &cache, &db, entry.track_id, true, volume)
            .await?;
    } else {
        let service_id = entry
            .service_id
            .ok_or_else(|| "la entrada efímera no tiene service_id".to_string())?;
        let service_track_id = entry
            .service_track_id
            .ok_or_else(|| "la entrada efímera no tiene service_track_id".to_string())?;
        state
            .open_provider_item(
                app,
                &cache,
                &db,
                crate::services::streaming::ProviderStreamItem {
                    entry_id: entry.entry_id,
                    service_id,
                    service_track_id,
                },
                true,
                volume,
            )
            .await?;
    }
    Ok(())
}

async fn seek_relative<R: Runtime>(app: &AppHandle<R>, offset_us: i64) -> Result<(), String> {
    let Some(state) = app.try_state::<PlayerState>() else {
        return Ok(());
    };
    if !state.is_loaded() {
        return Ok(());
    }
    let current = engine_position_sec(app).unwrap_or(0.0);
    let target = current + offset_us as f64 / 1_000_000.0;
    if !target.is_finite() {
        return Ok(());
    }
    seek_to(app, target.max(0.0)).await
}

async fn seek_absolute<R: Runtime>(
    app: &AppHandle<R>,
    track_id: i64,
    position_us: i64,
) -> Result<(), String> {
    let Some(state) = app.try_state::<PlayerState>() else {
        return Ok(());
    };
    // Spec MPRIS: SetPosition se ignora si el trackid no es el actual.
    if state.current_track_id() != Some(track_id) {
        return Ok(());
    }
    if !state.is_loaded() {
        return Ok(());
    }
    seek_to(app, position_us as f64 / 1_000_000.0).await
}

async fn seek_to<R: Runtime>(app: &AppHandle<R>, position_sec: f64) -> Result<(), String> {
    if !position_sec.is_finite() || position_sec < 0.0 {
        return Ok(());
    }
    let Some(state) = app.try_state::<PlayerState>() else {
        return Ok(());
    };
    let cache = app.state::<StreamCacheState>();
    state.seek(app, &cache, position_sec)?;
    persist_position(app, position_sec).await;
    Ok(())
}

/// Aplica shuffle con reordenación atómica de los pendientes y publica el
/// nuevo orden. Devuelve `Err` sin tocar nada si la BD falla.
pub async fn set_shuffle<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    let db = app.state::<crate::AppState>().db.clone();
    let snapshot = perform_get_playback_queue(&db).await?;
    if snapshot.shuffle == enabled {
        update_mpris_modes(app, enabled, &snapshot.repeat);
        return Ok(());
    }
    let pending = perform_set_shuffle(&db, enabled).await?;
    update_mpris_modes(app, enabled, &snapshot.repeat);
    let _ = app.emit(
        PLAYER_MODES_CHANGED_EVENT,
        ModesChangedPayload {
            shuffle: enabled,
            repeat: snapshot.repeat,
            pending_entry_ids: Some(pending),
        },
    );
    Ok(())
}

/// Aplica `LoopStatus`/repeat del backend sin tocar la cola.
pub async fn set_repeat<R: Runtime>(app: &AppHandle<R>, repeat: RepeatMode) -> Result<(), String> {
    let db = app.state::<crate::AppState>().db.clone();
    let snapshot = perform_get_playback_queue(&db).await?;
    if snapshot.repeat == repeat.as_str() {
        update_mpris_modes(app, snapshot.shuffle, repeat.as_str());
        return Ok(());
    }
    perform_set_modes(&db, snapshot.shuffle, repeat.as_str()).await?;
    update_mpris_modes(app, snapshot.shuffle, repeat.as_str());
    let _ = app.emit(
        PLAYER_MODES_CHANGED_EVENT,
        ModesChangedPayload {
            shuffle: snapshot.shuffle,
            repeat: repeat.as_str().to_string(),
            pending_entry_ids: None,
        },
    );
    Ok(())
}

/// Lanza el cambio de shuffle pedido por MPRIS; si falla, el modo anterior y
/// la cola quedan intactos (la transacción no se confirma).
pub fn spawn_set_shuffle<R: Runtime + 'static>(app: AppHandle<R>, enabled: bool) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = set_shuffle(&app, enabled).await {
            tracing::warn!("player: no se pudo cambiar shuffle: {e}");
        }
    });
}

/// Lanza el cambio de repeat pedido por MPRIS.
pub fn spawn_set_repeat<R: Runtime + 'static>(app: AppHandle<R>, repeat: RepeatMode) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = set_repeat(&app, repeat).await {
            tracing::warn!("player: no se pudo cambiar repeat: {e}");
        }
    });
}

fn update_mpris_modes<R: Runtime>(app: &AppHandle<R>, shuffle: bool, repeat: &str) {
    if let Some(mpris) = app.try_state::<MprisService>() {
        mpris.update_modes(shuffle, repeat);
    }
}

#[cfg(test)]
#[path = "player_control_tests.rs"]
mod tests;
