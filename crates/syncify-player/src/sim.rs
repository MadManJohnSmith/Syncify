//! Backend de salida simulado para caracterizar el motor sin dispositivo de
//! audio (la promesa de ADR-0011: «los tests del motor usan audio sintético»).
//!
//! El reloj es virtual: la posición solo avanza cuando el test llama
//! [`VirtualClock::advance`], de modo que los tests no duermen ni dependen de
//! temporización real. El backend replica la semántica observable del `Player`
//! de rodio que usa la producción:
//!
//! - pausar congela la posición; la velocidad multiplica el avance;
//! - el seek satura en el final de la pista (y es un no-op `Ok` sin fuente);
//! - llegar al final «vacía la cola» (`empty`) sin descargar la pista, que es
//!   como el motor distingue el fin natural (`track_ended`) del `stop()`;
//! - la posición se congela en la duración al llegar al final.
//!
//! No decodifica en streaming ni produce audio: la fuente entregada por el
//! motor se consume para leer su duración declarada y nada más.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::Source;

use crate::{EngineDecoder, PlaybackBackend};

/// Reloj virtual compartido entre el backend simulado y el test.
///
/// Clonar comparte la misma hora; el test avanza el tiempo y el backend lo
/// consulta en cada operación.
#[derive(Clone, Default)]
pub struct VirtualClock(Arc<Mutex<f64>>);

impl VirtualClock {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(0.0)))
    }

    /// Avanza el tiempo virtual.
    pub fn advance(&self, delta: Duration) {
        if let Ok(mut now) = self.0.lock() {
            *now += delta.as_secs_f64();
        }
    }

    fn now(&self) -> f64 {
        self.0.lock().map(|now| *now).unwrap_or(0.0)
    }
}

/// Estado del backend simulado.
struct SimState {
    /// Posición latcheada (segundos de pista, ya con la velocidad aplicada).
    pos_secs: f64,
    /// Hora virtual en la que empezó el tramo en reproducción actual.
    anchor_secs: f64,
    paused: bool,
    has_source: bool,
    /// Duración declarada por la fuente (`None` = sin duración conocida).
    duration: Option<f64>,
    speed: f32,
    /// Registro de operaciones de transporte, para congelar la coreografía del
    /// motor en los tests (`log()`).
    log: Vec<String>,
}

impl SimState {
    /// Incorpora el avance virtual pendiente a la posición latcheada.
    fn fold(&mut self, now: f64) {
        if self.has_source && !self.paused {
            self.pos_secs += (now - self.anchor_secs) * self.speed as f64;
            if let Some(d) = self.duration {
                self.pos_secs = self.pos_secs.min(d);
            }
        }
        self.anchor_secs = now;
    }

    fn ended(&self) -> bool {
        self.duration.is_some_and(|d| self.pos_secs >= d)
    }
}

/// Backend de salida simulado (pruebas y tooling; la producción usa rodio).
pub struct SimulatedBackend {
    clock: VirtualClock,
    state: Arc<Mutex<SimState>>,
}

impl SimulatedBackend {
    pub fn new(clock: VirtualClock) -> Self {
        Self {
            clock,
            state: Arc::new(Mutex::new(SimState {
                pos_secs: 0.0,
                anchor_secs: 0.0,
                paused: false,
                has_source: false,
                duration: None,
                speed: 1.0,
                log: Vec::new(),
            })),
        }
    }

    /// Asa compartido sobre el estado del backend: se toma ANTES de mover el
    /// backend dentro del motor y permite al test leer el registro de
    /// operaciones y la velocidad aplicada.
    pub fn handle(&self) -> SimHandle {
        SimHandle {
            state: Arc::clone(&self.state),
        }
    }
}

/// Vista de solo lectura sobre el backend simulado, válida después de moverlo
/// al motor (`PlayerEngine::with_backend`).
#[derive(Clone)]
pub struct SimHandle {
    state: Arc<Mutex<SimState>>,
}

impl SimHandle {
    /// Operaciones de transporte registradas hasta ahora (sin los sondeos de
    /// estado `get_pos`/`empty`, que son lecturas).
    pub fn log(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|state| state.log.clone())
            .unwrap_or_default()
    }

    /// Última velocidad aplicada al backend.
    pub fn speed(&self) -> f32 {
        self.state.lock().map(|state| state.speed).unwrap_or(1.0)
    }
}

impl PlaybackBackend for SimulatedBackend {
    fn append(&self, source: EngineDecoder) {
        let duration = Source::total_duration(&source).map(|d| d.as_secs_f64());
        drop(source);
        let now = self.clock.now();
        if let Ok(mut state) = self.state.lock() {
            state.fold(now);
            state.has_source = true;
            state.pos_secs = 0.0;
            state.duration = duration;
            state.log.push("append".to_string());
        }
    }

    fn play(&self) {
        let now = self.clock.now();
        if let Ok(mut state) = self.state.lock() {
            state.fold(now);
            state.paused = false;
            state.log.push("play".to_string());
        }
    }

    fn pause(&self) {
        let now = self.clock.now();
        if let Ok(mut state) = self.state.lock() {
            state.fold(now);
            state.paused = true;
            state.log.push("pause".to_string());
        }
    }

    fn is_paused(&self) -> bool {
        self.state.lock().map(|state| state.paused).unwrap_or(false)
    }

    fn stop(&self) {
        let now = self.clock.now();
        if let Ok(mut state) = self.state.lock() {
            state.fold(now);
            state.has_source = false;
            state.log.push("stop".to_string());
        }
    }

    fn set_volume(&self, volume: f32) {
        if let Ok(mut state) = self.state.lock() {
            state.log.push(format!("set_volume({volume})"));
        }
    }

    fn set_speed(&self, rate: f32) {
        let now = self.clock.now();
        if let Ok(mut state) = self.state.lock() {
            state.fold(now);
            state.speed = rate;
            state.log.push(format!("set_speed({rate})"));
        }
    }

    fn try_seek(&self, pos: Duration) -> Result<(), SeekError> {
        let now = self.clock.now();
        if let Ok(mut state) = self.state.lock() {
            if state.has_source {
                state.fold(now);
                let target = pos.as_secs_f64();
                state.pos_secs = match state.duration {
                    Some(d) => target.min(d),
                    None => target,
                };
            }
            // Sin fuente en cola rodio no hace nada y devuelve Ok.
            state.log.push(format!("try_seek({})", pos.as_secs_f64()));
        }
        Ok(())
    }

    fn get_pos(&self) -> Duration {
        let now = self.clock.now();
        let pos = self
            .state
            .lock()
            .map(|mut state| {
                state.fold(now);
                state.pos_secs
            })
            .unwrap_or(0.0);
        Duration::from_secs_f64(pos.max(0.0))
    }

    fn empty(&self) -> bool {
        let now = self.clock.now();
        self.state
            .lock()
            .map(|mut state| {
                state.fold(now);
                !state.has_source || state.ended()
            })
            .unwrap_or(true)
    }
}
