//! D01 (#113): gateway de proveedores en la frontera del orquestador.
//!
//! Es el seam principal de pruebas del pipeline de descargas (spec #111): los
//! descargadores concretos (Qobuz, Tidal, Amazon) quedan envueltos en
//! adaptadores que implementan [`DownloadProvider`] **sin cambiar su lógica**;
//! los flujos primarios del orquestador (FS-05, provenance, reconciliación
//! física, fallback) siguen mandando. D02 conectará el enrutado por evidencia
//! a este registro; hoy el registro se resuelve por nombre y acepta
//! sustituciones para probar job, enrutado y verificación sin red.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;

use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_core_domain::operation_recovery::OperationType;
use syncify_core_domain::quality::QualityDecision;

use crate::download::amazon::AmazonDownloader;
use crate::download::audio_inspector::inspect_physical_audio_file;
use crate::download::progress::{DownloadRequest, DownloadResult};
use crate::download::qobuz::QobuzDownloader;
use crate::download::tidal::{TidalDownloader, TidalOrchestratorExt};
use crate::services::operation_recovery::classify_operation_error;

/// Petición normalizada hacia un proveedor: la [`DownloadRequest`] del
/// orquestador (metadatos, identidad, calidad y políticas) más la cuenta
/// resuelta que D00 persiste en la cola (`resolved_account_id`).
///
// Consumida por tests/provider_gateway_test.rs y por el falso; D02 conectará
// el enrutado que la construye en producción.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub request: DownloadRequest,
    pub account_id: Option<i64>,
}

#[allow(dead_code)]
impl ProviderRequest {
    pub fn new(request: DownloadRequest) -> Self {
        Self {
            request,
            account_id: None,
        }
    }

    pub fn with_account(mut self, account_id: Option<i64>) -> Self {
        self.account_id = account_id;
        self
    }
}

/// Entrega normalizada de un proveedor: los datos que el enrutado por
/// evidencia (D02+) necesita para persistir el veredicto, más el
/// [`DownloadResult`] completo intacto para los flujos existentes.
#[derive(Debug, Clone)]
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
    /// Proyecta un [`DownloadResult`] y lee del fichero producido el formato y
    /// la duración que el resultado no lleva. Solo inspección local: no toca
    /// el pipeline ni la lógica del descargador. Es el constructor canónico
    /// para cualquier implementador de [`DownloadProvider`].
    pub fn from_result(result: DownloadResult, account_id: Option<i64>) -> Self {
        let physical = inspect_physical_audio_file(Path::new(&result.file_path));
        let (format, duration_secs) = match &physical {
            Some(physical) => (Some(physical.format.clone()), physical.duration_secs),
            None => (None, None),
        };
        Self {
            file_path: result.file_path.clone(),
            format,
            bit_depth: result.bit_depth,
            sample_rate: result.sample_rate,
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

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>>;
}

/// Comprueba el token antes de entrar al descargador envuelto: la cancelación
/// del usuario nunca llega a abrir red.
fn ensure_not_cancelled(cancel_token: Option<&CancellationToken>) -> Result<(), ProviderError> {
    match cancel_token {
        Some(token) if token.is_cancelled() => Err(ProviderError::cancelled()),
        _ => Ok(()),
    }
}

/// Adaptador de [`QobuzDownloader`]: reenvía la petición tal cual (la cuenta
/// la resuelve el descargador con su plan interno) y clasifica el error.
pub struct QobuzProvider {
    downloader: Arc<QobuzDownloader>,
    db: Option<SqlitePool>,
}

impl QobuzProvider {
    pub fn new(downloader: Arc<QobuzDownloader>, db: Option<SqlitePool>) -> Self {
        Self { downloader, db }
    }
}

impl DownloadProvider for QobuzProvider {
    fn service_name(&self) -> &'static str {
        "qobuz"
    }

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            ensure_not_cancelled(cancel_token)?;
            let result = self
                .downloader
                .download_track(&request.request, self.db.as_ref())
                .await
                .map_err(|err| ProviderError::classify("qobuz", &format!("{err:#}")))?;
            Ok(ProviderDelivery::from_result(result, request.account_id))
        })
    }
}

/// Adaptador de [`TidalDownloader`] (extensión [`TidalOrchestratorExt`]).
pub struct TidalProvider {
    downloader: Arc<TidalDownloader>,
    db: Option<SqlitePool>,
}

impl TidalProvider {
    pub fn new(downloader: Arc<TidalDownloader>, db: Option<SqlitePool>) -> Self {
        Self { downloader, db }
    }
}

impl DownloadProvider for TidalProvider {
    fn service_name(&self) -> &'static str {
        "tidal"
    }

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            ensure_not_cancelled(cancel_token)?;
            let result = self
                .downloader
                .download_track(&request.request, self.db.as_ref())
                .await
                .map_err(|err| ProviderError::classify("tidal", &format!("{err:#}")))?;
            Ok(ProviderDelivery::from_result(result, request.account_id))
        })
    }
}

/// Adaptador de [`AmazonDownloader`]: la URL del relay llega en
/// `service_track_id`, igual que en la rama directa del orquestador.
pub struct AmazonProvider {
    downloader: Arc<AmazonDownloader>,
}

impl AmazonProvider {
    pub fn new(downloader: Arc<AmazonDownloader>) -> Self {
        Self { downloader }
    }
}

impl DownloadProvider for AmazonProvider {
    fn service_name(&self) -> &'static str {
        "amazon"
    }

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            ensure_not_cancelled(cancel_token)?;
            let amazon_url = request.request.service_track_id.clone().unwrap_or_default();
            let result = self
                .downloader
                .download_track(&request.request, &amazon_url)
                .await
                .map_err(|err| ProviderError::classify("amazon", &format!("{err:#}")))?;
            Ok(ProviderDelivery::from_result(result, request.account_id))
        })
    }
}
