//! A03-ter (#342) — integración del camino de descarga con la puerta única.
//!
//! El orquestador completo (`download_track` por la API pública) con un
//! proveedor falso que responde sin red y un transporte de refresco falso que
//! cuenta llamadas. Contrato ADR-0007 decisión 14:
//!
//! - Con vencimiento publicado y vencido, la descarga refresca ANTES de
//!   despachar al proveedor y persiste el payload en la MISMA fila.
//! - Con vencimiento fresco, el transporte no recibe llamadas.
//! - Una cuenta ya marcada (puerta anti-bucle) no recibe intentos y la
//!   descarga sigue su curso (la puerta es mejor esfuerzo).
//!
//! BD `:memory:` completamente migrada y cuentas desechables.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Pool;
use sqlx::Sqlite;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use syncify_core_domain::quality::AudioTier;
use syncify_tauri_lib::commands::account_refresh::{CredentialRefreshTransport, RefreshError};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::{DownloadRequest, DownloadResult};
use syncify_tauri_lib::download::provider_gateway::{
    DownloadProvider, ProviderCapabilities, ProviderDelivery, ProviderError, ProviderRequest,
};
use syncify_tauri_lib::download::resolver::{
    CandidateAccount, CandidateIdentity, DownloadPlan, PlanCandidate, ResolvedQuality,
};
use tokio_util::sync::CancellationToken;

/// Transporte falso: cuenta intentos y devuelve el resultado programado.
struct FakeTransport {
    outcome: Result<serde_json::Value, RefreshError>,
    calls: AtomicUsize,
}

impl FakeTransport {
    fn ok(payload: serde_json::Value) -> Self {
        Self {
            outcome: Ok(payload),
            calls: AtomicUsize::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl CredentialRefreshTransport for FakeTransport {
    fn refresh<'a>(
        &'a self,
        _db: &'a sqlx::SqlitePool,
        _service: &'a str,
        _account_id: i64,
        _creds: &'a serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, RefreshError>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outcome.clone()
        })
    }
}

/// Proveedor falso: responde OK con un resultado mínimo; la descarga en sí
/// no es el sujeto del test, solo que la puerta actuó antes del despacho.
struct OkProvider;

impl DownloadProvider for OkProvider {
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
        _cancel_token: Option<&'a CancellationToken>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderDelivery, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(ProviderDelivery::from_result(
                DownloadResult {
                    file_path: "no-existe.flac".to_string(),
                    service: "tidal".to_string(),
                    ..Default::default()
                },
                request.account_id,
            ))
        })
    }
}

async fn setup_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones completas");
    let _ = crypto::init_crypto([42u8; 32]);
    pool
}

/// Cuenta Tidal con el payload de credenciales ya CIFRADO.
async fn insert_tidal_account(pool: &Pool<Sqlite>, creds: &serde_json::Value, marked: bool) -> i64 {
    let sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por las migraciones");
    let encrypted = crypto::encrypt(&creds.to_string()).expect("cifrado de prueba");
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email, is_active, credentials_json, credentials_invalid) \
         VALUES (?, 'Wiring', 'wiring@test.invalid', 1, ?, ?) RETURNING id",
    )
    .bind(sid)
    .bind(&encrypted)
    .bind(if marked { 1 } else { 0 })
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

