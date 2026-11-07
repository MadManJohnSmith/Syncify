//! Motor de reproducción local nativo de Syncify.
//!
//! Fase 1 del plan de reproducción: sustituye la ruta `<audio>` + custom scheme
//! `syncify-media://` (rota: el motor media de WebKitGTK falla con `FormatError`
//! al consumirla — ver `docs/auditoria/playback-fase0-evidencia-2026-10-17.md`)
//! por decodificación y salida en Rust puro:
//!
//! - **Salida**: `cpal` a través de `rodio` (ALSA/PipeWire en Linux, WASAPI en Windows).
//! - **Decodificación**: `symphonia` a través de `rodio` (FLAC 16/24-bit, MP3, M4A/AAC,
//!   WAV, Ogg Vorbis).
//! - Sin GStreamer, sin WebKit y sin dependencias externas en el camino de audio.
//!
//! El motor es síncrono y no conoce la cola, el historial ni el crédito de escucha
//! (eso vive en la capa Tauri/UI). Aquí solo hay: cargar → reproducir → pausar →
//! reanudar → buscar → parar, con estado y posición legibles en cualquier momento.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{Decoder, Player};

pub mod sim;

/// Errores del motor. Texto legible en inglés (la UI los sanea antes de mostrarlos).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// No hay dispositivo de salida por defecto (o `cpal` no pudo abrirlo).
    #[error("no default audio output available: {0}")]
    OutputUnavailable(String),
    /// El archivo no existe o no se puede leer. La ruta se registra en el log,
    /// nunca viaja en el mensaje (la UI lo muestra tal cual).
    #[error("audio file not found")]
    FileNotFound,
    /// `symphonia`/`rodio` no pudieron decodificar el archivo.
    #[error("could not decode audio file: {0}")]
    Decode(String),
    /// La fuente cargada no soporta búsqueda.
    #[error("seek not supported by the loaded source")]
    SeekNotSupported,
    /// No hay pista cargada sobre la que actuar.
    #[error("no track is loaded")]
    NothingLoaded,
}

/// Información de la pista cargada.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedTrack {
    /// Ruta tal y como la entregó el llamador (no canonicalizada para mostrarla).
    pub path: PathBuf,
    /// Duración reportada por el decodificador, si el contenedor la conoce.
    pub duration: Option<Duration>,
}

/// Estado observable del motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Sin pista cargada o pista detenida.
    Stopped,
    /// Reproduciendo ahora mismo.
    Playing,
    /// Pausado (la pista sigue cargada en su posición).
    Paused,
}

/// Contrato de la salida de audio que el motor gobierna.
///
/// Producción: `RodioBackend` (rodio + cpal sobre el dispositivo por defecto).
/// Las pruebas y la tooling pueden inyectar un backend simulado con
/// [`PlayerEngine::with_backend`] (véase [`sim::SimulatedBackend`]) para
/// caracterizar el motor sin dispositivo de audio ni red.
///
/// Todas las operaciones son `&self` con mutación interior, igual que el
/// `Player` de rodio que envuelve la implementación de producción.
pub trait PlaybackBackend: Send {
    /// Encola una fuente ya decodificada, sustituyendo a la anterior.
    fn append(&self, source: EngineDecoder);
    fn play(&self);
    fn pause(&self);
    fn is_paused(&self) -> bool;
    /// Vacía la cola; la pista deja de estar cargada para el motor.
    fn stop(&self);
    fn set_volume(&self, volume: f32);
    fn set_speed(&self, rate: f32);
    /// Busca una posición absoluta. Acotar por el final de la pista es
    /// responsabilidad del motor; rodio además satura por su cuenta.
    fn try_seek(&self, pos: Duration) -> Result<(), rodio::source::SeekError>;
    fn get_pos(&self) -> Duration;
    /// `true` cuando no queda ninguna fuente en cola (parada o fin natural).
    fn empty(&self) -> bool;
}

/// Fuente decodificada que el motor entrega al backend (symphonia vía rodio).
pub type EngineDecoder = Decoder<BufReader<std::fs::File>>;

