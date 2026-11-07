//! Sesión de reproducción (ADR-0024): la puerta única de UNA pista.
//!
//! Extraída de `commands::player` con comportamiento idéntico (Fase B de P6):
//! resuelve la fuente (fichero local → caché completa → streaming progresivo o
//! DASH), gobierna el transporte sobre el motor (`syncify-player`), la
//! frontera de streaming y la generación (`seq`), y produce un paso de tick
//! para que el ticker del comando emita `player:tick`, `player:ended`, MPRIS y
//! el estado de stream.
//!
//! Reparto:
//! - Aquí: la sesión (estado del motor, generación, prime/frontera, spawns de
//!   descarga). La resolución de streaming es inyectable (`StreamResolver`);
//!   producción usa `streaming::resolve_stream_for_track`.
//! - En `commands::player`: los comandos Tauri delegan en la sesión y el
//!   ticker de 250 ms consume `PlayerState::tick()`.
//! - En la UI (`usePlayer.ts`): cola, historial, modos, crédito de escucha y
//!   persistencia.
//!
//! Eventos (el frontend descarta todo lo que no case con su `seq`):
//! - `player:tick {seq, state, position_sec, duration_sec}` — cada 250 ms
//!   mientras suena, y una vez tras cada cambio de transporte (carga, pausa,
//!   seek en pausa...).
//! - `player:ended {seq}` — una única vez por carga cuando la pista llegó a su
//!   final natural (no con `player_stop`).

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::SqlitePool;
use syncify_player::{EngineError, EngineState, PlayerEngine};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

use super::mpris::PlaybackStatus;
use super::stream_cache::{self, StallCheck, StreamCacheState, StreamSession, StreamStatePayload};
use super::streaming::{self, DashManifest, ResolvedStream, StreamSourceInfo};

/// Buffer mínimo antes de abrir el motor para una pista en streaming.
pub(crate) const STREAM_MIN_PRIME_BYTES: u64 = 256 * 1024;
/// Techo del buffer inicial (no esperar más de esto aunque quede descarga).
pub(crate) const STREAM_MAX_PRIME_BYTES: u64 = 8 * 1024 * 1024;
/// Buffer inicial cuando el servidor no declara Content-Length.
pub(crate) const STREAM_FALLBACK_PRIME_BYTES: u64 = 4 * 1024 * 1024;
/// Paciencia máxima para el buffer inicial.
pub(crate) const STREAM_PRIME_TIMEOUT: Duration = Duration::from_secs(45);
/// Ventana de espera acotada del seek pendiente (Q4): si en este tiempo la
/// descarga no avanza nada, se re-resuelve la fuente una vez; si tras el
/// relleno sigue estancada otra ventana igual, error tipado.
pub(crate) const STREAM_PENDING_SEEK_TIMEOUT: Duration = Duration::from_secs(10);
/// Pausa por buffer cuando la posición se acerca a lo descargado.
pub(crate) const FRONTIER_PAUSE_MARGIN_SECS: f64 = 5.0;
/// Reanuda cuando lo descargado supera la posición + este margen.
pub(crate) const FRONTIER_RESUME_MARGIN_SECS: f64 = 10.0;

/// Errores internos tipados del relleno de la sesión de streaming (ADR-0024):
/// por dentro la sesión distingue los casos; por fuera el payload de
/// `player:stream-state` sigue llevando `error: Option<String>` con el
/// mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StreamFillError {
    /// La re-resolución única falló (sin fuente o sin red).
    ReResolve(String),
    /// La fuente re-resuelta no permite continuar el relleno en curso (DASH
    /// con motor ya cargado, calidad distinta, sin soporte de Range).
    Incompatible(String),
    /// Tras la re-resolución, la descarga siguió estancada dentro de la
    /// espera acotada del seek.
    StillStalled,
}

impl StreamFillError {
    /// Mensaje que viaja en `player:stream-state.error`.
    pub(crate) fn message(&self) -> String {
        match self {
            StreamFillError::ReResolve(e) => {
                format!("El buffer dejó de avanzar y el reintento de fuente falló: {e}")
            }
            StreamFillError::Incompatible(detail) => format!(
                "El buffer dejó de avanzar y el reintento no sirve para la descarga en curso \
                 ({detail})"
            ),
            StreamFillError::StillStalled => {
                "El buffer sigue sin avanzar tras reintentar la fuente".to_string()
            }
        }
    }
}
/// Cadencia máxima de eventos `player:stream-state` durante la descarga.
const STREAM_EVENT_INTERVAL: Duration = Duration::from_millis(750);

