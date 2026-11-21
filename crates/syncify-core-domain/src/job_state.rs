//! Máquina de estados de un job de descarga (`download_queue`).
//!
//! El estado vive en una columna TEXT y su string es contrato con la BD y la UI;
//! este módulo es la fuente de verdad de **qué transiciones existen** y de
//! **cuándo se reintenta**. No tiene I/O: la app aplica cada transición con un
//! `UPDATE ... WHERE status = <from>` (ADR-0022).

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::errors::ErrorTaxonomy;

/// Intentos automáticos antes de declarar el fallo permanente.
pub const MAX_AUTOMATIC_ATTEMPTS: u32 = 5;

/// Backoff por intento (índice = intentos ya consumidos): 1m, 5m, 15m, 1h, 4h.
pub const BACKOFF_SCHEDULE: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(4 * 60 * 60),
];

/// Jitter del backoff, como fracción del escalón: el delay efectivo cae en
/// `±BACKOFF_JITTER_FRACTION` del escalón. Sin jitter, todos los jobs que
/// fallaron en la misma ventana reintentan exactamente a la hora programada y
/// la tormenta vuelve sincronizada cada escalón.
pub const BACKOFF_JITTER_FRACTION: f64 = 0.25;

/// Estado de una fila de `download_queue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Downloading,
    Complete,
    Failed,
    Cancelled,
}

/// Evento que intenta mover un job de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobEvent {
    /// El worker toma el job de la cola.
    Claim,
    /// La descarga terminó bien.
    Complete,
    /// La descarga falló (la clase y el mensaje los persiste el llamador).
    Fail,
    /// El usuario canceló.
    Cancel,
    /// El usuario pidió reintentar un fallo.
    ManualRetry,
    /// Recuperación de arranque de un job interrumpido.
    RecoveryReset,
}

/// Transición que la máquina no permite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: JobState,
    pub event: JobEvent,
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Illegal job transition: {:?} on {:?}",
            self.event, self.from
        )
    }
}

impl std::error::Error for IllegalTransition {}

/// Veredicto de reintento para un job fallido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryVerdict {
    /// No se reintenta: clase terminal o presupuesto agotado.
    Permanent,
    /// Se reintenta en ese instante (`next_attempt_at`).
    RetryAt(Instant),
}

