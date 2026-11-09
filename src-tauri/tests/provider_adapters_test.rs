//! P5 Fase 1: adaptadores de proveedor y traits de motor.
//!
//! Sin red: cada motor es un grabador que escribe un FLAC sintético y anota lo
//! que recibió. Cubre el contrato del gateway (petición, cuenta, enrutado,
//! reconciliación de calidad, taxonomía y capacidades) sin tocar los
//! descargadores reales.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use sqlx::SqlitePool;
use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_core_domain::operation_recovery::OperationType;
use syncify_tauri_lib::download::progress::{DownloadRequest, DownloadResult};
use syncify_tauri_lib::download::provider_gateway::{
    DownloadProvider, ProviderRequest, ProviderRouting,
};
use syncify_tauri_lib::download::providers::amazon::{AmazonEngine, AmazonProvider};
use syncify_tauri_lib::download::providers::qobuz::{QobuzEngine, QobuzProvider};
use syncify_tauri_lib::download::providers::tidal::{TidalEngine, TidalProvider};
use syncify_tauri_lib::services::operation_recovery::classify_operation_error;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn write_synthetic_flac(path: &Path) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    std::fs::write(path, &flac_bytes).unwrap();
}

fn test_request(dir: &Path) -> ProviderRequest {
    ProviderRequest::new(DownloadRequest {
        item_id: "p5-1".to_string(),
        track_name: "Adapter Track".to_string(),
        artist_name: "Adapter Artist".to_string(),
        album_name: "Adapter Album".to_string(),
        output_dir: dir.to_string_lossy().to_string(),
        quality: "lossless".to_string(),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("qobuz-track-1".to_string()),
        ..Default::default()
    })
    .with_account(Some(11))
}

#[derive(Debug, Clone)]
struct Call {
    item_id: String,
    service_track_id: Option<String>,
    db_present: bool,
    token_addr: Option<usize>,
    url: Option<String>,
}

/// Motor grabador común para Qobuz y Tidal: misma firma de descarga.
struct RecordingEngine {
    service: &'static str,
    calls: Mutex<Vec<Call>>,
    fail: Option<&'static str>,
    bit_depth: i32,
    sample_rate: i32,
}

impl RecordingEngine {
    fn delivering(service: &'static str, bit_depth: i32, sample_rate: i32) -> Arc<Self> {
        Arc::new(Self {
            service,
            calls: Mutex::new(Vec::new()),
            fail: None,
            bit_depth,
            sample_rate,
        })
    }

    fn failing(service: &'static str, message: &'static str) -> Arc<Self> {
        Arc::new(Self {
            service,
            calls: Mutex::new(Vec::new()),
            fail: Some(message),
            bit_depth: 16,
            sample_rate: 44100,
        })
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    fn recorded_download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        self.calls.lock().unwrap().push(Call {
            item_id: request.item_id.clone(),
            service_track_id: request.service_track_id.clone(),
            db_present: db.is_some(),
            token_addr: cancel_token.map(|t| t as *const CancellationToken as usize),
            url: None,
        });
        let fail = self.fail;
        let service = self.service.to_string();
        let bit_depth = self.bit_depth;
        let sample_rate = self.sample_rate;
        let output_dir = request.output_dir.clone();
        let title = request.track_name.clone();
        let artist = request.artist_name.clone();
        let album = request.album_name.clone();
        Box::pin(async move {
            if let Some(message) = fail {
                return Err(anyhow!(message));
            }
            let path = Path::new(&output_dir).join(format!("{service}_adapter.flac"));
            write_synthetic_flac(&path);
            Ok(DownloadResult {
                file_path: path.to_string_lossy().to_string(),
                service,
                bit_depth,
                sample_rate,
                title,
                artist,
                album,
                ..Default::default()
            })
        })
    }
}

impl QobuzEngine for RecordingEngine {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        self.recorded_download(request, db, cancel_token)
    }
}

impl TidalEngine for RecordingEngine {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        db: Option<&'a SqlitePool>,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        self.recorded_download(request, db, cancel_token)
    }
}

/// Motor grabador de Amazon: la URL es un argumento propio del relay.
struct RecordingAmazonEngine {
    calls: Mutex<Vec<Call>>,
    fail: Option<&'static str>,
}

impl RecordingAmazonEngine {
    fn delivering() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            fail: None,
        })
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}

