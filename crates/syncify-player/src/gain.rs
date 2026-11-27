//! Motor de ganancia de reproducción (ReplayGain) con limiter de pico.
//!
//! La ganancia de reproducción es una decisión distinta del etiquetado de
//! descarga (ADR-0011): el etiquetado escribe `replaygain_*` en `tracks`, y este
//! módulo decide cuánto de ese valor se aplica al sonar. Vive en el motor
//! nativo, nunca en la webview, y no persiste ajustes ni conoce la UI.
//!
//! # Convención ReplayGain
//!
//! - Las ganancias son dB con signo relativos a una sonoridad de referencia.
//!   El etiquetado de este repo es ReplayGain 2.0, cuya referencia es
//!   [`REPLAYGAIN_REFERENCE_LUFS`] (-18 LUFS). La forma textual es la estándar
//!   de las columnas de `tracks`: `"+3.00 dB"`, `"-6.50 dB"` (también se acepta
//!   sin sufijo).
//! - Los peaks son ratios lineales (`"0.988553"`), no dB.
//! - El target de reproducción es propio: [`GainSettings::target_lufs`]. Si
//!   coincide con la referencia (valor por defecto), la etiqueta se aplica tal
//!   cual; si no, el motor suma `target_lufs - REPLAYGAIN_REFERENCE_LUFS` a la
//!   ganancia leída, porque la etiqueta se midió contra la referencia, no
//!   contra el target del usuario.
//! - Sin valor no se inventa nada: un `None` (o una cadena vacía, malformada o
//!   no finita) deja la ganancia en 1.0. En modo álbum, si falta la ganancia de
//!   álbum pero hay de pista, se cae a la de pista y se registra el aviso
//!   (ADR-0011); nunca se fabrica un valor de álbum.
//!
//! # Limiter de pico
//!
//! El limiter va después de la ganancia en la cadena fija de ADR-0014
//! (EQ → ganancia → limiter → salida). Protege contra el clipping que podría
//! introducir el boost de ReplayGain:
//!
//! 1. Si se conoce el peak de la pista (o del álbum, en modo álbum), la ganancia
//!    se atenúa estáticamente lo justo para que `peak * ganancia` no supere el
//!    techo. Es una atenuación constante, sin compresión ni latencia: no altera
//!    la dinámica, solo el nivel.
//! 2. Cada muestra se satura además al techo como guarda de última hora, por si
//!    la etiqueta de peak falta o miente. Con el peak bien etiquetado esta
//!    guarda no llega a actuar.
//!
//! El techo por defecto es [`DEFAULT_LIMITER_CEILING_DBFS`] (0 dBFS): con
//! material PCM nominal (`|x| ≤ 1.0`) no recorta nada salvo que la señal o su
//! ganancia superen fondo de escala. Un techo más bajo (por ejemplo -1 dBFS)
//! deja headroom configurable. El módulo no implementa AGC: esa es una
//! normalización alternativa, no un complemento.
//!
//! # Uso
//!
//! [`GainSource`] envuelve cualquier [`rodio::Source`] y reenvía canales,
//! frecuencia, duración y seek, de modo que puede intercalarse en el camino de
//! audio sin cambiar el transporte:
//!
//! ```
//! use std::num::NonZero;
//!
//! use rodio::buffer::SamplesBuffer;
//! use syncify_player::gain::{GainSettings, GainSource, ReplayGain};
//!
//! let channels = NonZero::new(1u16).unwrap();
//! let rate = NonZero::new(44_100u32).unwrap();
//! let source = SamplesBuffer::new(channels, rate, vec![0.5f32; 64]);
//! // La etiqueta dice -6.02 dB: la salida queda a la mitad.
//! let tags = ReplayGain {
//!     track_gain_db: Some(-6.0206),
//!     ..ReplayGain::default()
//! };
//! let output: Vec<f32> = GainSource::with_settings(source, GainSettings::track(), &tags)
//!     .collect();
//! assert!((output[0] - 0.25).abs() < 1e-4);
//! ```

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::math::{db_to_linear, linear_to_db};
use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

/// Sonoridad de referencia que asume el etiquetado ReplayGain 2.0 del repo
/// (-18 LUFS / 89 dB SPL). Es también el target de reproducción por defecto, de
/// modo que la etiqueta se aplica sin desplazamiento.
pub const REPLAYGAIN_REFERENCE_LUFS: f32 = -18.0;

/// Techo por defecto del limiter: 0 dBFS. No recorta material PCM nominal
/// (|x| ≤ 1.0); solo impide que la ganancia lo lleve por encima de fondo de
/// escala.
pub const DEFAULT_LIMITER_CEILING_DBFS: f32 = 0.0;