/// Payload de `player:tick`.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerTickPayload {
    /// Generación de la carga actual (1, 2, ...). La UI compara con el `seq`
    /// devuelto por `player_load_track` y descarta eventos de una pista ya
    /// sustituida.
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

/// Resultado de una carga (`player_load_track`).
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
pub(crate) struct PlayerCore {
    pub(crate) engine: Option<PlayerEngine>,
    pub(crate) seq: u64,
    /// Fuerza un `player:tick` en el siguiente paso del ticker aunque el
    /// transporte esté parado (p. ej. seek en pausa: la posición cambió sin
    /// ticks de reproducción).
    pub(crate) dirty: bool,
    /// `player:ended` ya emitido para la carga actual.
    pub(crate) ended_reported: bool,
}

/// Resolución de streaming inyectable (ADR-0024): producción delega en
/// `streaming::resolve_stream_for_track`; las pruebas de Fase A/C pueden
/// instalar un doble que resuelva sin red.
type ResolveFuture = Pin<Box<dyn Future<Output = Result<ResolvedStream, String>> + Send>>;
pub(crate) type StreamResolver =
    Arc<dyn Fn(SqlitePool, reqwest::Client, i64) -> ResolveFuture + Send + Sync>;

fn production_resolver() -> StreamResolver {
    Arc::new(|db, http, track_id| {
        Box::pin(async move { streaming::resolve_stream_for_track(&db, &http, track_id).await })
    })
}

/// Estado gestionado del player (singleton de la app): la sesión de
/// reproducción de ADR-0024. Única dueña de la generación (`seq`).
pub struct PlayerState {
    core: Mutex<PlayerCore>,
    resolver: StreamResolver,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            core: Mutex::new(PlayerCore::default()),
            resolver: production_resolver(),
        }
    }
}

#[cfg(test)]
impl PlayerState {
    /// Sólo pruebas: sesión con un resolver falso (las pruebas de Fase A/C
    /// resuelven sin red contra servidores locales).
    pub(crate) fn with_resolver(resolver: StreamResolver) -> Self {
        Self {
            core: Mutex::new(PlayerCore::default()),
            resolver,
        }
    }
}

/// Lo que un paso del ticker necesita emitir: `player:tick`, `player:ended`,
/// la actualización de MPRIS y los estados de stream que la frontera haya
/// producido (se emiten en este orden, como dentro de `collect_tick`).
/// `stall_refill` pide lanzar la re-resolución única del seek estancado (Q4).
#[derive(Default)]
pub struct TickOutput {
    pub stream_states: Vec<StreamStatePayload>,
    pub mpris: Option<(PlaybackStatus, Duration)>,
    pub tick: Option<PlayerTickPayload>,
    pub ended: Option<PlayerEndedPayload>,
    pub stall_refill: Option<Arc<StreamSession>>,
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

pub(crate) fn lock_core(
    state: &PlayerState,
) -> Result<std::sync::MutexGuard<'_, PlayerCore>, String> {
    state
        .core
        .lock()
        .map_err(|_| "player state poisoned".to_string())
}

