//! M06 (#171): lotes, concurrencia por proveedor, backoff y cancelación en el
//! límite de lote para los jobs de migración (ADR-0016, decisiones 2, 5 y 6).
//!
//! El job deja de recorrer los ítems de uno en uno con una pausa fija: cada
//! colección procesa su snapshot durable en lotes, con un techo de peticiones
//! simultáneas por proveedor de destino, reintentos con backoff exponencial y
//! jitter ante 429/5xx, cancelación observada entre lotes (nunca a mitad de
//! lote) y un evento de progreso por lote con creados, existentes, fallidos y
//! omitidos. Los estados por ítem siguen siendo la fuente de verdad de la
//! reanudación (M05): cancelar deja `pending` lo que no llegó a ejecutarse.
//!
//! Todo lo de este módulo es determinista salvo los `sleep` del backoff y el
//! jitter; los tests de comportamiento sustituyen el gateway y usan políticas
//! con retardo mínimo.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::Rng;

/// Ítems por lote: unidad de progreso y de cancelación.
pub const DEFAULT_BATCH_SIZE: usize = 50;

/// Techo de concurrencia cuando el destino no declara uno propio.
pub const DEFAULT_CONCURRENCY: usize = 4;

/// Reintentos totales por defecto de una operación de gateway (1 intento + 2
/// reintentos), el mismo orden de magnitud que los clientes HTTP del repo.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Retardo base del backoff; se duplica por intento.
pub const DEFAULT_BASE_DELAY: Duration = Duration::from_millis(250);

/// Techo del backoff calculado (el `Retry-After` del proveedor puede subir por
/// encima, hasta [`MAX_RETRY_AFTER_HONORED`]).
pub const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(8);

/// Un `Retry-After` mayor se recorta aquí: esperar más no cabe en un job
/// interactivo, y el ítem seguirá siendo reanudable.
pub const MAX_RETRY_AFTER_HONORED: Duration = Duration::from_secs(60);

/// Por qué falló una llamada al gateway, en lo que afecta al reintento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// 429 / «rate limit»: reintentar tras el backoff (o el `Retry-After`).
    RateLimited,
    /// 5xx, timeouts y fallos de red: reintentar con backoff.
    Transient,
    /// 4xx de contenido, auth, etc.: reintentar no arregla nada.
    Permanent,
}

/// Clasifica el mensaje de error de un gateway. Los gateways del repo
/// propagan el texto de sus clientes (`Qobuz API error (429): …`,
/// `Tidal API error 503: …`, `Request failed: …`), así que el contrato es el
/// texto, no un tipo: es lo único que atraviesa la costura de los tres traits.
pub fn classify_failure(message: &str) -> FailureClass {
    let lower = message.to_ascii_lowercase();
    if mentions_status(&lower, "429")
        || lower.contains("too many requests")
        || lower.contains("rate limit")
        || lower.contains("rate-limit")
    {
        return FailureClass::RateLimited;
    }
    // 5xx y 408: el proveedor no descartó la petición por su contenido.
    if ["500", "502", "503", "504", "408"]
        .iter()
        .any(|code| mentions_status(&lower, code))
    {
        return FailureClass::Transient;
    }
    // Errores de transporte: reqwest no da código HTTP y sus mensajes caen
    // aquí. Preferimos un reintento acotado a convertir un corte de red en
    // ítem fallido.
    if [
        "timeout",
        "timed out",
        "connection",
        "network",
        "temporarily",
        "unavailable",
        "request failed",
        "broken pipe",
        "connection reset",
        "unexpected eof",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return FailureClass::Transient;
    }
    FailureClass::Permanent
}

/// Un código HTTP aparece en el mensaje como `(503)`, `http 503`, `error 503`
/// o `status 503`; un `503` suelto puede ser un id y no se usa.
fn mentions_status(lower: &str, code: &str) -> bool {
    [
        format!("({})", code),
        format!("http {}", code),
        format!("error {}", code),
        format!("status {}", code),
        format!("code {}", code),
    ]
    .iter()
    .any(|probe| lower.contains(probe))
}

/// `Retry-After` en segundos, si el mensaje del proveedor lo trae.
pub fn retry_after_hint(message: &str) -> Option<Duration> {
    let lower = message.to_ascii_lowercase();
    for marker in ["retry-after:", "retry after", "retry_after"] {
        let Some(position) = lower.find(marker) else {
            continue;
        };
        let digits: String = lower[position + marker.len()..]
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(seconds) = digits.parse::<u64>() {
            if seconds > 0 {
                return Some(Duration::from_secs(seconds));
            }
        }
    }
    None
}

