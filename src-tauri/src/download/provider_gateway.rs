//! D01 (#113): gateway de proveedores en la frontera del orquestador.
//!
//! Es el seam principal de pruebas del pipeline de descargas (spec #111): los
//! descargadores concretos (Qobuz, Tidal, Amazon) quedan envueltos en
//! adaptadores que implementan [`DownloadProvider`] **sin cambiar su lógica**;
//! los flujos primarios del orquestador (FS-05, provenance, reconciliación
//! física, fallback) siguen mandando. D02 enruta toda descarga de proveedor por
//! este registro: el orquestador resuelve el adaptador por nombre, le pasa la
//! provenance del sitio en [`ProviderRouting`] y los tests sustituyen entradas
//! con `with_provider_override` para probar job, enrutado y verificación sin red.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use tokio_util::sync::CancellationToken;

use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_core_domain::operation_recovery::OperationType;
use syncify_core_domain::quality::QualityDecision;

use crate::download::audio_inspector::inspect_physical_audio_file;
use crate::download::progress::{DownloadRequest, DownloadResult};
use crate::services::operation_recovery::classify_operation_error;

pub use crate::download::providers::{AmazonProvider, QobuzProvider, TidalProvider};

/// Enrutado del que procede la petición. Es metadato informativo para el
/// veredicto de provenance: el adaptador lo copia al [`DownloadResult`] y no
/// lo usa para elegir cuenta ni para autorizar la descarga.
#[derive(Debug, Clone, Default)]
pub struct ProviderRouting {
    pub origin_service: Option<String>,
    pub origin_service_track_id: Option<String>,
    pub effective_service_track_id: Option<String>,
    pub match_method: Option<String>,
    pub match_confidence: Option<f64>,
    pub fallback_reason: Option<String>,
}

/// Capacidades estáticas declaradas por un proveedor. El enrutado puede
/// consultarlas antes de intentar una petición que el motor no sabe servir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_lossless: bool,
}

/// Petición normalizada hacia un proveedor: la [`DownloadRequest`] del
/// orquestador (metadatos, identidad, calidad y políticas) más la cuenta
/// resuelta que D00 persiste en la cola (`resolved_account_id`) y el enrutado
/// del que procede.
///
/// `account_id` es metadato informativo: el adaptador lo copia a la entrega,
/// pero los motores de descarga vuelven a resolver la cuenta internamente.
///
/// La construye el orquestador en producción (`download_via_provider`) y la
/// consumen también los tests del gateway y de los adaptadores.
#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub request: DownloadRequest,
    pub account_id: Option<i64>,
    pub routing: ProviderRouting,
}

// Constructores de los tests del gateway y de los adaptadores; producción
// construye el literal con el enrutado del sitio.
#[allow(dead_code)]
impl ProviderRequest {
    pub fn new(request: DownloadRequest) -> Self {
        Self {
            request,
            account_id: None,
            routing: ProviderRouting::default(),
        }
    }

    pub fn with_account(mut self, account_id: Option<i64>) -> Self {
        self.account_id = account_id;
        self
    }

    pub fn with_routing(mut self, routing: ProviderRouting) -> Self {
        self.routing = routing;
        self
    }
}

/// Entrega normalizada de un proveedor: los datos que el enrutado por
/// evidencia (D02+) necesita para persistir el veredicto, más el
/// [`DownloadResult`] completo intacto para los flujos existentes.
#[derive(Debug, Clone)]
// El despachador actual consume `result`; los campos normalizados son el
// contrato de entrega para el veredicto de provenance y la persistencia.
#[allow(dead_code)]
pub struct ProviderDelivery {
    pub file_path: String,
    pub format: Option<String>,
    pub bit_depth: i32,
    pub sample_rate: i32,
    pub duration_secs: Option<f64>,
    pub account_id: Option<i64>,
    pub quality_decision: Option<QualityDecision>,
    pub result: DownloadResult,
}

