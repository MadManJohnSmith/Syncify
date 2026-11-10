//! Q4 (#87): interruptor compartido por proveedor para tormentas de transporte.
//!
//! El backoff por job (`syncify_core_domain::job_state`) programa el reintento
//! de cada fila, pero nada impedía que N filas del mismo proveedor martillearan
//! el proveedor mientras está caído: cada reintento salía aunque los 50
//! anteriores hubieran sido 5xx. Este interruptor añade una pausa COMPARTIDA:
//! tras `FAILURE_THRESHOLD` fallos de transporte consecutivos de un proveedor,
//! el worker deja de reclamar filas de ese proveedor hasta que vence la
//! apertura; una sonda fallida en media apertura lo reabre con duración
//! exponencial (techo `OPEN_MAX`); una respuesta definitiva — éxito o fallo
//! por-elemento — lo cierra, porque prueba que el transporte del proveedor
//! responde.
//!
//! Estado en memoria del proceso, sin BD ni evento de UI: el contrato de la
//! cola ya persiste el reintento por fila; esto solo modula el ritmo de
//! reclamación compartido. Las aperturas quedan en el log (`warn`) como señal
//! de tormenta para la evidencia de #222.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use syncify_core_domain::errors::ErrorTaxonomy;

/// Fallos de transporte consecutivos de un proveedor que abren el circuito.
pub const FAILURE_THRESHOLD: u32 = 5;
/// Duración de la primera apertura; cada reapertura duplica la anterior.
pub const OPEN_BASE: Duration = Duration::from_secs(60);
/// Techo de la duración de apertura.
pub const OPEN_MAX: Duration = Duration::from_secs(30 * 60);

/// ¿Es este fallo señal de proveedor caído (frente a una respuesta definitiva
/// por-elemento)? Las clases que el reintento automático trataría como
/// transporte — más `AuthRefreshable`, que también es estado del proveedor y
/// no de la pista — cuentan; una respuesta definitiva prueba que el proveedor
/// responde y reinicia la racha.
fn is_transport_failure(class: &ErrorTaxonomy) -> bool {
    matches!(
        class,
        ErrorTaxonomy::RateLimited { .. }
            | ErrorTaxonomy::TemporaryNetworkFailure { .. }
            | ErrorTaxonomy::Timeout { .. }
            | ErrorTaxonomy::AuthRefreshable { .. }
    )
}

/// Duración de una apertura de generación `generation` (1 = la primera).
fn open_duration(generation: u32) -> Duration {
    let doublings = generation.saturating_sub(1).min(9);
    OPEN_BASE.saturating_mul(1u32 << doublings).min(OPEN_MAX)
}

#[derive(Debug, Default)]
struct BreakerCell {
    consecutive_transport_failures: u32,
    /// `Some(until)` con el circuito abierto. Una apertura vencida deja el
    /// proveedor en media apertura: reclamable como sonda, pero sin respuesta
    /// definitiva el siguiente fallo lo reabre de inmediato.
    open_until: Option<Instant>,
    open_generation: u32,
}

impl BreakerCell {
    fn is_open_at(&self, now: Instant) -> bool {
        self.open_until.map(|until| until > now).unwrap_or(false)
    }

    fn open(&mut self, now: Instant) {
        self.open_generation += 1;
        self.open_until = Some(now + open_duration(self.open_generation));
        self.consecutive_transport_failures = 0;
    }

    /// Respuesta definitiva del proveedor: transporte vivo, circuito cerrado.
    fn reset(&mut self) {
        self.consecutive_transport_failures = 0;
        self.open_until = None;
        self.open_generation = 0;
    }
}

/// Interruptor compartido por proveedor. `Clone` comparte el mismo mapa.
#[derive(Clone, Debug, Default)]
pub struct ProviderBreaker {
    cells: Arc<Mutex<HashMap<String, BreakerCell>>>,
}

impl ProviderBreaker {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, BreakerCell>> {
        // Un pánico con el lock tomado no debe envenenar el ritmo de la cola:
        // el mapa no tiene invariantes que un pánico pueda romper a medias.
        self.cells
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Clave canónica del mapa: los nombres de servicio llegan de la BD y de
    /// resultados de descarga, y la exclusión del claim compara en minúsculas.
    fn normalize(service: &str) -> String {
        service.trim().to_lowercase()
    }

    /// Proveedores con el circuito abierto ahora mismo; alimenta la exclusión
    /// del claim (`LOWER(service_name) NOT IN (...)`). Una apertura vencida no
    /// está abierta: media apertura, reclamable como sonda de recuperación.
    pub fn open_services(&self, now: Instant) -> Vec<String> {
        self.lock()
            .iter()
            .filter(|(_, cell)| cell.is_open_at(now))
            .map(|(service, _)| service.clone())
            .collect()
    }

    /// Registra el resultado de un intento contra `service`.
    ///
    /// Transporte fallido: incrementa la racha y abre el circuito al llegar al
    /// umbral; si el circuito ya estaba abierto o en media apertura, reabre de
    /// inmediato con la duración duplicada. Respuesta definitiva: resetea la
    /// racha y cierra. Un nombre vacío no es un proveedor identificable y no
    /// se registra.
    ///
    /// Devuelve `true` si esta llamada acaba de abrir (o reabrir) el circuito:
    /// el llamador lo usa para dejar señal de tormenta en el log.
    pub fn record_failure(&self, service: &str, class: &ErrorTaxonomy, now: Instant) -> bool {
        let key = Self::normalize(service);
        if key.is_empty() {
            return false;
        }
        let mut cells = self.lock();
        let cell = cells.entry(key).or_default();
        if !is_transport_failure(class) {
            cell.reset();
            return false;
        }
        if cell.open_until.is_some() {
            // Fallo de la sonda de media apertura: el proveedor acaba de
            // demostrar que sigue mal; reabrir sin reconstruir la racha.
            cell.open(now);
            return true;
        }
        cell.consecutive_transport_failures += 1;
        if cell.consecutive_transport_failures >= FAILURE_THRESHOLD {
            cell.open(now);
            return true;
        }
        false
    }

