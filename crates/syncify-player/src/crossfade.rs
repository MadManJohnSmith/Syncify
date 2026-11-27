//! Motor de crossfade entre pistas (P06a, #407): mezcla la cola de la pista
//! saliente con la cabecera de la entrante durante un ajuste de 0 a 12 s.
//!
//! # Curva
//!
//! La mezcla es de **potencia constante** (*equal-power*): la saliente pesa
//! `cos(p·π/2)` y la entrante `sin(p·π/2)`, con `p` el progreso `0..=1` de la
//! mezcla. Una rampa lineal de amplitudes abriría un hueco de hasta −3 dB en el
//! centro con pistas no correlacionadas; `cos² + sin² = 1` mantiene la potencia
//! percibida. [`mix_gains`] es la única definición de la curva.
//!
//! # Limiter
//!
//! Sumar dos pistas puede superar 1.0 (hasta `√2 ≈ 1.414` en el centro).
//! [`PeakLimiter`] es un limitador de pico *feed-forward* con ataque instantáneo
//! y release exponencial de 100 ms, con la misma ganancia para todos los canales
//! de un frame (no altera la imagen estéreo). El techo es [`LIMITER_CEILING`]
//! (≈ −0.18 dBFS). El ataque instantáneo garantiza que ninguna muestra
//! mezclada lo supere; no hay lookahead y por tanto no añade latencia.
//!
//! El limiter solo procesa la región mezclada: al terminar la mezcla la pista
//! entrante sigue intacta, sin atenuación permanente. La ganancia residual del
//! último frame (cero si el pico no supera el techo; ≤ 0.5 dB con ambas pistas
//! a escala completa) se abandona en ese límite.
//!
//! # 0 s: gapless
//!
//! Con [`CrossfadeConfig::OFF`] el adaptador es una concatenación exacta de la
//! saliente y la entrante, sin atenuación ni limiter. La mezcla tampoco existe
//! con un ajuste mayor si el tramo redondea a cero frames.
//!
//! # Transporte
//!
//! El crossfade solo acompaña al fin natural de una pista:
//! [`CrossfadeConfig::applies_to`] devuelve `false` para seek, pausa y parada
//! manual, que siguen siendo gapless y no disparan ninguna mezcla.
//! [`CrossfadeSource::try_seek`] no soporta seek: un seek reconstruye el tramo
//! (gapless) en vez de mezclarlo.
//!
//! # Contrato de integración
//!
//! El llamador construye el adaptador `duration` antes de que la pista saliente
//! termine, con la saliente aún por consumir y la entrante desde su posición 0.
//! Si a la saliente le queda más audio que la mezcla, el exceso se descarta (es
//! la cola que el crossfade sustituye). Ambas fuentes deben compartir canales y
//! frecuencia de muestreo; si no, [`CrossfadeSource::new`] devuelve
//! [`CrossfadeError::FormatMismatch`] y el integrador debe caer a gapless.

use std::time::Duration;

use rodio::source::SeekError;
use rodio::{Sample, Source};

/// Ajuste máximo de crossfade (12 s).
pub const MAX_CROSSFADE_DURATION: Duration = Duration::from_secs(12);

/// Techo del limitador en escala lineal (≈ −0.18 dBFS).
pub const LIMITER_CEILING: f32 = 0.98;

/// Release del limitador: tiempo en el que la ganancia vuelve a la unidad
/// tras un pico (constante de tiempo exponencial).
pub const LIMITER_RELEASE: Duration = Duration::from_millis(100);

/// Ajuste de crossfade ya validado: de 0 a 12 s.
///
/// El valor llega desde la capa de preferencias (#414); aquí se satura y nunca
/// se rechaza, para que un ajuste corrupto no rompa la reproducción. Cero
/// significa gapless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossfadeConfig {
    duration: Duration,
}

impl CrossfadeConfig {
    /// Crossfade desactivado: comportamiento gapless.
    pub const OFF: Self = Self {
        duration: Duration::ZERO,
    };

    /// Ajuste máximo admitido.
    pub const MAX: Self = Self {
        duration: MAX_CROSSFADE_DURATION,
    };

    /// Valida una duración, saturándola en [`MAX_CROSSFADE_DURATION`].
    pub fn new(duration: Duration) -> Self {
        Self {
            duration: duration.min(MAX_CROSSFADE_DURATION),
        }
    }

    /// Construye el ajuste desde segundos. Valores negativos, `NaN` e infinito
    /// se tratan como desactivado; por encima de 12 s se satura.
    pub fn from_secs(secs: f32) -> Self {
        if !secs.is_finite() || secs <= 0.0 {
            return Self::OFF;
        }
        Self::new(Duration::from_secs_f32(
            secs.min(MAX_CROSSFADE_DURATION.as_secs_f32()),
        ))
    }

    /// Duración de la mezcla (`0` = gapless).
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// `true` si el ajuste pide mezcla (duración mayor que cero).
    pub fn is_enabled(&self) -> bool {
        !self.duration.is_zero()
    }