/// Política de reintentos de una operación de gateway.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Intentos totales, contando el primero. Se fuerza a ≥ 1.
    pub max_attempts: u32,
    /// Retardo del primer reintento; se duplica por intento.
    pub base_delay: Duration,
    /// Techo del retardo calculado antes de aplicar el `Retry-After`.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            base_delay: DEFAULT_BASE_DELAY,
            max_delay: DEFAULT_MAX_DELAY,
        }
    }
}

impl RetryPolicy {
    /// Política sin esperas para tests de comportamiento: el reintento y su
    /// clasificación se ejercitan sin pagar los ~750 ms del backoff real.
    pub fn immediate() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        }
    }

    fn attempts(&self) -> u32 {
        self.max_attempts.max(1)
    }

    /// Retardo antes del reintento `attempt` (1 = tras el primer fallo):
    /// exponencial con jitter [½, 1] del techo, nunca por debajo de un
    /// `Retry-After` del proveedor.
    pub fn delay(&self, attempt: u32, class: FailureClass, hint: Option<Duration>) -> Duration {
        let exponent = attempt.saturating_sub(1).min(6);
        let ceiling = self
            .base_delay
            .saturating_mul(1u32 << exponent)
            .min(self.max_delay);
        let jittered = ceiling.mul_f64(rand::thread_rng().gen_range(0.5..=1.0));
        match (class, hint) {
            (FailureClass::RateLimited, Some(hint)) => {
                hint.min(MAX_RETRY_AFTER_HONORED).max(jittered)
            }
            _ => jittered,
        }
    }
}

/// Techo de concurrencia por proveedor de destino (ADR-0016, decisión 2).
/// Se deriva de los límites que ya declara `services::rate_limiter` para los
/// clientes HTTP, con margen: el job no añade un fan-out que el cliente tenga
/// que frenar a base de 429.
pub fn destination_concurrency(service: &str) -> usize {
    match service.to_ascii_lowercase().as_str() {
        // 10 req/s con 100 ms de separación mínima: 3 en vuelo deja margen.
        "qobuz" => 3,
        // 20 req/s con 50 ms: 4 en vuelo.
        "tidal" => 4,
        // 30 req/s: 5 en vuelo.
        "spotify" => 5,
        "apple_music" => 5,
        // 50 req / 5 s: techo efectivo 10 req/s.
        "deezer" => 4,
        "soundcloud" => 3,
        // Sin límite remoto: sigue habiendo techo, nunca fan-out abierto.
        "local" => 8,
        _ => DEFAULT_CONCURRENCY,
    }
}

/// Política de una fase de colección: tamaño de lote, concurrencia y backoff.
#[derive(Debug, Clone)]
pub struct BatchPolicy {
    pub batch_size: usize,
    pub concurrency: usize,
    pub retry: RetryPolicy,
}

impl BatchPolicy {
    /// Política de producción para el proveedor de destino del job.
    pub fn for_destination(service: &str) -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            concurrency: destination_concurrency(service),
            retry: RetryPolicy::default(),
        }
    }

    /// Política de tests: lotes y concurrencia explícitos, sin esperas.
    pub fn for_tests(batch_size: usize, concurrency: usize) -> Self {
        Self {
            batch_size: batch_size.max(1),
            concurrency: concurrency.max(1),
            retry: RetryPolicy::immediate(),
        }
    }

    /// Tamaño de lote normalizado; 0 no desactiva el progreso ni la cancelación.
    pub(crate) fn effective_batch_size(&self) -> usize {
        self.batch_size.max(1)
    }

    /// Concurrencia normalizada; nunca un fan-out abierto.
    pub(crate) fn effective_concurrency(&self) -> usize {
        self.concurrency.max(1)
    }

    pub(crate) fn batches(&self, len: usize) -> impl Iterator<Item = std::ops::Range<usize>> {
        batch_ranges(len, self.effective_batch_size())
    }

    pub(crate) fn total_batches(&self, len: usize) -> usize {
        batch_count(len, self.effective_batch_size())
    }
}

/// Cortes de lote sobre un snapshot de `len` ítems. Es la unidad que comparten
/// los tres flujos de colección, el progreso y la cancelación: 30k ítems con
/// el lote por defecto son 600 eventos de UI, no 30k.
pub fn batch_ranges(len: usize, batch_size: usize) -> impl Iterator<Item = std::ops::Range<usize>> {
    let size = batch_size.max(1);
    (0..len)
        .step_by(size)
        .map(move |start| start..(start + size).min(len))
}

/// Número de lotes de un snapshot.
pub fn batch_count(len: usize, batch_size: usize) -> usize {
    let size = batch_size.max(1);
    len.div_ceil(size)
}

