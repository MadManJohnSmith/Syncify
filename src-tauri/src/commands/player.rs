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
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use syncify_player::{EngineError, EngineState, PlayerEngine};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncWriteExt;

use crate::services::mpris::{MprisMetadataInput, MprisService, PlaybackStatus};
use crate::services::stream_cache::{self, StreamCacheState, StreamSession};
use crate::services::streaming::StreamSourceInfo;

/// Intervalo del ticker de posición.
const TICK_INTERVAL: Duration = Duration::from_millis(250);

/// Buffer mínimo antes de abrir el motor para una pista en streaming.
const STREAM_MIN_PRIME_BYTES: u64 = 256 * 1024;
/// Techo del buffer inicial (no esperar más de esto aunque quede descarga).
const STREAM_MAX_PRIME_BYTES: u64 = 8 * 1024 * 1024;
/// Buffer inicial cuando el servidor no declara Content-Length.
const STREAM_FALLBACK_PRIME_BYTES: u64 = 4 * 1024 * 1024;
/// Paciencia máxima para el buffer inicial.
const STREAM_PRIME_TIMEOUT: Duration = Duration::from_secs(45);
/// Pausa por buffer cuando la posición se acerca a lo descargado.
const FRONTIER_PAUSE_MARGIN_SECS: f64 = 5.0;
/// Reanuda cuando lo descargado supera la posición + este margen.
const FRONTIER_RESUME_MARGIN_SECS: f64 = 10.0;
/// Cadencia máxima de eventos `player:stream-state` durante la descarga.
const STREAM_EVENT_INTERVAL: Duration = Duration::from_millis(750);

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
    /// Presente sólo cuando la pista se resolvió por streaming: etiqueta
    /// honesta de la calidad obtenida (la URL nunca sale del backend).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamSourceInfo>,
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

fn lock_core(state: &PlayerState) -> Result<std::sync::MutexGuard<'_, PlayerCore>, String> {
    state
        .core
        .lock()
        .map_err(|_| "player state poisoned".to_string())
}

/// Carga un fichero (local o caché de streaming ya primada) en el motor.
/// No incrementa `seq`: el llamante decide la generación.
fn load_path_into_core(
    core: &mut PlayerCore,
    path: &Path,
    autoplay: bool,
    volume: f64,
) -> Result<Option<Duration>, String> {
    if core.engine.is_none() {
        core.engine = Some(PlayerEngine::new().map_err(engine_error)?);
    }
    let Some(engine) = core.engine.as_mut() else {
        return Err("player engine unavailable".to_string());
    };
    engine.set_volume(volume.clamp(0.0, 1.0) as f32);
    let loaded = engine.load(path, autoplay).map_err(engine_error)?;
    core.ended_reported = false;
    core.dirty = true;
    Ok(loaded.duration)
}

