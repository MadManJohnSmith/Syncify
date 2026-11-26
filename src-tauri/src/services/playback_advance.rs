//! Avance automático de cola en el backend (#132, P01 — parte de #130).
//!
//! Antes de P01, `player:ended` sólo notificaba a la webview y era la UI quien
//! decidía la siguiente pista (y, cerrada a bandeja o estancada, nadie
//! avanzaba). Ahora el backend decide: cuando el ticker reporta el final
//! natural de una pista, `spawn_auto_advance` consulta el agregado persistido
//! (migración 0109), resuelve la siguiente entrada según los modos —
//! pendientes primero; `repeat one` repite la actual; `repeat all` reconstruye
//! el ciclo desde el historial; el orden de shuffle es el que el backend
//! tiene persistido —, ejecuta la transición del agregado con los mismos
//! `perform_*` que la UI y carga la pista en el motor.
//!
//! La UI sigue siendo una vista: recibe el evento `player:advanced` con la
//! generación (`seq`) nueva y sincroniza su proyección. Los modos y la
//! decisión viven aquí; la UI no cronometra ni avanza.
//!
//! Carrera con la UI: una acción del usuario (`next`, play de otra pista) crea
//! su propia carga con `seq` mayor. El guard de generación comprueba, antes de
//! tocar nada, que el motor sigue en la carga que terminó (`ended_seq`): si la
//! UI ya cargó otra pista, el avance automático se retira. La ventana entre
//! ese guard y `open` es el único hueco residual y es la misma que el resto de
//! transiciones del player ya tolera.

use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::playback_queue::{
    perform_get_playback_queue, perform_rebuild_cycle, perform_set_current_entry,
    perform_set_position, PlaybackQueueEntryDto, PlaybackQueueSnapshot,
};
use crate::services::playback_session::{lock_core, PlayerState};
use crate::services::stream_cache::StreamCacheState;

/// Payload de `player:advanced`: la carga que el backend abrió al avanzar.
/// `seq` identifica la generación para el filtro de eventos obsoletos de la
/// UI (mismo contrato que `player_load_track`).
#[derive(Debug, Clone, Serialize)]
pub struct PlayerAdvancedPayload {
    pub seq: u64,
    /// `0` mientras la entrada actual sigue siendo efímera.
    pub track_id: i64,
    pub service_id: Option<i64>,
    pub service_track_id: Option<String>,
    pub entry_id: String,
    /// Presente sólo cuando el avance lo pidió `previous`: la pista que sonaba
    /// vuelve al FRENTE de los pendientes (no al historial) y la entrada
    /// avanzada (la última del historial) pasa a ser la actual. La UI usa este
    /// dato para proyectar el estado sin adivinar (#133, P02).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_entry_id: Option<String>,
}

/// Qué hacer cuando una pista llega a su final natural. Decisión PURA sobre la
/// instantánea del agregado: testeable sin motor ni red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvanceDecision {
    /// Hay pendientes: consumir la cabeza (en cualquier modo de repeat; la
    /// cabeza ya lleva el orden que el backend tiene persistido, incluida la
    /// permutación de shuffle).
    PlayHead { entry_id: String, track_id: i64 },
    /// Sin pendientes y `repeat one`: repetir la pista actual (nueva sesión de
    /// escucha, posición a 0; no duplica el historial).
    ReplayCurrent { entry_id: String, track_id: i64 },
    /// Sin pendientes y `repeat all`: reconstruir el ciclo (historial en orden
    /// de reproducción + pista actual, conservando la última ocurrencia por
    /// entrada) y consumir su cabeza. Con shuffle, la permutación se resuelve
    /// aquí, en el backend.
    RebuildCycle {
        order: Vec<String>,
        head_entry_id: String,
        head_track_id: i64,
    },
    /// Nada que hacer: sin pendientes y repeat `off` (o sin nada que repetir).
    Stop,
}

/// Orden del ciclo de `repeat all`: historial en orden de reproducción +
/// pista actual, conservando la ÚLTIMA ocurrencia por entrada (recorrer un
/// ciclo completo reproduce cada entrada una vez) y permutándolo si shuffle
/// está activo. `None` si no hay nada que repetir.
fn rebuild_cycle_decision(snapshot: &PlaybackQueueSnapshot) -> Option<AdvanceDecision> {
    let mut cycle: Vec<&PlaybackQueueEntryDto> = snapshot.history.iter().collect();
    if let Some(current) = snapshot.current.as_ref() {
        cycle.push(current);
    }
    if cycle.is_empty() {
        return None;
    }
    let mut last_index: HashMap<&str, usize> = HashMap::new();
    for (i, entry) in cycle.iter().enumerate() {
        last_index.insert(entry.entry_id.as_str(), i);
    }
    let mut order: Vec<String> = cycle
        .iter()
        .enumerate()
        .filter(|(i, entry)| last_index.get(entry.entry_id.as_str()) == Some(i))
        .map(|(_, entry)| entry.entry_id.clone())
        .collect();
    if snapshot.shuffle {
        use rand::seq::SliceRandom;
        order.shuffle(&mut rand::thread_rng());
    }
    let head_entry_id = order[0].clone();
    let head_track_id = cycle
        .iter()
        .find(|entry| entry.entry_id == head_entry_id)
        .map(|entry| entry.track_id)
        .unwrap_or_default();
    Some(AdvanceDecision::RebuildCycle {
        order,
        head_entry_id,
        head_track_id,
    })
}