/// Petición con plan que nombra la cuenta: el único caso en que la puerta
/// proactiva puede nombrar a quién refresca.
fn planned_tidal_request(account_id: i64) -> DownloadRequest {
    DownloadRequest {
        item_id: "wiring-item".to_string(),
        service_name: Some("tidal".to_string()),
        service_track_id: Some("999".to_string()),
        resolution_plan: Some(DownloadPlan {
            canonical_track_id: 1,
            candidates: vec![PlanCandidate {
                service_id: 3,
                service_name: "tidal".to_string(),
                service_track_id: "999".to_string(),
                candidate_tier: AudioTier::Lossless,
                identity: CandidateIdentity {
                    track_source_id: 1,
                    canonical_track_id: 1,
                    isrc: None,
                },
                account: Some(CandidateAccount {
                    account_id,
                    is_active: true,
                    streaming_entitled: Some(1),
                }),
                sibling_accounts: vec![],
                service_priority: None,
                quality_score: None,
                bit_depth: None,
            }],
            quality: ResolvedQuality {
                effective: "lossless".to_string(),
                candidate: "lossless".to_string(),
                clamped: false,
            },
            strict_quality: false,
            allow_fallback: false,
        }),
        ..Default::default()
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn stored_creds(pool: &Pool<Sqlite>, account_id: i64) -> serde_json::Value {
    let encrypted: String =
        sqlx::query_scalar("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(pool)
            .await
            .expect("cuenta presente");
    let plain = crypto::decrypt(&encrypted).expect("credenciales descifrables");
    serde_json::from_str(&plain).expect("credenciales JSON")
}

fn wiring_orchestrator(db: Pool<Sqlite>, transport: Arc<FakeTransport>) -> DownloadOrchestrator {
    DownloadOrchestrator::new()
        .with_db(db)
        .with_provider_override("tidal", Arc::new(OkProvider))
        .with_refresh_transport(transport)
}

/// Criterio del issue: la descarga refresca antes de usar una credencial con
/// vencimiento conocido — y solo una vez: el pipeline aguas abajo ve el token
/// ya fresco y no repite el refresco.
#[tokio::test]
async fn download_refreshes_expired_credential_before_dispatch() {
    let pool = setup_db().await;
    let now = now_secs();
    let account_id = insert_tidal_account(
        &pool,
        &serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "token_expiry": (now - 100) as f64,
        }),
        false,
    )
    .await;
    let transport = Arc::new(FakeTransport::ok(serde_json::json!({
        "access_token": "nuevo",
        "refresh_token": "rt",
        "token_expiry": (now + 3600) as f64,
    })));
    let orchestrator = wiring_orchestrator(pool.clone(), transport.clone());
    let request = planned_tidal_request(account_id);

    let result = orchestrator.download_track(&request).await;

    assert!(result.is_ok(), "la descarga falsa responde OK: {result:?}");
    assert_eq!(
        transport.call_count(),
        1,
        "la puerta refresca ANTES del despacho, una sola vez"
    );
    assert_eq!(
        stored_creds(&pool, account_id).await["access_token"],
        "nuevo",
        "el payload refrescado queda en la MISMA fila"
    );
}

/// Credencial fresca: confiar y despachar; el transporte ni se entera.
#[tokio::test]
async fn download_with_fresh_credential_skips_the_refresh_transport() {
    let pool = setup_db().await;
    let now = now_secs();
    let account_id = insert_tidal_account(
        &pool,
        &serde_json::json!({
            "access_token": "fresco",
            "refresh_token": "rt",
            "token_expiry": (now + 3600) as f64,
        }),
        false,
    )
    .await;
    let transport = Arc::new(FakeTransport::ok(serde_json::json!({
        "access_token": "no-debe-persistirse",
    })));
    let orchestrator = wiring_orchestrator(pool.clone(), transport.clone());
    let request = planned_tidal_request(account_id);

    let result = orchestrator.download_track(&request).await;

    assert!(result.is_ok(), "la descarga sigue: {result:?}");
    assert_eq!(transport.call_count(), 0);
    assert_eq!(
        stored_creds(&pool, account_id).await["access_token"],
        "fresco",
        "la credencial no se toca"
    );
}

/// Puerta anti-bucle: una cuenta marcada no recibe intentos y la descarga
/// sigue su curso (la puerta es mejor esfuerzo; el proveedor reporta sus
/// propios errores de autenticación).
#[tokio::test]
async fn download_with_marked_account_receives_no_refresh_attempts() {
    let pool = setup_db().await;
    let now = now_secs();
    let account_id = insert_tidal_account(
        &pool,
        &serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "token_expiry": (now - 100) as f64,
        }),
        true,
    )
    .await;
    let transport = Arc::new(FakeTransport::ok(serde_json::json!({
        "access_token": "no-debe-persistirse",
    })));
    let orchestrator = wiring_orchestrator(pool.clone(), transport.clone());
    let request = planned_tidal_request(account_id);

    let result = orchestrator.download_track(&request).await;

    assert!(result.is_ok(), "la descarga sigue: {result:?}");
    assert_eq!(
        transport.call_count(),
        0,
        "la puerta anti-bucle no permite intentos sobre cuentas marcadas"
    );
}
