//! Motor de EQ paramétrico (P09a, #427): hasta 10 bandas en cascada de biquads
//! estables; cero bandas es plano.
//!
//! Es el primer eslabón de la cadena fija de ADR-0014 (EQ → ganancia →
//! limiter → salida). El motor no recorta: un boost puede superar fondo de
//! escala y el limiter de `gain` es quien lo atrapa. Tampoco conoce la UI ni
//! persiste ajustes: [`EqSettings`] es el contrato que la capa Tauri construye
//! y [`EqSource`] lo aplica al audio.
//!
//! # Bandas
//!
//! Cada [`EqBand`] lleva tipo, frecuencia, Q y ganancia:
//!
//! - [`BandKind::Peak`]: campana simétrica en frecuencia; `q` es la Q clásica.
//! - [`BandKind::LowShelf`] / [`BandKind::HighShelf`]: estantería; `q` se
//!   interpreta como pendiente (0.1–1, saturada) y la ganancia se aplica a
//!   bajas o altas frecuencias respectivamente.
//! - [`BandKind::LowPass`] / [`BandKind::HighPass`]: corte de 12 dB/octava;
//!   la ganancia de la banda se ignora.
//!
//! Frecuencia y Q fuera de rango se saturan al rango estable, no se rechazan:
//! un ajuste corrupto no puede hacer explotar el hilo de audio. Una banda con
//! frecuencia o Q no finitos, o no positivos, se ignora. En peak y shelf, una
//! ganancia de 0 dB también se ignora.
//!
//! # Plano bit-idéntico
//!
//! [`EqSettings::flat`] no tiene bandas y [`EqSource`] devuelve cada muestra
//! del decodificador sin tocarla. Lo mismo ocurre con bandas peak o shelf a
//! 0 dB: el motor las omite al construir la cascada, así que el plano no paga
//! ni una multiplicación ni introduce redondeo.
//!
//! # Estabilidad
//!
//! Los coeficientes salen del *Audio EQ Cookbook* (RBJ) en `f64`, se
//! normalizan por `a0`, se validan contra el criterio de Jury
//! (`|a2| < 1` y `|a1| < 1 + a2`) y el estado de cada biquad también es `f64`.
//! Cada canal tiene su propia historia: los biquads son recursivos y
//! compartir estado entre canales ensuciaría la imagen estéreo. Un seek
//! exitoso vacía la historia para que el tramo nuevo no herede la cola del
//! anterior.
//!
//! # Uso
//!
//! El adaptador envuelve cualquier [`rodio::Source`] y reenvía canales,
//! frecuencia, duración, span y seek, igual que `GainSource`. Un cambio de
//! ajuste reconstruye el adaptador; no hay estado compartido con el hilo de
//! audio más allá del buffer:
//!
//! ```text
//! use syncify_player::eq::{EqBand, EqSettings, EqSource};
//!
//! let settings = EqSettings::from_bands([EqBand::peak(1_000.0, 1.0, 6.0)]);
//! let filtered = EqSource::new(decoder, settings);
//! ```
//!
//! El ejemplo no se compila como doctest: en Windows el linker de doctests no
//! resuelve la CRT de las dependencias de audio. El comportamiento está
//! cubierto por `tests/eq_test.rs`.

use std::time::Duration;

use rodio::source::SeekError;
use rodio::{Sample, Source};

/// Número máximo de bandas que admite el motor.
pub const MAX_EQ_BANDS: usize = 10;

/// Ganancia máxima por banda, en dB (valor simétrico para atenuar).
pub const MAX_EQ_GAIN_DB: f32 = 24.0;

/// Frecuencia mínima de una banda, en Hz.
const MIN_EQ_FREQUENCY_HZ: f32 = 10.0;
/// Q mínima de una banda.
const MIN_EQ_Q: f32 = 0.1;
/// Q máxima de una banda.
const MAX_EQ_Q: f32 = 10.0;
/// Pendiente mínima de una estantería.
const MIN_SHELF_SLOPE: f32 = 0.1;
/// Pendiente máxima de una estantería (sin rizado con ganancia positiva).
const MAX_SHELF_SLOPE: f32 = 1.0;
/// Q de una banda sin Q declarada: la respuesta máximamente plana.
const DEFAULT_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Frecuencia de una banda sin frecuencia declarada.
const DEFAULT_FREQUENCY_HZ: f32 = 1_000.0;

