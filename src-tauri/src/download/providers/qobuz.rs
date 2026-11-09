//! Adaptador de [`QobuzDownloader`] y su trait de motor.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;

use crate::download::progress::{DownloadRequest, DownloadResult};
use crate::download::provider_gateway::{
    apply_routing, ensure_not_cancelled, DownloadProvider, ProviderCapabilities, ProviderDelivery,
    ProviderError, ProviderRequest,
};
use crate::download::qobuz::QobuzDownloader;
use crate::download::quality_reconcile::reconcile_physical_audio_quality;

/// Motor de descarga Qobuz: aísla el descargador real para que los tests
/// inyecten un grabador sin red.
pub trait QobuzEngine: Send + Sync {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>>;
}

impl QobuzEngine for QobuzDownloader {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        self.download_track(request, db, cancel_token)
    }
}

/// Adaptador de [`QobuzDownloader`]: reenvía la petición tal cual (la cuenta
/// la resuelve el descargador con su plan interno) y clasifica el error.
pub struct QobuzProvider {
    engine: Arc<dyn QobuzEngine>,
    db: Option<SqlitePool>,
}

impl QobuzProvider {
    pub fn new(engine: Arc<dyn QobuzEngine>, db: Option<SqlitePool>) -> Self {
        Self { engine, db }
    }
}

impl DownloadProvider for QobuzProvider {
    fn service_name(&self) -> &'static str {
        "qobuz"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_lossless: true,
        }
    }

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            ensure_not_cancelled(cancel_token)?;
            let mut result = self
                .engine
                .download(&request.request, self.db.as_ref(), cancel_token)
                .await
                .map_err(|err| ProviderError::classify("qobuz", &format!("{err:#}")))?;
            apply_routing(&mut result, request, "qobuz");
            reconcile_physical_audio_quality(&mut result, &request.request);
            Ok(ProviderDelivery::from_result(result, request.account_id))
        })
    }
}
