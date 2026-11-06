//! Adaptador de [`AmazonDownloader`] y su trait de motor.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use crate::download::amazon::AmazonDownloader;
use crate::download::progress::{DownloadRequest, DownloadResult};
use crate::download::provider_gateway::{
    apply_routing, ensure_not_cancelled, DownloadProvider, ProviderCapabilities, ProviderDelivery,
    ProviderError, ProviderRequest,
};
use crate::download::quality_reconcile::reconcile_physical_audio_quality;

/// Motor de descarga Amazon: la URL del relay es un argumento explícito, no
/// parte del contrato general de descarga.
pub trait AmazonEngine: Send + Sync {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        url: &'a str,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>>;
}

impl AmazonEngine for AmazonDownloader {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        url: &'a str,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(self.download_track(request, url, cancel_token))
    }
}

/// Adaptador de [`AmazonDownloader`]: la URL del relay llega en
/// `service_track_id`, igual que en la rama directa del orquestador.
pub struct AmazonProvider {
    engine: Arc<dyn AmazonEngine>,
}

impl AmazonProvider {
    pub fn new(engine: Arc<dyn AmazonEngine>) -> Self {
        Self { engine }
    }
}

impl DownloadProvider for AmazonProvider {
    fn service_name(&self) -> &'static str {
        "amazon"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_lossless: false,
        }
    }

    fn download<'a>(
        &'a self,
        request: &'a ProviderRequest,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            ensure_not_cancelled(cancel_token)?;
            let amazon_url = request.request.service_track_id.clone().unwrap_or_default();
            let mut result = self
                .engine
                .download(&request.request, &amazon_url, cancel_token)
                .await
                .map_err(|err| ProviderError::classify("amazon", &format!("{err:#}")))?;
            apply_routing(&mut result, request, "amazon");
            reconcile_physical_audio_quality(&mut result, &request.request);
            Ok(ProviderDelivery::from_result(result, request.account_id))
        })
    }
}