/// Suelo de ganancia: por debajo de -120 dB la señal es inaudible y el valor
/// deja de ser representable con sentido en `f32` para la cadena.
const MIN_GAIN_DB: f32 = -120.0;
/// Techo de ganancia, simétrico con [`MIN_GAIN_DB`]: evita que una etiqueta
/// absurda convierta el multiplicador en `inf` (y `0 * inf` en `NaN`).
const MAX_GAIN_DB: f32 = 120.0;
/// Suelo del techo del limiter: un techo de -120 dBFS ya es silencio práctico.
const MIN_LIMITER_CEILING_DBFS: f32 = -120.0;

/// Modo de ganancia de reproducción.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GainMode {
    /// Sin ganancia: la etiqueta se ignora y el multiplicador es 1.0.
    #[default]
    Off,
    /// Ganancia por pista (`replaygain_track_gain`).
    Track,
    /// Ganancia por álbum (`replaygain_album_gain`). Si falta, cae a la de
    /// pista; si tampoco existe, no hay ganancia.
    Album,
}

/// Ganancia finalmente aplicada y de dónde salió.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainOrigin {
    /// El modo es [`GainMode::Off`]: la etiqueta no se usó.
    Off,
    /// El modo pedía ganancia, pero la etiqueta correspondiente no existe.
    Missing,
    /// Se usó `replaygain_track_gain`.
    Track,
    /// Se usó `replaygain_album_gain`.
    Album,
    /// El modo era álbum, faltaba la ganancia de álbum y se usó la de pista.
    TrackFallback,
}

/// Lecturas ReplayGain de una pista, tal y como viven en las columnas de
/// `tracks` (`replaygain_track_gain`, `replaygain_track_peak`,
/// `replaygain_album_gain`, `replaygain_album_peak`).
///
/// `None` significa «no hay medida» y nunca se sustituye por un valor
/// inventado. Las ganancias están en dB con signo; los peaks son ratios
/// lineales. Para las columnas TEXT del esquema, usa
/// [`ReplayGain::from_tag_strings`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReplayGain {
    /// `replaygain_track_gain` en dB (por ejemplo `-6.50`).
    pub track_gain_db: Option<f32>,
    /// `replaygain_track_peak` como ratio lineal (por ejemplo `0.988553`).
    pub track_peak: Option<f32>,
    /// `replaygain_album_gain` en dB, solo si el álbum estaba completo.
    pub album_gain_db: Option<f32>,
    /// `replaygain_album_peak` como ratio lineal (máximo de las pistas).
    pub album_peak: Option<f32>,
}

impl ReplayGain {
    /// Construye desde las columnas TEXT de `tracks`.
    ///
    /// Cada argumento acepta la forma textual de la columna (`"-6.50 dB"`,
    /// `"0.988553"`). Un `None`, una cadena vacía, un valor no numérico o no
    /// finito quedan como `None`: no se inventa una medida.
    pub fn from_tag_strings(
        track_gain: Option<&str>,
        track_peak: Option<&str>,
        album_gain: Option<&str>,
        album_peak: Option<&str>,
    ) -> Self {
        Self {
            track_gain_db: track_gain.and_then(parse_gain_db),
            track_peak: track_peak.and_then(parse_peak),
            album_gain_db: album_gain.and_then(parse_gain_db),
            album_peak: album_peak.and_then(parse_peak),
        }
    }

    fn usable_track_gain(&self) -> Option<f32> {
        self.track_gain_db.filter(|db| db.is_finite())
    }

    fn usable_album_gain(&self) -> Option<f32> {
        self.album_gain_db.filter(|db| db.is_finite())
    }

    fn usable_track_peak(&self) -> Option<f32> {
        self.track_peak
            .filter(|peak| peak.is_finite() && *peak >= 0.0)
    }

    fn usable_album_peak(&self) -> Option<f32> {
        self.album_peak
            .filter(|peak| peak.is_finite() && *peak >= 0.0)
    }
}

/// Ajustes de ganancia de reproducción.
///
/// El target es el de reproducción, no el de etiquetado: ReplayGain 2.0 mide
/// contra [`REPLAYGAIN_REFERENCE_LUFS`] y el motor suma
/// `target_lufs - REPLAYGAIN_REFERENCE_LUFS` a la ganancia leída. Con el target
/// por defecto la etiqueta se aplica tal cual. Un target no finito cae a la
/// referencia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainSettings {
    /// Modo de ganancia.
    pub mode: GainMode,
    /// Target de sonoridad de reproducción en LUFS.
    pub target_lufs: f32,
    /// Techo del limiter en dBFS (≤ 0; por encima de 0 no evitaría clipping).
    pub limiter_ceiling_dbfs: f32,
}