/// Carga un fichero (local o caché de streaming ya primada) en el motor.
/// No incrementa `seq`: el llamante decide la generación.
pub(crate) fn load_path_into_core(
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

impl PlayerState {
    /// Resolver de streaming en uso (producción o el doble de pruebas).
    pub(crate) fn resolver(&self) -> StreamResolver {
        self.resolver.clone()
    }

    /// Carga y reproduce una pista de la biblioteca: usa el archivo local
    /// descargado si existe y, si no, resuelve una fuente de streaming
    /// (Qobuz/Tidal) con caché progresiva. La URL firmada vive sólo en
    /// backend; el resultado expone la calidad realmente obtenida.
    pub async fn open<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        cache: &StreamCacheState,
        db: &SqlitePool,
        track_id: i64,
        autoplay: bool,
        volume: f64,
    ) -> Result<PlayerLoadResult, String> {
        // 1) Archivo local descargado.
        let local: Option<String> =
            sqlx::query_scalar("SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1")
                .bind(track_id)
                .fetch_optional(db)
                .await
                .map_err(|e| e.to_string())?;
        if let Some(path) = local {
            if Path::new(&path).is_file() {
                cache.cancel_current();
                let mut core = lock_core(self)?;
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
        let resolved = (self.resolver)(db.clone(), http.clone(), track_id).await?;
        let cache_path = stream_cache::cache_file_path(app, track_id, &resolved.info)?;
        stream_cache::cleanup_cache(app, Some(&cache_path));

        // Nueva generación: la sesión anterior se cancela antes de publicar el seq.
        cache.cancel_current();
        let seq = {
            let mut core = lock_core(self)?;
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
            let mut core = lock_core(self)?;
            if core.seq != seq {
                return Err("carga reemplazada por otra pista".to_string());
            }
            let duration = load_path_into_core(&mut core, &cache_path, autoplay, volume)?;
            session.started.store(true, Ordering::SeqCst);
            stream_cache::emit_state(app, &session, None);
            return Ok(PlayerLoadResult {
                seq,
                duration_sec: duration.map(|d| d.as_secs_f64()),
                stream: Some(info),
            });
        }

        // 2b) Descarga progresiva: prime (buffer inicial) + tarea de fondo.
        // El canal `ready` y la tarea viven en la sesión: si la descarga se
        // estanca y el relleno la sustituye (Q4), el prime tardío sigue
        // respondiendo a esta carga.
        let session = StreamSession::new(seq, track_id, resolved.info.clone(), cache_path);
        session.set_transport_intent(autoplay, volume);
        cache.install(session.clone());
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<Option<f64>, String>>();
        session.install_ready(ready_tx);

        if let Some(dash) = resolved.dash.clone() {
            // Tidal lossless/hi-res llega segmentado: el backend lo convierte a
            // FLAC nativo progresivo (init + frames) mientras se reproduce.
            let handle = spawn_dash_stream_download(
                app.clone(),
                session.clone(),
                http.clone(),
                dash,
                autoplay,
                volume,
            );
            session.set_task(handle);
        } else {
            let response = match http.get(&resolved.url).send().await {
                Ok(response) => response,
                Err(e) => {
                    session.canceled.store(true, Ordering::SeqCst);
                    return Err(format!("No se pudo abrir el stream: {}", e));
                }
            };
            if !response.status().is_success() {
                session.canceled.store(true, Ordering::SeqCst);
                return Err(format!(
                    "El proveedor rechazó el stream (HTTP {})",
                    response.status()
                ));
            }
            let handle = spawn_stream_download(
                app.clone(),
                session.clone(),
                response,
                autoplay,
                volume,
                None,
            );
            session.set_task(handle);
        }

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

    /// Pausa (no-op si no hay nada sonando).
    pub fn pause(&self) -> Result<(), String> {
        let mut core = lock_core(self)?;
        if let Some(engine) = core.engine.as_mut() {
            if matches!(engine.state(), EngineState::Playing) {
                engine.pause();
                core.dirty = true;
            }
        }
        Ok(())
    }

    /// Reanuda la pista pausada (no-op si no hay nada cargado).
    pub fn resume(&self) -> Result<(), String> {
        let mut core = lock_core(self)?;
        if let Some(engine) = core.engine.as_mut() {
            if engine.loaded().is_some() {
                engine.resume().map_err(engine_error)?;
                core.dirty = true;
            }
        }
        Ok(())
    }

    /// Detiene y descarga la pista actual. El stream de salida sigue abierto.
    pub fn stop(&self, cache: &StreamCacheState) -> Result<(), String> {
        cache.cancel_current();
        let mut core = lock_core(self)?;
        if let Some(engine) = core.engine.as_mut() {
            engine.stop();
        }
        core.ended_reported = false;
        core.dirty = true;
        Ok(())
    }

    /// Busca una posición absoluta en segundos (acotada por el motor a la
    /// pista). En streaming, un seek más allá de la frontera descargada queda
    /// pendiente: el motor se pausa ("buffering") y el ticker lo completa
    /// cuando la descarga alcanza la posición pedida.
    pub fn seek<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        cache: &StreamCacheState,
        sec: f64,
    ) -> Result<(), String> {
        if !sec.is_finite() || sec < 0.0 {
            return Err("invalid seek position".to_string());
        }
        let mut core = lock_core(self)?;
        let seq = core.seq;
        {
            let Some(session) = cache.for_seq(seq) else {
                drop(core);
                return self.seek_engine(sec);
            };
            if session.complete.load(Ordering::SeqCst) || session.is_canceled() {
                drop(session);
                drop(core);
                return self.seek_engine(sec);
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
                // Espera acotada (Q4): el reloj de estancamiento arranca aquí.
                session.begin_stall_watch();
                if was_playing {
                    engine.pause();
                }
                session.paused_for_buffer.store(true, Ordering::SeqCst);
                core.dirty = true;
                drop(core);
                stream_cache::emit_state(app, &session, None);
                return Ok(());
            }
        }
        drop(core);
        self.seek_engine(sec)
    }

    /// Seek directo al motor (sin lógica de frontera de streaming).
    fn seek_engine(&self, sec: f64) -> Result<(), String> {
        let mut core = lock_core(self)?;
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
    pub fn set_volume(&self, volume: f64) -> Result<(), String> {
        let mut core = lock_core(self)?;
        if let Some(engine) = core.engine.as_mut() {
            engine.set_volume(volume.clamp(0.0, 1.0) as f32);
        }
        Ok(())
    }

    /// Ritmo de reproducción (`1.0` = normal). Nota: rodio varía el tono con la
    /// velocidad (sin time-stretch), a diferencia de `audio.playbackRate`.
    pub fn set_rate(&self, rate: f64) -> Result<(), String> {
        if !rate.is_finite() || rate <= 0.0 {
            return Err("invalid playback rate".to_string());
        }
        let mut core = lock_core(self)?;
        if let Some(engine) = core.engine.as_mut() {
            engine.set_rate(rate as f32);
        }
        Ok(())
    }

    /// Un paso del ticker: produce lo que hay que emitir — los estados de
    /// stream que la frontera haya generado, la actualización de MPRIS,
    /// `player:tick` mientras suena (o si hay cambios pendientes) y
    /// `player:ended` la primera vez que la cola queda vacía con la pista aún
    /// cargada (final natural). El ticker del comando emite en ese orden.
    pub fn tick(&self, cache: Option<&StreamCacheState>) -> TickOutput {
        let Ok(mut core) = self.core.lock() else {
            return TickOutput::default();
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
            return TickOutput::default();
        };
        // Streaming: si la reproducción alcanza la frontera descargada, pausar y
        // esperar a que la descarga avance (y completar seeks pendientes, con
        // espera acotada si la descarga se estanca).
        let mut stream_states = Vec::new();
        let mut stall_refill = None;
        if let Some(cache) = cache {
            if let Some(session) = cache.for_seq(*seq) {
                match guard_stream_frontier(engine, &session, dirty) {
                    FrontierOutcome::Quiet => {}
                    FrontierOutcome::State(payload) => stream_states.push(payload),
                    FrontierOutcome::StallRefill => stall_refill = Some(session),
                }
            }
        }
        let engine_state = engine.state();
        // MPRIS: estado + posición (el hilo D-Bus emite PropertiesChanged si
        // cambian; la posición sólo se expone, nunca se emite en el diff).
        let mpris = Some((
            match engine_state {
                EngineState::Playing => PlaybackStatus::Playing,
                EngineState::Paused => PlaybackStatus::Paused,
                EngineState::Stopped => PlaybackStatus::Stopped,
            },
            engine.position(),
        ));
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
        TickOutput {
            stream_states,
            mpris,
            tick,
            ended,
            stall_refill,
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
pub(crate) fn load_stream_into_engine<R: tauri::Runtime>(
    app: &AppHandle<R>,
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
/// El primer tramo (`prime_target`) abre el motor y responde por el canal
/// `ready` que la sesión lleva; el resto continúa en segundo plano.
///
/// `resume_from` es el modo relevo tras un estancamiento (Q4): el motor ya
/// está cargado, así que se añade por Range desde el byte escrito (la respuesta
/// 206 empieza exactamente ahí) sin volver a primar.
pub(crate) fn spawn_stream_download<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    session: Arc<StreamSession>,
    mut response: reqwest::Response,
    autoplay: bool,
    volume: f64,
    resume_from: Option<u64>,
) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        if let Some(from) = resume_from {
            run_stream_takeover(&app, &session, response, from).await;
            return;
        }
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
                if let Some(tx) = session.take_ready() {
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
                if let Some(tx) = session.take_ready() {
                    let _ = tx.send(Err("carga reemplazada".to_string()));
                }
                return;
            }
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        let msg = format!("Error escribiendo la caché del stream: {}", e);
                        session.canceled.store(true, Ordering::SeqCst);
                        if let Some(tx) = session.take_ready() {
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
                                if let Some(tx) = session.take_ready() {
                                    let _ = tx.send(Ok(duration));
                                }
                            }
                            Err(err) => {
                                session.canceled.store(true, Ordering::SeqCst);
                                if let Some(tx) = session.take_ready() {
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
                                if let Some(tx) = session.take_ready() {
                                    let _ = tx.send(Ok(duration));
                                }
                            }
                            Err(err) => {
                                session.canceled.store(true, Ordering::SeqCst);
                                if let Some(tx) = session.take_ready() {
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
                    if let Some(tx) = session.take_ready() {
                        let _ = tx.send(Err(msg.clone()));
                    }
                    stream_cache::emit_state(&app, &session, Some(msg));
                    return;
                }
            }
        }
    })
}

/// Modo relevo del relleno tras un estancamiento (Q4): el motor ya está
/// cargado con la caché en curso, así que se añaden los bytes de la respuesta
/// 206 (que empieza en `from`) sin tocar lo escrito ni volver a primar. Si el
/// fichero tiene una cola rasgada de la tarea abortada, se recorta a `from`.
async fn run_stream_takeover<R: tauri::Runtime>(
    app: &AppHandle<R>,
    session: &Arc<StreamSession>,
    mut response: reqwest::Response,
    from: u64,
) {
    let mut file = match tokio::fs::OpenOptions::new()
        .write(true)
        .open(&session.path)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            let msg = format!("No se pudo reabrir la caché del stream: {}", e);
            session.canceled.store(true, Ordering::SeqCst);
            stream_cache::emit_state(app, session, Some(msg));
            return;
        }
    };
    use std::io::SeekFrom;
    use tokio::io::AsyncSeekExt;
    let reopen_failed = |session: &StreamSession, e: String| -> String {
        session.canceled.store(true, Ordering::SeqCst);
        session.paused_for_buffer.store(false, Ordering::SeqCst);
        format!("No se pudo reabrir la caché del stream: {}", e)
    };
    let disk_len = match file.metadata().await {
        Ok(meta) => meta.len(),
        Err(e) => {
            let msg = reopen_failed(session, e.to_string());
            stream_cache::emit_state(app, session, Some(msg));
            return;
        }
    };
    if disk_len < from {
        // Bytes contados que no están en el disco: no hay reanudación fiable.
        let msg = reopen_failed(
            session,
            format!("el fichero mide {} bytes y se esperaban {}", disk_len, from),
        );
        stream_cache::emit_state(app, session, Some(msg));
        return;
    }
    if disk_len > from {
        // Bytes contados de más (escritura abortada a medias): recorte limpio.
        if let Err(e) = file.set_len(from).await {
            let msg = reopen_failed(session, e.to_string());
            stream_cache::emit_state(app, session, Some(msg));
            return;
        }
    }
    if let Err(e) = file.seek(SeekFrom::Start(from)).await {
        let msg = reopen_failed(session, e.to_string());
        stream_cache::emit_state(app, session, Some(msg));
        return;
    }

    let mut last_emit = Instant::now();
    loop {
        if session.is_canceled() {
            let _ = file.flush().await;
            if !session.complete.load(Ordering::SeqCst) {
                let _ = tokio::fs::remove_file(&session.path).await;
            }
            return;
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if let Err(e) = file.write_all(&chunk).await {
                    let msg = format!("Error escribiendo la caché del stream: {}", e);
                    session.canceled.store(true, Ordering::SeqCst);
                    stream_cache::emit_state(app, session, Some(msg));
                    return;
                }
                session
                    .downloaded_bytes
                    .fetch_add(chunk.len() as u64, Ordering::SeqCst);
                if last_emit.elapsed() >= STREAM_EVENT_INTERVAL {
                    last_emit = Instant::now();
                    stream_cache::emit_state(app, session, None);
                }
            }
            Ok(None) => {
                let _ = file.flush().await;
                session.complete.store(true, Ordering::SeqCst);
                let _ = stream_cache::write_done_marker(session);
                stream_cache::emit_state(app, session, None);
                return;
            }
            Err(e) => {
                let msg = format!("Se interrumpió la descarga del stream: {}", e);
                stream_cache::emit_state(app, session, Some(msg));
                return;
            }
        }
    }
}