    /// Decide si un cierre de pista merece crossfade.
    ///
    /// Solo el fin natural de la pista lo merece: seek, pausa y parada manual
    /// son cortes de transporte y deben seguir siendo gapless (sin mezcla).
    pub fn applies_to(&self, transition: Transition) -> bool {
        transition == Transition::NaturalEnd && self.is_enabled()
    }
}

/// Causa por la que una pista deja de sonar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// La pista llegó a su final y la siguiente debe entrar.
    NaturalEnd,
    /// Salto de posición pedido por el usuario o la sesión.
    Seek,
    /// Pausa del transporte.
    Pause,
    /// Parada manual.
    Stop,
}

/// Ganancias `(saliente, entrante)` de la curva de potencia constante para un
/// progreso `0..=1` (fuera de rango se satura).
///
/// En `0` la saliente suena a unidad y la entrante a cero; en `1`, al revés.
#[inline]
pub fn mix_gains(progress: f32) -> (f32, f32) {
    let angle = progress.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    // `cos(π/2)` en f32 cae unas ulps por debajo de cero; la curva es no
    // negativa por contrato.
    (angle.cos().clamp(0.0, 1.0), angle.sin().clamp(0.0, 1.0))
}

/// Limitador de pico por frame, sin lookahead.
///
/// Detecta el pico absoluto del frame (todos los canales), calcula la ganancia
/// que lo deja en [`LIMITER_CEILING`] y la aplica por igual a cada canal. El
/// ataque es instantáneo (la ganancia baja en el mismo frame del pico); el
/// release es exponencial con constante [`LIMITER_RELEASE`], así que la
/// ganancia nunca sube por encima de la necesaria y la salida jamás supera el
/// techo.
#[derive(Debug, Clone)]
pub struct PeakLimiter {
    ceiling: f32,
    release: f32,
    gain: f32,
}

impl PeakLimiter {
    /// Crea el limitador para una frecuencia de muestreo (la del frame).
    pub fn new(sample_rate: u32) -> Self {
        let release_secs = LIMITER_RELEASE.as_secs_f32().max(f32::MIN_POSITIVE);
        let frames_per_release = sample_rate.max(1) as f32 * release_secs;
        Self {
            ceiling: LIMITER_CEILING,
            release: (-1.0 / frames_per_release).exp(),
            gain: 1.0,
        }
    }

    /// Limita el frame *in place*.
    pub fn process_frame(&mut self, frame: &mut [Sample]) {
        let peak = frame
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        let target = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        // Ataque instantáneo: la ganancia de este frame nunca supera la que
        // exige su pico; el release la acerca a la unidad cuando sobra margen.
        let released = self.release * self.gain + (1.0 - self.release) * target;
        self.gain = released.min(target);
        for sample in frame {
            *sample *= self.gain;
        }
    }

    /// Ganancia aplicada al último frame (1.0 = sin reducción).
    pub fn gain(&self) -> f32 {
        self.gain
    }
}

/// Error al construir el adaptador de crossfade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CrossfadeError {
    /// Las dos fuentes no comparten formato; mezclarlas exigiría remuestrear.
    #[error(
        "crossfade sources must share format: outgoing {outgoing_channels} ch @ \
         {outgoing_rate} Hz, incoming {incoming_channels} ch @ {incoming_rate} Hz"
    )]
    FormatMismatch {
        outgoing_channels: u16,
        outgoing_rate: u32,
        incoming_channels: u16,
        incoming_rate: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Ajuste desactivado: se emite la saliente completa, sin tocar.
    Outgoing,
    /// Región de mezcla; `frame` cuenta los frames ya mezclados.
    Blending,
    /// Mezcla terminada (o nunca empezada): solo queda la entrante.
    Incoming,
    /// Ambas fuentes agotadas.
    Done,
}

/// Adaptador [`Source`] que une dos pistas con un crossfade de potencia
/// constante y limiter.
///
/// Emite la cola de `outgoing` mezclada con la cabecera de `incoming` durante
/// el ajuste y después la entrante sola. Con [`CrossfadeConfig::OFF`] emite
/// `outgoing` y después `incoming` sin ninguna modificación (gapless).
///
/// El adaptador se crea al acercarse el final de la saliente (véase el contrato
/// de integración del módulo); no decide *cuándo* empieza la mezcla.
#[derive(Debug, Clone)]
pub struct CrossfadeSource<A: Source, B: Source> {
    outgoing: A,
    incoming: B,
    /// Canales del stream de salida (los de `outgoing`, validados en `new`).
    channels: usize,
    /// Frames de mezcla; `0` = gapless.
    fade_frames: u64,
    /// Frames de mezcla ya emitidos.
    frame: u64,
    phase: Phase,
    limiter: PeakLimiter,
    outgoing_frame: Vec<Sample>,
    incoming_frame: Vec<Sample>,
    /// Frame ya mezclado pendiente de entregar muestra a muestra.
    pending: Vec<Sample>,
    pending_pos: usize,
}