/// Salida de producción: rodio + cpal sobre el sink por defecto.
struct RodioBackend {
    /// Se conserva para mantener vivo el stream de salida (`cpal`).
    _sink: MixerDeviceSink,
    player: Player,
}

impl RodioBackend {
    /// Abre el dispositivo de salida por defecto una sola vez.
    fn new() -> Result<Self, EngineError> {
        let sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| EngineError::OutputUnavailable(e.to_string()))?;
        let player = Player::connect_new(sink.mixer());
        Ok(Self {
            _sink: sink,
            player,
        })
    }
}

impl PlaybackBackend for RodioBackend {
    fn append(&self, source: EngineDecoder) {
        self.player.append(source);
    }

    fn play(&self) {
        self.player.play();
    }

    fn pause(&self) {
        self.player.pause();
    }

    fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    fn stop(&self) {
        self.player.stop();
    }

    fn set_volume(&self, volume: f32) {
        self.player.set_volume(volume);
    }

    fn set_speed(&self, rate: f32) {
        self.player.set_speed(rate);
    }

    fn try_seek(&self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.player.try_seek(pos)
    }

    fn get_pos(&self) -> Duration {
        self.player.get_pos()
    }

    fn empty(&self) -> bool {
        self.player.empty()
    }
}

/// Motor de reproducción. Mantiene un único stream de salida abierto durante
/// toda la sesión (abrirlo por pista produce chasquidos y desperdicia el buffer).
pub struct PlayerEngine {
    backend: Box<dyn PlaybackBackend>,
    loaded: Option<LoadedTrack>,
    volume: f32,
    rate: f32,
}

impl PlayerEngine {
    /// Abre el dispositivo de salida por defecto una sola vez.
    pub fn new() -> Result<Self, EngineError> {
        Ok(Self::with_backend(Box::new(RodioBackend::new()?)))
    }

    /// Motor sobre un backend inyectado: pruebas y tooling sin dispositivo de
    /// audio. La producción usa [`PlayerEngine::new`].
    pub fn with_backend(backend: Box<dyn PlaybackBackend>) -> Self {
        Self {
            backend,
            loaded: None,
            volume: 1.0,
            rate: 1.0,
        }
    }

    /// Carga un archivo local sustituyendo cualquier pista anterior (cola
    /// vaciada, posición a cero). Con `autoplay = false` la fuente queda
    /// cargada y **pausada** en la posición 0 (restauración de sesión).
    pub fn load(&mut self, path: &Path, autoplay: bool) -> Result<LoadedTrack, EngineError> {
        if !path.is_file() {
            tracing::warn!("player: file not found: {}", path.display());
            return Err(EngineError::FileNotFound);
        }

        let file = std::fs::File::open(path).map_err(|e| {
            tracing::warn!("player: could not open {}: {e}", path.display());
            EngineError::FileNotFound
        })?;
        // El impl `TryFrom<File>` fija `byte_len` desde los metadatos y activa el
        // seek: imprescindible para duración fiable (MP3/Vorbis) y para contenedores
        // como MP4 cuyo `moov` puede estar al final del archivo.
        let decoder = Decoder::try_from(file).map_err(|e| EngineError::Decode(e.to_string()))?;
        let duration = rodio::Source::total_duration(&decoder);

        // Sustituir la pista anterior: `stop()` vacía la cola del reproductor.
        // La pausa se fija ANTES del `append` para que la fuente nazca en pausa;
        // el volumen y el ritmo se reaplican porque `append` envuelve cada
        // fuente con sus propios factores.
        self.backend.stop();
        if !autoplay {
            self.backend.pause();
        }
        self.backend.append(decoder);
        self.backend.set_volume(self.volume);
        self.backend.set_speed(self.rate);
        // La posición latcheada por rodio (cada ~5 ms) puede devolver todavía el
        // valor de la pista anterior tras el swap, porque la fuente nueva aún no
        // ha corrido su reloj. Un seek a 0 la fija de forma determinista antes de
        // devolver (bloquea hasta el feedback del hilo de audio). El error de una
        // fuente sin soporte de seek se ignora: es un no-op.
        let _ = self.backend.try_seek(Duration::ZERO);
        if autoplay {
            self.backend.play();
        }

        let info = LoadedTrack {
            path: path.to_path_buf(),
            duration,
        };
        self.loaded = Some(info.clone());
        tracing::info!(
            "player: {} {} ({:?})",
            if autoplay { "playing" } else { "loaded paused" },
            info.path.display(),
            info.duration
        );
        Ok(info)
    }