/// Carga un archivo local y, opcionalmente, lo reproduce. Con
/// `autoplay = false` queda cargado y pausado en la posición 0 (restauración).
#[tauri::command]
pub fn player_load(
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
    path: String,
    autoplay: bool,
    volume: f64,
) -> Result<PlayerLoadResult, String> {
    cache.cancel_current();
    let mut core = lock_core(&state)?;
    let duration = load_path_into_core(&mut core, Path::new(&path), autoplay, volume)?;
    core.seq += 1;
    Ok(PlayerLoadResult {
        seq: core.seq,
        duration_sec: duration.map(|d| d.as_secs_f64()),
        stream: None,
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
pub fn player_stop(
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
) -> Result<(), String> {
    cache.cancel_current();
    let mut core = lock_core(&state)?;
    if let Some(engine) = core.engine.as_mut() {
        engine.stop();
    }
    core.ended_reported = false;
    core.dirty = true;
    Ok(())
}

/// Busca una posición absoluta en segundos (acotada por el motor a la pista).
///
/// En streaming, un seek más allá de la frontera descargada queda pendiente:
/// el motor se pausa ("buffering") y el ticker lo completa cuando la descarga
/// alcanza la posición pedida.
#[tauri::command]
pub fn player_seek(
    app: AppHandle,
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
    sec: f64,
) -> Result<(), String> {
    if !sec.is_finite() || sec < 0.0 {
        return Err("invalid seek position".to_string());
    }
    let mut core = lock_core(&state)?;
    let seq = core.seq;
    {
        let Some(session) = cache.for_seq(seq) else {
            drop(core);
            return seek_engine(&state, sec);
        };
        if session.complete.load(Ordering::SeqCst) || session.is_canceled() {
            drop(session);
            drop(core);
            return seek_engine(&state, sec);
        }
        let Some(engine) = core.engine.as_mut() else {
            return Err(engine_error(EngineError::NothingLoaded));
        };
        let duration = engine.duration().map(|d| d.as_secs_f64());
        let gated = duration
            .and_then(|d| session.frontier_secs(d))
            .map(|frontier| sec > frontier - 1.0)
            .unwrap_or(false);
        if gated {
            let was_playing = matches!(engine.state(), EngineState::Playing);
            session.set_pending_seek(sec);
            session.set_resume_after_seek(was_playing);
            if was_playing {
                engine.pause();
            }
            session.paused_for_buffer.store(true, Ordering::SeqCst);
            core.dirty = true;
            drop(core);
            stream_cache::emit_state(&app, &session, None);
            return Ok(());
        }
    }
    drop(core);
    seek_engine(&state, sec)
}

/// Seek directo al motor (sin lógica de frontera de streaming).
fn seek_engine(state: &PlayerState, sec: f64) -> Result<(), String> {
    let mut core = lock_core(state)?;
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

/// Carga y reproduce una pista de la biblioteca: usa el archivo local
/// descargado si existe y, si no, resuelve una fuente de streaming
/// (Qobuz/Tidal) con caché progresiva.
///
/// La URL firmada vive sólo en backend; el resultado expone la calidad
/// realmente obtenida (`stream.qualityLabel`) para la etiqueta honesta.
#[tauri::command]
pub async fn player_load_track(
    app: AppHandle,
    state: State<'_, PlayerState>,
    cache: State<'_, StreamCacheState>,
    app_state: State<'_, crate::AppState>,
    track_id: i64,
    autoplay: bool,
    volume: f64,
) -> Result<PlayerLoadResult, String> {
    // 1) Archivo local descargado.
    let local: Option<String> =
        sqlx::query_scalar("SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1")
            .bind(track_id)
            .fetch_optional(&app_state.db)
            .await
            .map_err(|e| e.to_string())?;
    if let Some(path) = local {
        if Path::new(&path).is_file() {
            cache.cancel_current();
            let mut core = lock_core(&state)?;
            let duration = load_path_into_core(&mut core, Path::new(&path), autoplay, volume)?;
            core.seq += 1;
            return Ok(PlayerLoadResult {
                seq: core.seq,
                duration_sec: duration.map(|d| d.as_secs_f64()),
                stream: None,
            });
        }
    }

    // 2) Streaming: resolver la mejor fuente disponible.
    let http = stream_http_client();
    let resolved =
        crate::services::streaming::resolve_stream_for_track(&app_state.db, &http, track_id)
            .await?;
    let cache_path = stream_cache::cache_file_path(&app, track_id, &resolved.info)?;
    stream_cache::cleanup_cache(&app, Some(&cache_path));

    // Nueva generación: la sesión anterior se cancela antes de publicar el seq.
    cache.cancel_current();
    let seq = {
        let mut core = lock_core(&state)?;
        core.seq += 1;
        core.ended_reported = false;
        core.seq
    };

    // 2a) Caché completa de una reproducción anterior: sin red.
    if let Some(info) = stream_cache::read_complete_cache(&cache_path) {
        let session = StreamSession::new(seq, track_id, info.clone(), cache_path.clone());
        let total = std::fs::metadata(&cache_path).map(|m| m.len()).unwrap_or(0);
        session.total_bytes.store(total, Ordering::SeqCst);
        session.downloaded_bytes.store(total, Ordering::SeqCst);
        session.complete.store(true, Ordering::SeqCst);
        cache.install(session.clone());
        let mut core = lock_core(&state)?;
        if core.seq != seq {
            return Err("carga reemplazada por otra pista".to_string());
        }
        let duration = load_path_into_core(&mut core, &cache_path, autoplay, volume)?;
        session.started.store(true, Ordering::SeqCst);
        stream_cache::emit_state(&app, &session, None);
        return Ok(PlayerLoadResult {
            seq,
            duration_sec: duration.map(|d| d.as_secs_f64()),
            stream: Some(info),
        });
    }

    // 2b) Descarga progresiva: prime (buffer inicial) + tarea de fondo.
    let response = http
        .get(&resolved.url)
        .send()
        .await
        .map_err(|e| format!("No se pudo abrir el stream: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "El proveedor rechazó el stream (HTTP {})",
            response.status()
        ));
    }

    let session = StreamSession::new(seq, track_id, resolved.info.clone(), cache_path);
    cache.install(session.clone());
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<Option<f64>, String>>();
    spawn_stream_download(
        app.clone(),
        session.clone(),
        response,
        autoplay,
        volume,
        ready_tx,
    );

    match tokio::time::timeout(STREAM_PRIME_TIMEOUT, ready_rx).await {
        Ok(Ok(Ok(duration_sec))) => Ok(PlayerLoadResult {
            seq,
            duration_sec,
            stream: Some(resolved.info),
        }),
        Ok(Ok(Err(err))) => {
            session.canceled.store(true, Ordering::SeqCst);
            Err(err)
        }
        Ok(Err(_)) => {
            session.canceled.store(true, Ordering::SeqCst);
            Err("La descarga del stream terminó antes de poder reproducir".to_string())
        }
        Err(_) => {
            session.canceled.store(true, Ordering::SeqCst);
            Err("Tiempo de espera agotado obteniendo el buffer inicial del stream".to_string())
        }
    }
}

/// Cliente HTTP para streaming: sin timeout global (una pista puede tardar
/// minutos) pero con timeout de conexión.
fn stream_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/120.0.0.0 Safari/537.36",
        )
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Carga en el motor la caché en curso, validando que siga siendo la
/// generación vigente.
fn load_stream_into_engine(
    app: &AppHandle,
    session: &StreamSession,
    autoplay: bool,
    volume: f64,
) -> Result<Option<f64>, String> {
    let state = app.state::<PlayerState>();
    let mut core = lock_core(&state)?;
    if core.seq != session.seq {
        return Err("carga reemplazada por otra pista".to_string());
    }
    let duration = load_path_into_core(&mut core, &session.path, autoplay, volume)?;
    session.started.store(true, Ordering::SeqCst);
    Ok(duration.map(|d| d.as_secs_f64()))
}

/// Descarga progresiva: escribe la caché mientras el motor la reproduce.
/// El primer tramo (`prime_target`) abre el motor y responde por `ready`; el
/// resto continúa en segundo plano.
fn spawn_stream_download(
    app: AppHandle,
    session: Arc<StreamSession>,
    mut response: reqwest::Response,
    autoplay: bool,
    volume: f64,
    ready: tokio::sync::oneshot::Sender<Result<Option<f64>, String>>,
) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut ready = Some(ready);
        let total = response.content_length().unwrap_or(0);
        session.total_bytes.store(total, Ordering::SeqCst);
        let prime_target = if total > 0 {
            (total / 10).clamp(STREAM_MIN_PRIME_BYTES, STREAM_MAX_PRIME_BYTES)
        } else {
            STREAM_FALLBACK_PRIME_BYTES
        };

        let mut file = match tokio::fs::File::create(&session.path).await {
            Ok(file) => file,
            Err(e) => {
                let msg = format!("No se pudo crear la caché del stream: {}", e);
                if let Some(tx) = ready.take() {
                    let _ = tx.send(Err(msg.clone()));
                }
                stream_cache::emit_state(&app, &session, Some(msg));
                return;
            }
        };

        let mut primed = session.started.load(Ordering::SeqCst);
        let mut last_emit = Instant::now();

        loop {
            if session.is_canceled() {
                let _ = file.flush().await;
                if !session.complete.load(Ordering::SeqCst) {
                    let _ = tokio::fs::remove_file(&session.path).await;
                }
                if let Some(tx) = ready.take() {
                    let _ = tx.send(Err("carga reemplazada".to_string()));
                }
                return;
            }
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        let msg = format!("Error escribiendo la caché del stream: {}", e);
                        session.canceled.store(true, Ordering::SeqCst);
                        if let Some(tx) = ready.take() {
                            let _ = tx.send(Err(msg.clone()));
                        }
                        stream_cache::emit_state(&app, &session, Some(msg));
                        return;
                    }
                    let downloaded = session
                        .downloaded_bytes
                        .fetch_add(chunk.len() as u64, Ordering::SeqCst)
                        + chunk.len() as u64;
                    if !primed && downloaded >= prime_target {
                        primed = true;
                        match load_stream_into_engine(&app, &session, autoplay, volume) {
                            Ok(duration) => {
                                if let Some(tx) = ready.take() {
                                    let _ = tx.send(Ok(duration));
                                }
                            }
                            Err(err) => {
                                session.canceled.store(true, Ordering::SeqCst);
                                if let Some(tx) = ready.take() {
                                    let _ = tx.send(Err(err.clone()));
                                }
                                stream_cache::emit_state(&app, &session, Some(err));
                                return;
                            }
                        }
                    }
                    if last_emit.elapsed() >= STREAM_EVENT_INTERVAL {
                        last_emit = Instant::now();
                        stream_cache::emit_state(&app, &session, None);
                    }
                }
                Ok(None) => {
                    let _ = file.flush().await;
                    session.complete.store(true, Ordering::SeqCst);
                    if !primed {
                        match load_stream_into_engine(&app, &session, autoplay, volume) {
                            Ok(duration) => {
                                if let Some(tx) = ready.take() {
                                    let _ = tx.send(Ok(duration));
                                }
                            }
                            Err(err) => {
                                session.canceled.store(true, Ordering::SeqCst);
                                if let Some(tx) = ready.take() {
                                    let _ = tx.send(Err(err.clone()));
                                }
                                stream_cache::emit_state(&app, &session, Some(err));
                                return;
                            }
                        }
                    }
                    let _ = stream_cache::write_done_marker(&session);
                    stream_cache::emit_state(&app, &session, None);
                    return;
                }
                Err(e) => {
                    let msg = format!("Se interrumpió la descarga del stream: {}", e);
                    if let Some(tx) = ready.take() {
                        let _ = tx.send(Err(msg.clone()));
                    }
                    stream_cache::emit_state(&app, &session, Some(msg));
                    return;
                }
            }
        }
    })
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
    // Streaming: si la reproducción alcanza la frontera descargada, pausar y
    // esperar a que la descarga avance (y completar seeks pendientes).
    if let Some(cache_state) = app.try_state::<StreamCacheState>() {
        if let Some(session) = cache_state.for_seq(*seq) {
            guard_stream_frontier(app, engine, &session, dirty);
        }
    }
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