impl Default for GainSettings {
    fn default() -> Self {
        Self {
            mode: GainMode::Off,
            target_lufs: REPLAYGAIN_REFERENCE_LUFS,
            limiter_ceiling_dbfs: DEFAULT_LIMITER_CEILING_DBFS,
        }
    }
}

impl GainSettings {
    /// Ajustes por defecto: modo `off`, target ReplayGain 2.0 y techo 0 dBFS.
    pub fn off() -> Self {
        Self::default()
    }

    /// Modo pista con target y techo por defecto.
    pub fn track() -> Self {
        Self {
            mode: GainMode::Track,
            ..Self::default()
        }
    }

    /// Modo álbum con target y techo por defecto.
    pub fn album() -> Self {
        Self {
            mode: GainMode::Album,
            ..Self::default()
        }
    }

    /// Fija el target de reproducción en LUFS.
    pub fn with_target_lufs(mut self, target_lufs: f32) -> Self {
        self.target_lufs = target_lufs;
        self
    }

    /// Fija el techo del limiter en dBFS.
    pub fn with_limiter_ceiling_dbfs(mut self, dbfs: f32) -> Self {
        self.limiter_ceiling_dbfs = dbfs;
        self
    }

    /// Calcula la ganancia y el estado del limiter para una pista.
    ///
    /// Es puro: no toca audio ni estado global, y por eso los tests pueden
    /// comprobar la decisión sin crear una fuente.
    pub fn plan(&self, tags: &ReplayGain) -> GainPlan {
        let ceiling = self.linear_ceiling();
        let (tag_gain_db, origin, peak) = match self.mode {
            GainMode::Off => (0.0, GainOrigin::Off, None),
            GainMode::Track => match tags.usable_track_gain() {
                Some(db) => (db, GainOrigin::Track, tags.usable_track_peak()),
                None => (0.0, GainOrigin::Missing, None),
            },
            GainMode::Album => match tags.usable_album_gain() {
                Some(db) => (
                    db,
                    GainOrigin::Album,
                    // El peak de álbum es el máximo de las pistas: atenúa por
                    // igual todo el álbum y conserva sus niveles relativos.
                    tags.usable_album_peak()
                        .or_else(|| tags.usable_track_peak()),
                ),
                None => match tags.usable_track_gain() {
                    Some(db) => {
                        tracing::warn!(
                            "gain: album ReplayGain missing; falling back to track gain"
                        );
                        (db, GainOrigin::TrackFallback, tags.usable_track_peak())
                    }
                    None => (0.0, GainOrigin::Missing, None),
                },
            },
        };

        let requested_gain_db = if matches!(origin, GainOrigin::Off | GainOrigin::Missing) {
            0.0
        } else {
            (tag_gain_db + self.target_shift_db()).clamp(MIN_GAIN_DB, MAX_GAIN_DB)
        };
        let requested_gain = db_to_linear(requested_gain_db);

        // Atenuación estática: si el boost llevaría el peak medido por encima
        // del techo, se recorta la ganancia (no la señal) para conservar la
        // dinámica. La guarda por muestra del `GainSource` cubre el resto.
        let (gain, limited) = match peak {
            Some(peak) if requested_gain * peak > ceiling => (ceiling / peak, true),
            _ => (requested_gain, false),
        };

        GainPlan {
            gain,
            requested_gain_db,
            gain_db: if limited {
                linear_to_db(gain)
            } else {
                requested_gain_db
            },
            origin,
            limited,
            ceiling,
        }
    }

    fn target_shift_db(&self) -> f32 {
        let target = if self.target_lufs.is_finite() {
            self.target_lufs
        } else {
            REPLAYGAIN_REFERENCE_LUFS
        };
        target - REPLAYGAIN_REFERENCE_LUFS
    }

    fn linear_ceiling(&self) -> f32 {
        let dbfs = if self.limiter_ceiling_dbfs.is_finite() {
            self.limiter_ceiling_dbfs
                .clamp(MIN_LIMITER_CEILING_DBFS, DEFAULT_LIMITER_CEILING_DBFS)
        } else {
            DEFAULT_LIMITER_CEILING_DBFS
        };
        db_to_linear(dbfs)
    }
}

/// Decisión de ganancia para una pista: multiplicador, origen y limiter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainPlan {
    /// Multiplicador lineal que aplica la fuente (ya con el limiter estático).
    pub gain: f32,
    /// Ganancia pedida por la etiqueta y el target, en dB, antes del limiter.
    pub requested_gain_db: f32,
    /// Ganancia realmente aplicada en dB, después del limiter estático.
    pub gain_db: f32,
    /// Etiqueta que originó la ganancia.
    pub origin: GainOrigin,
    /// El limiter estático atenuó la ganancia para que `peak * gain` no supere
    /// el techo. La guarda por muestra puede actuar además en tiempo de
    /// reproducción; eso lo publica [`PeakMeter::limited`].
    pub limited: bool,
    /// Techo lineal del limiter.
    pub ceiling: f32,
}