/// Descarga progresiva de un manifiesto DASH de Tidal: convierte cada segmento
/// (FLAC-en-ISOBMFF) en frames de un FLAC nativo que crece, para que el motor
/// pueda reproducirlo y buscar como con cualquier FLAC local. El prime
/// responde por el canal `ready` que la sesión lleva.
fn spawn_dash_stream_download<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    session: Arc<StreamSession>,
    http: reqwest::Client,
    dash: DashManifest,
    autoplay: bool,
    volume: f64,
) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut prime = || match load_stream_into_engine(&app, &session, autoplay, volume) {
            Ok(duration) => {
                if let Some(tx) = session.take_ready() {
                    let _ = tx.send(Ok(duration));
                }
                Ok(())
            }
            Err(err) => {
                if let Some(tx) = session.take_ready() {
                    let _ = tx.send(Err(err.clone()));
                }
                Err(err)
            }
        };
        let mut progress = || stream_cache::emit_state(&app, &session, None);
        let result =
            stream_cache::download_dash_to_cache(&http, &session, &dash, &mut prime, &mut progress)
                .await;
        match result {
            Ok(()) => {
                if session.is_canceled() {
                    if let Some(tx) = session.take_ready() {
                        let _ = tx.send(Err("carga reemplazada".to_string()));
                    }
                } else {
                    stream_cache::emit_state(&app, &session, None);
                }
            }
            Err(err) => {
                session.canceled.store(true, Ordering::SeqCst);
                if let Some(tx) = session.take_ready() {
                    let _ = tx.send(Err(err.clone()));
                }
                stream_cache::emit_state(&app, &session, Some(err));
            }
        }
    })
}