/// Guarda de frontera del streaming: mantiene la reproducción por detrás de
/// lo descargado.
///
/// - Seek pendiente: se aplica cuando la descarga alcanza el destino (+3 s de
///   margen) y se reanuda si el usuario buscaba mientras sonaba.
/// - Reproducción en directo: si la posición entra en los últimos segundos
///   descargados, se pausa (`buffering`); se reanuda cuando lo descargado
///   supera la posición + 10 s.
fn guard_stream_frontier(
    app: &AppHandle,
    engine: &mut PlayerEngine,
    session: &StreamSession,
    dirty: &mut bool,
) {
    if session.complete.load(Ordering::SeqCst) || session.is_canceled() {
        return;
    }
    let Some(duration) = engine.duration().map(|d| d.as_secs_f64()) else {
        return;
    };
    let Some(frontier) = session.frontier_secs(duration) else {
        return;
    };

    if let Some(target) = session.take_pending_seek() {
        if frontier >= target + 3.0 {
            if engine.seek(Duration::from_secs_f64(target)).is_ok() {
                if session.take_resume_after_seek() {
                    let _ = engine.resume();
                }
                session.paused_for_buffer.store(false, Ordering::SeqCst);
                *dirty = true;
                stream_cache::emit_state(app, session, None);
            } else {
                session.set_pending_seek(target);
            }
        } else {
            session.set_pending_seek(target);
            session.paused_for_buffer.store(true, Ordering::SeqCst);
        }
        return;
    }

    let position = engine.position().as_secs_f64();
    let playing = matches!(engine.state(), EngineState::Playing);
    if playing && position >= frontier - FRONTIER_PAUSE_MARGIN_SECS {
        engine.pause();
        session.paused_for_buffer.store(true, Ordering::SeqCst);
        *dirty = true;
        stream_cache::emit_state(app, session, None);
    } else if !playing
        && session.paused_for_buffer.load(Ordering::SeqCst)
        && frontier >= position + FRONTIER_RESUME_MARGIN_SECS
    {
        let _ = engine.resume();
        session.paused_for_buffer.store(false, Ordering::SeqCst);
        *dirty = true;
        stream_cache::emit_state(app, session, None);
    }
}
