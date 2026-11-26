//! Fase 1 del plan de reproducción: comandos Tauri del motor nativo
//! (`syncify-player`, rodio + symphonia).
//!
//! Sustituye la ruta `<audio>` + `syncify-media://`, demostrada rota en el
//! motor media de WebKitGTK (`FormatError`; ver
//! `docs/auditoria/playback-fase0-evidencia-2026-10-17.md`).
//!
//! Reparto de responsabilidades (ADR-0024):
//! - La orquestación vive en `services::playback_session` (`PlayerState`):
//!   resolver la fuente, cargar el motor, la frontera de streaming y la
//!   generación. Los comandos de aquí son delgados y delegan en ella.
//! - Aquí también: el ticker de 250 ms, que consume `PlayerState::tick()` y
//!   emite los eventos del player.
//! - La UI (`usePlayer.ts`) proyecta cola, historial y modos; el crédito de
//!   escucha y la promoción de efímeras viven en el backend.
//!
//! Eventos (el frontend descarta todo lo que no case con su `seq`):
//! - `player:tick {seq, state, position_sec, duration_sec}` — cada 250 ms
//!   mientras suena, y una vez tras cada cambio de transporte (carga, pausa,
//!   seek en pausa...).
//! - `player:ended {seq}` — una única vez por carga cuando la pista llegó a su
//!   final natural (no con `player_stop`).
//! - `player:queue-entry-changed {entry}` — una efímera cambió de identidad o
//!   persistió un nuevo motivo de promoción.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::services::playback_session::{PlayerLoadResult, PlayerState, TickOutput};
use crate::services::stream_cache::StreamCacheState;

/// Intervalo del ticker de posición.
const TICK_INTERVAL: Duration = Duration::from_millis(250);

/// Carga y reproduce una pista de la biblioteca: usa el archivo local
/// descargado si existe y, si no, resuelve una fuente de streaming
/// (Qobuz/Tidal) con caché progresiva.
///
/// La URL firmada vive sólo en backend; el resultado expone la calidad
/// realmente obtenida (`stream.qualityLabel`) para la etiqueta honesta.
#[tauri::command]
pub async fn player_load_track<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
    app_state: State<'_, crate::AppState>,
    track_id: i64,
    autoplay: bool,
    volume: f64,
) -> Result<PlayerLoadResult, String> {
    state
        .open(&app, &cache, &app_state.db, track_id, autoplay, volume)
        .await
}

/// Carga una entrada de cola por su identidad, sea de biblioteca o efímera.
#[tauri::command]
pub async fn player_load_queue_entry<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
    app_state: State<'_, crate::AppState>,
    entry_id: String,
    autoplay: bool,
    volume: f64,
) -> Result<PlayerLoadResult, String> {
    let entry =
        crate::commands::playback_queue::perform_get_queue_entry(&app_state.db, &entry_id).await?;
    if entry.track_id > 0 {
        state
            .open(
                &app,
                &cache,
                &app_state.db,
                entry.track_id,
                autoplay,
                volume,
            )
            .await
    } else {
        let service_id = entry
            .service_id
            .ok_or_else(|| "la entrada efímera no tiene service_id".to_string())?;
        let service_track_id = entry
            .service_track_id
            .ok_or_else(|| "la entrada efímera no tiene service_track_id".to_string())?;
        state
            .open_provider_item(
                &app,
                &cache,
                &app_state.db,
                crate::services::streaming::ProviderStreamItem {
                    entry_id,
                    service_id,
                    service_track_id,
                },
                autoplay,
                volume,
            )
            .await
    }
}

/// Pausa (no-op si no hay nada sonando).
#[tauri::command]
pub fn player_pause(state: State<'_, PlayerState>) -> Result<(), String> {
    state.pause()
}

/// Reanuda la pista pausada (no-op si no hay nada cargado).
#[tauri::command]
pub fn player_resume(state: State<'_, PlayerState>) -> Result<(), String> {
    state.resume()
}

/// Detiene y descarga la pista actual. El stream de salida sigue abierto.
#[tauri::command]
pub fn player_stop(
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
) -> Result<(), String> {
    state.stop(&cache)
}

/// Busca una posición absoluta en segundos (acotada por el motor a la pista).
///
/// En streaming, un seek más allá de la frontera descargada queda pendiente:
/// el motor se pausa ("buffering") y el ticker lo completa cuando la descarga
/// alcanza la posición pedida.
#[tauri::command]
pub fn player_seek<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
    sec: f64,
) -> Result<(), String> {
    state.seek(&app, &cache, sec)
}

/// Volumen lineal `[0, 1]`. Antes de abrir el motor es un no-op: la carga
/// aplica el volumen recibido.
#[tauri::command]
pub fn player_set_volume(state: State<'_, PlayerState>, volume: f64) -> Result<(), String> {
    state.set_volume(volume)
}

