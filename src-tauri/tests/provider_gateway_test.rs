//! D01 (#113): gateway de proveedores — trait, registro y proveedor falso.
//!
//! Sin red: el falso escribe un FLAC sintético (patrón de
//! `tidal_single_track_pipeline_test`) y la verificación física corre sobre
//! el fichero local. Los adaptadores reales se ejercitan solo por nombre y
//! mapeo de error; los flujos completos siguen caracterizados por
//! `songlink_native_engines_test` y `provider_fallback_identity_test`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use syncify_tauri_lib::download::audio_inspector::inspect_physical_audio_file;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::{DownloadRequest, DownloadResult};
use syncify_tauri_lib::download::provider_gateway::{
    DownloadProvider, ProviderCapabilities, ProviderDelivery, ProviderError, ProviderRequest,
    ProviderRouting,
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

fn write_synthetic_flac(path: &std::path::Path) {
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

fn test_request(output_dir: &std::path::Path) -> ProviderRequest {
    ProviderRequest::new(DownloadRequest {
        item_id: "gw-1".to_string(),
        track_name: "Gateway Track".to_string(),
        artist_name: "Gateway Artist".to_string(),
        album_name: "Gateway Album".to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        quality: "lossless".to_string(),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("gw-track-1".to_string()),
        ..Default::default()
    })
    .with_account(Some(7))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FakeBehavior {
    Deliver,
    AuthInvalid,
    RejectedQuality,
    Timeout,
}

/// Proveedor falso del ticket: entrega, 401, rechazo de calidad y timeout.
/// Los errores pasan por `ProviderError::classify` para que la taxonomía sea
/// la real, no una copia de test.
struct FakeProvider {
    name: &'static str,
    behavior: FakeBehavior,
}

impl FakeProvider {
    fn deliver(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            behavior: FakeBehavior::Deliver,
        })
    }

    fn failing(name: &'static str, behavior: FakeBehavior) -> Arc<Self> {
        Arc::new(Self { name, behavior })
    }
}

impl DownloadProvider for FakeProvider {
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
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(ProviderError::cancelled());
                }
            }
            match self.behavior {
                FakeBehavior::Deliver => {
                    let path = std::path::Path::new(&request.request.output_dir)
                        .join("gateway_track.flac");
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
                FakeBehavior::AuthInvalid => Err(ProviderError::classify(
                    self.name,
                    "HTTP (401 Unauthorized) while fetching the stream for the account",
                )),
                FakeBehavior::RejectedQuality => Err(ProviderError::classify(
                    self.name,
                    "RejectedQuality: requested hires, provider only serves MP3 320",
                )),
                FakeBehavior::Timeout => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    Err(ProviderError::classify(
                        self.name,
                        "request timed out while waiting for the provider stream",
                    ))
                }
            }
        })
    }
}

#[tokio::test]
async fn orchestrator_resolves_provider_by_name_from_registry() {
    let orchestrator = DownloadOrchestrator::new();

    for service in ["qobuz", "tidal", "amazon"] {
        let provider = orchestrator
            .resolve_provider(service)
            .unwrap_or_else(|| panic!("falta el proveedor {service}"));
        assert_eq!(provider.service_name(), service);
    }
    assert!(
        orchestrator
            .resolve_provider("qobuz")
            .unwrap()
            .capabilities()
            .supports_lossless
    );
    assert!(
        orchestrator
            .resolve_provider("tidal")
            .unwrap()
            .capabilities()
            .supports_lossless
    );
    assert!(
        !orchestrator
            .resolve_provider("amazon")
            .unwrap()
            .capabilities()
            .supports_lossless,
        "Amazon solo sirve lossy"
    );
    assert!(
        orchestrator.resolve_provider("deezer").is_none(),
        "catalog-only no es proveedor de descarga"
    );
    assert!(
        orchestrator.resolve_provider("QOBUZ").is_some(),
        "la resolución es insensible a mayúsculas"
    );

    // with_db reconstruye el registro con el pool para la resolución de tokens.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let with_db = DownloadOrchestrator::new().with_db(pool);
    assert!(with_db.resolve_provider("tidal").is_some());
}

