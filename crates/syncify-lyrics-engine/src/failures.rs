//! Clasificación de los fallos por Fuente de letras y contrato que el núcleo
//! persiste en la caché negativa (LY03, #184).
//!
//! El motor no conoce la base de datos: devuelve qué Fuente no entregó
//! candidato y por qué, y la fachada decide si lo recuerda con caducidad. El
//! clasificador es deliberadamente pequeño y probado: los fetchers devuelven
//! `anyhow::Error`, así que se inspecciona primero el error tipado de
//! `reqwest` y, cuando el fetcher lo envolvió en un mensaje, se clasifica el
//! texto con el vocabulario estable que consume la tabla.

use anyhow::Error;

/// Motivo estable de un fallo de resolución por Fuente de letras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsFailureKind {
    /// La Fuente respondió y no hay letra usable para esta pista.
    NotFound,
    /// La Fuente devolvió contenido que la política (idioma o calidad) rechaza.
    PolicyRejected,
    /// Faltan credenciales o la Fuente las rechazó.
    RequiresAuth,
    /// La Fuente limitó la tasa de peticiones.
    RateLimited,
    /// Fallo de transporte, timeout o error de servidor.
    SourceUnavailable,
    /// La Fuente respondió algo que no se pudo interpretar.
    Failed,
}

impl LyricsFailureKind {
    /// Valor que se guarda en `lyrics_resolution_failures.reason`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::PolicyRejected => "policy_rejected",
            Self::RequiresAuth => "requires_auth",
            Self::RateLimited => "rate_limited",
            Self::SourceUnavailable => "source_unavailable",
            Self::Failed => "failed",
        }
    }
}

/// Un fallo observado al sondear una Fuente para una pista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsSourceFailure {
    pub provider_id: String,
    pub kind: LyricsFailureKind,
    pub detail: String,
}

/// Resultado de una cascada: el candidato (si lo hubo), los fallos de las
/// Fuentes sondeadas y las Fuentes saltadas por la caché negativa. La fachada
/// persiste `failures`; `skipped` permite auditar el salto sin red.
#[derive(Debug, Default)]
pub struct LyricsCascadeReport {
    pub response: Option<super::types::LyricsResponse>,
    pub failures: Vec<LyricsSourceFailure>,
    pub skipped: Vec<String>,
    pub error: Option<String>,
}

/// Resultado completo de una orquestación, con la evidencia por Fuente.
#[derive(Debug, Clone)]
pub struct LyricsResolutionReport {
    pub resolution: syncify_lyrics_domain::LyricsResolution,
    pub elapsed_ms: u64,
    pub failures: Vec<LyricsSourceFailure>,
    pub skipped: Vec<String>,
}

/// Clasifica el error de un fetcher. Primero el tipo de `reqwest` (que conserva
/// estado HTTP y causa); si el fetcher ya lo envolvió en un mensaje, se
/// clasifica el texto con `classify_failure_message`.
pub fn classify_source_error(error: &Error) -> LyricsFailureKind {
    if let Some(http_error) = error.downcast_ref::<reqwest::Error>() {
        return classify_http_failure(http_error);
    }
    classify_failure_message(&error.to_string())
}

fn classify_http_failure(error: &reqwest::Error) -> LyricsFailureKind {
    if error.is_timeout() || error.is_connect() || error.is_request() {
        return LyricsFailureKind::SourceUnavailable;
    }
    match error.status().map(|status| status.as_u16()) {
        Some(401 | 403) => LyricsFailureKind::RequiresAuth,
        Some(429) => LyricsFailureKind::RateLimited,
        Some(404) => LyricsFailureKind::NotFound,
        Some(500..=599) => LyricsFailureKind::SourceUnavailable,
        _ => LyricsFailureKind::Failed,
    }
}

/// Vocabulario de motivos sobre el mensaje del fetcher. Los patrones siguen a
/// los errores que producen las 12 Fuentes del motor; el orden importa: una
/// credencial caducada gana a un 403 genérico y un límite de tasa gana a un
/// «request failed» de transporte. La palabra «token» por sí sola no es señal
/// de auth: aparece en URLs de transporte (`?usertoken=…`) y en nombres de
/// endpoint; las señales son el estado HTTP y los mensajes explícitos.
pub fn classify_failure_message(message: &str) -> LyricsFailureKind {
    let message = message.to_lowercase();

    if has_any(
        &message,
        &["429", "too many requests", "rate limit", "rate-limit"],
    ) {
        return LyricsFailureKind::RateLimited;
    }
    if has_any(
        &message,
        &[
            "401",
            "403",
            "unauthorized",
            "forbidden",
            "requires auth",
            "sp_dc",
            "credential",
            "not configured",
            "could not extract apple music token",
            "invalid token",
            "token expired",
            "missing token",
        ],
    ) {
        return LyricsFailureKind::RequiresAuth;
    }
    if has_any(
        &message,
        &[
            "timed out",
            "timeout",
            "network",
            "connection",
            "connect",
            "dns",
            "sending request",
            "request failed",
            "500",
            "502",
            "503",
            "service unavailable",
        ],
    ) {
        return LyricsFailureKind::SourceUnavailable;
    }
    if has_any(
        &message,
        &[
            "parse",
            "decode",
            "deserialize",
            "invalid json",
            "decompression",
            "regex",
        ],
    ) {
        return LyricsFailureKind::Failed;
    }
    LyricsFailureKind::NotFound
}

/// Redacta credenciales que viajan en URLs antes de guardar el diagnóstico del
/// proveedor: parámetros de query con token y el userinfo de cualquier URL.
/// El `detail` se persiste y se registra; una cookie `sp_dc` o un `usertoken`
/// de Musixmatch no deben quedar ahí.
pub fn redact_failure_detail(detail: &str) -> String {
    let query_credentials =
        regex::Regex::new(r"(?i)([?&](?:token|usertoken|access_token|sp_dc)=)[^&\s#)\]},]+")
            .expect("regex de query params válida");
    let sanitized = query_credentials.replace_all(detail, "${1}[REDACTED]");
    let url_userinfo =
        regex::Regex::new(r"(?i)(https?://)[^/\s@]+@").expect("regex de userinfo válida");
    url_userinfo
        .replace_all(&sanitized, "${1}[REDACTED]@")
        .into_owned()
}

fn has_any(message: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| message.contains(needle))
}