/// Ejecuta `call` reintentando 429/5xx y errores transitorios con backoff.
/// Un error permanente sale a la primera; el último error retryable se devuelve
/// tal cual para que el ítem lo registre.
pub async fn retry_gateway<T, F, Fut>(policy: &RetryPolicy, mut call: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let attempts = policy.attempts();
    let mut attempt = 0;
    loop {
        attempt += 1;
        match call().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let class = classify_failure(&error);
                if attempt >= attempts || class == FailureClass::Permanent {
                    return Err(error);
                }
                let hint = retry_after_hint(&error);
                tokio::time::sleep(policy.delay(attempt, class, hint)).await;
            }
        }
    }
}

/// Resultado de `TrackTransferGateway::match_track` (la costura no devuelve
/// `Result`, así que el reintento se decide por el mensaje de error embebido).
pub type TrackMatch = (f64, &'static str, Option<String>, Option<String>);

/// Reintenta una búsqueda de pista 429/5xx. Un error permanente o agotado se
/// devuelve tal cual; el llamador lo trata como no-match con motivo.
pub async fn retry_track_match<F, Fut>(policy: &RetryPolicy, mut call: F) -> TrackMatch
where
    F: FnMut() -> Fut,
    Fut: Future<Output = TrackMatch>,
{
    let attempts = policy.attempts();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let result = call().await;
        let Some(error) = result.3.as_deref() else {
            return result;
        };
        let class = classify_failure(error);
        if attempt >= attempts || class == FailureClass::Permanent {
            return result;
        }
        let hint = retry_after_hint(error);
        tokio::time::sleep(policy.delay(attempt, class, hint)).await;
    }
}

/// Un cerrojo por id de destino dentro de un lote: dos ítems distintos pueden
/// resolver al mismo id (la misma pista en dos playlists), y la comprobación de
/// existencia + escritura debe seguir siendo una sola operación para ellos.
/// Ids distintos no se estorban: la concurrencia efectiva la fija el lote.
#[derive(Default)]
pub struct DestinationLocks {
    by_id: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl DestinationLocks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lock_for(&self, destination_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut by_id = self
            .by_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        by_id
            .entry(destination_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}

/// Snapshot de pertenencia al destino, cargado una vez por fase y extendido
/// tras cada escritura: la comprobación previa es lo que evita duplicados y lo
/// que marca `preexisting` (M05).
pub type MembershipSnapshot = tokio::sync::Mutex<Option<HashSet<String>>>;

/// Carga el snapshot si aún no está; un fallo de lectura falla el ítem (y el
/// job), nunca se interpreta como «no existe».
pub async fn ensure_membership<F, Fut>(snapshot: &MembershipSnapshot, load: F) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<HashSet<String>, String>>,
{
    let mut guard = snapshot.lock().await;
    if guard.is_none() {
        *guard = Some(load().await?);
    }
    Ok(())
}

pub async fn membership_contains(snapshot: &MembershipSnapshot, destination_id: &str) -> bool {
    snapshot
        .lock()
        .await
        .as_ref()
        .is_some_and(|ids| ids.contains(destination_id))
}

pub async fn membership_insert(snapshot: &MembershipSnapshot, destination_id: &str) {
    if let Some(ids) = snapshot.lock().await.as_mut() {
        ids.insert(destination_id.to_string());
    }
}

/// Procesa un lote con concurrencia acotada preservando el orden de llegada de
/// los resultados a quien los consume. Devuelve el primer error del lote: un
/// fallo de persistencia no se convierte en resultado parcial silencioso.
pub async fn collect_batch<T, F>(items: Vec<F>, concurrency: usize) -> Result<Vec<T>, String>
where
    F: Future<Output = Result<T, String>>,
{
    use futures_util::StreamExt;

    let mut stream = futures_util::stream::iter(items).buffer_unordered(concurrency.max(1));
    let mut outcomes = Vec::new();
    while let Some(result) = stream.next().await {
        outcomes.push(result?);
    }
    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_rate_limits_from_provider_error_text() {
        assert_eq!(
            classify_failure("Qobuz API error (429): slow down"),
            FailureClass::RateLimited
        );
        assert_eq!(
            classify_failure("Tidal API error 429: too many requests"),
            FailureClass::RateLimited
        );
        assert_eq!(
            classify_failure("rate limited, retry later"),
            FailureClass::RateLimited
        );
    }

    #[test]
    fn classifies_server_and_transport_failures_as_transient() {
        for message in [
            "Tidal API error 503: unavailable",
            "Qobuz API error (502): bad gateway",
            "Request failed: connection reset by peer",
            "operation timed out",
        ] {
            assert_eq!(
                classify_failure(message),
                FailureClass::Transient,
                "{message}"
            );
        }
    }

    #[test]
    fn keeps_authentication_and_content_errors_permanent() {
        for message in [
            "RequiresAuth: Tidal API authentication failed (HTTP 401)",
            "Qobuz API error (404): not found",
            "write refused for qb-1",
        ] {
            assert_eq!(
                classify_failure(message),
                FailureClass::Permanent,
                "{message}"
            );
        }
    }

    #[test]
    fn reads_retry_after_hints_and_ignores_ids_that_look_like_codes() {
        assert_eq!(
            retry_after_hint("Qobuz API error (429): retry-after: 7"),
            Some(Duration::from_secs(7))
        );
        assert_eq!(
            retry_after_hint("rate limited, retry after 3 seconds"),
            Some(Duration::from_secs(3))
        );
        assert_eq!(retry_after_hint("track 429 moved"), None);
        assert_eq!(
            classify_failure("track 503 moved"),
            FailureClass::Permanent,
            "a bare number can be an id, not a status"
        );
    }

    #[test]
    fn backoff_grows_exponentially_and_honours_rate_limit_hints() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
        };
        let first = policy.delay(1, FailureClass::Transient, None);
        let second = policy.delay(2, FailureClass::Transient, None);
        assert!(first >= Duration::from_millis(50) && first <= Duration::from_millis(100));
        assert!(second >= Duration::from_millis(100) && second <= Duration::from_millis(200));
        let hinted = policy.delay(1, FailureClass::RateLimited, Some(Duration::from_secs(30)));
        assert_eq!(hinted, Duration::from_secs(30));
        let capped = policy.delay(1, FailureClass::RateLimited, Some(Duration::from_secs(600)));
        assert_eq!(capped, MAX_RETRY_AFTER_HONORED);
    }