/// Medidor compartido del pico post-proceso y del estado del limiter.
///
/// Clonar comparte el mismo medidor, de modo que quien construye la fuente
/// puede leerlo después de mover [`GainSource`] al backend de audio.
#[derive(Debug, Clone, Default)]
pub struct PeakMeter {
    peak: Arc<AtomicU32>,
    pre_limit_peak: Arc<AtomicU32>,
    limited: Arc<AtomicBool>,
}

impl PeakMeter {
    /// Medidor a cero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pico absoluto de salida observado hasta ahora (post-limiter).
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.load(Ordering::Relaxed))
    }

    /// Pico absoluto antes de la guarda por muestra (post-ganancia). Puede
    /// superar el techo si la etiqueta de peak faltaba o mentía.
    pub fn pre_limit_peak(&self) -> f32 {
        f32::from_bits(self.pre_limit_peak.load(Ordering::Relaxed))
    }

    /// `true` si el limiter actuó: atenuación estática al construir la fuente o
    /// saturación de alguna muestra durante la reproducción.
    pub fn limited(&self) -> bool {
        self.limited.load(Ordering::Relaxed)
    }

    fn record(&self, pre_limit: f32, output: f32) {
        self.pre_limit_peak
            .fetch_max(pre_limit.to_bits(), Ordering::Relaxed);
        self.peak.fetch_max(output.to_bits(), Ordering::Relaxed);
    }

    fn mark_limited(&self) {
        self.limited.store(true, Ordering::Relaxed);
    }
}

/// Adaptador de [`rodio::Source`] que aplica un [`GainPlan`] y el limiter.
///
/// Reenvía al vuelo canales, frecuencia de muestreo, duración, span y seek, así
/// que puede sustituir a la fuente original en el camino de audio. El plan se
/// calcula una vez por pista; el medidor viaja en un [`PeakMeter`] compartido
/// para publicar pico y estado del limiter sin bloquear el hilo de audio.
#[derive(Debug)]
pub struct GainSource<S> {
    input: S,
    plan: GainPlan,
    meter: PeakMeter,
}

impl<S> GainSource<S> {
    /// Envuelve una fuente con un plan ya calculado.
    pub fn new(input: S, plan: GainPlan) -> Self {
        let meter = PeakMeter::new();
        if plan.limited {
            meter.mark_limited();
        }
        Self { input, plan, meter }
    }

    /// Envuelve una fuente calculando el plan desde los ajustes y las etiquetas.
    pub fn with_settings(input: S, settings: GainSettings, tags: &ReplayGain) -> Self {
        Self::new(input, settings.plan(tags))
    }

    /// Plan aplicado (inmutable durante la vida de la fuente).
    pub fn plan(&self) -> GainPlan {
        self.plan
    }

    /// Asa compartida al medidor. Tómala **antes** de mover la fuente al
    /// backend; clonarla conserva acceso a los valores acumulados.
    pub fn meter(&self) -> PeakMeter {
        self.meter.clone()
    }

    /// Fuente original.
    pub fn inner(&self) -> &S {
        &self.input
    }

    /// Fuente original, mutable.
    pub fn inner_mut(&mut self) -> &mut S {
        &mut self.input
    }

    /// Desenvuelve la fuente original descartando el medidor.
    pub fn into_inner(self) -> S {
        self.input
    }
}

impl<S: Source> Iterator for GainSource<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.input.next()?;
        let gained = sample * self.plan.gain;
        let output = gained.clamp(-self.plan.ceiling, self.plan.ceiling);
        // `NaN` no compara y no debe marcar el limiter; ±inf sí: es una
        // muestra fuera de rango que la guarda recorta.
        if gained > self.plan.ceiling || gained < -self.plan.ceiling {
            self.meter.mark_limited();
        }
        if gained.is_finite() {
            self.meter.record(gained.abs(), output.abs());
        }
        Some(output)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<S: Source> Source for GainSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.input.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)
    }
}

/// Parsea una ganancia textual de columna (`"-6.50 dB"`, `"-6.50"`).
fn parse_gain_db(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let numeric = if lower.ends_with("db") {
        trimmed[..trimmed.len() - 2].trim_end()
    } else {
        trimmed
    };
    numeric.parse::<f32>().ok().filter(|db| db.is_finite())
}

/// Parsea un peak textual de columna (`"0.988553"`). Un ratio negativo no es
/// una medida válida y se descarta.
fn parse_peak(raw: &str) -> Option<f32> {
    raw.trim()
        .parse::<f32>()
        .ok()
        .filter(|peak| peak.is_finite() && *peak >= 0.0)
}