/// Re-resolución única tras la espera acotada del seek pendiente (Q4;
/// ADR-0010.enm.4): resuelve la fuente de nuevo y rellena la MISMA caché.
///
/// - Motor ya cargado (`started`): la fuente re-resuelta continúa por Range
///   desde el byte escrito (modo relevo de `spawn_stream_download`); si la
///   fuente nueva no es compatible con la descarga en curso, error tipado.
/// - Sin prime aún: reinicia la descarga desde cero con la intención de
///   transporte original; el prime responde por el `ready` que la sesión
///   lleva, así que la carga que espera no se entera del relevo.
///
/// Cualquier fallo termina en el error tipado de la sesión y en un estado de
/// error de `player:stream-state` (payload sin cambiar). La tarea que lo
/// invoca la lanza el ticker cuando `PlayerState::tick` reporta
/// `FrontierOutcome::StallRefill`; en pruebas se llama directamente.
pub(crate) fn spawn_stall_refill<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    session: Arc<StreamSession>,
) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move { run_stall_refill(&app, session).await })
}

async fn run_stall_refill<R: tauri::Runtime>(app: &AppHandle<R>, session: Arc<StreamSession>) {
    // La pista pudo cambiar entre el aviso del guard y aquí: sesión muerta,
    // nada que rellenar.
    if session.is_canceled() || session.complete.load(Ordering::SeqCst) {
        return;
    }
    // La tarea estancada no puede escribir más: se aborta antes de resolver
    // para que no compita con el relevo por el fichero de caché.
    session.abort_task();
    let db = app.state::<crate::AppState>().db.clone();
    let http = stream_http_client();
    let state = app.state::<PlayerState>();
    let resolver = state.resolver();
    let resolved = match resolver(db, http.clone(), session.track_id).await {
        Ok(resolved) => resolved,
        Err(e) => return finish_stall(app, &session, StreamFillError::ReResolve(e)),
    };
    let (autoplay, volume) = session.transport_intent();

    if session.started.load(Ordering::SeqCst) {
        if resolved.dash.is_some() || !compatible_source(&session.info, &resolved.info) {
            return finish_stall(
                app,
                &session,
                StreamFillError::Incompatible(
                    "la nueva fuente no es la misma descarga".to_string(),
                ),
            );
        }
        let from = session.downloaded_bytes.load(Ordering::SeqCst);
        let response = match http
            .get(&resolved.url)
            .header(reqwest::header::RANGE, format!("bytes={from}-"))
            .send()
            .await
        {
            Ok(response) if response.status() == reqwest::StatusCode::PARTIAL_CONTENT => response,
            Ok(response) => {
                return finish_stall(
                    app,
                    &session,
                    StreamFillError::Incompatible(format!(
                        "sin soporte de Range (HTTP {})",
                        response.status()
                    )),
                );
            }
            Err(e) => {
                return finish_stall(app, &session, StreamFillError::ReResolve(e.to_string()))
            }
        };
        tracing::info!(
            track_id = session.track_id,
            from,
            "streaming: relleno tras estancamiento por Range desde el byte escrito"
        );
        let handle = spawn_stream_download(
            app.clone(),
            session.clone(),
            response,
            autoplay,
            volume,
            Some(from),
        );
        session.set_task(handle);
    } else {
        // Sin prime aún: la caché se reescribe desde cero (nada la está
        // leyendo) y el prime responde por el `ready` de la sesión.
        if !resolved.dash.is_some() && !compatible_source(&session.info, &resolved.info) {
            return finish_stall(
                app,
                &session,
                StreamFillError::Incompatible("la nueva fuente es otra calidad".to_string()),
            );
        }
        session.downloaded_bytes.store(0, Ordering::SeqCst);
        session.audio_millis.store(0, Ordering::SeqCst);
        match resolved.dash.clone() {
            Some(dash) => {
                tracing::info!(
                    track_id = session.track_id,
                    "streaming: relleno tras estancamiento reinicia la descarga DASH"
                );
                let handle = spawn_dash_stream_download(
                    app.clone(),
                    session.clone(),
                    http.clone(),
                    dash,
                    autoplay,
                    volume,
                );
                session.set_task(handle);
            }
            None => match http.get(&resolved.url).send().await {
                Ok(response) if response.status().is_success() => {
                    tracing::info!(
                        track_id = session.track_id,
                        "streaming: relleno tras estancamiento reinicia la descarga desde cero"
                    );
                    let handle = spawn_stream_download(
                        app.clone(),
                        session.clone(),
                        response,
                        autoplay,
                        volume,
                        None,
                    );
                    session.set_task(handle);
                }
                Ok(response) => {
                    finish_stall(
                        app,
                        &session,
                        StreamFillError::ReResolve(format!("HTTP {}", response.status())),
                    );
                }
                Err(e) => {
                    finish_stall(app, &session, StreamFillError::ReResolve(e.to_string()));
                }
            },
        }
    }
}