#[tokio::test]
async fn fake_provider_delivers_and_verifies_without_network() {
    let dir = TempDir::new().unwrap();
    let orchestrator =
        DownloadOrchestrator::new().with_provider_override("qobuz", FakeProvider::deliver("qobuz"));

    let provider = orchestrator.resolve_provider("qobuz").unwrap();
    assert_eq!(provider.service_name(), "qobuz");

    let delivery = provider
        .download(&test_request(dir.path()), None)
        .await
        .expect("el falso entrega sin red");

    assert!(std::path::Path::new(&delivery.file_path).exists());
    assert_eq!(delivery.account_id, Some(7));
    assert_eq!(delivery.result.bit_depth, 16);
    assert_eq!(delivery.result.sample_rate, 44100);

    // La verificación física corre sobre el fichero local producido.
    let physical = inspect_physical_audio_file(std::path::Path::new(&delivery.file_path));
    assert!(physical.is_some(), "el FLAC sintético es inspeccionable");
    let physical = physical.unwrap();
    assert_eq!(delivery.format.as_deref(), Some(physical.format.as_str()));
    assert_eq!(delivery.bit_depth, physical.bit_depth);
    assert_eq!(delivery.sample_rate, physical.sample_rate);
}

#[tokio::test]
async fn fake_provider_variants_classify_with_shared_taxonomy() {
    let dir = TempDir::new().unwrap();
    let request = test_request(dir.path());

    // 401: veredicto sobre la cuenta, terminal y con acción del usuario.
    let err = FakeProvider::failing("qobuz", FakeBehavior::AuthInvalid)
        .download(&request, None)
        .await
        .expect_err("401 es error");
    assert!(err.taxonomy.requires_user_action());
    assert!(err.taxonomy.is_terminal());

    // Rechazo de calidad: terminal pero sin acción del usuario.
    let err = FakeProvider::failing("qobuz", FakeBehavior::RejectedQuality)
        .download(&request, None)
        .await
        .expect_err("rechazo de calidad es error");
    assert!(err.taxonomy.is_terminal());
    assert!(!err.taxonomy.requires_user_action());

    // Timeout de transporte: reintentable (la heurística real manda
    // TemporaryNetworkFailure sobre el marcador «timed out»).
    let err = FakeProvider::failing("qobuz", FakeBehavior::Timeout)
        .download(&request, None)
        .await
        .expect_err("timeout es error");
    assert!(err.taxonomy.is_retryable());
    assert_eq!(err.taxonomy.max_attempts(), 3);

    // Cancelación antes de empezar: nunca llega a abrir red.
    let token = CancellationToken::new();
    token.cancel();
    let err = FakeProvider::deliver("qobuz")
        .download(&request, Some(&token))
        .await
        .expect_err("cancelado es error");
    assert!(!err.taxonomy.is_retryable());
}

#[tokio::test]
async fn real_adapters_resolve_from_the_same_registry() {
    // Caracterización sin red: los adaptadores reales existen por nombre y
    // conservan el mapeo proveedor→taxonomía; sus descargas completas ya
    // están caracterizadas por los tests de SongLink y de fallback.
    let orchestrator = DownloadOrchestrator::new();
    assert_eq!(
        orchestrator
            .resolve_provider("qobuz")
            .unwrap()
            .service_name(),
        "qobuz"
    );
    assert_eq!(
        orchestrator
            .resolve_provider("tidal")
            .unwrap()
            .service_name(),
        "tidal"
    );
    assert_eq!(
        orchestrator
            .resolve_provider("amazon")
            .unwrap()
            .service_name(),
        "amazon"
    );

    let err = ProviderError::classify("amazon", "HTTP (403 Forbidden) for this item");
    assert!(err.taxonomy.requires_user_action());
}

#[test]
fn provider_request_routing_defaults_and_override() {
    let dir = TempDir::new().unwrap();

    let default = test_request(dir.path());
    assert!(default.routing.origin_service.is_none());
    assert!(default.routing.origin_service_track_id.is_none());
    assert!(default.routing.effective_service_track_id.is_none());
    assert!(default.routing.match_method.is_none());
    assert!(default.routing.match_confidence.is_none());
    assert!(default.routing.fallback_reason.is_none());

    let routed = test_request(dir.path()).with_routing(ProviderRouting {
        origin_service: Some("deezer".to_string()),
        origin_service_track_id: Some("dz-1".to_string()),
        effective_service_track_id: Some("qobuz-1".to_string()),
        match_method: Some("isrc_match".to_string()),
        match_confidence: Some(0.87),
        fallback_reason: Some("quality".to_string()),
    });
    assert_eq!(routed.routing.origin_service.as_deref(), Some("deezer"));
    assert_eq!(
        routed.routing.origin_service_track_id.as_deref(),
        Some("dz-1")
    );
    assert_eq!(
        routed.routing.effective_service_track_id.as_deref(),
        Some("qobuz-1")
    );
    assert_eq!(routed.routing.match_method.as_deref(), Some("isrc_match"));
    assert_eq!(routed.routing.match_confidence, Some(0.87));
    assert_eq!(routed.routing.fallback_reason.as_deref(), Some("quality"));
}