    /// Carga y reproduce (equivale a `load(path, true)`).
    pub fn load_and_play(&mut self, path: &Path) -> Result<LoadedTrack, EngineError> {
        self.load(path, true)
    }

    /// Pausa si está reproduciendo (no-op en cualquier otro estado).
    pub fn pause(&self) {
        if !self.backend.is_paused() {
            self.backend.pause();
        }
    }

    /// Reanuda si hay una pista pausada.
    pub fn resume(&self) -> Result<(), EngineError> {
        if self.loaded.is_none() {
            return Err(EngineError::NothingLoaded);
        }
        if self.backend.is_paused() {
            self.backend.play();
        }
        Ok(())
    }

    /// Alterna pausa/reanudación. Sin pista cargada es un no-op.
    pub fn toggle(&self) {
        if self.loaded.is_none() {
            return;
        }
        if self.backend.is_paused() {
            self.backend.play();
        } else {
            self.backend.pause();
        }
    }

    /// Detiene y descarga la pista actual. El stream de salida sigue abierto.
    pub fn stop(&mut self) {
        self.backend.stop();
        self.loaded = None;
    }

    /// Busca una posición absoluta. Fuera de rango se acota a `[0, duración]`
    /// (rodio ya acota por el final; aquí además se rechaza sin pista cargada).
    pub fn seek(&self, position: Duration) -> Result<(), EngineError> {
        let Some(loaded) = &self.loaded else {
            return Err(EngineError::NothingLoaded);
        };
        let clamped = match loaded.duration {
            Some(total) => position.min(total),
            None => position,
        };
        self.backend.try_seek(clamped).map_err(|e| match e {
            rodio::source::SeekError::NotSupported { .. } => EngineError::SeekNotSupported,
            other => EngineError::Decode(other.to_string()),
        })
    }

    /// Volumen lineal `[0.0, 1.0+]` aplicado por el reproductor.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.max(0.0);
        self.backend.set_volume(self.volume);
    }

    /// Volumen actual.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Ritmo de reproducción (`1.0` = normal). rodio varía el tono con la
    /// velocidad (no hay time-stretch); el valor se reaplica en cada carga.
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = if rate.is_finite() && rate > 0.0 {
            rate
        } else {
            1.0
        };
        self.backend.set_speed(self.rate);
    }

    /// Ritmo actual.
    pub fn rate(&self) -> f32 {
        self.rate
    }

    /// Estado observable. Una pista que llegó al final (cola vacía) se reporta
    /// como `Stopped` aunque `loaded` siga presente hasta el siguiente `stop()`.
    pub fn state(&self) -> EngineState {
        if self.loaded.is_none() || self.backend.empty() {
            EngineState::Stopped
        } else if self.backend.is_paused() {
            EngineState::Paused
        } else {
            EngineState::Playing
        }
    }

    /// Posición actual de reproducción (se congela al llegar al final).
    pub fn position(&self) -> Duration {
        self.backend.get_pos()
    }

    /// Duración de la pista cargada, si el contenedor la declara.
    pub fn duration(&self) -> Option<Duration> {
        self.loaded.as_ref().and_then(|l| l.duration)
    }

    /// Pista cargada actualmente.
    pub fn loaded(&self) -> Option<&LoadedTrack> {
        self.loaded.as_ref()
    }

    /// `true` cuando la pista terminó de reproducirse (la cola quedó vacía por
    /// final natural, no por `stop()`; `stop()` además limpia `loaded`).
    pub fn track_ended(&self) -> bool {
        self.loaded.is_some() && self.backend.empty()
    }
}

impl std::fmt::Debug for PlayerEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerEngine")
            .field("state", &self.state())
            .field("position", &self.position())
            .field("loaded", &self.loaded)
            .field("volume", &self.volume)
            .finish()
    }
}
