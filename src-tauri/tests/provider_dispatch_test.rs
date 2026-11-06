//! P5 Fase 2: el orquestador despacha toda descarga de proveedor por el
//! registro (`resolve_provider` + `DownloadProvider::download`).
//!
//! Sin red: cada proveedor es un grabador que entrega un FLAC sintético o un
//! error clasificado, y anota cuántas veces se le llamó y con qué `routing`.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::{DownloadRequest, DownloadResult};
use syncify_tauri_lib::download::provider_gateway::{
    DownloadProvider, ProviderCapabilities, ProviderDelivery, ProviderError, ProviderRequest,
    ProviderRouting,
};
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

#[derive(Debug, Clone, Copy)]
enum Behavior {
    Deliver,
    AuthRejected,
    RejectLosslessThenDeliver,
    Unavailable,
    EntityLookup401,
}

struct RecordingProvider {
    name: &'static str,
    behavior: Behavior,
    calls: AtomicUsize,
    last_routing: Mutex<Option<ProviderRouting>>,
}

impl RecordingProvider {
    fn new(name: &'static str, behavior: Behavior) -> Arc<Self> {
        Arc::new(Self {
            name,
            behavior,
            calls: AtomicUsize::new(0),
            last_routing: Mutex::new(None),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn last_routing(&self) -> Option<ProviderRouting> {
        self.last_routing.lock().unwrap().clone()
    }

    fn deliver(&self, request: &ProviderRequest) -> Result<ProviderDelivery, ProviderError> {
        let path = Path::new(&request.request.output_dir).join(format!("{}.flac", self.name));
        write_synthetic_flac(&path);
        let result = DownloadResult {
            file_path: path.to_string_lossy().to_string(),
            service: self.name.to_string(),
            bit_depth: 16,
            sample_rate: 44100,
            title: request.request.track_name.clone(),
            artist: request.request.artist_name.clone(),
            album: request.request.album_name.clone(),
            ..Default::default()
        };
        Ok(ProviderDelivery::from_result(result, request.account_id))
    }
}

impl DownloadProvider for RecordingProvider {
    fn service_name(&self) -> &'static str {
        self.name
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
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.last_routing.lock().unwrap() = Some(request.routing.clone());
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(ProviderError::cancelled());
                }
            }
            match self.behavior {
                Behavior::Deliver => self.deliver(request),
                Behavior::AuthRejected => Err(ProviderError::classify(
                    self.name,
                    "RequiresAuth: invalid credentials for this account",
                )),
                Behavior::RejectLosslessThenDeliver => {
                    if request.request.quality == "HIGH" {
                        self.deliver(request)
                    } else {
                        Err(ProviderError::classify(
                            self.name,
                            "RejectedQuality: requested lossless but received aac",
                        ))
                    }
                }
                Behavior::Unavailable => Err(ProviderError::classify(
                    self.name,
                    "StaleSource: track/get failed: HTTP 404 not found",
                )),
                Behavior::EntityLookup401 => Err(ProviderError::classify(
                    self.name,
                    "Qobuz track/get failed for ID 123: HTTP 401 Unauthorized",
                )),
            }
        })
    }
}