impl AmazonEngine for RecordingAmazonEngine {
    fn download<'a>(
        &'a self,
        request: &'a DownloadRequest,
        url: &'a str,
        cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        self.calls.lock().unwrap().push(Call {
            item_id: request.item_id.clone(),
            service_track_id: request.service_track_id.clone(),
            db_present: false,
            token_addr: cancel_token.map(|t| t as *const CancellationToken as usize),
            url: Some(url.to_string()),
        });
        let fail = self.fail;
        let output_dir = request.output_dir.clone();
        let title = request.track_name.clone();
        let artist = request.artist_name.clone();
        let album = request.album_name.clone();
        Box::pin(async move {
            if let Some(message) = fail {
                return Err(anyhow!(message));
            }
            let path = Path::new(&output_dir).join("amazon_adapter.flac");
            write_synthetic_flac(&path);
            Ok(DownloadResult {
                file_path: path.to_string_lossy().to_string(),
                service: "amazon".to_string(),
                bit_depth: 16,
                sample_rate: 44100,
                title,
                artist,
                album,
                ..Default::default()
            })
        })
    }
}

async fn memory_pool() -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

#[tokio::test]
async fn qobuz_adapter_forwards_request_db_and_token() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("qobuz", 16, 44100);
    let provider = QobuzProvider::new(engine.clone(), Some(memory_pool().await));
    let token = CancellationToken::new();

    let delivery = provider
        .download(&test_request(dir.path()), Some(&token))
        .await
        .expect("el motor grabador entrega sin red");

    let calls = engine.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].item_id, "p5-1");
    assert_eq!(calls[0].service_track_id.as_deref(), Some("qobuz-track-1"));
    assert!(calls[0].db_present, "el adaptador reenvía el pool");
    assert_eq!(
        calls[0].token_addr,
        Some(&token as *const CancellationToken as usize),
        "el adaptador reenvía el mismo token"
    );
    assert_eq!(delivery.account_id, Some(11));
}

#[tokio::test]
async fn tidal_adapter_forwards_request_db_and_token() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("tidal", 16, 44100);
    let provider = TidalProvider::new(engine.clone(), Some(memory_pool().await));
    let token = CancellationToken::new();

    let delivery = provider
        .download(&test_request(dir.path()), Some(&token))
        .await
        .expect("el motor grabador entrega sin red");

    let calls = engine.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].service_track_id.as_deref(), Some("qobuz-track-1"));
    assert!(calls[0].db_present);
    assert_eq!(
        calls[0].token_addr,
        Some(&token as *const CancellationToken as usize)
    );
    assert_eq!(delivery.result.service, "tidal");
}

#[tokio::test]
async fn amazon_adapter_forwards_url_and_token() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingAmazonEngine::delivering();
    let provider = AmazonProvider::new(engine.clone());
    let token = CancellationToken::new();
    let mut request = test_request(dir.path());
    request.request.service_name = Some("amazon".to_string());
    request.request.service_track_id = Some("https://amazon.example/relay/track.flac".to_string());

    let delivery = provider
        .download(&request, Some(&token))
        .await
        .expect("el motor grabador entrega sin red");

    let calls = engine.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].url.as_deref(),
        Some("https://amazon.example/relay/track.flac")
    );
    assert_eq!(
        calls[0].token_addr,
        Some(&token as *const CancellationToken as usize)
    );
    assert_eq!(delivery.result.service, "amazon");
}

#[tokio::test]
async fn routing_is_applied_to_the_result() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("qobuz", 16, 44100);
    let provider = QobuzProvider::new(engine, None);
    let request = test_request(dir.path()).with_routing(ProviderRouting {
        origin_service: Some("deezer".to_string()),
        origin_service_track_id: Some("dz-1".to_string()),
        effective_service_track_id: Some("qz-1".to_string()),
        match_method: Some("isrc_match".to_string()),
        match_confidence: Some(0.87),
        fallback_reason: Some("quality".to_string()),
    });

    let delivery = provider.download(&request, None).await.unwrap();

    assert_eq!(delivery.result.origin_service.as_deref(), Some("deezer"));
    assert_eq!(
        delivery.result.origin_service_track_id.as_deref(),
        Some("dz-1")
    );
    assert_eq!(delivery.result.effective_service.as_deref(), Some("qobuz"));
    assert_eq!(
        delivery.result.effective_service_track_id.as_deref(),
        Some("qz-1")
    );
    assert_eq!(delivery.result.match_method.as_deref(), Some("isrc_match"));
    assert_eq!(delivery.result.match_confidence, Some(0.87));
    assert_eq!(delivery.result.fallback_reason.as_deref(), Some("quality"));
}