/// Fin de línea del relleno estancado: aborta la tarea, suelta el seek
/// pendiente, limpia el buffering, cancela la sesión y devuelve el estado de
/// error para `player:stream-state` (payload sin cambiar).
fn terminate_stall(session: &StreamSession, err: StreamFillError) -> StreamStatePayload {
    tracing::warn!(
        track_id = session.track_id,
        error = %err.message(),
        "streaming: relleno tras estancamiento falló"
    );
    session.abort_task();
    session.clear_pending_seek();
    session.paused_for_buffer.store(false, Ordering::SeqCst);
    session.canceled.store(true, Ordering::SeqCst);
    stream_cache::state_payload(session, Some(err.message()))
}

/// Fin de línea del relleno lanzado por el ticker: terminaliza y emite.
fn finish_stall<R: tauri::Runtime>(
    app: &AppHandle<R>,
    session: &Arc<StreamSession>,
    err: StreamFillError,
) {
    let payload = terminate_stall(session, err);
    stream_cache::emit_state(app, session, payload.error);
}

/// Dos fuentes son la misma descarga reanudable: mismo servicio, formato y
/// calidad física (el nombre de caché codifica exactamente esos campos).
pub(crate) fn compatible_source(a: &StreamSourceInfo, b: &StreamSourceInfo) -> bool {
    a.service_name == b.service_name
        && a.extension == b.extension
        && a.bit_depth == b.bit_depth
        && a.sample_rate == b.sample_rate
}

