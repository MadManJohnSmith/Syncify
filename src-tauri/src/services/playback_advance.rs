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
use tauri::{AppHandle, Manager, Runtime};

use crate::commands::playback_queue::{
    perform_get_playback_queue, perform_rebuild_cycle, perform_set_current, perform_set_position,
    PlaybackQueueEntryDto, PlaybackQueueSnapshot,
};
use crate::services::playback_session::{lock_core, PlayerState};
use crate::services::stream_cache::StreamCacheState;

/// Payload de `player:advanced`: la carga que el backend abrió al avanzar.
/// `seq` identifica la generación para el filtro de eventos obsoletos de la
/// UI (mismo contrato que `player_load_track`).
#[derive(Debug, Clone, Serialize)]
pub struct PlayerAdvancedPayload {
    pub seq: u64,
    pub track_id: i64,
    pub entry_id: String,
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
        "all" => {
            let mut cycle: Vec<&PlaybackQueueEntryDto> = snapshot.history.iter().collect();
            if let Some(current) = snapshot.current.as_ref() {
                cycle.push(current);
            }
            if cycle.is_empty() {
                return AdvanceDecision::Stop;
            }
            // Última ocurrencia por entrada: recorrer un ciclo completo
            // reproduce cada entrada una vez, así que conservar la última
            // posición de cada `entry_id` devuelve el orden del ciclo sin
            // duplicados acumulados entre rondas.
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
            AdvanceDecision::RebuildCycle {
                order,
                head_entry_id,
                head_track_id,
            }
        }
        _ => AdvanceDecision::Stop,
    }
}

/// Ejecuta la transición completa del avance: decide, aplica el agregado con
/// los `perform_*` reales (los mismos comandos que la UI) y carga la pista en
/// el motor. Devuelve el payload de `player:advanced`, o `None` si nada que
/// hacer (o generación superada por una carga más nueva).
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
    let (entry_id, track_id) = match decide_auto_advance(&snapshot) {
        AdvanceDecision::Stop => return Ok(None),
        AdvanceDecision::PlayHead { entry_id, track_id } => {
            perform_set_current(&db, &entry_id, track_id).await?;
            (entry_id, track_id)
        }
        AdvanceDecision::ReplayCurrent { entry_id, track_id } => {
            perform_set_position(&db, 0.0).await?;
            (entry_id, track_id)
        }
        AdvanceDecision::RebuildCycle {
            order,
            head_entry_id,
            head_track_id,
        } => {
            perform_rebuild_cycle(&db, &order).await?;
            perform_set_current(&db, &head_entry_id, head_track_id).await?;
            (head_entry_id, head_track_id)
        }
    };
    // El volumen del avance es el estado persistido del backend (modos y
    // volumen viven en SQLite desde #131); el silencio viaja como 0.
    let volume = if snapshot.muted { 0.0 } else { snapshot.volume };
    let cache = app.state::<StreamCacheState>();
    let result = state.open(app, &cache, &db, track_id, true, volume).await?;
    Ok(Some(PlayerAdvancedPayload {
        seq: result.seq,
        track_id,
        entry_id,
    }))
}

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