#[tokio::test]
async fn routing_defaults_are_exact_locked_source() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("qobuz", 16, 44100);
    let provider = QobuzProvider::new(engine, None);

    let delivery = provider
        .download(&test_request(dir.path()), None)
        .await
        .unwrap();

    assert_eq!(delivery.result.origin_service.as_deref(), Some("qobuz"));
    assert_eq!(
        delivery.result.origin_service_track_id.as_deref(),
        Some("qobuz-track-1")
    );
    assert_eq!(delivery.result.effective_service.as_deref(), Some("qobuz"));
    assert_eq!(
        delivery.result.effective_service_track_id.as_deref(),
        Some("qobuz-track-1")
    );
    assert_eq!(
        delivery.result.match_method.as_deref(),
        Some("exact_locked_source")
    );
    assert_eq!(delivery.result.match_confidence, Some(1.0));
    assert!(delivery.result.fallback_reason.is_none());
}

#[tokio::test]
async fn origin_service_falls_back_to_the_adapter_name() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("qobuz", 16, 44100);
    let provider = QobuzProvider::new(engine, None);
    let mut request = test_request(dir.path());
    request.request.service_name = None;

    let delivery = provider.download(&request, None).await.unwrap();

    assert_eq!(delivery.result.origin_service.as_deref(), Some("qobuz"));
}

#[tokio::test]
async fn quality_reconciliation_overrides_declared_metrics() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("qobuz", 24, 48000);
    let provider = QobuzProvider::new(engine, None);

    let delivery = provider
        .download(&test_request(dir.path()), None)
        .await
        .unwrap();

    assert_eq!(
        delivery.result.bit_depth, 16,
        "manda el FLAC sintético, no lo declarado por el motor"
    );
    assert_eq!(delivery.result.sample_rate, 44100);
    assert_eq!(delivery.bit_depth, 16);
    assert_eq!(delivery.sample_rate, 44100);
    assert!(delivery.quality_decision.is_some());
}

#[tokio::test]
async fn auth_failure_is_classified_at_the_boundary() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::failing(
        "qobuz",
        "HTTP (401 Unauthorized) while fetching the stream for the account",
    );
    let provider = QobuzProvider::new(engine, None);

    let error = provider
        .download(&test_request(dir.path()), None)
        .await
        .expect_err("401 no es una entrega");

    assert!(matches!(error.taxonomy, ErrorTaxonomy::AuthInvalid { .. }));
    assert!(error.message.contains("401"));
}

#[tokio::test]
async fn rejected_quality_is_classified_at_the_boundary() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::failing(
        "tidal",
        "RejectedQuality: requested hires, provider only serves MP3 320",
    );
    let provider = TidalProvider::new(engine, None);

    let error = provider
        .download(&test_request(dir.path()), None)
        .await
        .expect_err("la calidad rechazada no es una entrega");

    assert!(matches!(
        error.taxonomy,
        ErrorTaxonomy::RejectedQuality { .. }
    ));
}

#[tokio::test]
async fn cancellation_before_start_never_reaches_the_engine() {
    let dir = TempDir::new().unwrap();
    let engine = RecordingEngine::delivering("qobuz", 16, 44100);
    let provider = QobuzProvider::new(engine.clone(), None);
    let token = CancellationToken::new();
    token.cancel();

    let error = provider
        .download(&test_request(dir.path()), Some(&token))
        .await
        .expect_err("la cancelación previa corta la petición");

    assert!(matches!(error.taxonomy, ErrorTaxonomy::Cancelled { .. }));
    assert!(engine.calls().is_empty(), "no se llega a abrir red");
}

#[test]
fn amazon_maps_to_download_amazon() {
    assert_eq!(OperationType::DownloadAmazon.as_str(), "download_amazon");
    assert_eq!(
        OperationType::from_str("download_amazon"),
        Some(OperationType::DownloadAmazon)
    );

    // El brazo por defecto de classify_operation_error usa "amazon" cuando el
    // proveedor no llega explícito: así se clasifica una operación Amazon.
    let taxonomy = classify_operation_error(
        OperationType::DownloadAmazon,
        "",
        "request timed out while waiting for the provider stream",
    );
    match taxonomy {
        ErrorTaxonomy::TemporaryNetworkFailure { endpoint, .. } => {
            assert_eq!(endpoint, "amazon operation");
        }
        other => panic!("taxonomía inesperada: {other:?}"),
    }
}

#[test]
fn capabilities_declare_lossless_support() {
    let qobuz = QobuzProvider::new(RecordingEngine::delivering("qobuz", 16, 44100), None);
    let tidal = TidalProvider::new(RecordingEngine::delivering("tidal", 16, 44100), None);
    let amazon = AmazonProvider::new(RecordingAmazonEngine::delivering());

    assert!(qobuz.capabilities().supports_lossless);
    assert!(tidal.capabilities().supports_lossless);
    assert!(!amazon.capabilities().supports_lossless);
}