/// Ritmo de reproducción (`1.0` = normal). Nota: rodio varía el tono con la
/// velocidad (sin time-stretch), a diferencia de `audio.playbackRate`.
#[tauri::command]
pub fn player_set_rate(state: State<'_, PlayerState>, rate: f64) -> Result<(), String> {
    state.set_rate(rate)
}

/// Arranca el ticker de eventos del player (se llama una vez en `setup`).
/// Cada paso consume `PlayerState::tick()` y emite, en orden: los estados de
/// stream que la frontera haya producido, la actualización de MPRIS,
/// `player:tick` y `player:ended`.
pub fn spawn_player_ticker<R: tauri::Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        if let Some(app_state) = app.try_state::<crate::AppState>() {
            let db = app_state.db.clone();
            tauri::async_runtime::spawn(async move {
                match crate::services::playback_promotion::reconcile_queue_promotions(&db).await {
                    Ok(recovered) if recovered > 0 => tracing::info!(
                        recovered,
                        "créditos de promoción recuperados al arrancar el player"
                    ),
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!("no se pudieron reconciliar promociones pendientes: {error}")
                    }
                }
                match crate::services::playback_promotion::retry_unresolved_queue_promotions(&db)
                    .await
                {
                    Ok(retried) if retried > 0 => tracing::info!(
                        retried,
                        "promociones efímeras reintentadas al arrancar el player"
                    ),
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!("no se pudieron reintentar promociones efímeras: {error}")
                    }
                }
            });
        }
        loop {
            interval.tick().await;
            let Some(state) = app.try_state::<PlayerState>() else {
                continue;
            };
            let cache = app.try_state::<StreamCacheState>();
            let out: TickOutput = state.tick(cache.as_deref());
            for payload in out.stream_states {
                let _ = app.emit("player:stream-state", payload);
            }
            // Seek pendiente estancado (Q4): la re-resolución única y el
            // relleno de la misma caché corren fuera del paso del ticker.
            if let Some(session) = out.stall_refill {
                crate::services::playback_session::spawn_stall_refill(app.clone(), session);
            }
            if let Some(mpris_tick) = out.mpris {
                if let Some(mpris) = app.try_state::<crate::services::mpris::MprisService>() {
                    mpris.update_playback(
                        &app,
                        mpris_tick.status,
                        mpris_tick.position,
                        mpris_tick
                            .track_id
                            .map(|track_id| crate::services::mpris::NowPlaying {
                                track_id,
                                duration: mpris_tick.duration,
                            }),
                    );
                }
            }
            if let Some(payload) = out.tick {
                let _ = app.emit("player:tick", payload);
            }
            if let Some(payload) = out.ended {
                let _ = app.emit("player:ended", &payload);
                // P01 (#132): el backend decide la siguiente pista y avanza.
                // La UI ya no orquesta el fin de pista; sincroniza su vista
                // con `player:advanced`.
                crate::services::playback_advance::spawn_auto_advance(app.clone(), payload.seq);
            }
            if let Some(credit) = out.listen_credit {
                // P01/P03: el backend acumula 30 s reales. Una efímera se
                // promociona y acredita con el token congelado; una pista de
                // biblioteca mantiene el camino directo idempotente.
                let db = match app.try_state::<crate::AppState>() {
                    Some(state) => state.db.clone(),
                    None => continue,
                };
                let provider_entry_id = match &credit.target {
                    crate::services::streaming::StreamTarget::Provider(item) => {
                        Some(item.entry_id.clone())
                    }
                    crate::services::streaming::StreamTarget::Library(_) => None,
                };
                let event_app = provider_entry_id.as_ref().map(|_| app.clone());
                tauri::async_runtime::spawn(async move {
                    let result =
                        crate::commands::playback::perform_apply_listen_credit(&db, &credit).await;
                    match &result {
                        Ok(outcome) => tracing::info!(
                            target = ?credit.target,
                            ?outcome,
                            "escucha procesada por el backend (30 s reales)"
                        ),
                        Err(error) => tracing::warn!(
                            target = ?credit.target,
                            "no se pudo procesar la escucha: {error}"
                        ),
                    }
                    if let (Some(entry_id), Some(event_app)) = (provider_entry_id, event_app) {
                        match crate::commands::playback_queue::perform_get_queue_entry(
                            &db, &entry_id,
                        )
                        .await
                        {
                            Ok(entry) => {
                                let _ = event_app.emit(
                                    crate::commands::playback_queue::PLAYER_QUEUE_ENTRY_CHANGED_EVENT,
                                    crate::commands::playback_queue::PlayerQueueEntryChangedPayload {
                                        entry,
                                    },
                                );
                            }
                            Err(error) => tracing::warn!(
                                entry_id = %entry_id,
                                "no se pudo publicar el cambio de la entrada: {error}"
                            ),
                        }
                    }
                });
            }
        }
    });
}

#[cfg(test)]
#[path = "player_tests.rs"]
mod tests;