    #[test]
    fn batch_ranges_cover_thirty_thousand_items_in_six_hundred_batches() {
        assert_eq!(batch_count(30_000, DEFAULT_BATCH_SIZE), 600);
        let ranges: Vec<_> = batch_ranges(30_000, DEFAULT_BATCH_SIZE).collect();
        assert_eq!(ranges.len(), 600);
        assert_eq!(ranges.first().unwrap(), &(0..50));
        assert_eq!(ranges.last().unwrap(), &(29_950..30_000));
        assert_eq!(batch_ranges(0, 50).count(), 0);
        assert_eq!(batch_ranges(7, 0).count(), 7, "a zero size still advances");
    }

    #[test]
    fn per_provider_concurrency_never_exceeds_the_ceiling() {
        assert_eq!(destination_concurrency("qobuz"), 3);
        assert_eq!(destination_concurrency("tidal"), 4);
        assert_eq!(destination_concurrency("spotify"), 5);
        assert_eq!(
            destination_concurrency("unknown-service"),
            DEFAULT_CONCURRENCY
        );
        for service in ["qobuz", "tidal", "spotify", "deezer", "local", "unknown"] {
            let policy = BatchPolicy::for_destination(service);
            let effective = policy.effective_concurrency();
            assert!(effective <= 8, "{service}");
            assert_eq!(effective, policy.concurrency.max(1));
        }
    }

    #[tokio::test]
    async fn destination_locks_serialise_the_same_id_and_not_distinct_ids() {
        let locks = DestinationLocks::new();
        let same_a = locks.lock_for("qb-1");
        let same_b = locks.lock_for("qb-1");
        let other = locks.lock_for("qb-2");
        assert!(Arc::ptr_eq(&same_a, &same_b));
        assert!(!Arc::ptr_eq(&same_a, &other));

        let guard = same_a.lock().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(20), same_b.lock())
                .await
                .is_err(),
            "the same destination id must wait"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), other.lock())
                .await
                .is_ok(),
            "distinct destination ids are independent"
        );
        drop(guard);
    }

    #[tokio::test]
    async fn retry_gateway_stops_at_the_last_attempt_and_reports_the_error() {
        let policy = RetryPolicy::immediate();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = attempts.clone();
        let result: Result<(), String> = retry_gateway(&policy, || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err("Tidal API error 503: unavailable".to_string())
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(
            attempts.load(std::sync::atomic::Ordering::SeqCst),
            policy.attempts(),
            "a retryable failure is tried max_attempts times"
        );
    }

    #[tokio::test]
    async fn retry_gateway_does_not_retry_a_permanent_failure() {
        let policy = RetryPolicy::immediate();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = attempts.clone();
        let result: Result<(), String> = retry_gateway(&policy, || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err("Qobuz API error (404): not found".to_string())
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn collect_batch_never_runs_more_than_the_concurrency_ceiling() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let in_flight = std::sync::Arc::new(AtomicUsize::new(0));
        let peak = std::sync::Arc::new(AtomicUsize::new(0));
        let futures: Vec<_> = (0..12)
            .map(|_| {
                let in_flight = in_flight.clone();
                let peak = peak.clone();
                async move {
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    Ok::<_, String>(())
                }
            })
            .collect();
        collect_batch(futures, 3).await.unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 3);
    }
}