/// Tipo de banda del EQ.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BandKind {
    /// Campana paramétrica: realza o atenúa alrededor de la frecuencia.
    #[default]
    Peak,
    /// Estantería de bajos: la ganancia se aplica por debajo de la frecuencia.
    LowShelf,
    /// Estantería de agudos: la ganancia se aplica por encima de la frecuencia.
    HighShelf,
    /// Paso bajo de 12 dB/octava; la ganancia se ignora.
    LowPass,
    /// Paso alto de 12 dB/octava; la ganancia se ignora.
    HighPass,
}

impl BandKind {
    /// `true` si la ganancia de la banda participa en la respuesta.
    fn uses_gain(self) -> bool {
        matches!(self, Self::Peak | Self::LowShelf | Self::HighShelf)
    }
}

/// Una banda del EQ.
///
/// Los campos son públicos para que la capa de persistencia construya el
/// ajuste sin pasar por un constructor por tipo. Al diseñar los coeficientes,
/// el motor satura frecuencia, Q y ganancia al rango estable; una banda con
/// valores no finitos o no positivos se ignora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqBand {
    /// Tipo de banda.
    pub kind: BandKind,
    /// Frecuencia central o de corte, en Hz.
    pub frequency_hz: f32,
    /// Q de la campana o pendiente de la estantería.
    pub q: f32,
    /// Ganancia en dB (ignorada en paso bajo y paso alto).
    pub gain_db: f32,
}

impl EqBand {
    /// Banda peak (campana) con frecuencia, Q y ganancia.
    pub fn peak(frequency_hz: f32, q: f32, gain_db: f32) -> Self {
        Self {
            kind: BandKind::Peak,
            frequency_hz,
            q,
            gain_db,
        }
    }

    /// Estantería de bajos.
    pub fn low_shelf(frequency_hz: f32, q: f32, gain_db: f32) -> Self {
        Self {
            kind: BandKind::LowShelf,
            frequency_hz,
            q,
            gain_db,
        }
    }

    /// Estantería de agudos.
    pub fn high_shelf(frequency_hz: f32, q: f32, gain_db: f32) -> Self {
        Self {
            kind: BandKind::HighShelf,
            frequency_hz,
            q,
            gain_db,
        }
    }

    /// Paso bajo de 12 dB/octava; la ganancia se ignora.
    pub fn low_pass(frequency_hz: f32, q: f32) -> Self {
        Self {
            kind: BandKind::LowPass,
            frequency_hz,
            q,
            gain_db: 0.0,
        }
    }

    /// Paso alto de 12 dB/octava; la ganancia se ignora.
    pub fn high_pass(frequency_hz: f32, q: f32) -> Self {
        Self {
            kind: BandKind::HighPass,
            frequency_hz,
            q,
            gain_db: 0.0,
        }
    }

    /// `true` si la banda modifica la señal con los valores declarados.
    ///
    /// No mira la frecuencia de muestreo: decide si merece la pena diseñar el
    /// biquad. Las bandas corruptas o sin efecto se omiten.
    fn is_effective(&self) -> bool {
        if !self.frequency_hz.is_finite()
            || self.frequency_hz <= 0.0
            || !self.q.is_finite()
            || self.q <= 0.0
        {
            return false;
        }
        if self.kind.uses_gain() {
            self.gain_db.is_finite() && self.gain_db != 0.0
        } else {
            true
        }
    }
}

impl Default for EqBand {
    fn default() -> Self {
        Self::peak(DEFAULT_FREQUENCY_HZ, DEFAULT_Q, 0.0)
    }
}