/// Resuelve la siguiente entrada tras un fin natural, con la semántica que la
/// UI aplicaba antes de P01: los pendientes mandan sobre cualquier modo de
/// repeat; `one` sólo repite cuando la cola se agotó; `all` reconstruye el
/// ciclo deduplicado por identidad de entrada.
pub fn decide_auto_advance(snapshot: &PlaybackQueueSnapshot) -> AdvanceDecision {
    if let Some(head) = snapshot.pending.first() {
        return AdvanceDecision::PlayHead {
            entry_id: head.entry_id.clone(),
            track_id: head.track_id,
        };
    }
    match snapshot.repeat.as_str() {
        "one" => snapshot
            .current
            .as_ref()
            .map(|c| AdvanceDecision::ReplayCurrent {
                entry_id: c.entry_id.clone(),
                track_id: c.track_id,
            })
            .unwrap_or(AdvanceDecision::Stop),
        "all" => rebuild_cycle_decision(snapshot).unwrap_or(AdvanceDecision::Stop),
        _ => AdvanceDecision::Stop,
    }
}

/// Decisión de un `next` MANUAL (MPRIS, bandeja, teclado): consume la cabeza
/// de los pendientes; si la cola se agotó, sólo `repeat all` reconstruye el
/// ciclo. A diferencia del fin natural, `repeat one` NO repite la actual: un
/// next manual con la cola vacía es un no-op (misma regla que la UI).
#[cfg(target_os = "linux")]
pub fn decide_manual_next(snapshot: &PlaybackQueueSnapshot) -> AdvanceDecision {
    if let Some(head) = snapshot.pending.first() {
        return AdvanceDecision::PlayHead {
            entry_id: head.entry_id.clone(),
            track_id: head.track_id,
        };
    }
    if snapshot.repeat == "all" {
        return rebuild_cycle_decision(snapshot).unwrap_or(AdvanceDecision::Stop);
    }
    AdvanceDecision::Stop
}

/// Transición decidida y ya aplicada al agregado persistido.
struct DecidedAdvance {
    entry_id: String,
    track_id: i64,
    service_id: Option<i64>,
    service_track_id: Option<String>,
    previous_entry_id: Option<String>,
}

impl DecidedAdvance {
    fn from_entry(entry: PlaybackQueueEntryDto, previous_entry_id: Option<String>) -> Self {
        Self {
            entry_id: entry.entry_id,
            track_id: entry.track_id,
            service_id: entry.service_id,
            service_track_id: entry.service_track_id,
            previous_entry_id,
        }
    }
}

/// Aplica una decisión al agregado con los `perform_*` reales. `previous` sólo
/// viaja en la decisión de retroceso manual (la pista que sonaba vuelve a los
/// pendientes); el resto de avances lo dejan vacío.
async fn apply_decision(
    db: &crate::DbPool,
    decision: AdvanceDecision,
    previous_entry_id: Option<String>,
) -> Result<Option<DecidedAdvance>, String> {
    match decision {
        AdvanceDecision::Stop => Ok(None),
        AdvanceDecision::PlayHead { entry_id, .. } => {
            let entry = perform_set_current_entry(db, &entry_id).await?;
            Ok(Some(DecidedAdvance::from_entry(entry, previous_entry_id)))
        }
        AdvanceDecision::ReplayCurrent { entry_id, .. } => {
            perform_set_position(db, 0.0).await?;
            let entry =
                crate::commands::playback_queue::perform_get_queue_entry(db, &entry_id).await?;
            Ok(Some(DecidedAdvance::from_entry(entry, previous_entry_id)))
        }
        AdvanceDecision::RebuildCycle {
            order,
            head_entry_id,
            ..
        } => {
            perform_rebuild_cycle(db, &order).await?;
            let entry = perform_set_current_entry(db, &head_entry_id).await?;
            Ok(Some(DecidedAdvance::from_entry(entry, previous_entry_id)))
        }
    }
}