impl JobState {
    /// String canónico: el mismo que guarda la BD y consume la UI.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Downloading => "downloading",
            JobState::Complete => "complete",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }

    /// Parsea el string de la BD. Un valor desconocido se queda en `None`: no se
    /// inventa un estado para una fila que no debería existir (el CHECK lo impide).
    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw {
            "queued" => Some(JobState::Queued),
            "downloading" => Some(JobState::Downloading),
            "complete" => Some(JobState::Complete),
            "failed" => Some(JobState::Failed),
            "cancelled" => Some(JobState::Cancelled),
            _ => None,
        }
    }

    /// Estados en los que la fila ya no evoluciona por sí sola.
    pub fn is_finished(&self) -> bool {
        matches!(self, JobState::Complete | JobState::Cancelled)
    }

    /// ¿Puede el worker tomar este job ahora?
    ///
    /// `queued` con espera vencida (o sin espera) y `failed` con un
    /// `next_attempt_at` vencido (reintento automático programado).
    pub fn can_claim(&self, next_attempt_at: Option<Instant>, now: Instant) -> bool {
        match self {
            JobState::Queued => next_attempt_at.map(|at| at <= now).unwrap_or(true),
            JobState::Failed => next_attempt_at.map(|at| at <= now).unwrap_or(false),
            _ => false,
        }
    }

    /// Aplica un evento. Las aristas inexistentes son error, no overwrite.
    pub fn transition(self, event: JobEvent) -> Result<JobState, IllegalTransition> {
        let next = match (self, event) {
            (JobState::Queued, JobEvent::Claim) => JobState::Downloading,
            (JobState::Downloading, JobEvent::Complete) => JobState::Complete,
            (JobState::Downloading, JobEvent::Fail) => JobState::Failed,
            (JobState::Queued, JobEvent::Cancel) => JobState::Cancelled,
            (JobState::Downloading, JobEvent::Cancel) => JobState::Cancelled,
            (JobState::Failed, JobEvent::ManualRetry) => JobState::Queued,
            (JobState::Downloading, JobEvent::RecoveryReset) => JobState::Queued,
            _ => return Err(IllegalTransition { from: self, event }),
        };
        Ok(next)
    }

    /// ¿Se reintenta un job en este estado, con esta clase de fallo y estos intentos?
    ///
    /// Solo un `failed` reintenta; la clase terminal manda sobre el presupuesto y
    /// un `RateLimited` con directiva del servidor la usa como suelo del backoff.
    pub fn retry_verdict(
        &self,
        class: &ErrorTaxonomy,
        attempts: u32,
        now: Instant,
    ) -> RetryVerdict {
        if !self.has_retry(class, attempts) {
            return RetryVerdict::Permanent;
        }
        RetryVerdict::RetryAt(now + Self::effective_delay(class, backoff_step(attempts)))
    }

    /// Igual que [`Self::retry_verdict`], con jitter determinista por semilla.
    ///
    /// El escalón exacto sincroniza los reintentos: toda una tormenta de 5xx
    /// vuelve a golpear a la vez cada escalón. La semilla — en la práctica el
    /// id de la fila — reparte el delay en `±BACKOFF_JITTER_FRACTION` del
    /// escalón de forma reproducible (misma semilla, mismo veredicto). El suelo
    /// `retry_after` de `RateLimited` se aplica después del jitter, así que la
    /// directiva del servidor nunca se acorta.
    pub fn retry_verdict_jittered(
        &self,
        class: &ErrorTaxonomy,
        attempts: u32,
        now: Instant,
        seed: u64,
    ) -> RetryVerdict {
        if !self.has_retry(class, attempts) {
            return RetryVerdict::Permanent;
        }
        let jittered = backoff_step(attempts).mul_f64(1.0 + jitter_fraction(seed));
        RetryVerdict::RetryAt(now + Self::effective_delay(class, jittered))
    }

    /// ¿Hay reintento para este estado con esta clase y este presupuesto?
    fn has_retry(&self, class: &ErrorTaxonomy, attempts: u32) -> bool {
        *self == JobState::Failed && class.is_retryable() && attempts < MAX_AUTOMATIC_ATTEMPTS
    }

    /// Escalón efectivo: la directiva `retry_after` del servidor es un suelo
    /// y nunca se acorta por el jitter.
    fn effective_delay(class: &ErrorTaxonomy, step: Duration) -> Duration {
        match class {
            ErrorTaxonomy::RateLimited {
                retry_after_sec: Some(secs),
                ..
            } => step.max(Duration::from_secs(*secs)),
            _ => step,
        }
    }
}

/// Escalón del schedule para `attempts` intentos ya consumidos.
fn backoff_step(attempts: u32) -> Duration {
    BACKOFF_SCHEDULE[(attempts as usize).min(BACKOFF_SCHEDULE.len() - 1)]
}