fn request(output_dir: &Path, service: &str, track_id: &str) -> DownloadRequest {
    DownloadRequest {
        item_id: format!("dispatch-{service}-{track_id}"),
        track_name: "Dispatch Track".to_string(),
        artist_name: "Dispatch Artist".to_string(),
        album_name: "Dispatch Album".to_string(),
        duration_ms: 180_000,
        output_dir: output_dir.to_string_lossy().to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        service_name: Some(service.to_string()),
        service_track_id: Some(track_id.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn primary_qobuz_and_tidal_dispatch_through_the_override() {
    for service in ["qobuz", "tidal"] {
        let dir = TempDir::new().unwrap();
        let fake = RecordingProvider::new(service, Behavior::Deliver);
        let orchestrator =
            DownloadOrchestrator::new().with_provider_override(service, fake.clone());
        let req = request(dir.path(), service, "primary-1");

        let result = orchestrator
            .download_track_cancellable(&req, None)
            .await
            .expect("the fake provider must complete the download");

        assert_eq!(fake.calls(), 1, "{service} must be dispatched once");
        assert!(result.file_path.ends_with(&format!("{service}.flac")));
        assert_eq!(result.origin_service.as_deref(), Some(service));
        assert_eq!(result.effective_service.as_deref(), Some(service));
        assert_eq!(result.match_method.as_deref(), Some("exact_locked_source"));
        assert_eq!(result.match_confidence, Some(1.0));
        assert!(result.fallback_reason.is_none());

        let routing = fake.last_routing().expect("routing recorded");
        assert_eq!(routing.origin_service.as_deref(), Some(service));
        assert_eq!(
            routing.origin_service_track_id.as_deref(),
            Some("primary-1")
        );
        assert_eq!(
            routing.effective_service_track_id.as_deref(),
            Some("primary-1")
        );
        assert_eq!(routing.match_method.as_deref(), Some("exact_locked_source"));
        assert_eq!(routing.match_confidence, Some(1.0));
    }
}

#[tokio::test]
async fn songlink_engine_helpers_use_the_override_and_keep_provenance() {
    let dir = TempDir::new().unwrap();
    let qobuz = RecordingProvider::new("qobuz", Behavior::Deliver);
    let tidal = RecordingProvider::new("tidal", Behavior::Deliver);
    let orchestrator = DownloadOrchestrator::new()
        .with_provider_override("qobuz", qobuz.clone())
        .with_provider_override("tidal", tidal.clone());

    let mut req = request(dir.path(), "spotify", "spotify-1");
    req.spotify_id = Some("spotify-1".to_string());

    let result = orchestrator
        .download_qobuz_track(&req, "qobuz-9", None)
        .await
        .expect("qobuz helper must complete through the fake");
    assert_eq!(qobuz.calls(), 1);
    assert_eq!(tidal.calls(), 0);
    assert_eq!(result.origin_service.as_deref(), Some("spotify"));
    assert_eq!(result.origin_service_track_id.as_deref(), Some("spotify-1"));
    assert_eq!(result.effective_service.as_deref(), Some("qobuz"));
    assert_eq!(
        result.effective_service_track_id.as_deref(),
        Some("qobuz-9")
    );
    assert_eq!(
        result.match_method.as_deref(),
        Some("songlink_cross_platform")
    );
    assert_eq!(result.match_confidence, Some(1.0));
    assert_eq!(
        result.fallback_reason.as_deref(),
        Some("SongLink cross-platform match")
    );

    let result = orchestrator
        .download_tidal_track(&req, "tidal-9", None)
        .await
        .expect("tidal helper must complete through the fake");
    assert_eq!(tidal.calls(), 1);
    assert_eq!(result.origin_service.as_deref(), Some("spotify"));
    assert_eq!(result.origin_service_track_id.as_deref(), Some("spotify-1"));
    assert_eq!(result.effective_service.as_deref(), Some("tidal"));
    assert_eq!(
        result.effective_service_track_id.as_deref(),
        Some("tidal-9")
    );
    assert_eq!(
        result.match_method.as_deref(),
        Some("songlink_cross_platform")
    );
    assert_eq!(result.match_confidence, Some(1.0));
    assert_eq!(
        result.fallback_reason.as_deref(),
        Some("SongLink cross-platform match")
    );
}

#[tokio::test]
async fn a_cancelled_request_never_reaches_the_provider() {
    let dir = TempDir::new().unwrap();
    let fake = RecordingProvider::new("qobuz", Behavior::Deliver);
    let orchestrator = DownloadOrchestrator::new().with_provider_override("qobuz", fake.clone());
    let req = request(dir.path(), "qobuz", "cancel-1");
    let token = CancellationToken::new();
    token.cancel();

    let error = orchestrator
        .download_track_cancellable(&req, Some(&token))
        .await
        .expect_err("a cancelled request must not download");

    assert!(error.to_string().contains("cancelled"));
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn auth_rejection_aborts_without_cross_service_fallback() {
    let dir = TempDir::new().unwrap();
    let qobuz = RecordingProvider::new("qobuz", Behavior::AuthRejected);
    let tidal = RecordingProvider::new("tidal", Behavior::Deliver);
    let orchestrator = DownloadOrchestrator::new()
        .with_provider_override("qobuz", qobuz.clone())
        .with_provider_override("tidal", tidal.clone());
    let req = request(dir.path(), "qobuz", "auth-1");

    let error = orchestrator
        .download_track_cancellable(&req, None)
        .await
        .expect_err("an auth rejection aborts the attempt");

    assert!(
        error.to_string().contains("RequiresAuth"),
        "unexpected error: {error}"
    );
    assert_eq!(qobuz.calls(), 1);
    assert_eq!(tidal.calls(), 0, "auth never crosses services");
}

#[tokio::test]
async fn rejected_quality_degrades_to_320_instead_of_aborting() {
    let dir = TempDir::new().unwrap();
    let qobuz = RecordingProvider::new("qobuz", Behavior::RejectLosslessThenDeliver);
    let orchestrator = DownloadOrchestrator::new().with_provider_override("qobuz", qobuz.clone());
    let req = request(dir.path(), "qobuz", "quality-1");

    let result = orchestrator
        .download_track_cancellable(&req, None)
        .await
        .expect("a rejected lossless stream degrades to 320");

    assert_eq!(qobuz.calls(), 2, "lossless attempt plus the 320 rung");
    assert_eq!(
        result.fallback_reason.as_deref(),
        Some("No Tidal match; degraded to 320 kbps on Qobuz")
    );
    assert_eq!(result.match_method.as_deref(), Some("exact_locked_source"));
}

#[tokio::test]
async fn unavailable_source_keeps_the_stale_diagnosis_without_fallback() {
    let dir = TempDir::new().unwrap();
    let fake = RecordingProvider::new("qobuz", Behavior::Unavailable);
    let orchestrator = DownloadOrchestrator::new().with_provider_override("qobuz", fake.clone());
    let mut req = request(dir.path(), "qobuz", "stale-1");
    req.allow_fallback = false;

    let error = orchestrator
        .download_track_cancellable(&req, None)
        .await
        .expect_err("a stale source without fallback aborts");

    assert!(
        error.to_string().contains("StaleSource"),
        "unexpected error: {error}"
    );
    assert_eq!(fake.calls(), 1);
}

#[tokio::test]
async fn entity_lookup_401_is_not_treated_as_an_account_auth_abort() {
    let dir = TempDir::new().unwrap();
    let fake = RecordingProvider::new("qobuz", Behavior::EntityLookup401);
    let orchestrator = DownloadOrchestrator::new().with_provider_override("qobuz", fake.clone());
    let req = request(dir.path(), "qobuz", "entity-1");

    let error = orchestrator
        .download_track_cancellable(&req, None)
        .await
        .expect_err("an entity lookup rejection is still a failure");

    let message = error.to_string();
    assert!(
        message.contains("ProviderEntityFailure"),
        "unexpected error: {message}"
    );
    assert!(
        !message.contains("authentication required"),
        "the account must not be blamed: {message}"
    );
    assert_eq!(fake.calls(), 1);
}