/// Carga en el motor la entrada decidida con el volumen persistido y devuelve
/// el payload de `player:advanced` (el mismo contrato que P01).
async fn open_decided<R: tauri::Runtime>(
    app: &AppHandle<R>,
    snapshot: &PlaybackQueueSnapshot,
    decided: DecidedAdvance,
) -> Result<PlayerAdvancedPayload, String> {
    let db = app.state::<crate::AppState>().db.clone();
    let state = app.state::<PlayerState>();
    let volume = if snapshot.muted { 0.0 } else { snapshot.volume };
    let cache = app.state::<StreamCacheState>();
    let result = if decided.track_id > 0 {
        state
            .open(app, &cache, &db, decided.track_id, true, volume)
            .await?
    } else {
        let service_id = decided
            .service_id
            .ok_or_else(|| "la entrada efímera no tiene service_id".to_string())?;
        let service_track_id = decided
            .service_track_id
            .clone()
            .ok_or_else(|| "la entrada efímera no tiene service_track_id".to_string())?;
        state
            .open_provider_item(
                app,
                &cache,
                &db,
                crate::services::streaming::ProviderStreamItem {
                    entry_id: decided.entry_id.clone(),
                    service_id,
                    service_track_id,
                },
                true,
                volume,
            )
            .await?
    };
    Ok(PlayerAdvancedPayload {
        seq: result.seq,
        track_id: decided.track_id,
        service_id: decided.service_id,
        service_track_id: decided.service_track_id,
        entry_id: decided.entry_id,
        previous_entry_id: decided.previous_entry_id,
    })
}

/// Ejecuta el avance automático tras un fin natural: guard de generación,
/// decisión pura sobre la instantánea real y transición completa.
/// Devuelve `None` si no hay nada que hacer (o la generación fue superada).
pub async fn run_auto_advance<R: tauri::Runtime>(
    app: &AppHandle<R>,
    ended_seq: u64,
) -> Result<Option<PlayerAdvancedPayload>, String> {
    let state = app.state::<PlayerState>();
    {
        let core = lock_core(&state)?;
        if core.seq != ended_seq {
            return Ok(None);
        }
    }
    let db = app.state::<crate::AppState>().db.clone();
    let snapshot = perform_get_playback_queue(&db).await?;
    let Some(decided) = apply_decision(&db, decide_auto_advance(&snapshot), None).await? else {
        return Ok(None);
    };
    open_decided(app, &snapshot, decided).await.map(Some)
}

/// `next` manual desde el backend (#133): la cola manda; repeat one no repite
/// y repeat all reconstruye el ciclo cuando no quedan pendientes. La UI, MPRIS
/// y la bandeja comparten esta transición.
#[cfg(target_os = "linux")]
pub async fn run_manual_next<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<PlayerAdvancedPayload>, String> {
    let db = app.state::<crate::AppState>().db.clone();
    let snapshot = perform_get_playback_queue(&db).await?;
    let Some(decided) = apply_decision(&db, decide_manual_next(&snapshot), None).await? else {
        return Ok(None);
    };
    open_decided(app, &snapshot, decided).await.map(Some)
}

/// `previous` manual desde el backend (#133): con más de 3 s reproducidos
/// reinicia la pista actual; si no, recupera la última entrada del historial y
/// devuelve la pista que sonaba al frente de los pendientes.
#[cfg(target_os = "linux")]
pub async fn run_manual_previous<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<PlayerAdvancedPayload>, String> {
    let state = app.state::<PlayerState>();
    if !state.is_loaded() {
        return Ok(None);
    }
    let db = app.state::<crate::AppState>().db.clone();
    let position = state
        .position()
        .map(|position| position.as_secs_f64())
        .unwrap_or(0.0);
    if position > PREVIOUS_RESTART_SECS {
        let cache = app.state::<StreamCacheState>();
        state.seek(app, &cache, 0.0)?;
        perform_set_position(&db, 0.0).await?;
        return Ok(None);
    }
    let snapshot = perform_get_playback_queue(&db).await?;
    let Some(current) = snapshot.current.clone() else {
        return Ok(None);
    };
    let Some(previous) = snapshot.history.last().cloned() else {
        let cache = app.state::<StreamCacheState>();
        state.seek(app, &cache, 0.0)?;
        perform_set_position(&db, 0.0).await?;
        return Ok(None);
    };
    let previous =
        crate::commands::playback_queue::perform_return_to_pending_and_set_current_entry(
            &db,
            &previous.entry_id,
        )
        .await?;
    let decided = DecidedAdvance {
        entry_id: previous.entry_id,
        track_id: previous.track_id,
        service_id: previous.service_id,
        service_track_id: previous.service_track_id,
        previous_entry_id: Some(current.entry_id),
    };
    open_decided(app, &snapshot, decided).await.map(Some)
}

/// Segundos a partir de los cuales `previous` reinicia la pista en vez de
/// saltar al historial (paridad con la UI desde la fase 1).
#[cfg(target_os = "linux")]
pub const PREVIOUS_RESTART_SECS: f64 = 3.0;

/// Avance automático lanzado por el ticker cuando `player:ended` se emite.
/// Jamás escala: un fallo (sin fuente, red caída, BD ocupada) deja la
/// reproducción parada y el fallo en el log; la cola no se corrompe porque
/// cada transición del agregado es transaccional y previa a la carga.
pub fn spawn_auto_advance<R: tauri::Runtime + 'static>(app: AppHandle<R>, ended_seq: u64) {
    tauri::async_runtime::spawn(async move {
        match run_auto_advance(&app, ended_seq).await {
            Ok(Some(payload)) => {
                let _ = app.emit("player:advanced", &payload);
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("avance automático de cola falló tras el fin de pista: {e}");
            }
        }
    });
}

#[cfg(test)]
#[path = "playback_advance_tests.rs"]
mod tests;