impl ProviderDelivery {
    /// Proyecta un [`DownloadResult`] y lee del fichero producido el formato,
    /// la duración y los parámetros físicos que el resultado no lleva. Solo
    /// inspección local: no toca el pipeline ni la lógica del descargador. Es
    /// el constructor canónico para cualquier implementador de
    /// [`DownloadProvider`].
    ///
    /// Cuando la inspección física funciona, sus valores de bit depth y sample
    /// rate mandan sobre los declarados por el motor: el fichero es la fuente
    /// de verdad, no la respuesta del proveedor.
    pub fn from_result(result: DownloadResult, account_id: Option<i64>) -> Self {
        let physical = inspect_physical_audio_file(Path::new(&result.file_path));
        let (format, duration_secs, bit_depth, sample_rate) = match &physical {
            Some(physical) => (
                Some(physical.format.clone()),
                physical.duration_secs,
                physical.bit_depth,
                physical.sample_rate,
            ),
            None => (None, None, result.bit_depth, result.sample_rate),
        };
        Self {
            file_path: result.file_path.clone(),
            format,
            bit_depth,
            sample_rate,
            duration_secs,
            account_id,
            quality_decision: result.quality_decision.clone(),
            result,
        }
    }
}

/// Error de proveedor clasificado con la taxonomía compartida
/// (`syncify-core-domain`), para que el enrutado y la recuperación de arranque
/// decidan con las mismas clases que el worker.
#[derive(Debug, Clone)]
pub struct ProviderError {
    // La consume la decisión de fallback; el despacho de Commit A solo
    // transporta el error hasta la frontera anyhow.
    #[allow(dead_code)]
    pub taxonomy: ErrorTaxonomy,
    pub message: String,
}

impl ProviderError {
    /// Clasifica el mensaje con [`classify_operation_error`], la misma
    /// heurística que usan el journal y la reconciliación de arranque.
    pub fn classify(provider: &str, message: &str) -> Self {
        let operation_type = match provider.to_ascii_lowercase().as_str() {
            "qobuz" => OperationType::DownloadQobuz,
            "tidal" => OperationType::DownloadTidal,
            "amazon" => OperationType::DownloadAmazon,
            _ => OperationType::CrossProviderFallback,
        };
        Self {
            taxonomy: classify_operation_error(operation_type, provider, message),
            message: message.to_string(),
        }
    }

    pub fn cancelled() -> Self {
        Self {
            taxonomy: ErrorTaxonomy::Cancelled {
                reason: "provider download cancelled before start".to_string(),
            },
            message: "Download cancelled".to_string(),
        }
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProviderError {}

/// Contrato normalizado de un proveedor de descarga. Los adaptadores envuelven
/// los descargadores existentes; el proveedor falso de los tests implementa
/// esto mismo para probar job, enrutado y verificación sin red.
pub trait DownloadProvider: Send + Sync {
    fn service_name(&self) -> &'static str;

    /// Capacidades estáticas del motor; el adaptador las declara sin tocar su
    /// lógica interna.
    fn capabilities(&self) -> ProviderCapabilities;

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>>;
}

/// Comprueba el token antes de entrar al descargador envuelto: la cancelación
/// del usuario nunca llega a abrir red.
pub(crate) fn ensure_not_cancelled(
    cancel_token: Option<&CancellationToken>,
) -> Result<(), ProviderError> {
    match cancel_token {
        Some(token) if token.is_cancelled() => Err(ProviderError::cancelled()),
        _ => Ok(()),
    }
}

/// Copia el enrutado de la petición al resultado de descarga con los defaults
/// del gateway (`exact_locked_source` / confianza 1.0) cuando el enrutado no
/// los aporta. Los adaptadores llaman a esto antes de la reconciliación de
/// calidad para que la política vea el servicio de origen real.
pub(crate) fn apply_routing(result: &mut DownloadResult, request: &ProviderRequest, service: &str) {
    let routing = &request.routing;
    result.origin_service = routing
        .origin_service
        .clone()
        .or_else(|| request.request.service_name.clone())
        .or_else(|| Some(service.to_string()));
    result.origin_service_track_id = routing
        .origin_service_track_id
        .clone()
        .or_else(|| request.request.service_track_id.clone());
    result.effective_service = Some(service.to_string());
    result.effective_service_track_id = routing
        .effective_service_track_id
        .clone()
        .or_else(|| request.request.service_track_id.clone());
    result.fallback_reason = routing.fallback_reason.clone();
    result.match_method = routing
        .match_method
        .clone()
        .or_else(|| Some("exact_locked_source".to_string()));
    result.match_confidence = routing.match_confidence.or(Some(1.0));
}