/// Ajuste de EQ: de 0 a [`MAX_EQ_BANDS`] bandas.
///
/// Es `Copy` y no asigna memoria: diez bandas caben en una estructura fija que
/// viaja al hilo de audio sin locks. El ajuste por defecto es plano.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqSettings {
    bands: [EqBand; MAX_EQ_BANDS],
    len: usize,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self::flat()
    }
}

impl EqSettings {
    /// EQ plano: ninguna banda, salida idéntica a la entrada.
    pub fn flat() -> Self {
        Self {
            bands: [EqBand::default(); MAX_EQ_BANDS],
            len: 0,
        }
    }

    /// Construye el ajuste desde un iterable de bandas.
    ///
    /// Las bandas que exceden [`MAX_EQ_BANDS`] se descartan (con aviso): el
    /// motor nunca procesa más de diez, y truncar es preferible a rechazar el
    /// ajuste entero.
    pub fn from_bands(bands: impl IntoIterator<Item = EqBand>) -> Self {
        let mut settings = Self::flat();
        for band in bands {
            if !settings.push_band(band) {
                tracing::warn!("eq: se ignoran bandas por encima de {MAX_EQ_BANDS}");
                break;
            }
        }
        settings
    }

    /// Bandas activas, en el orden de la cascada.
    pub fn bands(&self) -> &[EqBand] {
        &self.bands[..self.len]
    }

    /// Banda en una posición, si existe.
    pub fn band(&self, index: usize) -> Option<EqBand> {
        self.bands().get(index).copied()
    }

    /// Añade una banda al final. Devuelve `false` si ya hay
    /// [`MAX_EQ_BANDS`] y no cabe.
    pub fn push_band(&mut self, band: EqBand) -> bool {
        if self.len == MAX_EQ_BANDS {
            return false;
        }
        self.bands[self.len] = band;
        self.len += 1;
        true
    }

    /// Sustituye una banda existente. Devuelve `false` si el índice no existe.
    pub fn set_band(&mut self, index: usize, band: EqBand) -> bool {
        if index >= self.len {
            return false;
        }
        self.bands[index] = band;
        true
    }

    /// Quita una banda y desplaza las siguientes. Devuelve la banda quitada.
    pub fn remove_band(&mut self, index: usize) -> Option<EqBand> {
        if index >= self.len {
            return None;
        }
        let removed = self.bands[index];
        self.bands.copy_within(index + 1..self.len, index);
        self.len -= 1;
        Some(removed)
    }

    /// Vacía el ajuste: vuelve a plano.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Número de bandas declaradas.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` si no hay bandas declaradas.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `true` si ninguna banda modifica la señal: el motor no filtra.
    ///
    /// No basta con mirar la ganancia: un paso bajo o alto sigue cortando a
    /// 0 dB, y una banda corrupta se ignora.
    pub fn is_flat(&self) -> bool {
        self.bands().iter().all(|band| !band.is_effective())
    }

    /// Añade una banda y devuelve el ajuste (para encadenar constructores).
    ///
    /// Si ya hay [`MAX_EQ_BANDS`], la banda se descarta.
    pub fn with_band(mut self, band: EqBand) -> Self {
        self.push_band(band);
        self
    }

    /// Respuesta de magnitud de la cascada en `frequency_hz`, en dB.
    ///
    /// Es la misma matemática que usa el audio (los coeficientes se diseñan
    /// una sola vez), útil para probar la forma de la curva y para dibujarla.
    /// La frecuencia se satura a `[0, Nyquist]`; por encima de Nyquist no hay
    /// respuesta que medir. Una frecuencia no finita o una frecuencia de
    /// muestreo cero devuelven 0 dB (sin medida).
    pub fn response_db(&self, frequency_hz: f32, sample_rate: u32) -> f32 {
        if sample_rate == 0 || !frequency_hz.is_finite() {
            return 0.0;
        }
        let nyquist = sample_rate as f32 / 2.0;
        let frequency_hz = frequency_hz.clamp(0.0, nyquist);
        let mut db = 0.0_f64;
        for band in self.bands() {
            if let Some(coefficients) = BiquadCoefficients::design(*band, sample_rate) {
                db += coefficients.magnitude_db(frequency_hz, sample_rate);
            }
        }
        db as f32
    }
}