    /// Registra una descarga entregada: respuesta definitiva del proveedor.
    pub fn record_success(&self, service: &str) {
        let key = Self::normalize(service);
        if key.is_empty() {
            return;
        }
        self.lock().entry(key).or_default().reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net() -> ErrorTaxonomy {
        ErrorTaxonomy::TemporaryNetworkFailure {
            endpoint: "qobuz".into(),
            message: "HTTP 500".into(),
        }
    }

    fn definitive() -> ErrorTaxonomy {
        ErrorTaxonomy::UnavailableFromProvider {
            provider: "qobuz".into(),
            item_id: "42".into(),
            reason: "not in catalog".into(),
        }
    }

    fn at(offset_sec: impl Into<u64>) -> Instant {
        Instant::now() + Duration::from_secs(offset_sec.into())
    }

    /// ¿Está abierto el circuito de `service` en `now`?
    fn is_open(breaker: &ProviderBreaker, service: &str, now: Instant) -> bool {
        breaker.open_services(now).iter().any(|s| s == service)
    }

    #[test]
    fn threshold_of_transport_failures_opens_the_circuit() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD - 1 {
            assert!(
                !breaker.record_failure("qobuz", &net(), at(i)),
                "failure {i} below the threshold must not open"
            );
            assert!(!is_open(&breaker, "qobuz", at(i)));
        }
        assert!(breaker.record_failure("qobuz", &net(), at(FAILURE_THRESHOLD)));
        assert!(is_open(&breaker, "qobuz", at(FAILURE_THRESHOLD)));
        assert_eq!(
            breaker.open_services(at(FAILURE_THRESHOLD)),
            vec!["qobuz".to_string()]
        );
    }

    #[test]
    fn a_definitive_response_resets_the_streak() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD - 1 {
            breaker.record_failure("qobuz", &net(), at(i));
        }
        breaker.record_failure("qobuz", &definitive(), at(10u64));
        for i in 11..11 + FAILURE_THRESHOLD - 2 {
            breaker.record_failure("qobuz", &net(), at(i));
        }
        assert!(
            !is_open(&breaker, "qobuz", at(100u64)),
            "a definitive answer proves the transport is alive and resets the streak"
        );
    }

    #[test]
    fn success_closes_an_open_circuit() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD {
            breaker.record_failure("qobuz", &net(), at(i));
        }
        assert!(is_open(&breaker, "qobuz", at(FAILURE_THRESHOLD)));
        breaker.record_success("qobuz");
        assert!(!is_open(&breaker, "qobuz", at(FAILURE_THRESHOLD)));
        assert!(breaker.open_services(at(FAILURE_THRESHOLD)).is_empty());
    }

    #[test]
    fn a_failed_probe_reopens_immediately_with_a_doubled_window() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD {
            breaker.record_failure("qobuz", &net(), at(i));
        }
        // Apertura vencida: media apertura, reclamable como sonda.
        let probe_time = at(FAILURE_THRESHOLD + 61);
        assert!(!is_open(&breaker, "qobuz", probe_time));
        // La sonda falla: reabrir de inmediato por 120 s, sin nueva racha.
        assert!(breaker.record_failure("qobuz", &net(), probe_time));
        assert!(is_open(
            &breaker,
            "qobuz",
            probe_time + Duration::from_secs(1)
        ));
        assert!(!is_open(
            &breaker,
            "qobuz",
            probe_time + Duration::from_secs(121)
        ));
    }

    #[test]
    fn reopen_windows_grow_exponentially_up_to_the_cap() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD {
            breaker.record_failure("qobuz", &net(), at(i));
        }
        let mut now = at(FAILURE_THRESHOLD);
        let mut window = OPEN_BASE;
        loop {
            assert!(is_open(&breaker, "qobuz", now), "the window must be open");
            now += window;
            assert!(
                !is_open(&breaker, "qobuz", now),
                "the window must expire into half-open"
            );
            breaker.record_failure("qobuz", &net(), now);
            if window >= OPEN_MAX {
                break;
            }
            window = (window * 2).min(OPEN_MAX);
        }
        assert_eq!(window, OPEN_MAX, "the growth must stop at the cap");
    }

    #[test]
    fn service_names_normalize_into_one_cell() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD {
            // Mezcla de caja y espacios: la BD y los resultados no coinciden siempre.
            breaker.record_failure(" Qobuz ", &net(), at(i));
        }
        assert!(is_open(&breaker, "qobuz", at(FAILURE_THRESHOLD)));
    }

    #[test]
    fn empty_service_names_are_not_tracked() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD * 2 {
            breaker.record_failure("", &net(), at(i));
        }
        assert!(
            breaker.open_services(at(FAILURE_THRESHOLD)).is_empty(),
            "un servicio sin nombre no puede excluir filas del claim"
        );
    }

    #[test]
    fn providers_are_independent() {
        let breaker = ProviderBreaker::default();
        for i in 0..FAILURE_THRESHOLD {
            breaker.record_failure("qobuz", &net(), at(i));
        }
        assert!(is_open(&breaker, "qobuz", at(FAILURE_THRESHOLD)));
        assert!(!is_open(&breaker, "tidal", at(FAILURE_THRESHOLD)));
    }
}