/// Lo que un paso de la guarda de frontera pide al ticker.
#[derive(Debug)]
pub(crate) enum FrontierOutcome {
    /// Nada que hacer.
    Quiet,
    /// Estado de stream a emitir por `player:stream-state`.
    State(StreamStatePayload),
    /// El seek pendiente lleva una ventana completa de espera acotada sin
    /// progreso de descarga: lanzar la re-resolución única (relleno).
    StallRefill,
}

/// Guarda de frontera del streaming: mantiene la reproducción por detrás de
/// lo descargado.
///
/// - Seek pendiente: se aplica cuando la descarga alcanza el destino (+3 s de
///   margen) y se reanuda si el usuario buscaba mientras sonaba. La espera es
///   ACOTADA (Q4): si la descarga no avanza nada durante
///   `STREAM_PENDING_SEEK_TIMEOUT`, se pide la re-resolución única; si tras el
///   relleno sigue estancada otra ventana igual, se suelta el seek con un
///   error tipado. Una sesión completa sirve el seek directamente (los datos
///   ya están) y una sesión interrumpida lo suelta sin dejar buffering.
/// - Reproducción en directo: si la posición entra en los últimos segundos
///   descargados, se pausa (`buffering`); se reanuda cuando lo descargado
///   supera la posición + 10 s.
pub(crate) fn guard_stream_frontier(
    engine: &mut PlayerEngine,
    session: &StreamSession,
    dirty: &mut bool,
) -> FrontierOutcome {
    // El seek pendiente se atiende antes que nada: una sesión completa sirve
    // el seek directamente (los datos ya están) y una interrumpida lo suelta
    // sin dejar buffering; ninguna de las dos puede quedarse esperando.
    if let Some(target) = session.take_pending_seek() {
        if session.complete.load(Ordering::SeqCst) {
            return serve_pending_seek(engine, session, dirty, target);
        }
        if session.is_canceled() {
            session.paused_for_buffer.store(false, Ordering::SeqCst);
            return FrontierOutcome::State(stream_cache::state_payload(session, None));
        }
        let duration = engine.duration().map(|d| d.as_secs_f64());
        let reachable = duration
            .and_then(|d| session.frontier_secs(d))
            .map(|frontier| frontier >= target + 3.0)
            .unwrap_or(false);
        if reachable {
            return serve_pending_seek(engine, session, dirty, target);
        }
        // Aparcar de nuevo y vigilar el estancamiento: ventana acotada sin
        // progreso -> re-resolución única; otra ventana tras el relleno ->
        // error tipado.
        session.set_pending_seek(target);
        session.paused_for_buffer.store(true, Ordering::SeqCst);
        return match session.stall_check(STREAM_PENDING_SEEK_TIMEOUT) {
            StallCheck::Progressing => FrontierOutcome::Quiet,
            StallCheck::Stalled => FrontierOutcome::StallRefill,
            StallCheck::Exhausted => {
                FrontierOutcome::State(terminate_stall(session, StreamFillError::StillStalled))
            }
        };
    }

    if session.complete.load(Ordering::SeqCst) || session.is_canceled() {
        return FrontierOutcome::Quiet;
    }
    let duration = match engine.duration().map(|d| d.as_secs_f64()) {
        Some(duration) => duration,
        None => return FrontierOutcome::Quiet,
    };
    let Some(frontier) = session.frontier_secs(duration) else {
        return FrontierOutcome::Quiet;
    };

    let position = engine.position().as_secs_f64();
    let playing = matches!(engine.state(), EngineState::Playing);
    if playing && position >= frontier - FRONTIER_PAUSE_MARGIN_SECS {
        engine.pause();
        session.paused_for_buffer.store(true, Ordering::SeqCst);
        *dirty = true;
        FrontierOutcome::State(stream_cache::state_payload(session, None))
    } else if !playing
        && session.paused_for_buffer.load(Ordering::SeqCst)
        && frontier >= position + FRONTIER_RESUME_MARGIN_SECS
    {
        let _ = engine.resume();
        session.paused_for_buffer.store(false, Ordering::SeqCst);
        *dirty = true;
        FrontierOutcome::State(stream_cache::state_payload(session, None))
    } else {
        FrontierOutcome::Quiet
    }
}

/// Sirve un seek pendiente: mueve el motor, reanuda si el usuario buscaba
/// sonando y limpia el buffering. Si el motor no puede moverse, el seek vuelve
/// a la cola para el siguiente paso.
fn serve_pending_seek(
    engine: &mut PlayerEngine,
    session: &StreamSession,
    dirty: &mut bool,
    target: f64,
) -> FrontierOutcome {
    if engine.seek(Duration::from_secs_f64(target)).is_ok() {
        if session.take_resume_after_seek() {
            let _ = engine.resume();
        }
        session.paused_for_buffer.store(false, Ordering::SeqCst);
        *dirty = true;
        FrontierOutcome::State(stream_cache::state_payload(session, None))
    } else {
        session.set_pending_seek(target);
        FrontierOutcome::Quiet
    }
}