/// Coeficientes de un biquad normalizados por `a0`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BiquadCoefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl BiquadCoefficients {
    /// Diseña el biquad de una banda para una frecuencia de muestreo.
    ///
    /// Devuelve `None` si la banda no tiene efecto o si el diseño no sale
    /// estable; en ambos casos el motor la omite.
    fn design(band: EqBand, sample_rate: u32) -> Option<Self> {
        if !band.is_effective() || sample_rate == 0 {
            return None;
        }

        let rate = sample_rate as f64;
        let max_frequency = 0.45 * rate;
        let frequency = (band.frequency_hz as f64).clamp(
            MIN_EQ_FREQUENCY_HZ as f64,
            max_frequency.max(MIN_EQ_FREQUENCY_HZ as f64),
        );
        let q = (band.q as f64).clamp(MIN_EQ_Q as f64, MAX_EQ_Q as f64);
        let gain_db = if band.kind.uses_gain() {
            (band.gain_db as f64).clamp(-(MAX_EQ_GAIN_DB as f64), MAX_EQ_GAIN_DB as f64)
        } else {
            0.0
        };

        let omega = 2.0 * std::f64::consts::PI * frequency / rate;
        let cos_omega = omega.cos();
        let sin_omega = omega.sin();
        let a = 10.0_f64.powf(gain_db / 40.0);
        let sqrt_a = a.sqrt();

        let (b0, b1, b2, a0, a1, a2) = match band.kind {
            BandKind::Peak => {
                let alpha = sin_omega / (2.0 * q);
                (
                    1.0 + alpha * a,
                    -2.0 * cos_omega,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos_omega,
                    1.0 - alpha / a,
                )
            }
            BandKind::LowShelf => {
                let slope = q.clamp(MIN_SHELF_SLOPE as f64, MAX_SHELF_SLOPE as f64);
                let alpha = sin_omega / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
                (
                    a * ((a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos_omega),
                    a * ((a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha),
                    (a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos_omega),
                    (a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha,
                )
            }
            BandKind::HighShelf => {
                let slope = q.clamp(MIN_SHELF_SLOPE as f64, MAX_SHELF_SLOPE as f64);
                let alpha = sin_omega / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
                (
                    a * ((a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_omega),
                    a * ((a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha),
                    (a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos_omega),
                    (a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha,
                )
            }
            BandKind::LowPass => {
                let alpha = sin_omega / (2.0 * q);
                (
                    (1.0 - cos_omega) / 2.0,
                    1.0 - cos_omega,
                    (1.0 - cos_omega) / 2.0,
                    1.0 + alpha,
                    -2.0 * cos_omega,
                    1.0 - alpha,
                )
            }
            BandKind::HighPass => {
                let alpha = sin_omega / (2.0 * q);
                (
                    (1.0 + cos_omega) / 2.0,
                    -(1.0 + cos_omega),
                    (1.0 + cos_omega) / 2.0,
                    1.0 + alpha,
                    -2.0 * cos_omega,
                    1.0 - alpha,
                )
            }
        };

        if !a0.is_finite() || a0 == 0.0 {
            return None;
        }
        let coefficients = Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        };
        coefficients.is_stable().then_some(coefficients)
    }

    /// Criterio de Jury para un biquad de segundo orden.
    fn is_stable(&self) -> bool {
        self.b0.is_finite()
            && self.b1.is_finite()
            && self.b2.is_finite()
            && self.a1.is_finite()
            && self.a2.is_finite()
            && self.a2.abs() < 1.0
            && self.a1.abs() < 1.0 + self.a2
    }

    /// Magnitud de la respuesta en `frequency_hz`, en dB.
    fn magnitude_db(&self, frequency_hz: f32, sample_rate: u32) -> f64 {
        let omega = 2.0 * std::f64::consts::PI * frequency_hz as f64 / sample_rate as f64;
        let cos_omega = omega.cos();
        let sin_omega = omega.sin();
        let cos_2omega = (2.0 * omega).cos();
        let sin_2omega = (2.0 * omega).sin();

        let numerator_re = self.b0 + self.b1 * cos_omega + self.b2 * cos_2omega;
        let numerator_im = -(self.b1 * sin_omega + self.b2 * sin_2omega);
        let denominator_re = 1.0 + self.a1 * cos_omega + self.a2 * cos_2omega;
        let denominator_im = -(self.a1 * sin_omega + self.a2 * sin_2omega);

        let numerator = (numerator_re * numerator_re + numerator_im * numerator_im).sqrt();
        let denominator =
            (denominator_re * denominator_re + denominator_im * denominator_im).sqrt();
        20.0 * (numerator / denominator).log10()
    }
}

/// Estado de un biquad (forma directa II transpuesta), uno por canal.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct BiquadState {
    z1: f64,
    z2: f64,
}

impl BiquadState {
    fn process(&mut self, coefficients: &BiquadCoefficients, input: f64) -> f64 {
        let output = coefficients.b0 * input + self.z1;
        self.z1 = coefficients.b1 * input - coefficients.a1 * output + self.z2;
        self.z2 = coefficients.b2 * input - coefficients.a2 * output;
        output
    }
}

/// Un biquad de la cascada con su historia independiente por canal.
#[derive(Debug, Clone)]
struct Stage {
    coefficients: BiquadCoefficients,
    states: Vec<BiquadState>,
}

/// Adaptador [`Source`] que aplica un [`EqSettings`] al audio.
///
/// Con un ajuste plano es un pass-through exacto (no toca las muestras). Con
/// bandas, cada canal pasa por la cascada en el orden declarado. Reenvía
/// canales, frecuencia de muestreo, duración, span y seek; un seek exitoso
/// vacía la historia de los biquads.
#[derive(Debug, Clone)]
pub struct EqSource<S> {
    input: S,
    settings: EqSettings,
    stages: Vec<Stage>,
    channels: usize,
    channel: usize,
}

impl<S: Source> EqSource<S> {
    /// Envuelve una fuente y diseña la cascada para su formato.
    ///
    /// Las bandas sin efecto o inestables se omiten. Un cambio de ajuste
    /// exige construir un adaptador nuevo.
    pub fn new(input: S, settings: EqSettings) -> Self {
        let channels = input.channels().get() as usize;
        let sample_rate = input.sample_rate().get();
        let mut stages = Vec::with_capacity(settings.len());
        for band in settings.bands() {
            if let Some(coefficients) = BiquadCoefficients::design(*band, sample_rate) {
                stages.push(Stage {
                    coefficients,
                    states: vec![BiquadState::default(); channels],
                });
            }
        }
        Self {
            input,
            settings,
            stages,
            channels,
            channel: 0,
        }
    }

    /// Ajuste con el que se construyó el adaptador (incluye las bandas que se
    /// omitieron por no tener efecto o por no ser estables).
    pub fn settings(&self) -> &EqSettings {
        &self.settings
    }

    /// Fuente original.
    pub fn inner(&self) -> &S {
        &self.input
    }

    /// `true` si la cascada está vacía y las muestras pasan intactas.
    pub fn is_bypassing(&self) -> bool {
        self.stages.is_empty()
    }

    fn reset_state(&mut self) {
        for stage in &mut self.stages {
            stage.states.fill(BiquadState::default());
        }
        self.channel = 0;
    }
}

impl<S: Source> Iterator for EqSource<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.input.next()?;
        if self.stages.is_empty() {
            return Some(sample);
        }
        let channel = self.channel;
        self.channel += 1;
        if self.channel == self.channels {
            self.channel = 0;
        }
        let mut value = sample as f64;
        for stage in &mut self.stages {
            value = stage.states[channel].process(&stage.coefficients, value);
        }
        Some(value as f32)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<S: Source> Source for EqSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.input.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;
        self.reset_state();
        Ok(())
    }
}