impl<A: Source, B: Source> CrossfadeSource<A, B> {
    /// Crea el adaptador.
    ///
    /// Falla si las fuentes no comparten canales y frecuencia de muestreo: el
    /// llamador debe reproducir el par en gapless en ese caso.
    pub fn new(outgoing: A, incoming: B, config: CrossfadeConfig) -> Result<Self, CrossfadeError> {
        if outgoing.channels() != incoming.channels()
            || outgoing.sample_rate() != incoming.sample_rate()
        {
            return Err(CrossfadeError::FormatMismatch {
                outgoing_channels: outgoing.channels().get(),
                outgoing_rate: outgoing.sample_rate().get(),
                incoming_channels: incoming.channels().get(),
                incoming_rate: incoming.sample_rate().get(),
            });
        }
        let channels = outgoing.channels().get() as usize;
        let sample_rate = outgoing.sample_rate().get();
        let rate = sample_rate as f64;
        // Redondeo al frame más cercano: un ajuste menor que medio frame no
        // puede mezclar y degrada a gapless.
        let fade_frames = (rate * config.duration().as_secs_f64()).round() as u64;
        let phase = if fade_frames == 0 {
            Phase::Outgoing
        } else {
            Phase::Blending
        };
        Ok(Self {
            outgoing,
            incoming,
            channels,
            fade_frames,
            frame: 0,
            phase,
            limiter: PeakLimiter::new(sample_rate),
            outgoing_frame: Vec::with_capacity(channels),
            incoming_frame: Vec::with_capacity(channels),
            pending: Vec::with_capacity(channels),
            pending_pos: 0,
        })
    }

    /// Ajuste efectivo (0 s tras redondear a cero frames = gapless).
    pub fn fade_frames(&self) -> u64 {
        self.fade_frames
    }

    fn next_frame(&mut self) -> Option<()> {
        loop {
            match self.phase {
                Phase::Outgoing => {
                    if !read_frame(&mut self.outgoing, self.channels, &mut self.outgoing_frame) {
                        self.phase = Phase::Incoming;
                        continue;
                    }
                    std::mem::swap(&mut self.pending, &mut self.outgoing_frame);
                    return Some(());
                }
                Phase::Blending => {
                    if self.frame >= self.fade_frames {
                        self.phase = Phase::Incoming;
                        continue;
                    }
                    let has_outgoing =
                        read_frame(&mut self.outgoing, self.channels, &mut self.outgoing_frame);
                    let has_incoming =
                        read_frame(&mut self.incoming, self.channels, &mut self.incoming_frame);
                    if !has_outgoing && !has_incoming {
                        self.phase = Phase::Done;
                        return None;
                    }
                    let progress = (self.frame as f32 + 0.5) / self.fade_frames as f32;
                    let (out_gain, in_gain) = mix_gains(progress);
                    self.pending.resize(self.channels, 0.0);
                    for (index, sample) in self.pending.iter_mut().enumerate() {
                        let outgoing = if has_outgoing {
                            self.outgoing_frame[index] * out_gain
                        } else {
                            0.0
                        };
                        let incoming = if has_incoming {
                            self.incoming_frame[index] * in_gain
                        } else {
                            0.0
                        };
                        *sample = outgoing + incoming;
                    }
                    self.limiter.process_frame(&mut self.pending);
                    self.frame += 1;
                    return Some(());
                }
                Phase::Incoming => {
                    if !read_frame(&mut self.incoming, self.channels, &mut self.incoming_frame) {
                        self.phase = Phase::Done;
                        return None;
                    }
                    std::mem::swap(&mut self.pending, &mut self.incoming_frame);
                    return Some(());
                }
                Phase::Done => return None,
            }
        }
    }
}

/// Lee un frame completo de `source`; `false` si se agotó (un frame parcial se
/// descarta: no se puede mezclar media trama).
fn read_frame<S: Source>(source: &mut S, channels: usize, frame: &mut Vec<Sample>) -> bool {
    frame.clear();
    for _ in 0..channels {
        match source.next() {
            Some(sample) => frame.push(sample),
            None => return false,
        }
    }
    true
}

impl<A: Source, B: Source> Iterator for CrossfadeSource<A, B> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(&sample) = self.pending.get(self.pending_pos) {
            self.pending_pos += 1;
            return Some(sample);
        }
        self.pending.clear();
        self.pending_pos = 0;
        self.next_frame()?;
        self.pending_pos = 1;
        self.pending.first().copied()
    }
}

impl<A: Source, B: Source> Source for CrossfadeSource<A, B> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.outgoing.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.outgoing.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        let incoming = self.incoming.total_duration();
        if self.fade_frames > 0 {
            // La saliente se trunca al tramo de mezcla; la entrante define el resto.
            return incoming;
        }
        self.outgoing.total_duration()?.checked_add(incoming?)
    }

    /// El seek no mezcla: el transporte reconstruye el tramo en gapless. Un
    /// adaptador a mitad de crossfade no puede reposicionar la saliente ya
    /// descartada ni reconstruir la mezcla.
    fn try_seek(&mut self, _pos: Duration) -> Result<(), SeekError> {
        Err(SeekError::NotSupported {
            underlying_source: "syncify_player::crossfade::CrossfadeSource",
        })
    }
}