/// Fracción de jitter en `[-FRACTION, FRACTION)` para una semilla, vía
/// splitmix64. El hash hace que ids contiguos (1, 2, 3…) no produzcan
/// fracciones contiguas: la manada se reparte aunque las filas sean vecinas.
fn jitter_fraction(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    let unit = (z >> 11) as f64 / (1u64 << 53) as f64;
    (unit * 2.0 - 1.0) * BACKOFF_JITTER_FRACTION
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick() -> Instant {
        Instant::now()
    }

    fn retryable() -> ErrorTaxonomy {
        ErrorTaxonomy::TemporaryNetworkFailure {
            endpoint: "qobuz".into(),
            message: "reset".into(),
        }
    }

    fn terminal() -> ErrorTaxonomy {
        ErrorTaxonomy::AuthInvalid {
            message: "bad credentials".into(),
        }
    }

    #[test]
    fn legal_edges_are_exactly_the_declared_graph() {
        let legal = [
            (JobState::Queued, JobEvent::Claim, JobState::Downloading),
            (
                JobState::Downloading,
                JobEvent::Complete,
                JobState::Complete,
            ),
            (JobState::Downloading, JobEvent::Fail, JobState::Failed),
            (JobState::Queued, JobEvent::Cancel, JobState::Cancelled),
            (JobState::Downloading, JobEvent::Cancel, JobState::Cancelled),
            (JobState::Failed, JobEvent::ManualRetry, JobState::Queued),
            (
                JobState::Downloading,
                JobEvent::RecoveryReset,
                JobState::Queued,
            ),
        ];
        for (from, event, to) in legal {
            assert_eq!(
                from.transition(event).unwrap(),
                to,
                "{from:?} --{event:?}-->"
            );
        }
    }

    #[test]
    fn every_other_pair_is_illegal() {
        let states = [
            JobState::Queued,
            JobState::Downloading,
            JobState::Complete,
            JobState::Failed,
            JobState::Cancelled,
        ];
        let events = [
            JobEvent::Claim,
            JobEvent::Complete,
            JobEvent::Fail,
            JobEvent::Cancel,
            JobEvent::ManualRetry,
            JobEvent::RecoveryReset,
        ];
        for from in states {
            for event in events {
                let allowed = matches!(
                    (from, event),
                    (JobState::Queued, JobEvent::Claim)
                        | (JobState::Downloading, JobEvent::Complete)
                        | (JobState::Downloading, JobEvent::Fail)
                        | (JobState::Queued, JobEvent::Cancel)
                        | (JobState::Downloading, JobEvent::Cancel)
                        | (JobState::Failed, JobEvent::ManualRetry)
                        | (JobState::Downloading, JobEvent::RecoveryReset)
                );
                assert_eq!(
                    allowed,
                    from.transition(event).is_ok(),
                    "{from:?} {event:?}"
                );
            }
        }
    }

    #[test]
    fn cancelled_and_complete_never_revive() {
        for finished in [JobState::Complete, JobState::Cancelled] {
            assert!(finished.is_finished());
            for event in [
                JobEvent::Claim,
                JobEvent::Complete,
                JobEvent::Fail,
                JobEvent::ManualRetry,
                JobEvent::RecoveryReset,
            ] {
                assert!(
                    finished.transition(event).is_err(),
                    "{finished:?} {event:?}"
                );
            }
        }
    }

    #[test]
    fn claim_respects_due_time() {
        let now = tick();
        assert!(JobState::Queued.can_claim(None, now));
        assert!(JobState::Queued.can_claim(Some(now - Duration::from_secs(1)), now));
        assert!(!JobState::Queued.can_claim(Some(now + Duration::from_secs(1)), now));
        assert!(!JobState::Failed.can_claim(None, now));
        assert!(JobState::Failed.can_claim(Some(now - Duration::from_secs(1)), now));
        assert!(!JobState::Downloading.can_claim(None, now));
        assert!(!JobState::Complete.can_claim(None, now));
        assert!(!JobState::Cancelled.can_claim(None, now));
    }

    #[test]
    fn retryable_failures_follow_the_declared_backoff() {
        let now = tick();
        let expected = [60u64, 300, 900, 3600, 14400];
        for (attempts, secs) in expected.iter().enumerate() {
            match JobState::Failed.retry_verdict(&retryable(), attempts as u32, now) {
                RetryVerdict::RetryAt(at) => {
                    assert_eq!((at - now).as_secs(), *secs, "attempt {attempts}")
                }
                RetryVerdict::Permanent => panic!("attempt {attempts} must retry"),
            }
        }
    }

    #[test]
    fn budget_exhaustion_is_permanent() {
        assert_eq!(
            JobState::Failed.retry_verdict(&retryable(), MAX_AUTOMATIC_ATTEMPTS, tick()),
            RetryVerdict::Permanent
        );
    }

    #[test]
    fn terminal_class_is_permanent_regardless_of_attempts() {
        assert_eq!(
            JobState::Failed.retry_verdict(&terminal(), 0, tick()),
            RetryVerdict::Permanent
        );
    }

    #[test]
    fn rate_limit_directive_is_a_floor_for_backoff() {
        let now = tick();
        let big = ErrorTaxonomy::RateLimited {
            provider: "qobuz".into(),
            retry_after_sec: Some(9000),
        };
        match JobState::Failed.retry_verdict(&big, 0, now) {
            RetryVerdict::RetryAt(at) => assert_eq!((at - now).as_secs(), 9000),
            RetryVerdict::Permanent => panic!("rate limited must retry"),
        }
        let small = ErrorTaxonomy::RateLimited {
            provider: "qobuz".into(),
            retry_after_sec: Some(5),
        };
        match JobState::Failed.retry_verdict(&small, 0, now) {
            RetryVerdict::RetryAt(at) => assert_eq!((at - now).as_secs(), 60, "backoff wins"),
            RetryVerdict::Permanent => panic!("rate limited must retry"),
        }
    }

    #[test]
    fn jittered_verdict_stays_within_the_step_bounds() {
        let now = tick();
        for attempts in 0..BACKOFF_SCHEDULE.len() as u32 {
            let step = BACKOFF_SCHEDULE[attempts as usize];
            for seed in 0..500u64 {
                match JobState::Failed.retry_verdict_jittered(&retryable(), attempts, now, seed) {
                    RetryVerdict::RetryAt(at) => {
                        let delay = at - now;
                        let lower = step.mul_f64(1.0 - BACKOFF_JITTER_FRACTION);
                        let upper = step.mul_f64(1.0 + BACKOFF_JITTER_FRACTION);
                        assert!(
                            delay >= lower - Duration::from_millis(1)
                                && delay <= upper + Duration::from_millis(1),
                            "seed {seed} attempt {attempts}: {delay:?} outside {lower:?}..{upper:?}"
                        );
                    }
                    RetryVerdict::Permanent => panic!("attempt {attempts} must retry"),
                }
            }
        }
    }

    #[test]
    fn jitter_is_deterministic_per_seed() {
        let now = tick();
        let a = JobState::Failed.retry_verdict_jittered(&retryable(), 0, now, 42);
        let b = JobState::Failed.retry_verdict_jittered(&retryable(), 0, now, 42);
        assert_eq!(a, b, "same seed must reproduce the same verdict");
    }

    #[test]
    fn contiguous_seeds_spread_the_herd() {
        let now = tick();
        let delays: Vec<u64> = (1..=64u64)
            .map(
                |seed| match JobState::Failed.retry_verdict_jittered(&retryable(), 0, now, seed) {
                    RetryVerdict::RetryAt(at) => (at - now).as_secs(),
                    RetryVerdict::Permanent => panic!("must retry"),
                },
            )
            .collect();
        // El escalón de 60 s con ±25 % cubre 30 valores de segundo (45..75);
        // sin jitter, los 64 vecinos caerían en UNO. El hash debe repartirlos
        // por la ventana, no concentrarlos.
        let distinct = delays
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert!(
            distinct >= 15,
            "64 vecinos produjeron solo {distinct} delays distintos (techo físico: 30): la manada no se reparte"
        );
    }

    #[test]
    fn rate_limit_floor_survives_the_jitter() {
        let now = tick();
        let big = ErrorTaxonomy::RateLimited {
            provider: "qobuz".into(),
            retry_after_sec: Some(9000),
        };
        for seed in 0..100u64 {
            match JobState::Failed.retry_verdict_jittered(&big, 0, now, seed) {
                RetryVerdict::RetryAt(at) => assert_eq!(
                    (at - now).as_secs(),
                    9000,
                    "the server directive is a floor, jitter must not shorten it"
                ),
                RetryVerdict::Permanent => panic!("rate limited must retry"),
            }
        }
    }

    #[test]
    fn permanent_verdicts_ignore_the_seed() {
        let now = tick();
        for seed in [0u64, 1, 12345] {
            assert_eq!(
                JobState::Failed.retry_verdict_jittered(&terminal(), 0, now, seed),
                RetryVerdict::Permanent
            );
            assert_eq!(
                JobState::Failed.retry_verdict_jittered(
                    &retryable(),
                    MAX_AUTOMATIC_ATTEMPTS,
                    now,
                    seed
                ),
                RetryVerdict::Permanent
            );
        }
    }

    #[test]
    fn only_failed_jobs_have_a_retry_verdict() {
        for state in [
            JobState::Queued,
            JobState::Downloading,
            JobState::Complete,
            JobState::Cancelled,
        ] {
            assert_eq!(
                state.retry_verdict(&retryable(), 0, tick()),
                RetryVerdict::Permanent
            );
        }
    }

    #[test]
    fn db_strings_round_trip() {
        for state in [
            JobState::Queued,
            JobState::Downloading,
            JobState::Complete,
            JobState::Failed,
            JobState::Cancelled,
        ] {
            assert_eq!(JobState::from_db_str(state.as_db_str()), Some(state));
        }
        assert_eq!(JobState::from_db_str("running"), None);
    }
}
