//! Adaptador de [`TidalDownloader`] y su trait de motor.

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
use crate::download::quality_reconcile::reconcile_physical_audio_quality;
use crate::download::tidal::{TidalDownloader, TidalOrchestratorExt};

/// Motor de descarga Tidal: el descargador delega en
/// [`TidalOrchestratorExt::download_track`] y el trait lo hace sustituible.
pub trait TidalEngine: Send + Sync {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>>;
}

impl TidalEngine for TidalDownloader {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(TidalOrchestratorExt::download_track(
            self,
            request,
            db,
            cancel_token,
        ))
    }
}

/// Adaptador de [`TidalDownloader`].
pub struct TidalProvider {
    engine: Arc<dyn TidalEngine>,
    db: Option<SqlitePool>,
}

impl TidalProvider {
    pub fn new(engine: Arc<dyn TidalEngine>, db: Option<SqlitePool>) -> Self {
        Self { engine, db }
    }
}

impl DownloadProvider for TidalProvider {
    fn service_name(&self) -> &'static str {
        "tidal"
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
                .map_err(|err| ProviderError::classify("tidal", &format!("{err:#}")))?;
            apply_routing(&mut result, request, "tidal");
            reconcile_physical_audio_quality(&mut result, &request.request);
            Ok(ProviderDelivery::from_result(result, request.account_id))
        })
    }
}
